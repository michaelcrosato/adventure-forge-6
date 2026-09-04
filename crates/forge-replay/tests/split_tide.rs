use forge_content::parse_and_compile_production;
use forge_kernel::{CanonicalAction, EventKind, KnowledgeProvenance};
use forge_replay::{PlayerTrace, Session, Trace, resume_player_trace, verify};

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
