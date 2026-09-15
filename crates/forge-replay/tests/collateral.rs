use forge_content::parse_and_compile_production;
use forge_kernel::{
    CharacterChoiceSelection, CharacterSelection, CompiledContent, EntropyState, EventKind,
    KnowledgeProvenance, enumerate_legal_actions,
};
use forge_replay::{PlayerTrace, Session, Trace, resume_player_trace, verify};
use std::collections::BTreeMap;

const SOURCE: &str = include_str!("../../../content/split-tide.json");
const ASH: &str = "fume_yards.ash_beds";
const BAY: &str = "fume_yards.kiln_bay";
const DARO: &str = "fume_yards.daro_venn";
const BRANN: &str = "fume_yards.brann_coil";
const PERA: &str = "fume_yards.pera_senn";
const CAGE: &str = "fume_yards.collateral_cage";
const CASK: &str = "fume_yards.water_cask";
const FILTER: &str = "fume_yards.filter";
const FUEL: &str = "fume_yards.fuel";

type Action = (&'static str, Option<&'static str>);

const COMMON: &[Action] = &[
    ("travel_adjacent", Some("lowsail.levee")),
    ("travel_adjacent", Some("fume_yards.workshop")),
    ("travel_adjacent", Some(BAY)),
    ("fume_yards.take_cask", None),
    ("fume_yards.take_fuel", None),
    ("fume_yards.enter_ash_hatch", None),
    ("fume_yards.read_collateral_docket", None),
];

fn content() -> CompiledContent {
    parse_and_compile_production(SOURCE).expect("collateral production compiles")
}

fn custom_start<'a>(content: &'a CompiledContent, calling: &str) -> Session<'a> {
    let selection = CharacterSelection {
        name: "Collateral method comparison".to_owned(),
        choices: [
            ("lineage", "fenborn"),
            ("origin", "lowsail"),
            ("calling", calling),
            ("value", "order"),
            ("burden", "indebted"),
            ("history", "saved-worker"),
        ]
        .into_iter()
        .map(|(slot_id, choice_id)| CharacterChoiceSelection {
            slot_id: slot_id.to_owned(),
            choice_id: choice_id.to_owned(),
        })
        .collect(),
    };
    Session::new_custom_game(&selection, 71, content).unwrap()
}

fn select(session: &Session<'_>, content: &CompiledContent, (id, destination): Action) {
    let parameters = destination
        .map(|value| BTreeMap::from([(String::from("destination"), value.to_owned())]))
        .unwrap_or_default();
    let action = enumerate_legal_actions(session.state(), content)
        .unwrap()
        .into_iter()
        .find(|action| action.definition_id == id && action.parameters == parameters)
        .unwrap_or_else(|| panic!("missing {id} at turn {}", session.state().world.time));
    let view = content
        .action_page(session.state(), 0, usize::MAX)
        .unwrap()
        .actions
        .into_iter()
        .find(|view| view.action_id == action.action_id)
        .unwrap();
    assert_eq!(
        (view.time_cost.minimum_ticks, view.time_cost.maximum_ticks),
        (1, 1)
    );
}

fn record(session: &mut Session<'_>, content: &CompiledContent, action: Action) {
    let before = session.state().world.time;
    select(session, content, action);
    let parameters = action
        .1
        .map(|value| BTreeMap::from([(String::from("destination"), value.to_owned())]))
        .unwrap_or_default();
    let canonical = enumerate_legal_actions(session.state(), content)
        .unwrap()
        .into_iter()
        .find(|candidate| candidate.definition_id == action.0 && candidate.parameters == parameters)
        .unwrap();
    let transition = session.record(&canonical).unwrap();
    assert_eq!(transition.observation.world_time, before + 1);
    assert!(
        transition.observation.text.split_whitespace().count()
            + transition
                .observation
                .supplies
                .summary()
                .split_whitespace()
                .count()
            < 100
    );
}

fn has_action(session: &Session<'_>, content: &CompiledContent, id: &str) -> bool {
    enumerate_legal_actions(session.state(), content)
        .unwrap()
        .iter()
        .any(|action| action.definition_id == id)
}

fn checkpoint<'a>(session: &Session<'a>, content: &'a CompiledContent) -> Session<'a> {
    let encoded = session.player_trace().unwrap().to_json().unwrap();
    for private in [
        "\"inventory\"",
        "\"storages\"",
        "\"knowledge\"",
        "\"events\"",
        "\"entropy\"",
    ] {
        assert!(!encoded.contains(private), "save leaked {private}");
    }
    let resumed = resume_player_trace(&PlayerTrace::from_json(&encoded).unwrap(), content).unwrap();
    assert_eq!(resumed.state(), session.state());
    assert_eq!(resumed.trace(), session.trace());
    assert_eq!(resumed.player_trace().unwrap().to_json().unwrap(), encoded);
    assert_eq!(
        content.action_page(resumed.state(), 0, usize::MAX).unwrap(),
        content.action_page(session.state(), 0, usize::MAX).unwrap()
    );
    resumed
}

fn checkpoint_route(
    content: &CompiledContent,
    calling: &str,
    actions: &[Action],
    uninterrupted: &Session<'_>,
) {
    for checkpoint_index in 0..=actions.len() {
        let mut prefix = custom_start(content, calling);
        for &action in &actions[..checkpoint_index] {
            record(&mut prefix, content, action);
        }
        let mut resumed = checkpoint(&prefix, content);
        for &action in &actions[checkpoint_index..] {
            record(&mut resumed, content, action);
        }
        assert_eq!(
            resumed.state(),
            uninterrupted.state(),
            "checkpoint {checkpoint_index}"
        );
        assert_eq!(
            resumed.trace(),
            uninterrupted.trace(),
            "checkpoint {checkpoint_index}"
        );
        assert_eq!(
            resumed.player_trace().unwrap(),
            uninterrupted.player_trace().unwrap(),
            "checkpoint {checkpoint_index}"
        );
        assert_eq!(
            content.action_page(resumed.state(), 0, usize::MAX).unwrap(),
            content
                .action_page(uninterrupted.state(), 0, usize::MAX)
                .unwrap(),
            "checkpoint {checkpoint_index}"
        );
    }
    let _ = checkpoint(uninterrupted, content);
}

#[test]
fn collateral_fuel_method_replays_for_ledger_clerk_against_lock_runner() {
    let content = content();
    let mut ledger = custom_start(&content, "ledger-clerk");
    let mut runner = custom_start(&content, "lock-runner");
    for &action in COMMON {
        record(&mut ledger, &content, action);
        record(&mut runner, &content, action);
    }

    assert_eq!(ledger.state().world.time, 7);
    assert_eq!(runner.state().world.time, 7);
    assert_eq!(ledger.state().world.current_location, ASH);
    assert_eq!(runner.state().world.current_location, ASH);
    assert!(has_action(
        &ledger,
        &content,
        "fume_yards.settle_collateral_fuel"
    ));
    assert!(!has_action(
        &runner,
        &content,
        "fume_yards.settle_collateral_fuel"
    ));
    assert_eq!(
        ledger.state().world.npcs[DARO].knowledge["fume_yards.collateral_terms"].provenance,
        KnowledgeProvenance::Read {
            source: "fume_yards.collateral_docket".to_owned()
        }
    );
    assert_eq!(
        runner.state().world.npcs[DARO].knowledge["fume_yards.collateral_terms"].provenance,
        KnowledgeProvenance::Read {
            source: "fume_yards.collateral_docket".to_owned()
        }
    );

    record(
        &mut ledger,
        &content,
        ("fume_yards.settle_collateral_fuel", None),
    );
    let final_state = ledger.state();
    assert_eq!(final_state.world.time, 8);
    assert_eq!(final_state.character.resources["coin"], 10);
    assert_eq!(final_state.character.resources["stamina"], 3);
    assert_eq!(final_state.character.inventory["rope"], 1);
    assert_eq!(final_state.character.inventory[CASK], 1);
    assert_eq!(final_state.character.inventory[FILTER], 1);
    assert!(!final_state.character.inventory.contains_key(FUEL));
    assert_eq!(final_state.world.storages[CAGE].inventory[FUEL], 1);
    assert!(
        !final_state.world.storages[CAGE]
            .inventory
            .contains_key(FILTER)
    );
    assert_eq!(final_state.world.npcs[DARO].inventory[FILTER], 1);
    assert!(final_state.world.npcs[BRANN].inventory.is_empty());
    assert!(final_state.world.npcs[PERA].inventory.is_empty());
    assert!(
        final_state.world.locations[ASH]
            .flags
            .contains("fume_yards.collateral_settled")
    );
    assert!(
        final_state.world.locations[BAY]
            .flags
            .contains("fume_yards.fuel_taken")
    );
    assert!(
        final_state.world.locations[BAY]
            .flags
            .contains("fume_yards.fuel_settled")
    );
    assert!(
        final_state.world.npcs[DARO]
            .memories
            .contains_key("fume_yards.collateral_docket_read")
    );
    assert!(
        final_state.world.npcs[DARO]
            .memories
            .contains_key("fume_yards.collateral_fuel_received")
    );
    assert!(
        ledger
            .trace()
            .steps
            .last()
            .unwrap()
            .events
            .iter()
            .any(|event| matches!(
                &event.kind,
                EventKind::CharacterItemTransferredToStorage { storage, item, count }
                    if storage == CAGE && item == FUEL && *count == 1
            ))
    );
    assert!(
        ledger
            .trace()
            .steps
            .last()
            .unwrap()
            .events
            .iter()
            .any(|event| matches!(
                &event.kind,
                EventKind::StorageItemTransferredToCharacter { storage, item, count }
                    if storage == CAGE && item == FILTER && *count == 1
            ))
    );
    assert_eq!(ledger.state().entropy, EntropyState::new(71));

    assert_eq!(runner.state().character.resources["coin"], 5);
    assert_eq!(runner.state().character.resources["stamina"], 4);
    assert_eq!(
        runner.state().character.inventory,
        BTreeMap::from([
            (CASK.to_owned(), 1),
            (FUEL.to_owned(), 1),
            ("rope".to_owned(), 1),
            ("wire".to_owned(), 1),
        ])
    );
    assert_eq!(runner.state().world.storages[CAGE].inventory[FILTER], 1);
    assert!(
        !runner.state().world.storages[CAGE]
            .inventory
            .contains_key(FUEL)
    );
    assert!(
        !runner.state().world.locations[ASH]
            .flags
            .contains("fume_yards.collateral_settled")
    );
    assert!(
        !runner.state().world.locations[BAY]
            .flags
            .contains("fume_yards.fuel_settled")
    );
    assert_eq!(runner.state().entropy, EntropyState::new(71));

    let mut ledger_actions = COMMON.to_vec();
    ledger_actions.push(("fume_yards.settle_collateral_fuel", None));
    checkpoint_route(&content, "ledger-clerk", &ledger_actions, &ledger);
    checkpoint_route(&content, "lock-runner", COMMON, &runner);
    let trace = Trace::from_json(&ledger.trace().to_json().unwrap()).unwrap();
    assert_eq!(verify(&trace, &content).unwrap(), *ledger.state());
}
