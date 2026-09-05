use forge_content::parse_and_compile_production;
use forge_kernel::{
    CanonicalAction, CompiledContent, Event, EventKind, KernelError, enumerate_legal_actions,
};
use forge_replay::{PlayerTrace, ReplayError, Session, resume_player_trace, verify};
use std::collections::BTreeMap;

const SPLIT_TIDE: &str = include_str!("../../../content/split-tide.json");
const PRESS: &str = "fume_yards.press_repair_plugs";

fn select(
    session: &Session<'_>,
    content: &CompiledContent,
    definition: &str,
    parameters: &[(&str, &str)],
) -> CanonicalAction {
    let expected: BTreeMap<_, _> = parameters
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect();
    enumerate_legal_actions(session.state(), content)
        .expect("production action catalog enumerates")
        .into_iter()
        .find(|action| action.definition_id == definition && action.parameters == expected)
        .unwrap_or_else(|| panic!("missing exact canonical action {definition}: {expected:?}"))
}

fn stocked_workshop(content: &CompiledContent) -> Session<'_> {
    let mut session = Session::new_game("rook", 71, content).expect("Rook starts canonically");
    for (definition, parameters) in [
        ("travel_adjacent", vec![("destination", "lowsail.levee")]),
        (
            "travel_adjacent",
            vec![("destination", "fume_yards.workshop")],
        ),
        ("fume_yards.take_stock", Vec::new()),
    ] {
        let action = select(&session, content, definition, &parameters);
        session
            .record(&action)
            .unwrap_or_else(|error| panic!("workshop preparation {definition} failed: {error}"));
    }
    assert_eq!(session.state().world.time, 3);
    assert_eq!(
        session.state().world.current_location,
        "fume_yards.workshop"
    );
    assert_eq!(session.state().character.inventory["fume_yards.clay"], 2);
    assert_eq!(session.state().character.inventory["fume_yards.mesh"], 1);
    assert!(
        session.state().world.npcs["fume_yards.nessa_tern"]
            .inventory
            .is_empty()
    );
    assert!(
        !session
            .state()
            .character
            .inventory
            .contains_key("fume_yards.repair_lot")
    );
    session
}

fn assert_finished_trace(
    session: &mut Session<'_>,
    content: &CompiledContent,
    press: &CanonicalAction,
    original_inventory: &BTreeMap<String, u32>,
) {
    let mut expected_inventory = original_inventory.clone();
    expected_inventory.remove("fume_yards.clay");
    expected_inventory.remove("fume_yards.mesh");
    expected_inventory.insert("fume_yards.repair_lot".to_owned(), 1);
    assert_eq!(session.state().character.inventory, expected_inventory);
    assert_eq!(session.state().world.time, 4);
    assert!(
        session.state().world.npcs["fume_yards.nessa_tern"]
            .inventory
            .is_empty()
    );
    let step = session.trace().steps.last().expect("press step recorded");
    let recipe_events: Vec<_> = step
        .events
        .iter()
        .filter(|event| matches!(event.kind, EventKind::RecipeApplied { .. }))
        .cloned()
        .collect();
    assert_eq!(
        recipe_events,
        vec![Event {
            turn: 3,
            kind: EventKind::RecipeApplied {
                recipe: PRESS.to_owned(),
                inputs: BTreeMap::from([
                    ("fume_yards.clay".to_owned(), 2),
                    ("fume_yards.mesh".to_owned(), 1),
                ]),
                outputs: BTreeMap::from([("fume_yards.repair_lot".to_owned(), 1)]),
            },
        }]
    );
    assert_eq!(step.events.first(), recipe_events.first());
    let catalog = enumerate_legal_actions(session.state(), content).expect("finished catalog");
    for retired in [
        PRESS,
        "fume_yards.pack_catch_screen",
        "fume_yards.take_stock",
    ] {
        assert!(catalog.iter().all(|action| action.definition_id != retired));
    }

    let before_state = session.state().clone();
    let before_trace = session.trace().clone();
    let before_save = session.player_trace().expect("save after recipe");
    assert!(matches!(
        session.record(press),
        Err(ReplayError::Kernel(KernelError::StaleAction { .. }))
    ));
    assert_eq!(session.state(), &before_state);
    assert_eq!(session.trace(), &before_trace);
    assert_eq!(
        session.player_trace().expect("save after rejection"),
        before_save
    );
    assert_eq!(
        verify(&before_trace, content).expect("detailed replay"),
        before_state
    );

    let save_json = before_save.to_json().expect("player-safe save serializes");
    let value: serde_json::Value = serde_json::from_str(&save_json).expect("save JSON parses");
    let mut keys: Vec<_> = value
        .as_object()
        .expect("save is an object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec![
            "action_ids",
            "build_id",
            "final_receipt",
            "final_state_id",
            "format_version",
            "start"
        ]
    );
    for private in [
        "fume_yards.clay",
        "fume_yards.mesh",
        "fume_yards.repair_lot",
        "recipe_applied",
        "inventory",
        "event_log",
        "entropy",
    ] {
        assert!(
            !save_json.contains(private),
            "player save contains private field or material identity {private}"
        );
    }
    let decoded = PlayerTrace::from_json(&save_json).expect("player save decodes");
    assert_eq!(decoded.action_count(), 4);
    let resumed = resume_player_trace(&decoded, content).expect("player save replays canonically");
    assert_eq!(resumed.state(), &before_state);
    assert_eq!(resumed.trace(), &before_trace);
}

#[test]
fn production_recipe_consumes_owned_inputs_once() {
    let content = parse_and_compile_production(SPLIT_TIDE).expect("production content compiles");
    let mut session = stocked_workshop(&content);
    let original_inventory = session.state().character.inventory.clone();
    let press = select(&session, &content, PRESS, &[]);
    session
        .record(&press)
        .unwrap_or_else(|error| panic!("recipe consumption bypassed owned inputs: {error}"));
    for item in ["fume_yards.clay", "fume_yards.mesh"] {
        assert!(
            !session.state().character.inventory.contains_key(item),
            "recipe consumption bypassed owned inputs: {item}"
        );
    }
    assert_finished_trace(&mut session, &content, &press, &original_inventory);
}

#[test]
fn production_recipe_produces_exact_finished_quantity() {
    let content = parse_and_compile_production(SPLIT_TIDE).expect("production content compiles");
    let mut session = stocked_workshop(&content);
    let original_inventory = session.state().character.inventory.clone();
    let press = select(&session, &content, PRESS, &[]);
    session
        .record(&press)
        .unwrap_or_else(|error| panic!("recipe output duplicated finished goods: {error}"));
    assert_eq!(
        session
            .state()
            .character
            .inventory
            .get("fume_yards.repair_lot"),
        Some(&1),
        "recipe output duplicated finished goods"
    );
    assert_finished_trace(&mut session, &content, &press, &original_inventory);
}
