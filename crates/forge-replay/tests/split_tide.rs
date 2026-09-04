use forge_content::parse_and_compile_production;
use forge_kernel::{
    ActionDefinition, CanonicalAction, Character, CompiledContent, Condition, ContentContract,
    ContentDraft, Effect, EntropyState, EventKind, GameState, KnowledgeProvenance,
    LocationDefinition, ScheduledEvent, TimedEventDefinition, WorldState,
};
use forge_replay::{PlayerTrace, Session, Trace, resume, resume_player_trace, verify};
use std::collections::{BTreeMap, BTreeSet};

const SPLIT_TIDE: &str = include_str!("../../../content/split-tide.json");

fn select(
    session: &Session<'_>,
    content: &forge_kernel::CompiledContent,
    definition_id: &str,
    parameter: Option<(&str, &str)>,
) -> CanonicalAction {
    forge_kernel::enumerate_legal_actions(session.state(), content)
        .expect("real state must enumerate")
        .into_iter()
        .find(|action| {
            action.definition_id == definition_id
                && parameter.is_none_or(|(name, value)| {
                    action
                        .parameters
                        .get(name)
                        .is_some_and(|found| found == value)
                })
        })
        .unwrap_or_else(|| panic!("missing legal action {definition_id}"))
}

fn content() -> forge_kernel::CompiledContent {
    parse_and_compile_production(SPLIT_TIDE).expect("Split Tide must compile")
}

fn record(
    session: &mut Session<'_>,
    content: &forge_kernel::CompiledContent,
    definition_id: &str,
    parameter: Option<(&str, &str)>,
) {
    let action = select(session, content, definition_id, parameter);
    session
        .record(&action)
        .unwrap_or_else(|error| panic!("recording {definition_id} failed: {error}"));
}

#[derive(Clone, Copy)]
struct ActionSpec {
    definition_id: &'static str,
    parameter: Option<(&'static str, &'static str)>,
}

const ILYAN_RELIEF_PATH: [ActionSpec; 11] = [
    ActionSpec {
        definition_id: "checkpoint.show_charter",
        parameter: None,
    },
    ActionSpec {
        definition_id: "travel_adjacent",
        parameter: Some(("destination", "lowsail.docks")),
    },
    ActionSpec {
        definition_id: "docks.ring_warning",
        parameter: None,
    },
    ActionSpec {
        definition_id: "travel_adjacent",
        parameter: Some(("destination", "lowsail.levee")),
    },
    ActionSpec {
        definition_id: "levee.relay_warning",
        parameter: None,
    },
    ActionSpec {
        definition_id: "levee.authority_path",
        parameter: None,
    },
    ActionSpec {
        definition_id: "floor.open_relief",
        parameter: None,
    },
    ActionSpec {
        definition_id: "travel_adjacent",
        parameter: Some(("destination", "red_sluice.top")),
    },
    ActionSpec {
        definition_id: "top.divert_relief",
        parameter: None,
    },
    ActionSpec {
        definition_id: "world.enter_aftermath",
        parameter: None,
    },
    ActionSpec {
        definition_id: "return.move_inland",
        parameter: None,
    },
];

const ROOK_TIDE_KEY_SPLIT_PATH: [ActionSpec; 11] = [
    ActionSpec {
        definition_id: "travel_adjacent",
        parameter: Some(("destination", "lowsail.docks")),
    },
    ActionSpec {
        definition_id: "docks.press_yara",
        parameter: None,
    },
    ActionSpec {
        definition_id: "docks.ask_oren",
        parameter: None,
    },
    ActionSpec {
        definition_id: "travel_adjacent",
        parameter: Some(("destination", "lowsail.levee")),
    },
    ActionSpec {
        definition_id: "levee.culvert_path",
        parameter: None,
    },
    ActionSpec {
        definition_id: "floor.key_calibration",
        parameter: None,
    },
    ActionSpec {
        definition_id: "travel_adjacent",
        parameter: Some(("destination", "red_sluice.top")),
    },
    ActionSpec {
        definition_id: "top.check_wheels",
        parameter: None,
    },
    ActionSpec {
        definition_id: "top.split_flow",
        parameter: None,
    },
    ActionSpec {
        definition_id: "world.enter_aftermath",
        parameter: None,
    },
    ActionSpec {
        definition_id: "return.share_water",
        parameter: None,
    },
];

fn record_specs(
    session: &mut Session<'_>,
    content: &forge_kernel::CompiledContent,
    specs: &[ActionSpec],
) {
    for spec in specs {
        record(session, content, spec.definition_id, spec.parameter);
    }
}

fn long_session_specs() -> Vec<ActionSpec> {
    let mut specs = ILYAN_RELIEF_PATH.to_vec();

    // Leave the ending through every connected post-outcome location. The
    // fifth move reaches the exact deadline, and the final move returns to
    // Lowsail after a long sequence of valid revisits.
    let roundtrip = [
        ActionSpec {
            definition_id: "travel_adjacent",
            parameter: Some(("destination", "red_sluice.top")),
        },
        ActionSpec {
            definition_id: "travel_adjacent",
            parameter: Some(("destination", "red_sluice.floor")),
        },
        ActionSpec {
            definition_id: "travel_adjacent",
            parameter: Some(("destination", "lowsail.levee")),
        },
        ActionSpec {
            definition_id: "travel_adjacent",
            parameter: Some(("destination", "lowsail_market")),
        },
        ActionSpec {
            definition_id: "travel_adjacent",
            parameter: Some(("destination", "lowsail.docks")),
        },
        ActionSpec {
            definition_id: "travel_adjacent",
            parameter: Some(("destination", "lowsail.levee")),
        },
        ActionSpec {
            definition_id: "world.enter_aftermath",
            parameter: None,
        },
    ];

    // Fifteen early roundtrips exercise persistent post-outcome revisits;
    // five waits make the final roundtrip begin at turn 121 and end at 128.
    for _ in 0..15 {
        specs.extend(roundtrip);
    }
    for _ in 0..5 {
        specs.push(ActionSpec {
            definition_id: "wait_tide",
            parameter: None,
        });
    }
    specs.extend(roundtrip);

    assert_eq!(specs.len(), 128);
    specs
}

fn resume_player_save<'content>(
    session: &Session<'content>,
    content: &'content forge_kernel::CompiledContent,
) -> Session<'content> {
    let save = session.player_trace().expect("authored session saves");
    let encoded = save.to_json().expect("player save serializes");
    let decoded = PlayerTrace::from_json(&encoded).expect("player save parses");
    resume_player_trace(&decoded, content).expect("player save resumes")
}

fn assert_not_legal(
    session: &Session<'_>,
    content: &forge_kernel::CompiledContent,
    definition_id: &str,
) {
    assert!(
        !forge_kernel::enumerate_legal_actions(session.state(), content)
            .expect("real state must enumerate")
            .iter()
            .any(|action| action.definition_id == definition_id),
        "{definition_id} unexpectedly reopened at {}",
        session.state().world.current_location
    );
}

fn assert_tide_key_inventory(session: &Session<'_>) {
    assert_eq!(
        session
            .state()
            .character
            .inventory
            .get("split_tide.tide_key"),
        Some(&1)
    );
    assert!(
        !session.state().world.npcs["yara_dene"]
            .inventory
            .contains_key("split_tide.tide_key")
    );
}

fn assert_closed_post_outcome_choices(
    session: &Session<'_>,
    content: &forge_kernel::CompiledContent,
) {
    for definition_id in [
        "checkpoint.show_charter",
        "docks.ring_warning",
        "levee.relay_warning",
        "floor.open_relief",
        "top.check_wheels",
        "top.split_flow",
        "top.hold_market",
        "top.divert_relief",
        "top.break_toll",
        "top.overload",
    ] {
        assert_not_legal(session, content, definition_id);
    }
}

#[test]
fn real_split_tide_path_round_trips_and_replays() {
    let content = content();
    let mut session = Session::new_game("ilyan", 71, &content).expect("session starts");

    record(&mut session, &content, "checkpoint.audit_order", None);
    record(&mut session, &content, "checkpoint.show_charter", None);
    record(
        &mut session,
        &content,
        "travel_adjacent",
        Some(("destination", "lowsail.levee")),
    );
    record(&mut session, &content, "levee.authority_path", None);
    record(&mut session, &content, "floor.read_harmonics", None);
    record(
        &mut session,
        &content,
        "travel_adjacent",
        Some(("destination", "red_sluice.top")),
    );
    record(&mut session, &content, "top.check_wheels", None);
    record(&mut session, &content, "top.split_flow", None);
    record(&mut session, &content, "world.enter_aftermath", None);

    assert!(session.state().world.flags.contains("flow_split"));
    assert_eq!(
        session.trace().steps.last().unwrap().observation.text,
        "You enter Lowsail's aftermath and face what the changed water has done. The market stands above calm water while both shores still receive a share."
    );

    let encoded = session.trace().to_json().expect("trace serializes");
    let decoded = Trace::from_json(&encoded).expect("trace parses");
    let verified = verify(&decoded, &content).expect("real trace verifies");
    assert_eq!(&verified, session.state());
    assert_eq!(decoded.final_state_id, verified.state_id());
    assert_eq!(decoded.final_receipt, decoded.steps.last().unwrap().receipt);
}

#[test]
fn rook_tide_key_path_resumes_across_transfer_and_calibration_checkpoints() {
    let content = content();
    let mut uninterrupted = Session::new_game("rook", 71, &content).expect("session starts");
    let mut after_transfer = None;
    let mut after_calibration = None;

    for (index, spec) in ROOK_TIDE_KEY_SPLIT_PATH.iter().enumerate() {
        record(
            &mut uninterrupted,
            &content,
            spec.definition_id,
            spec.parameter,
        );
        match index + 1 {
            2 => after_transfer = Some(resume_player_save(&uninterrupted, &content)),
            6 => after_calibration = Some(resume_player_save(&uninterrupted, &content)),
            _ => {}
        }
    }

    let mut resumed_after_transfer = after_transfer.expect("transfer checkpoint");
    assert_eq!(resumed_after_transfer.trace().steps.len(), 2);
    assert_eq!(resumed_after_transfer.state().world.time, 2);
    assert_tide_key_inventory(&resumed_after_transfer);
    assert_not_legal(&resumed_after_transfer, &content, "docks.press_yara");
    record_specs(
        &mut resumed_after_transfer,
        &content,
        &ROOK_TIDE_KEY_SPLIT_PATH[2..],
    );

    let mut resumed_after_calibration = after_calibration.expect("calibration checkpoint");
    assert_eq!(resumed_after_calibration.trace().steps.len(), 6);
    assert_eq!(resumed_after_calibration.state().world.time, 6);
    assert!(
        resumed_after_calibration
            .state()
            .world
            .flags
            .contains("sluice_calibrated")
    );
    assert!(
        resumed_after_calibration
            .state()
            .character
            .deeds
            .contains("calibrated_with_tide_key")
    );
    assert_tide_key_inventory(&resumed_after_calibration);
    assert_not_legal(
        &resumed_after_calibration,
        &content,
        "floor.key_calibration",
    );
    record_specs(
        &mut resumed_after_calibration,
        &content,
        &ROOK_TIDE_KEY_SPLIT_PATH[6..],
    );

    assert_eq!(resumed_after_transfer.state(), uninterrupted.state());
    assert_eq!(resumed_after_calibration.state(), uninterrupted.state());
    assert_eq!(
        resumed_after_transfer.trace().final_receipt,
        uninterrupted.trace().final_receipt
    );
    assert_eq!(
        resumed_after_calibration.trace().final_receipt,
        uninterrupted.trace().final_receipt
    );

    assert_eq!(uninterrupted.state().world.time, 11);
    assert!(
        uninterrupted
            .state()
            .world
            .flags
            .contains("tide_key_offered")
    );
    assert!(
        uninterrupted
            .state()
            .world
            .flags
            .contains("sluice_calibrated")
    );
    assert!(uninterrupted.state().world.flags.contains("flow_split"));
    assert!(uninterrupted.state().world.flags.contains("ending_accord"));
    assert_tide_key_inventory(&uninterrupted);

    let transfers = uninterrupted
        .trace()
        .steps
        .iter()
        .flat_map(|step| step.events.iter())
        .filter(|event| {
            matches!(
                &event.kind,
                EventKind::NpcItemTransferredToCharacter { npc, item, count }
                    if npc == "yara_dene"
                        && item == "split_tide.tide_key"
                        && *count == 1
            )
        })
        .count();
    assert_eq!(transfers, 1);
    assert_eq!(
        uninterrupted.trace().steps[1].action.definition_id,
        "docks.press_yara"
    );
    assert_eq!(
        uninterrupted.trace().steps[5].action.definition_id,
        "floor.key_calibration"
    );

    let resumed_full = resume_player_save(&uninterrupted, &content);
    assert_eq!(resumed_full.state(), uninterrupted.state());
    assert_eq!(
        resumed_full.trace().final_receipt,
        uninterrupted.trace().final_receipt
    );
}

#[test]
fn ilyan_relief_path_survives_128_turn_player_save_checkpoints() {
    let content = content();
    let specs = long_session_specs();

    let mut uninterrupted = Session::new_game("ilyan", 71, &content).expect("session starts");
    record_specs(&mut uninterrupted, &content, &specs);

    let resumed_final = resume_player_save(&uninterrupted, &content);
    assert_eq!(resumed_final.state(), uninterrupted.state());
    assert_eq!(
        resumed_final.trace().final_receipt,
        uninterrupted.trace().final_receipt
    );

    assert_eq!(uninterrupted.trace().steps.len(), 128);
    assert_eq!(uninterrupted.state().world.time, 128);
    assert!(uninterrupted.state().world.flags.contains("flow_relief"));
    assert!(
        uninterrupted
            .state()
            .world
            .flags
            .contains("sluice_outcome_chosen")
    );
    assert!(uninterrupted.state().world.flags.contains("ending_relief"));
    assert!(!uninterrupted.state().world.flags.contains("surge_missed"));
    assert_eq!(
        uninterrupted.state().world.current_location,
        "lowsail.return"
    );
    let final_aftermath = uninterrupted
        .trace()
        .steps
        .last()
        .expect("final roundtrip returns to Lowsail");
    assert_eq!(
        final_aftermath.action.definition_id,
        "world.enter_aftermath"
    );
    assert!(final_aftermath.observation.text.contains(
        "Empty stalls mark the old market as families carry goods toward higher ground."
    ));
    assert_eq!(
        content
            .location_description(uninterrupted.state())
            .expect("return aftermath observes"),
        "Empty stalls mark the old market as families carry goods toward higher ground."
    );

    let oren = uninterrupted
        .state()
        .world
        .npcs
        .get("oren_pell")
        .expect("Oren exists");
    let oren_warning = oren
        .knowledge
        .get("market_warned")
        .expect("Oren witnessed the warning");
    assert_eq!(oren_warning.subject, "Lowsail expects a surge.");
    assert_eq!(oren_warning.turn, 2);
    assert_eq!(oren_warning.provenance, KnowledgeProvenance::Witnessed);

    let edrik = uninterrupted
        .state()
        .world
        .npcs
        .get("edrik_voss")
        .expect("Edrik exists");
    let edrik_warning = edrik
        .knowledge
        .get("market_warned")
        .expect("Edrik received the warning");
    assert_eq!(edrik_warning.subject, "Lowsail expects a surge.");
    assert_eq!(edrik_warning.turn, 4);
    assert_eq!(
        edrik_warning.provenance,
        KnowledgeProvenance::Rumor {
            from: Some("oren_pell".to_owned()),
        }
    );
    let relief_plan = edrik
        .knowledge
        .get("relief_plan")
        .expect("Edrik witnessed the relief plan");
    assert_eq!(
        relief_plan.subject,
        "A relief channel can spare the market."
    );
    assert_eq!(relief_plan.turn, 6);
    assert_eq!(relief_plan.provenance, KnowledgeProvenance::Witnessed);

    // Relay is one-shot before the outcome, and the outcome choices remain
    // closed after the ending, including during post-outcome revisits.
    let mut before_relay = Session::new_game("ilyan", 71, &content).expect("session starts");
    record_specs(&mut before_relay, &content, &ILYAN_RELIEF_PATH[..4]);
    assert_eq!(before_relay.state().world.time, 4);
    assert_eq!(before_relay.state().world.scheduled_events.len(), 1);
    assert_eq!(before_relay.state().world.scheduled_events[0].due_time, 16);
    let mut resumed_before_relay = resume_player_save(&before_relay, &content);
    record_specs(&mut resumed_before_relay, &content, &specs[4..]);

    let mut after_outcome = Session::new_game("ilyan", 71, &content).expect("session starts");
    record_specs(&mut after_outcome, &content, &ILYAN_RELIEF_PATH[..9]);
    assert_eq!(after_outcome.state().world.time, 9);
    assert_eq!(after_outcome.state().world.scheduled_events.len(), 1);
    assert_eq!(after_outcome.state().world.scheduled_events[0].due_time, 16);
    assert!(
        after_outcome
            .state()
            .world
            .flags
            .contains("sluice_outcome_chosen")
    );
    let mut resumed_after_outcome = resume_player_save(&after_outcome, &content);
    record_specs(&mut resumed_after_outcome, &content, &specs[9..]);

    for resumed in [&resumed_before_relay, &resumed_after_outcome] {
        assert_eq!(resumed.trace().steps.len(), 128);
        assert_eq!(resumed.state(), uninterrupted.state());
        assert_eq!(
            resumed.trace().final_state_id,
            uninterrupted.trace().final_state_id
        );
        assert_eq!(
            resumed.trace().final_receipt,
            uninterrupted.trace().final_receipt
        );
    }

    let boundary_step = uninterrupted
        .trace()
        .steps
        .get(ILYAN_RELIEF_PATH.len() + 4)
        .expect("deadline boundary step exists");
    assert_eq!(uninterrupted.state().world.time, 128);
    assert!(boundary_step.events.iter().any(|event| {
        event.turn == 16
            && matches!(
                event.kind,
                EventKind::ScheduledEventResolved {
                    ref event_id,
                    ref event_kind,
                    applied: false,
                } if event_id == "lowsail.next_surge" && event_kind == "deadline"
            )
    }));
    let resolutions = uninterrupted
        .state()
        .event_log
        .iter()
        .filter(|event| {
            matches!(
                event.kind,
                EventKind::ScheduledEventResolved {
                    ref event_id,
                    ref event_kind,
                    applied: false,
                } if event_id == "lowsail.next_surge" && event_kind == "deadline"
            )
        })
        .count();
    assert_eq!(resolutions, 1);
    assert!(uninterrupted.state().world.scheduled_events.is_empty());

    // The first six suffix moves revisit top, floor, levee, market, docks,
    // then levee again. Check the relevant catalogs at those actual states.
    let mut catalog_probe = Session::new_game("ilyan", 71, &content).expect("session starts");
    record_specs(&mut catalog_probe, &content, &ILYAN_RELIEF_PATH);
    record(
        &mut catalog_probe,
        &content,
        "travel_adjacent",
        Some(("destination", "red_sluice.top")),
    );
    assert_eq!(
        catalog_probe.state().world.current_location,
        "red_sluice.top"
    );
    assert_closed_post_outcome_choices(&catalog_probe, &content);
    record(
        &mut catalog_probe,
        &content,
        "travel_adjacent",
        Some(("destination", "red_sluice.floor")),
    );
    assert_eq!(
        catalog_probe.state().world.current_location,
        "red_sluice.floor"
    );
    assert_closed_post_outcome_choices(&catalog_probe, &content);
    record(
        &mut catalog_probe,
        &content,
        "travel_adjacent",
        Some(("destination", "lowsail.levee")),
    );
    assert_eq!(
        catalog_probe.state().world.current_location,
        "lowsail.levee"
    );
    assert_closed_post_outcome_choices(&catalog_probe, &content);
    record(
        &mut catalog_probe,
        &content,
        "travel_adjacent",
        Some(("destination", "lowsail_market")),
    );
    record(
        &mut catalog_probe,
        &content,
        "travel_adjacent",
        Some(("destination", "lowsail.docks")),
    );
    assert_eq!(
        catalog_probe.state().world.current_location,
        "lowsail.docks"
    );
    assert_closed_post_outcome_choices(&catalog_probe, &content);
    record(
        &mut catalog_probe,
        &content,
        "travel_adjacent",
        Some(("destination", "lowsail.levee")),
    );
    assert_eq!(
        catalog_probe.state().world.current_location,
        "lowsail.levee"
    );
    assert_closed_post_outcome_choices(&catalog_probe, &content);
}

#[test]
fn malformed_production_state_is_rejected_without_panicking() {
    let content = content();
    let mut malformed = content.new_game("ilyan", 71).expect("new game");
    malformed.world.locations.remove("lowsail_market");
    let error = match Session::new(malformed, &content) {
        Ok(_) => panic!("malformed state must fail"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("location"));
}

#[test]
fn rescued_worker_report_replays_with_its_source_and_miras_role() {
    let content = content();
    let mut session = Session::new_game("rook", 71, &content).unwrap();
    record(
        &mut session,
        &content,
        "travel_adjacent",
        Some(("destination", "lowsail.levee")),
    );
    record(&mut session, &content, "levee.help_worker", None);
    let decoded = Trace::from_json(&session.trace().to_json().unwrap()).unwrap();
    let verified = verify(&decoded, &content).unwrap();
    assert_eq!(&verified, session.state());
    assert_eq!(
        verified.world.npcs["mira_kett"].memories["levee_worker_helped"].provenance,
        KnowledgeProvenance::Read {
            source: "levee_worker_report".to_owned()
        }
    );
    assert_eq!(
        decoded.steps[1].observation.result.as_deref(),
        Some("You pull the worker clear. Their report reaches Mira, Red Sluice's crew leader.")
    );
    let resumed = resume_player_save(&session, &content);
    assert_eq!(resumed.state(), session.state());
    assert_eq!(resumed.trace().final_receipt, session.trace().final_receipt);
}

fn timed_event_fixture_content() -> CompiledContent {
    let event = |id: &str, due_time: u64, condition: Condition, flag: &str, result: &str| {
        TimedEventDefinition {
            id: id.to_owned(),
            due_time,
            event_kind: "fixture".to_owned(),
            label: format!("Event {id}"),
            result: result.to_owned(),
            condition,
            effects: vec![Effect::SetWorldFlag {
                flag: flag.to_owned(),
                value: true,
            }],
        }
    };

    CompiledContent::try_compile(ContentDraft {
        schema_version: "forge-schema-v5".to_owned(),
        rules_version: "forge-rules-v3".to_owned(),
        world_id: "timed-fixture".to_owned(),
        contract: ContentContract::Fixture,
        start_location: "room".to_owned(),
        character_presets: Vec::new(),
        character_creation: None,
        locations: vec![LocationDefinition {
            id: "room".to_owned(),
            name: "Room".to_owned(),
            description: "A quiet test room.".to_owned(),
            description_variants: Vec::new(),
            exits: Vec::new(),
            terminal: true,
        }],
        npcs: Vec::new(),
        // Deliberately shuffled: equal-time events must be ordered by ID.
        timed_events: vec![
            event(
                "c.after",
                4,
                Condition::WorldFlag {
                    flag: "b_seen".to_owned(),
                },
                "c_seen",
                "C resolved.",
            ),
            event(
                "b.requires",
                2,
                Condition::WorldFlag {
                    flag: "a_seeded".to_owned(),
                },
                "b_seen",
                "B resolved.",
            ),
            event(
                "future",
                8,
                Condition::Always,
                "future_seen",
                "Future resolved.",
            ),
            event(
                "a.seed",
                2,
                Condition::WorldFlag {
                    flag: "jumped".to_owned(),
                },
                "a_seeded",
                "A resolved.",
            ),
        ],
        actions: vec![ActionDefinition {
            id: "jump_five".to_owned(),
            label: "Jump Five".to_owned(),
            category: "Travel".to_owned(),
            result: "Jumped five ticks.".to_owned(),
            result_variants: Vec::new(),
            locations: vec!["room".to_owned()],
            condition: Condition::Always,
            effects: vec![
                // This flag makes the action-before-events boundary observable.
                Effect::SetWorldFlag {
                    flag: "jumped".to_owned(),
                    value: true,
                },
                Effect::AdvanceTime { ticks: 5 },
            ],
            parameters: Vec::new(),
            meaningful: false,
            movement: false,
        }],
    })
    .expect("timed event fixture compiles")
}

fn timed_event_fixture_state(content: &CompiledContent) -> GameState {
    let mut state = GameState::new(
        content.build_id().to_owned(),
        WorldState::new(
            content.world_id().to_owned(),
            "room",
            content.empty_location_runtime(),
            BTreeMap::new(),
        ),
        Character {
            id: "hero".to_owned(),
            lineage: "fenborn".to_owned(),
            origin: "room".to_owned(),
            background: "tester".to_owned(),
            aptitudes: BTreeMap::new(),
            skills: BTreeSet::new(),
            values: BTreeSet::new(),
            traits: BTreeSet::new(),
            flaws: BTreeSet::new(),
            appearance: BTreeMap::new(),
            affiliations: BTreeMap::new(),
            reputation: BTreeMap::new(),
            knowledge: BTreeSet::new(),
            inventory: BTreeMap::new(),
            resources: BTreeMap::new(),
            injuries: BTreeSet::new(),
            deeds: BTreeSet::new(),
            promises: BTreeSet::new(),
            discoveries: BTreeSet::new(),
            facets: BTreeMap::new(),
        },
        EntropyState::new(17),
    );
    state.world.scheduled_events = content
        .timed_events()
        .map(|(_, event)| ScheduledEvent {
            id: event.id.clone(),
            due_time: event.due_time,
            event_kind: event.event_kind.clone(),
        })
        .collect();
    state.world.scheduled_events.sort_by(|left, right| {
        left.due_time
            .cmp(&right.due_time)
            .then_with(|| left.id.cmp(&right.id))
    });
    state
}

fn timed_event_action(session: &Session<'_>, content: &CompiledContent) -> CanonicalAction {
    forge_kernel::enumerate_legal_actions(session.state(), content)
        .expect("fixture state enumerates")
        .into_iter()
        .find(|action| action.definition_id == "jump_five")
        .expect("jump action is legal")
}

fn resolved_event_ids(events: &[forge_kernel::Event]) -> Vec<&str> {
    events
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::ScheduledEventResolved { event_id, .. } => Some(event_id.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn shuffled_timed_events_cross_atomically_and_resume_with_parity() {
    let content = timed_event_fixture_content();
    let initial = timed_event_fixture_state(&content);
    content
        .validate_state(&initial)
        .expect("fixture state has the compiled schedule");
    assert_eq!(
        initial
            .world
            .scheduled_events
            .iter()
            .map(|event| event.id.as_str())
            .collect::<Vec<_>>(),
        vec!["a.seed", "b.requires", "c.after", "future"]
    );

    let initial_before_step = initial.clone();
    let mut continuous = Session::new(initial.clone(), &content).expect("session starts");
    let first_action = timed_event_action(&continuous, &content);
    let direct = forge_kernel::step(&initial, &first_action, &content, &initial.entropy)
        .expect("five-tick jump succeeds");
    assert_eq!(initial, initial_before_step, "step leaves input immutable");
    assert_eq!(direct.state().world.time, 5);
    assert_eq!(
        resolved_event_ids(direct.events()),
        vec!["a.seed", "b.requires", "c.after"]
    );
    assert!(
        direct
            .events()
            .iter()
            .filter_map(|event| match &event.kind {
                EventKind::ScheduledEventResolved { .. } => Some(event.turn),
                _ => None,
            })
            .all(|turn| turn == 5)
    );

    let first = continuous
        .record(&first_action)
        .expect("first jump records");
    assert_eq!(first.events, direct.events().to_vec());
    assert_eq!(continuous.state().world.time, 5);
    assert!(continuous.state().world.flags.contains("jumped"));
    assert!(continuous.state().world.flags.contains("a_seeded"));
    assert!(continuous.state().world.flags.contains("b_seen"));
    assert!(continuous.state().world.flags.contains("c_seen"));
    assert!(!continuous.state().world.flags.contains("future_seen"));
    assert_eq!(continuous.state().world.scheduled_events.len(), 1);
    assert_eq!(continuous.state().world.scheduled_events[0].id, "future");
    assert_eq!(
        first.observation.result.as_deref(),
        Some("Jumped five ticks. A resolved. B resolved. C resolved.")
    );
    assert!(first.observation.text.contains("A resolved."));
    assert!(first.observation.text.contains("B resolved."));
    assert!(first.observation.text.contains("C resolved."));

    let checkpoint = Trace::from_json(
        &continuous
            .trace()
            .to_json()
            .expect("checkpoint trace serializes"),
    )
    .expect("checkpoint trace parses");
    let mut resumed = resume(&checkpoint, &content).expect("checkpoint resumes");
    let second_action = timed_event_action(&continuous, &content);
    continuous
        .record(&second_action)
        .expect("second jump records");
    resumed
        .record(&second_action)
        .expect("resumed jump records");

    assert_eq!(continuous.state(), resumed.state());
    assert_eq!(continuous.trace(), resumed.trace());
    assert_eq!(continuous.state().world.time, 10);
    assert!(continuous.state().world.flags.contains("future_seen"));
    assert!(continuous.state().world.scheduled_events.is_empty());
    assert_eq!(
        resolved_event_ids(&continuous.trace().steps[1].events),
        vec!["future"]
    );
    assert!(
        continuous.trace().steps[1]
            .events
            .iter()
            .filter_map(|event| match &event.kind {
                EventKind::ScheduledEventResolved { .. } => Some(event.turn),
                _ => None,
            })
            .all(|turn| turn == 10)
    );
    assert_eq!(
        continuous.trace().steps[1].observation.result.as_deref(),
        Some("Jumped five ticks. Future resolved.")
    );
    assert_eq!(
        continuous
            .trace()
            .steps
            .iter()
            .flat_map(|step| resolved_event_ids(&step.events))
            .collect::<Vec<_>>(),
        vec!["a.seed", "b.requires", "c.after", "future"]
    );

    let encoded = continuous.trace().to_json().expect("trace serializes");
    let decoded = Trace::from_json(&encoded).expect("trace parses");
    let verified = verify(&decoded, &content).expect("trace verifies");
    assert_eq!(verified, *continuous.state());
    let resumed_full = resume(&decoded, &content).expect("full trace resumes");
    assert_eq!(resumed_full.state(), continuous.state());
    assert_eq!(resumed_full.trace(), continuous.trace());

    let page = content
        .action_page(&initial, 0, 1)
        .expect("action page renders");
    assert_eq!(page.actions[0].time_cost.minimum_ticks, 5);
    assert_eq!(page.actions[0].time_cost.maximum_ticks, 5);
}
