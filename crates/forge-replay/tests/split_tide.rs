use forge_content::parse_and_compile_production;
use forge_kernel::{
    ActionDefinition, CanonicalAction, Character, CompiledContent, Condition, ContentContract,
    ContentDraft, Effect, EntropyState, EventKind, GameState, KnowledgeProvenance,
    LocationDefinition, ScheduledEvent, SupplyLabels, TimedEventDefinition, WorldState,
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

fn record_one_tick(
    session: &mut Session<'_>,
    content: &forge_kernel::CompiledContent,
    definition_id: &str,
    parameter: Option<(&str, &str)>,
) {
    let action = select(session, content, definition_id, parameter);
    let page = content
        .action_page(session.state(), 0, usize::MAX)
        .expect("one-tick action page renders");
    let view = page
        .actions
        .iter()
        .find(|view| view.action_id == action.action_id)
        .unwrap_or_else(|| panic!("missing action view for {definition_id}"));
    assert_eq!(view.time_cost.minimum_ticks, 1);
    assert_eq!(view.time_cost.maximum_ticks, 1);
    let before_time = session.state().world.time;
    let recorded = session
        .record(&action)
        .unwrap_or_else(|error| panic!("recording {definition_id} failed: {error}"));
    let expected_time = before_time
        .checked_add(1)
        .expect("one-tick route time does not overflow");
    assert_eq!(session.state().world.time, expected_time);
    assert_eq!(recorded.observation.world_time, expected_time);
}

fn assert_views_inert(session: &Session<'_>, content: &forge_kernel::CompiledContent) {
    let state_id = session.state().state_id();
    let receipt = session.trace().final_receipt.clone();
    let save = session.player_trace().expect("save before viewing");
    let first_observation = content
        .observe(session.state())
        .expect("observation renders");
    let second_observation = content
        .observe(session.state())
        .expect("observation repeats");
    assert_eq!(first_observation, second_observation);
    let first_page = content
        .action_page(session.state(), 0, usize::MAX)
        .expect("catalog renders");
    let second_page = content
        .action_page(session.state(), 0, usize::MAX)
        .expect("catalog repeats");
    assert_eq!(first_page, second_page);
    assert_eq!(session.state().state_id(), state_id);
    assert_eq!(session.trace().final_receipt, receipt);
    assert_eq!(session.player_trace().expect("save after viewing"), save);
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

const ROOK_PAID_TOWLINE_RELIEF_PATH: [ActionSpec; 11] = [
    ActionSpec {
        definition_id: "travel_adjacent",
        parameter: Some(("destination", "lowsail.docks")),
    },
    ActionSpec {
        definition_id: "docks.ring_warning",
        parameter: None,
    },
    ActionSpec {
        definition_id: "docks.rig_towline",
        parameter: None,
    },
    ActionSpec {
        definition_id: "levee.relay_warning",
        parameter: None,
    },
    ActionSpec {
        definition_id: "levee.culvert_path",
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
        definition_id: "top.check_wheels",
        parameter: None,
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

const ROOK_HOT_ROUTE_FERRY_PATH: [ActionSpec; 10] = [
    ActionSpec {
        definition_id: "travel_adjacent",
        parameter: Some(("destination", "lowsail.docks")),
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
        definition_id: "floor.climb_hot_face",
        parameter: None,
    },
    ActionSpec {
        definition_id: "top.break_toll",
        parameter: None,
    },
    ActionSpec {
        definition_id: "world.enter_aftermath",
        parameter: None,
    },
    ActionSpec {
        definition_id: "return.open_ferry",
        parameter: None,
    },
    ActionSpec {
        definition_id: "travel_adjacent",
        parameter: Some(("destination", "lowsail.docks")),
    },
    ActionSpec {
        definition_id: "world.enter_aftermath",
        parameter: None,
    },
];

const ROOK_ROUTE_GUIDANCE_PATH: [ActionSpec; 8] = [
    ActionSpec {
        definition_id: "checkpoint.read_flag",
        parameter: None,
    },
    ActionSpec {
        definition_id: "checkpoint.ask_sava",
        parameter: None,
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

fn assert_supply_view(
    observation: &forge_kernel::Observation,
    resources: &[(&str, &str, i64)],
    items: &[(&str, &str, u32)],
) {
    let expected_resources: Vec<_> = resources
        .iter()
        .map(|(id, name, amount)| ((*id).to_owned(), (*name).to_owned(), *amount))
        .collect();
    let actual_resources: Vec<_> = observation
        .supplies
        .resources
        .iter()
        .map(|resource| (resource.id.clone(), resource.name.clone(), resource.amount))
        .collect();
    assert_eq!(actual_resources, expected_resources);

    let expected_items: Vec<_> = items
        .iter()
        .map(|(id, name, count)| ((*id).to_owned(), (*name).to_owned(), *count))
        .collect();
    let actual_items: Vec<_> = observation
        .supplies
        .items
        .iter()
        .map(|item| (item.id.clone(), item.name.clone(), item.count))
        .collect();
    assert_eq!(actual_items, expected_items);
}

fn assert_npc_fields_unchanged(before: &forge_kernel::NpcState, after: &forge_kernel::NpcState) {
    assert_eq!(after.id, before.id);
    assert_eq!(after.goals, before.goals);
    assert_eq!(after.values, before.values);
    assert_eq!(after.tags, before.tags);
    assert_eq!(after.relationships, before.relationships);
    assert_eq!(after.memories, before.memories);
    assert_eq!(after.knowledge, before.knowledge);
    assert_eq!(after.inventory, before.inventory);
    assert_eq!(after.suspicion, before.suspicion);
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
        "You return to Lowsail's changed market. Oren, Sava, and Mira wait by calm water; both shores still receive a share."
    );

    let encoded = session.trace().to_json().expect("trace serializes");
    let decoded = Trace::from_json(&encoded).expect("trace parses");
    let verified = verify(&decoded, &content).expect("real trace verifies");
    assert_eq!(&verified, session.state());
    assert_eq!(decoded.final_state_id, verified.state_id());
    assert_eq!(decoded.final_receipt, decoded.steps.last().unwrap().receipt);
}

#[test]
fn rook_route_guidance_replays_blocked_and_earned_culvert_checkpoints() {
    let content = content();
    let mut uninterrupted = Session::new_game("rook", 71, &content).expect("session starts");
    let mut after_blocked = None;
    let mut after_return = None;
    let mut after_earned = None;

    for (index, spec) in ROOK_ROUTE_GUIDANCE_PATH.iter().enumerate() {
        record_one_tick(
            &mut uninterrupted,
            &content,
            spec.definition_id,
            spec.parameter,
        );
        match index + 1 {
            3 => after_blocked = Some(resume_player_save(&uninterrupted, &content)),
            4 => after_return = Some(resume_player_save(&uninterrupted, &content)),
            7 => after_earned = Some(resume_player_save(&uninterrupted, &content)),
            _ => {}
        }
    }

    let warning_step = &uninterrupted.trace().steps[0];
    assert_eq!(warning_step.action.definition_id, "checkpoint.read_flag");
    assert_eq!(
        warning_step.observation.result.as_deref(),
        Some("Go to the Red Sluice and redirect the next surge before it floods Lowsail.")
    );
    let sava_step = &uninterrupted.trace().steps[1];
    assert_eq!(sava_step.action.definition_id, "checkpoint.ask_sava");
    assert_eq!(
        sava_step.observation.result.as_deref(),
        Some(
            "Sava explains: the Red Sluice road needs entry papers; Oren at Lowsail Docks knows another way."
        )
    );

    let blocked_step = &uninterrupted.trace().steps[2];
    assert_eq!(blocked_step.action.definition_id, "travel_adjacent");
    assert_eq!(blocked_step.observation.location_id, "lowsail.levee");
    assert_eq!(
        blocked_step.observation.text,
        "You move along the connected path. Guards bar Red Sluice; return through Lowsail Checkpoint and ask Oren at Lowsail Docks about another route."
    );

    let mut resumed_after_blocked = after_blocked.expect("blocked route checkpoint");
    assert_eq!(resumed_after_blocked.trace().steps.len(), 3);
    assert_eq!(resumed_after_blocked.state().world.time, 3);
    assert_eq!(
        resumed_after_blocked.state().world.current_location,
        "lowsail.levee"
    );
    assert_eq!(
        resumed_after_blocked.trace().steps[2].observation,
        blocked_step.observation
    );
    assert!(
        !resumed_after_blocked
            .state()
            .world
            .flags
            .contains("culvert_revealed")
    );
    assert!(
        !resumed_after_blocked
            .state()
            .world
            .flags
            .contains("council_route")
    );
    assert!(
        !resumed_after_blocked
            .state()
            .world
            .flags
            .contains("stolen_route")
    );
    assert!(
        !resumed_after_blocked.state().world.locations["lowsail_market"]
            .flags
            .contains("market_permit")
    );
    assert!(
        !resumed_after_blocked.state().world.locations["lowsail_market"]
            .flags
            .contains("worker_cover")
    );
    assert_eq!(
        content
            .location_description(resumed_after_blocked.state())
            .expect("blocked description renders"),
        "Guards bar Red Sluice; return through Lowsail Checkpoint and ask Oren at Lowsail Docks about another route."
    );
    assert_not_legal(&resumed_after_blocked, &content, "levee.culvert_path");
    assert_views_inert(&resumed_after_blocked, &content);

    let mut resumed_after_return = after_return.expect("return checkpoint");
    assert_eq!(resumed_after_return.trace().steps.len(), 4);
    assert_eq!(resumed_after_return.state().world.time, 4);
    assert_eq!(
        resumed_after_return.state().world.current_location,
        "lowsail_market"
    );
    assert_eq!(
        resumed_after_return.trace().steps[3].observation.text,
        "You move along the connected path. The posted warning points to Red Sluice; its guarded road needs entry papers or another route."
    );
    assert_views_inert(&resumed_after_return, &content);

    let earned_step = &uninterrupted.trace().steps[6];
    assert_eq!(earned_step.action.definition_id, "travel_adjacent");
    assert_eq!(earned_step.observation.location_id, "lowsail.levee");
    assert_eq!(
        earned_step.observation.text,
        "You move along the connected path. The levee road runs east to Red Sluice. Workers brace the wet embankment."
    );
    assert_eq!(
        uninterrupted.trace().steps[5].observation.result.as_deref(),
        Some(
            "Oren reveals the submerged Culvert Path at Lowsail Levee. Take it into the Sluice, climb to Red Sluice Top, then choose Open Old Channel."
        )
    );

    let mut resumed_after_earned = after_earned.expect("earned route checkpoint");
    assert_eq!(resumed_after_earned.trace().steps.len(), 7);
    assert_eq!(resumed_after_earned.state().world.time, 7);
    assert_eq!(
        resumed_after_earned.state().world.current_location,
        "lowsail.levee"
    );
    assert!(
        resumed_after_earned
            .state()
            .world
            .flags
            .contains("culvert_revealed")
    );
    let _culvert = select(&resumed_after_earned, &content, "levee.culvert_path", None);
    assert_views_inert(&resumed_after_earned, &content);
    assert_eq!(
        resumed_after_earned.trace().steps[6].observation,
        earned_step.observation
    );

    for spec in &ROOK_ROUTE_GUIDANCE_PATH[3..] {
        record_one_tick(
            &mut resumed_after_blocked,
            &content,
            spec.definition_id,
            spec.parameter,
        );
    }
    for spec in &ROOK_ROUTE_GUIDANCE_PATH[4..] {
        record_one_tick(
            &mut resumed_after_return,
            &content,
            spec.definition_id,
            spec.parameter,
        );
    }
    for spec in &ROOK_ROUTE_GUIDANCE_PATH[7..] {
        record_one_tick(
            &mut resumed_after_earned,
            &content,
            spec.definition_id,
            spec.parameter,
        );
    }

    let culvert_step = uninterrupted.trace().steps.last().expect("culvert step");
    assert_eq!(culvert_step.action.definition_id, "levee.culvert_path");
    assert_eq!(culvert_step.observation.location_id, "red_sluice.floor");
    assert_eq!(
        culvert_step.observation.result.as_deref(),
        Some("The hidden culvert opens below the Sluice.")
    );
    assert_eq!(uninterrupted.state().world.time, 8);
    assert_eq!(
        uninterrupted.state().world.current_location,
        "red_sluice.floor"
    );
    assert!(
        uninterrupted
            .state()
            .world
            .flags
            .contains("culvert_revealed")
    );

    for resumed in [
        &resumed_after_blocked,
        &resumed_after_return,
        &resumed_after_earned,
    ] {
        assert_eq!(resumed.state(), uninterrupted.state());
        assert_eq!(
            resumed.player_trace().expect("resumed save"),
            uninterrupted.player_trace().expect("uninterrupted save")
        );
        assert_eq!(
            resumed.trace().final_state_id,
            uninterrupted.trace().final_state_id
        );
        assert_eq!(
            resumed.trace().final_receipt,
            uninterrupted.trace().final_receipt
        );
        assert_eq!(
            resumed.trace().steps.last().unwrap().observation,
            culvert_step.observation
        );
    }
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

    assert_supply_view(
        &uninterrupted.trace().initial_observation,
        &[("coin", "Coin", 5), ("stamina", "Stamina", 4)],
        &[("rope", "Rope", 1), ("wire", "Wire", 1)],
    );
    assert_supply_view(
        &uninterrupted.trace().steps[1].observation,
        &[("coin", "Coin", 5), ("stamina", "Stamina", 4)],
        &[
            ("rope", "Rope", 1),
            ("split_tide.tide_key", "Tide Key", 1),
            ("wire", "Wire", 1),
        ],
    );
    assert_supply_view(
        &uninterrupted.trace().steps[5].observation,
        &[("coin", "Coin", 5), ("stamina", "Stamina", 4)],
        &[
            ("rope", "Rope", 1),
            ("split_tide.tide_key", "Tide Key", 1),
            ("wire", "Wire", 1),
        ],
    );

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
    assert_supply_view(
        &resumed_after_calibration.trace().steps[5].observation,
        &[("coin", "Coin", 5), ("stamina", "Stamina", 4)],
        &[
            ("rope", "Rope", 1),
            ("split_tide.tide_key", "Tide Key", 1),
            ("wire", "Wire", 1),
        ],
    );
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
    assert_supply_view(
        &uninterrupted.trace().steps[10].observation,
        &[("coin", "Coin", 5), ("stamina", "Stamina", 4)],
        &[
            ("rope", "Rope", 1),
            ("split_tide.tide_key", "Tide Key", 1),
            ("wire", "Wire", 1),
        ],
    );

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
fn rook_paid_towline_relief_path_resumes_across_purchase_and_return() {
    let content = content();
    let mut uninterrupted = Session::new_game("rook", 71, &content).expect("session starts");
    let mut after_purchase = None;
    let mut before_aftermath = None;
    let mut after_aftermath = None;

    for (index, spec) in ROOK_PAID_TOWLINE_RELIEF_PATH.iter().enumerate() {
        record(
            &mut uninterrupted,
            &content,
            spec.definition_id,
            spec.parameter,
        );
        match index + 1 {
            3 => after_purchase = Some(resume_player_save(&uninterrupted, &content)),
            9 => before_aftermath = Some(resume_player_save(&uninterrupted, &content)),
            10 => after_aftermath = Some(resume_player_save(&uninterrupted, &content)),
            _ => {}
        }
    }

    assert_supply_view(
        &uninterrupted.trace().initial_observation,
        &[("coin", "Coin", 5), ("stamina", "Stamina", 4)],
        &[("rope", "Rope", 1), ("wire", "Wire", 1)],
    );
    let purchase_step = &uninterrupted.trace().steps[2];
    assert_eq!(purchase_step.action.definition_id, "docks.rig_towline");
    assert_eq!(purchase_step.observation.location_id, "lowsail.levee");
    assert_supply_view(
        &purchase_step.observation,
        &[("coin", "Coin", 2), ("stamina", "Stamina", 4)],
        &[("rope", "Rope", 1), ("wire", "Wire", 1)],
    );
    assert_eq!(uninterrupted.state().world.time, 11);
    assert_eq!(
        uninterrupted.state().character.resources.get("coin"),
        Some(&2)
    );
    assert_eq!(
        uninterrupted.state().character.inventory.get("rope"),
        Some(&1)
    );
    assert_eq!(
        uninterrupted.state().character.inventory.get("wire"),
        Some(&1)
    );
    assert!(
        uninterrupted
            .state()
            .world
            .flags
            .contains("culvert_revealed")
    );
    assert!(
        uninterrupted
            .state()
            .character
            .deeds
            .contains("rigged_towline")
    );
    let towline_memory = uninterrupted.state().world.npcs["oren_pell"]
        .memories
        .get("oren_saw_towline")
        .expect("Oren retains the paid towline memory");
    assert_eq!(
        towline_memory.subject,
        "Oren watched the player rig a paid towline."
    );
    assert_eq!(towline_memory.turn, 2);
    assert_eq!(towline_memory.provenance, KnowledgeProvenance::Witnessed);
    assert!(purchase_step.events.iter().any(|event| {
        matches!(
            &event.kind,
            EventKind::ResourceAdjusted { resource, amount }
                if resource == "coin" && *amount == -3
        )
    }));
    assert!(purchase_step.events.iter().any(|event| {
        matches!(
            &event.kind,
            EventKind::NpcMemoryAdded { npc, memory }
                if npc == "oren_pell" && memory == "oren_saw_towline"
        )
    }));
    assert!(purchase_step.events.iter().any(|event| {
        matches!(
            &event.kind,
            EventKind::Moved { from, to }
                if from == "lowsail.docks" && to == "lowsail.levee"
        )
    }));

    let mut resumed_after_purchase = after_purchase.expect("purchase checkpoint");
    assert_eq!(resumed_after_purchase.trace().steps.len(), 3);
    assert_eq!(resumed_after_purchase.state().world.time, 3);
    assert_eq!(
        resumed_after_purchase.state().world.current_location,
        "lowsail.levee"
    );
    assert_eq!(
        resumed_after_purchase.state().world.npcs["oren_pell"].location,
        "lowsail.docks"
    );
    assert!(
        resumed_after_purchase.state().world.locations["lowsail.docks"]
            .entities
            .contains("oren_pell")
    );
    assert_eq!(
        resumed_after_purchase
            .state()
            .character
            .resources
            .get("coin"),
        Some(&2)
    );
    assert_eq!(
        resumed_after_purchase
            .state()
            .character
            .inventory
            .get("rope"),
        Some(&1)
    );
    assert_eq!(
        resumed_after_purchase
            .state()
            .character
            .inventory
            .get("wire"),
        Some(&1)
    );
    assert_not_legal(&resumed_after_purchase, &content, "docks.rig_towline");
    record_specs(
        &mut resumed_after_purchase,
        &content,
        &ROOK_PAID_TOWLINE_RELIEF_PATH[3..],
    );

    let mut resumed_before_aftermath = before_aftermath.expect("pre-aftermath checkpoint");
    assert_eq!(resumed_before_aftermath.trace().steps.len(), 9);
    assert_eq!(resumed_before_aftermath.state().world.time, 9);
    assert_eq!(
        resumed_before_aftermath.state().world.current_location,
        "red_sluice.top"
    );
    assert!(
        resumed_before_aftermath
            .state()
            .world
            .flags
            .contains("flow_relief")
    );
    record_specs(
        &mut resumed_before_aftermath,
        &content,
        &ROOK_PAID_TOWLINE_RELIEF_PATH[9..],
    );

    let mut resumed_after_aftermath = after_aftermath.expect("aftermath checkpoint");
    assert_eq!(resumed_after_aftermath.trace().steps.len(), 10);
    assert_eq!(resumed_after_aftermath.state().world.time, 10);
    assert_eq!(
        resumed_after_aftermath.state().world.current_location,
        "lowsail.return"
    );
    record_specs(
        &mut resumed_after_aftermath,
        &content,
        &ROOK_PAID_TOWLINE_RELIEF_PATH[10..],
    );

    for resumed in [
        &resumed_after_purchase,
        &resumed_before_aftermath,
        &resumed_after_aftermath,
    ] {
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

    assert_eq!(uninterrupted.trace().steps.len(), 11);
    assert!(uninterrupted.state().world.flags.contains("ending_relief"));
    assert_eq!(
        uninterrupted.state().world.current_location,
        "lowsail.return"
    );
    for (npc_id, location) in [
        ("oren_pell", "lowsail.return"),
        ("sava_rusk", "lowsail.return"),
        ("mira_kett", "lowsail.return"),
        ("yara_dene", "lowsail.docks"),
        ("edrik_voss", "red_sluice.floor"),
    ] {
        let npc = &uninterrupted.state().world.npcs[npc_id];
        assert_eq!(npc.location, location);
        assert!(
            uninterrupted.state().world.locations[location]
                .entities
                .contains(npc_id)
        );
    }
    let first_return = &uninterrupted.trace().steps[9];
    let npc_moves: Vec<_> = first_return
        .events
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::NpcMoved { npc, from, to } => {
                Some((npc.as_str(), from.as_str(), to.as_str()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        npc_moves,
        vec![
            ("oren_pell", "lowsail.docks", "lowsail.return"),
            ("sava_rusk", "lowsail_market", "lowsail.return"),
            ("mira_kett", "red_sluice.top", "lowsail.return"),
        ]
    );
}

#[test]
fn rook_hot_route_ferry_path_resumes_across_climb_and_return() {
    let content = content();
    let mut uninterrupted = Session::new_game("rook", 71, &content).expect("session starts");
    let mut after_climb = None;
    let mut after_return = None;
    let mut after_ending = None;

    for (index, spec) in ROOK_HOT_ROUTE_FERRY_PATH.iter().enumerate() {
        record(
            &mut uninterrupted,
            &content,
            spec.definition_id,
            spec.parameter,
        );
        match index + 1 {
            5 => after_climb = Some(resume_player_save(&uninterrupted, &content)),
            7 => after_return = Some(resume_player_save(&uninterrupted, &content)),
            8 => after_ending = Some(resume_player_save(&uninterrupted, &content)),
            _ => {}
        }
    }

    let climb_step = &uninterrupted.trace().steps[4];
    assert_eq!(climb_step.action.definition_id, "floor.climb_hot_face");
    assert_eq!(climb_step.observation.location_id, "red_sluice.top");
    assert_eq!(
        climb_step.observation.result.as_deref(),
        Some(
            "You climb the hot service face to Red Sluice Top. The route exposes Overload Gates, which would flood Lowsail."
        )
    );
    let memory_event = climb_step
        .events
        .iter()
        .position(|event| {
            matches!(
                &event.kind,
                EventKind::NpcMemoryAdded { npc, memory }
                    if npc == "edrik_voss" && memory == "edrik_saw_hot_route"
            )
        })
        .expect("climb records Edrik's witnessed memory");
    let moved_event = climb_step
        .events
        .iter()
        .position(|event| {
            matches!(
                &event.kind,
                EventKind::Moved { from, to }
                    if from == "red_sluice.floor" && to == "red_sluice.top"
            )
        })
        .expect("climb records a typed move to Red Sluice Top");
    assert!(
        memory_event < moved_event,
        "Edrik must witness the climb before the movement event"
    );
    assert_eq!(
        uninterrupted.state().world.current_location,
        "lowsail.return"
    );

    let open_channel_step = &uninterrupted.trace().steps[5];
    assert_eq!(open_channel_step.action.definition_id, "top.break_toll");
    assert_eq!(
        open_channel_step.observation.result.as_deref(),
        Some(
            "You open the old channel. Return to Lowsail and choose Abolish Ferry Toll to launch a free ferry."
        )
    );
    assert!(
        !open_channel_step.observation.text.contains(
            "Oren, Sava, and Mira watch the free ferry carry people between both shores."
        )
    );

    let launch_step = &uninterrupted.trace().steps[7];
    assert_eq!(launch_step.action.definition_id, "return.open_ferry");
    assert!(
        launch_step.observation.text.contains(
            "Oren, Sava, and Mira watch the free ferry carry people between both shores."
        )
    );
    assert!(
        uninterrupted
            .state()
            .world
            .flags
            .contains("high_route_open")
            && uninterrupted
                .state()
                .world
                .flags
                .contains("sluice_outcome_chosen")
            && uninterrupted.state().world.flags.contains("ending_freedom")
    );
    assert!(
        uninterrupted
            .state()
            .character
            .deeds
            .contains("climbed_service_face")
            && uninterrupted
                .state()
                .character
                .deeds
                .contains("freed_ferry")
            && uninterrupted
                .state()
                .character
                .deeds
                .contains("opened_free_ferry")
    );
    let edrik_route_memory = uninterrupted.state().world.npcs["edrik_voss"]
        .memories
        .get("edrik_saw_hot_route")
        .expect("Edrik retains the climb memory after reentry");
    assert_eq!(
        edrik_route_memory.subject,
        "The Kilnborn lock-runner climbed the hot service face."
    );
    assert_eq!(
        edrik_route_memory.provenance,
        KnowledgeProvenance::Witnessed
    );

    let first_return = &uninterrupted.trace().steps[6];
    assert_eq!(first_return.action.definition_id, "world.enter_aftermath");
    assert_eq!(first_return.observation.location_id, "lowsail.return");
    assert!(
        first_return
            .observation
            .text
            .contains(
                "Oren, Sava, and Mira stand by the reopened channel; the ferry toll still awaits your decision."
            )
    );
    assert_eq!(
        first_return.observation.result.as_deref(),
        Some("You return to Lowsail's changed market.")
    );
    let npc_moves: Vec<_> = first_return
        .events
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::NpcMoved { npc, from, to } => {
                Some((npc.as_str(), from.as_str(), to.as_str()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        npc_moves,
        vec![
            ("oren_pell", "lowsail.docks", "lowsail.return"),
            ("sava_rusk", "lowsail_market", "lowsail.return"),
            ("mira_kett", "red_sluice.top", "lowsail.return"),
        ]
    );

    let later_return = &uninterrupted.trace().steps[9];
    assert_eq!(later_return.action.definition_id, "world.enter_aftermath");
    assert!(
        later_return.observation.text.contains(
            "Oren, Sava, and Mira watch the free ferry carry people between both shores."
        )
    );

    let mut resumed_after_climb = after_climb.expect("climb checkpoint");
    assert_eq!(resumed_after_climb.trace().steps.len(), 5);
    assert_eq!(resumed_after_climb.state().world.time, 5);
    assert_eq!(
        resumed_after_climb.state().world.current_location,
        "red_sluice.top"
    );
    assert!(
        resumed_after_climb
            .state()
            .world
            .flags
            .contains("high_route_open")
    );
    assert!(
        resumed_after_climb
            .state()
            .character
            .deeds
            .contains("climbed_service_face")
    );
    record_specs(
        &mut resumed_after_climb,
        &content,
        &ROOK_HOT_ROUTE_FERRY_PATH[5..],
    );

    let mut before_return = Session::new_game("rook", 71, &content).expect("session starts");
    record_specs(
        &mut before_return,
        &content,
        &ROOK_HOT_ROUTE_FERRY_PATH[..6],
    );
    assert!(
        before_return
            .state()
            .world
            .flags
            .contains("old_channel_open")
    );
    assert!(
        before_return.state().world.locations["lowsail.return"]
            .flags
            .contains("ferry_free")
    );
    assert!(!before_return.state().world.flags.contains("ending_freedom"));
    assert!(
        !before_return
            .state()
            .character
            .deeds
            .contains("opened_free_ferry")
    );
    let mut resumed_after_return = after_return.expect("return checkpoint");
    assert_eq!(resumed_after_return.trace().steps.len(), 7);
    assert_eq!(resumed_after_return.state().world.time, 7);
    assert_eq!(
        resumed_after_return.state().world.current_location,
        "lowsail.return"
    );
    assert!(
        resumed_after_return
            .state()
            .world
            .flags
            .contains("sluice_outcome_chosen")
    );
    assert!(
        resumed_after_return.state().world.locations["lowsail.return"]
            .flags
            .contains("ferry_free")
    );
    assert!(
        resumed_after_return
            .state()
            .world
            .flags
            .contains("sluice_outcome_chosen")
    );
    assert!(
        !resumed_after_return
            .state()
            .world
            .flags
            .contains("ending_freedom")
    );
    assert!(
        !resumed_after_return
            .state()
            .character
            .deeds
            .contains("opened_free_ferry")
    );
    let _pending_launch = select(&resumed_after_return, &content, "return.open_ferry", None);
    for (npc_id, source_location) in [
        ("oren_pell", "lowsail.docks"),
        ("sava_rusk", "lowsail_market"),
        ("mira_kett", "red_sluice.top"),
    ] {
        let before_npc = &before_return.state().world.npcs[npc_id];
        let after_npc = &resumed_after_return.state().world.npcs[npc_id];
        assert_eq!(before_npc.location, source_location);
        assert_eq!(after_npc.location, "lowsail.return");
        assert_npc_fields_unchanged(before_npc, after_npc);
        assert!(
            before_return.state().world.locations[source_location]
                .entities
                .contains(npc_id)
        );
        assert!(
            !resumed_after_return.state().world.locations[source_location]
                .entities
                .contains(npc_id)
        );
        assert!(
            resumed_after_return.state().world.locations["lowsail.return"]
                .entities
                .contains(npc_id)
        );
    }
    record_specs(
        &mut resumed_after_return,
        &content,
        &ROOK_HOT_ROUTE_FERRY_PATH[7..],
    );

    let mut resumed_after_ending = after_ending.expect("ending checkpoint");
    assert_eq!(resumed_after_ending.trace().steps.len(), 8);
    assert_eq!(resumed_after_ending.state().world.time, 8);
    assert!(
        resumed_after_ending
            .state()
            .world
            .flags
            .contains("ending_freedom")
    );
    assert!(
        resumed_after_ending
            .state()
            .character
            .deeds
            .contains("opened_free_ferry")
    );
    assert!(
        resumed_after_ending.trace().steps[7]
            .observation
            .text
            .contains(
                "Oren, Sava, and Mira watch the free ferry carry people between both shores."
            )
    );
    record_specs(
        &mut resumed_after_ending,
        &content,
        &ROOK_HOT_ROUTE_FERRY_PATH[8..],
    );

    assert_eq!(resumed_after_climb.state(), uninterrupted.state());
    assert_eq!(resumed_after_return.state(), uninterrupted.state());
    assert_eq!(resumed_after_ending.state(), uninterrupted.state());
    assert_eq!(
        resumed_after_climb.trace().final_state_id,
        uninterrupted.trace().final_state_id
    );
    assert_eq!(
        resumed_after_return.trace().final_state_id,
        uninterrupted.trace().final_state_id
    );
    assert_eq!(
        resumed_after_climb.trace().final_receipt,
        uninterrupted.trace().final_receipt
    );
    assert_eq!(
        resumed_after_return.trace().final_receipt,
        uninterrupted.trace().final_receipt
    );
    assert_eq!(
        resumed_after_ending.trace().final_state_id,
        uninterrupted.trace().final_state_id
    );
    assert_eq!(
        resumed_after_ending.trace().final_receipt,
        uninterrupted.trace().final_receipt
    );
    assert_eq!(uninterrupted.trace().steps.len(), 10);
    assert_eq!(uninterrupted.state().world.time, 10);
    assert!(uninterrupted.trace().steps[7..].iter().all(|step| {
        step.events
            .iter()
            .all(|event| !matches!(event.kind, EventKind::NpcMoved { .. }))
    }));
    assert!(
        uninterrupted
            .state()
            .world
            .flags
            .contains("old_channel_open")
    );
    assert_eq!(
        uninterrupted.state().world.current_location,
        "lowsail.return"
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
        "Oren, Sava, and Mira watch families carry goods toward higher ground beyond the empty stalls."
    ));
    assert_eq!(
        content
            .location_description(uninterrupted.state())
            .expect("return aftermath observes"),
        "Oren, Sava, and Mira watch families carry goods toward higher ground beyond the empty stalls."
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
        schema_version: "forge-schema-v8".to_owned(),
        rules_version: "forge-rules-v6".to_owned(),
        world_id: "timed-fixture".to_owned(),
        contract: ContentContract::Fixture,
        start_location: "room".to_owned(),
        character_presets: Vec::new(),
        character_creation: None,
        supply_labels: SupplyLabels::default(),
        recipes: Vec::new(),
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
