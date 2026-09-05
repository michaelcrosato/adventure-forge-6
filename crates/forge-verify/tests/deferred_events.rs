use forge_content::parse_and_compile_production;
use forge_kernel::{
    CanonicalAction, CompiledContent, Event, EventKind, KernelError, ScheduledEvent,
    enumerate_legal_actions,
};
use forge_replay::{PlayerTrace, ReplayError, Session, resume_player_trace, verify};
use std::collections::BTreeMap;

const SOURCE: &str = include_str!("../../../content/split-tide.json");
const KILN: &str = "fume_yards.kiln_bay";
const WORKSHOP: &str = "fume_yards.workshop";
const IGNITE: &str = "fume_yards.ignite_batch";
const DRAW: &str = "fume_yards.draw_filter";
const READY: &str = "fume_yards.batch_ready";
const SPOIL: &str = "fume_yards.batch_spoil";
const ABSOLUTE_MARKER: &str = "deferred deadlines ignored ignition world time";
const REMOTE_MARKER: &str = "deferred batch paused outside its kiln";

fn select(
    session: &Session<'_>,
    content: &CompiledContent,
    definition: &str,
    destination: Option<&str>,
) -> CanonicalAction {
    let parameters = destination
        .map(|location| BTreeMap::from([("destination".to_owned(), location.to_owned())]))
        .unwrap_or_default();
    enumerate_legal_actions(session.state(), content)
        .expect("production action catalog enumerates")
        .into_iter()
        .find(|action| action.definition_id == definition && action.parameters == parameters)
        .unwrap_or_else(|| panic!("missing exact canonical action {definition}: {parameters:?}"))
}

fn record(
    session: &mut Session<'_>,
    content: &CompiledContent,
    definition: &str,
    destination: Option<&str>,
    marker: &str,
) {
    let action = select(session, content, definition, destination);
    session
        .record(&action)
        .unwrap_or_else(|error| panic!("{marker}: {definition}: {error}"));
}

fn prepared_late_batch(content: &CompiledContent) -> Session<'_> {
    let mut session = Session::new_game("ilyan", 71, content).expect("Ilyan starts canonically");
    for (definition, destination) in [
        ("checkpoint.show_charter", None),
        ("travel_adjacent", Some("lowsail.levee")),
        ("levee.authority_path", None),
        ("travel_adjacent", Some("red_sluice.top")),
        ("top.hold_market", None),
        ("world.enter_aftermath", None),
        ("return.count_dry_stalls", None),
        ("return.visit_workshop", None),
        ("fume_yards.take_stock", None),
        ("travel_adjacent", Some(KILN)),
        ("fume_yards.take_cask", None),
        ("fume_yards.take_fuel", None),
        ("fume_yards.prepare_charge", None),
        ("fume_yards.fit_wet_screen", None),
    ] {
        record(
            &mut session,
            content,
            definition,
            destination,
            "batch preparation failed",
        );
    }
    assert_eq!(session.state().world.time, 14);
    assert_eq!(session.state().world.current_location, KILN);
    assert_eq!(
        session.state().character.inventory["fume_yards.prepared_charge"],
        1
    );
    assert_eq!(session.state().character.inventory["fume_yards.fuel"], 1);
    assert!(
        !session
            .state()
            .character
            .inventory
            .contains_key("fume_yards.batch_claim")
    );
    assert!(session.state().world.flags.contains("ending_council"));
    assert_eq!(
        session.state().world.scheduled_events,
        vec![ScheduledEvent {
            id: "lowsail.next_surge".to_owned(),
            due_time: 16,
            event_kind: "deadline".to_owned(),
        }]
    );
    session
}

fn checkpoint(session: &Session<'_>, content: &CompiledContent) {
    let before = session.state().clone();
    let save = session
        .player_trace()
        .expect("canonical checkpoint exports");
    let json = save.to_json().expect("checkpoint serializes");
    for hidden in [
        "EventScheduled",
        "event_scheduled",
        "scheduled_events",
        "batch_claim",
        "due_time",
        "inventory",
        "entropy",
    ] {
        assert!(
            !json.contains(hidden),
            "checkpoint exposed hidden field {hidden}"
        );
    }
    let decoded = PlayerTrace::from_json(&json).expect("checkpoint decodes");
    let resumed =
        resume_player_trace(&decoded, content).expect("checkpoint reconstructs canonically");
    assert_eq!(resumed.state(), &before);
    assert_eq!(resumed.trace(), session.trace());
    assert_eq!(
        content.observe(resumed.state()).unwrap(),
        content.observe(&before).unwrap()
    );
    assert_eq!(
        enumerate_legal_actions(resumed.state(), content).unwrap(),
        enumerate_legal_actions(&before, content).unwrap()
    );
    assert_eq!(
        verify(session.trace(), content).expect("detailed trace verifies"),
        before
    );
    assert_eq!(session.state(), &before);
}

fn assert_scheduling(session: &Session<'_>) {
    let expected = vec![
        Event {
            turn: 14,
            kind: EventKind::EventScheduled {
                event_id: READY.to_owned(),
                event_kind: "production".to_owned(),
                due_time: 16,
            },
        },
        Event {
            turn: 14,
            kind: EventKind::EventScheduled {
                event_id: SPOIL.to_owned(),
                event_kind: "production".to_owned(),
                due_time: 19,
            },
        },
    ];
    let actual: Vec<_> = session
        .state()
        .event_log
        .iter()
        .filter(|event| matches!(event.kind, EventKind::EventScheduled { .. }))
        .cloned()
        .collect();
    assert_eq!(actual, expected, "{ABSOLUTE_MARKER}");
}

fn assert_resolutions(session: &Session<'_>, spoiled: bool) {
    let actual: Vec<_> = session
        .state()
        .event_log
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::ScheduledEventResolved {
                event_id,
                event_kind,
                applied,
            } => Some((event.turn, event_id.as_str(), event_kind.as_str(), *applied)),
            _ => None,
        })
        .collect();
    assert_eq!(
        actual,
        vec![
            (16, READY, "production", true),
            (16, "lowsail.next_surge", "deadline", false),
            (19, SPOIL, "production", spoiled),
        ]
    );
    assert!(session.state().world.scheduled_events.is_empty());
    assert!(session.state().world.flags.contains("ending_council"));
    assert!(session.state().world.flags.contains("flow_locked_market"));
    assert!(!session.state().world.flags.contains("surge_missed"));
    assert!(!session.state().world.flags.contains("sluice_failure"));
}

fn assert_stale_inert(
    session: &mut Session<'_>,
    content: &CompiledContent,
    draw: &CanonicalAction,
) {
    let before = session.state().clone();
    let trace = session.trace().clone();
    let save = session.player_trace().unwrap();
    assert!(matches!(
        session.record(draw),
        Err(ReplayError::Kernel(KernelError::StaleAction { .. }))
    ));
    let current =
        CanonicalAction::new(content.build_id(), before.state_id(), DRAW, BTreeMap::new());
    assert!(matches!(
        session.record(&current),
        Err(ReplayError::Kernel(KernelError::IllegalAction(_)))
    ));
    assert_eq!(session.state(), &before);
    assert_eq!(session.trace(), &trace);
    assert_eq!(session.player_trace().unwrap(), save);
}

#[test]
fn production_deferred_deadlines_use_ignition_world_time() {
    let content = parse_and_compile_production(SOURCE).expect("production content compiles");
    let mut session = prepared_late_batch(&content);
    checkpoint(&session, &content);
    record(&mut session, &content, IGNITE, None, ABSOLUTE_MARKER);
    assert_eq!(session.state().world.time, 15);
    assert_scheduling(&session);
    assert_eq!(
        session.state().character.inventory["fume_yards.batch_claim"],
        1
    );
    for spent in ["fume_yards.prepared_charge", "fume_yards.fuel"] {
        assert!(!session.state().character.inventory.contains_key(spent));
    }
    let scheduled: Vec<_> = session
        .state()
        .world
        .scheduled_events
        .iter()
        .map(|event| (event.id.as_str(), event.due_time))
        .collect();
    assert_eq!(
        scheduled,
        vec![(READY, 16), ("lowsail.next_surge", 16), (SPOIL, 19)],
        "{ABSOLUTE_MARKER}"
    );
    checkpoint(&session, &content);
    record(&mut session, &content, "wait_tide", None, ABSOLUTE_MARKER);
    assert_eq!(session.state().world.time, 16);
    let draw = select(&session, &content, DRAW, None);
    checkpoint(&session, &content);
    session
        .record(&draw)
        .expect("draw during the ready window succeeds");
    while session.state().world.time < 19 {
        record(&mut session, &content, "wait_tide", None, ABSOLUTE_MARKER);
    }
    assert_eq!(
        session.state().character.inventory.get("fume_yards.filter"),
        Some(&1)
    );
    assert!(
        !session
            .state()
            .character
            .inventory
            .contains_key("fume_yards.batch_claim")
    );
    assert!(
        !session
            .state()
            .character
            .inventory
            .contains_key("fume_yards.spoiled_charge")
    );
    assert_resolutions(&session, false);
    assert_stale_inert(&mut session, &content, &draw);
    checkpoint(&session, &content);
}

#[test]
fn production_deferred_spoil_consumes_claim_away_from_kiln() {
    let content = parse_and_compile_production(SOURCE).expect("production content compiles");
    let mut session = prepared_late_batch(&content);
    record(
        &mut session,
        &content,
        IGNITE,
        None,
        "remote batch ignition failed",
    );
    assert_scheduling(&session);
    record(
        &mut session,
        &content,
        "travel_adjacent",
        Some(WORKSHOP),
        REMOTE_MARKER,
    );
    assert_eq!(session.state().world.time, 16);
    assert_eq!(session.state().world.current_location, WORKSHOP);
    assert!(
        session.state().world.locations[KILN]
            .flags
            .contains("fume_yards.batch_ready")
    );
    assert_eq!(
        session.state().character.inventory["fume_yards.batch_claim"],
        1
    );
    checkpoint(&session, &content);
    record(
        &mut session,
        &content,
        "travel_adjacent",
        Some(KILN),
        REMOTE_MARKER,
    );
    let draw = select(&session, &content, DRAW, None);
    record(
        &mut session,
        &content,
        "travel_adjacent",
        Some(WORKSHOP),
        REMOTE_MARKER,
    );
    assert_eq!(session.state().world.time, 18);
    checkpoint(&session, &content);
    record(&mut session, &content, "wait_tide", None, REMOTE_MARKER);
    assert_eq!(session.state().world.time, 19);
    assert_eq!(session.state().world.current_location, WORKSHOP);
    assert_eq!(
        session
            .state()
            .character
            .inventory
            .get("fume_yards.spoiled_charge"),
        Some(&1),
        "{REMOTE_MARKER}"
    );
    assert!(
        !session
            .state()
            .character
            .inventory
            .contains_key("fume_yards.batch_claim"),
        "{REMOTE_MARKER}"
    );
    assert!(
        !session
            .state()
            .character
            .inventory
            .contains_key("fume_yards.filter")
    );
    let spoil_events: Vec<_> = session
        .state()
        .event_log
        .iter()
        .filter(|event| {
            matches!(&event.kind,
        EventKind::RecipeApplied { recipe, .. } if recipe == "fume_yards.spoil_batch")
        })
        .cloned()
        .collect();
    assert_eq!(
        spoil_events,
        vec![Event {
            turn: 19,
            kind: EventKind::RecipeApplied {
                recipe: "fume_yards.spoil_batch".to_owned(),
                inputs: BTreeMap::from([("fume_yards.batch_claim".to_owned(), 1)]),
                outputs: BTreeMap::from([("fume_yards.spoiled_charge".to_owned(), 1)]),
            }
        }],
        "{REMOTE_MARKER}"
    );
    assert_resolutions(&session, true);
    checkpoint(&session, &content);
    record(
        &mut session,
        &content,
        "travel_adjacent",
        Some(KILN),
        REMOTE_MARKER,
    );
    assert_eq!(
        session.state().character.inventory["fume_yards.spoiled_charge"],
        1
    );
    assert!(
        session.state().world.locations[KILN]
            .flags
            .contains("fume_yards.freight_spoiled")
    );
    let catalog = enumerate_legal_actions(session.state(), &content).unwrap();
    for lost_job in [
        "fume_yards.load_kiln_freight",
        "fume_yards.load_filtered_kiln_freight",
    ] {
        assert!(
            catalog
                .iter()
                .all(|action| action.definition_id != lost_job)
        );
    }
    assert_stale_inert(&mut session, &content, &draw);
    checkpoint(&session, &content);
}
