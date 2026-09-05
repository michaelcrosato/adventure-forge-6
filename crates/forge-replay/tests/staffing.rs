use forge_content::parse_and_compile_production;
use forge_kernel::{
    CanonicalAction, CharacterChoiceSelection, CharacterSelection, CompiledContent, EntropyState,
    EventKind, KnowledgeProvenance, enumerate_legal_actions,
};
use forge_replay::{PlayerTrace, Session, Trace, resume_player_trace, verify};
use std::collections::BTreeMap;

const SOURCE: &str = include_str!("../../../content/split-tide.json");
const BAY: &str = "fume_yards.kiln_bay";
const ASH: &str = "fume_yards.ash_beds";
const BRANN: &str = "fume_yards.brann_coil";
const DARO: &str = "fume_yards.daro_venn";
const PERA: &str = "fume_yards.pera_senn";
const FILTER: &str = "fume_yards.filter";
type Action = (&'static str, Option<&'static str>);
const PREFIX: &[Action] = &[
    ("checkpoint.show_charter", None),
    ("travel_adjacent", Some("lowsail.levee")),
    ("levee.authority_path", None),
    ("travel_adjacent", Some("red_sluice.top")),
    ("top.hold_market", None),
    ("world.enter_aftermath", None),
    ("return.count_dry_stalls", None),
    ("return.visit_workshop", None),
    ("travel_adjacent", Some(ASH)),
    ("fume_yards.buy_collateral_filter", None),
    ("travel_adjacent", Some("fume_yards.workshop")),
    ("travel_adjacent", Some(BAY)),
    ("fume_yards.fit_dust_filter", None),
];
const STAFFED: &[Action] = &[
    ("fume_yards.share_rescue_account", None),
    ("fume_yards.assign_brann_salvage", None),
    ("fume_yards.recover_staffed_filter", None),
    ("fume_yards.return_with_brann", None),
    ("fume_yards.load_cold_freight", None),
];
const ORDINARY: &[Action] = &[
    ("fume_yards.enter_ash_hatch", None),
    ("fume_yards.brace_rack", None),
    ("fume_yards.recover_braced_filter", None),
    ("fume_yards.report_with_daro", None),
    ("fume_yards.load_cold_freight", None),
    ("wait_tide", None),
    ("wait_tide", None),
];
const WATER: &[Action] = &[
    ("travel_adjacent", Some("fume_yards.workshop")),
    ("fume_yards.take_stock", None),
    ("fume_yards.press_repair_plugs", None),
    ("world.enter_aftermath", None),
    ("return.patch_stand", None),
    ("return.order_water_stand", None),
    ("return.fit_market_filter", None),
    ("return.visit_workshop", None),
    ("travel_adjacent", Some(BAY)),
    ("fume_yards.take_market_cask", None),
    ("fume_yards.escort_market_cask", None),
    ("return.install_market_cask", None),
    ("return.draw_clean_water", None),
];

fn start<'a>(content: &'a CompiledContent, history: &str) -> Session<'a> {
    let selection = CharacterSelection {
        name: "Staffing save comparison".into(),
        choices: [
            ("lineage", "fenborn"),
            ("origin", "lowsail"),
            ("calling", "ledger-clerk"),
            ("value", "order"),
            ("burden", "indebted"),
            ("history", history),
        ]
        .into_iter()
        .map(|(slot, choice)| CharacterChoiceSelection {
            slot_id: slot.into(),
            choice_id: choice.into(),
        })
        .collect(),
    };
    Session::new_custom_game(&selection, 71, content).unwrap()
}

fn select(
    session: &Session<'_>,
    content: &CompiledContent,
    (id, destination): Action,
) -> CanonicalAction {
    let parameters = destination
        .map(|to| BTreeMap::from([("destination".into(), to.into())]))
        .unwrap_or_default();
    enumerate_legal_actions(session.state(), content)
        .unwrap()
        .into_iter()
        .find(|action| action.definition_id == id && action.parameters == parameters)
        .unwrap_or_else(|| panic!("missing {id} at {}", session.state().world.time))
}

fn record(session: &mut Session<'_>, content: &CompiledContent, action: Action) {
    session.record(&select(session, content, action)).unwrap();
}

fn checkpoint<'a>(session: &Session<'a>, content: &'a CompiledContent) -> Session<'a> {
    let bytes = session.player_trace().unwrap().to_json().unwrap();
    for private in [
        "\"inventory\"",
        "\"storages\"",
        "\"knowledge\"",
        "\"events\"",
        "\"entropy\"",
    ] {
        assert!(!bytes.contains(private));
    }
    let resumed = resume_player_trace(&PlayerTrace::from_json(&bytes).unwrap(), content).unwrap();
    assert_eq!(resumed.state(), session.state());
    assert_eq!(resumed.trace(), session.trace());
    assert_eq!(resumed.player_trace().unwrap().to_json().unwrap(), bytes);
    assert_eq!(
        content.observe(resumed.state()).unwrap(),
        content.observe(session.state()).unwrap()
    );
    assert_eq!(
        enumerate_legal_actions(resumed.state(), content).unwrap(),
        enumerate_legal_actions(session.state(), content).unwrap()
    );
    assert_eq!(
        verify(
            &Trace::from_json(&session.trace().to_json().unwrap()).unwrap(),
            content
        )
        .unwrap(),
        *session.state()
    );
    resumed
}

fn reject_stale(session: &mut Session<'_>, action: &CanonicalAction) {
    let state = session.state().clone();
    let trace = session.trace().clone();
    let save = session.player_trace().unwrap().to_json().unwrap();
    assert!(session.record(action).is_err());
    assert_eq!(session.state(), &state);
    assert_eq!(session.trace(), &trace);
    assert_eq!(session.player_trace().unwrap().to_json().unwrap(), save);
}

#[test]
fn staffing_serialized_boundaries_preserve_three_tick_lift_and_composed_water() {
    let content = parse_and_compile_production(SOURCE).unwrap();
    let mut continuous = start(&content, "saved-worker");
    for &action in PREFIX {
        record(&mut continuous, &content, action);
    }
    assert_eq!(continuous.state().world.time, 13);
    assert_eq!(
        continuous.state().character.resources,
        BTreeMap::from([("coin".into(), 6), ("stamina".into(), 3)])
    );
    let mut resumed = checkpoint(&continuous, &content);
    for (&action, expected_time) in STAFFED.iter().zip([14, 15, 18, 19, 20]) {
        let stale = select(&continuous, &content, action);
        assert_eq!(stale, select(&resumed, &content, action));
        let left = continuous.record(&stale).unwrap();
        let right = resumed.record(&stale).unwrap();
        assert_eq!(left, right);
        assert_eq!(continuous.state().world.time, expected_time);
        reject_stale(&mut resumed, &stale);
        resumed = checkpoint(&resumed, &content);
    }
    assert_eq!(
        (
            continuous.trace().steps.len(),
            continuous.state().world.time
        ),
        (18, 20)
    );
    assert_eq!(
        continuous.state().character.resources,
        BTreeMap::from([("coin".into(), 10), ("stamina".into(), 3)])
    );
    assert_eq!(
        continuous.state().character.inventory,
        BTreeMap::from([("rope".into(), 1), (FILTER.into(), 1)])
    );
    assert_eq!(continuous.state().world.npcs[BRANN].location, BAY);
    assert_eq!(continuous.state().world.npcs[DARO].location, ASH);
    assert!(continuous.state().world.npcs[DARO].inventory.is_empty());
    let fact = &continuous.state().world.npcs[BRANN].knowledge["fume_yards.rack_cleared"];
    assert_eq!(
        (fact.turn, &fact.provenance),
        (15, &KnowledgeProvenance::Witnessed)
    );
    let lift = &continuous.trace().steps[15];
    assert!(
        lift.events
            .iter()
            .any(|event| event.turn == 15 && event.kind == EventKind::TimeAdvanced { ticks: 3 })
    );
    assert_eq!(lift.events.iter().filter(|event| matches!(&event.kind, EventKind::NpcItemTransferredToCharacter { npc, item, count: 1 } if npc == DARO && item == FILTER)).map(|event| event.turn).collect::<Vec<_>>(), [15]);
    assert_eq!(lift.events.iter().filter(|event| matches!(&event.kind, EventKind::ScheduledEventResolved { event_id, applied: false, .. } if event_id == "lowsail.next_surge")).map(|event| event.turn).collect::<Vec<_>>(), [18]);
    for (index, &action) in WATER.iter().enumerate() {
        record(&mut continuous, &content, action);
        record(&mut resumed, &content, action);
        assert_eq!(continuous.trace(), resumed.trace());
        if [1, 2, 4, 6, 9, 10, 11, 12].contains(&index) {
            resumed = checkpoint(&resumed, &content);
        }
    }
    assert_eq!(
        (
            continuous.trace().steps.len(),
            continuous.state().world.time
        ),
        (31, 33)
    );
    assert_eq!(
        continuous.state().character.resources,
        BTreeMap::from([("coin".into(), 10), ("stamina".into(), 5)])
    );
    assert_eq!(
        continuous.state().character.inventory,
        BTreeMap::from([("rope".into(), 1)])
    );
    assert_eq!(
        continuous.state().world.npcs[PERA].location,
        "lowsail.return"
    );
    assert!(continuous.state().world.npcs[PERA].inventory.is_empty());
    assert_eq!(
        continuous.state().world.npcs[PERA].knowledge["fume_yards.market_cask"].turn,
        30
    );
    assert_eq!(
        continuous.state().world.npcs["oren_pell"].knowledge["fume_yards.market_cask"].provenance,
        KnowledgeProvenance::Told { by: PERA.into() }
    );
    assert_eq!(continuous.state().entropy, EntropyState::new(71));
    assert_eq!(
        continuous.player_trace().unwrap().to_json().unwrap(),
        resumed.player_trace().unwrap().to_json().unwrap()
    );
}

#[test]
fn staffing_history_pairs_and_cancelled_absence_survive_native_saves_without_remote_return() {
    let content = parse_and_compile_production(SOURCE).unwrap();
    for history in ["saved-worker", "stole-permit"] {
        let mut session = start(&content, history);
        for &action in PREFIX {
            record(&mut session, &content, action);
        }
        session = checkpoint(&session, &content);
        let ids: Vec<_> = enumerate_legal_actions(session.state(), &content)
            .unwrap()
            .into_iter()
            .map(|action| action.definition_id)
            .collect();
        assert_eq!(
            ids.iter().any(|id| id == "fume_yards.share_rescue_account"),
            history == "saved-worker"
        );
        assert!(!ids.iter().any(|id| id == "fume_yards.assign_brann_salvage"));
        for &action in ORDINARY {
            record(&mut session, &content, action);
        }
        session = checkpoint(&session, &content);
        assert_eq!(session.state().world.time, 20);
        assert_eq!(
            session.state().character.resources,
            BTreeMap::from([("coin".into(), 10), ("stamina".into(), 1)])
        );
        assert!(
            !session.state().world.npcs[BRANN]
                .knowledge
                .contains_key("fume_yards.rescue_account_heard")
        );
        assert_eq!(
            session.state().world.npcs[BRANN].knowledge["fume_yards.rack_cleared"].provenance,
            KnowledgeProvenance::Told { by: DARO.into() }
        );
    }
    let mut session = start(&content, "saved-worker");
    for &action in PREFIX {
        record(&mut session, &content, action);
    }
    for &action in &STAFFED[..2] {
        record(&mut session, &content, action);
    }
    let stale_lift = select(&session, &content, STAFFED[2]);
    record(&mut session, &content, ("fume_yards.leave_ash_hatch", None));
    session = checkpoint(&session, &content);
    assert_eq!(session.state().world.time, 16);
    assert_eq!(session.state().world.current_location, BAY);
    assert_eq!(session.state().world.npcs[BRANN].location, ASH);
    assert_eq!(
        session.state().world.npcs[BRANN].inventory["fume_yards.fuel"],
        1
    );
    assert!(
        !enumerate_legal_actions(session.state(), &content)
            .unwrap()
            .iter()
            .any(
                |action| ["fume_yards.take_fuel", "fume_yards.load_cold_freight"]
                    .contains(&action.definition_id.as_str())
            )
    );
    reject_stale(&mut session, &stale_lift);
    record(&mut session, &content, ("fume_yards.enter_ash_hatch", None));
    record(
        &mut session,
        &content,
        ("fume_yards.return_with_brann", None),
    );
    session = checkpoint(&session, &content);
    assert_eq!(session.state().world.time, 18);
    assert_eq!(session.state().world.npcs[BRANN].location, BAY);
    assert_eq!(session.state().character.resources["coin"], 6);
    assert_eq!(session.state().world.npcs[DARO].inventory[FILTER], 1);
    assert!(
        !session.state().world.npcs[BRANN]
            .knowledge
            .contains_key("fume_yards.rack_cleared")
    );
    assert!(
        !session.state().world.locations[ASH]
            .flags
            .contains("fume_yards.salvage_assignment_active")
    );
    assert!(
        session.state().world.locations[ASH]
            .flags
            .contains("fume_yards.salvage_assignment_spent")
    );
    for &action in &ORDINARY[..5] {
        record(&mut session, &content, action);
    }
    session = checkpoint(&session, &content);
    assert_eq!(session.state().world.time, 23);
    assert_eq!(
        session.state().character.resources,
        BTreeMap::from([("coin".into(), 10), ("stamina".into(), 1)])
    );
    assert_eq!(session.state().character.inventory[FILTER], 1);
    assert_eq!(
        session.state().world.npcs[BRANN].knowledge["fume_yards.rack_cleared"].turn,
        21
    );
}

#[test]
fn staffing_saves_reject_forged_history_reordered_movement_and_shortened_work() {
    let content = parse_and_compile_production(SOURCE).unwrap();
    let mut session = start(&content, "saved-worker");
    for &action in PREFIX.iter().chain(STAFFED) {
        record(&mut session, &content, action);
    }
    checkpoint(&session, &content);
    let safe = session.player_trace().unwrap().to_json().unwrap();
    let wrong_history = safe.replace("saved-worker", "stole-permit");
    assert_ne!(wrong_history, safe);
    assert!(
        resume_player_trace(&PlayerTrace::from_json(&wrong_history).unwrap(), &content).is_err()
    );
    for defect in ["move", "order", "time", "transfer", "source"] {
        let mut trace = session.trace().clone();
        match defect {
            "move" => trace.steps[14]
                .events
                .retain(|event| !matches!(event.kind, EventKind::NpcMoved { .. })),
            "order" => trace.steps[14].events.reverse(),
            "time" => {
                let event = trace.steps[15]
                    .events
                    .iter_mut()
                    .find(|event| matches!(event.kind, EventKind::TimeAdvanced { .. }))
                    .unwrap();
                event.kind = EventKind::TimeAdvanced { ticks: 1 };
            }
            "transfer" => {
                let event = trace.steps[15]
                    .events
                    .iter_mut()
                    .find(|event| {
                        matches!(event.kind, EventKind::NpcItemTransferredToCharacter { .. })
                    })
                    .unwrap();
                event.turn = 18;
            }
            "source" => trace
                .initial_state
                .world
                .npcs
                .get_mut(DARO)
                .unwrap()
                .inventory
                .clear(),
            _ => unreachable!(),
        }
        let decoded = Trace::from_json(&trace.to_json().unwrap()).unwrap();
        assert!(verify(&decoded, &content).is_err(), "accepted {defect}");
    }
}
