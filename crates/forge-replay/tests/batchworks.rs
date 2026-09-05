use forge_content::parse_and_compile_production;
use forge_kernel::{CompiledContent, EventKind, KnowledgeProvenance, enumerate_legal_actions};
use forge_replay::{PlayerTrace, Session, Trace, resume_player_trace, verify};

const SOURCE: &str = include_str!("../../../content/split-tide.json");
const WORKSHOP: &str = "fume_yards.workshop";
const NESSA: &str = "fume_yards.nessa_tern";
const CLAY: &str = "fume_yards.clay";
const MESH: &str = "fume_yards.mesh";
const PLUGS: &str = "fume_yards.repair_lot";
const BAY: &str = "fume_yards.kiln_bay";
const BRANN: &str = "fume_yards.brann_coil";
const PERA: &str = "fume_yards.pera_senn";
const FUEL: &str = "fume_yards.fuel";
const CASK: &str = "fume_yards.water_cask";
const CHARGE: &str = "fume_yards.prepared_charge";
const CLAIM: &str = "fume_yards.batch_claim";
const FILTER: &str = "fume_yards.filter";
const SPOILED: &str = "fume_yards.spoiled_charge";

#[derive(Clone, Copy)]
struct ActionSpec {
    id: &'static str,
    destination: Option<&'static str>,
}

const fn act(id: &'static str) -> ActionSpec {
    ActionSpec {
        id,
        destination: None,
    }
}

const fn travel(destination: &'static str) -> ActionSpec {
    ActionSpec {
        id: "travel_adjacent",
        destination: Some(destination),
    }
}

// Exact existing reviewed scenario prefixes are extended, not replaced by
// faster outcome recipes. Their consequences remain independent of crafting.
const SPLIT: &[ActionSpec] = &[
    act("checkpoint.show_charter"),
    travel("lowsail.levee"),
    act("levee.authority_path"),
    act("floor.read_harmonics"),
    travel("red_sluice.top"),
    act("top.check_wheels"),
    act("top.split_flow"),
    act("world.enter_aftermath"),
    act("return.share_water"),
];
const HOLD: &[ActionSpec] = &[
    act("checkpoint.show_charter"),
    travel("lowsail.levee"),
    act("levee.authority_path"),
    travel("red_sluice.top"),
    act("top.hold_market"),
    act("world.enter_aftermath"),
    act("return.count_dry_stalls"),
];
const RELIEF: &[ActionSpec] = &[
    travel("lowsail.docks"),
    act("docks.ring_warning"),
    act("docks.ask_oren"),
    travel("lowsail.levee"),
    act("levee.relay_warning"),
    act("levee.culvert_path"),
    act("floor.open_relief"),
    travel("red_sluice.top"),
    act("top.divert_relief"),
    act("world.enter_aftermath"),
    act("return.move_inland"),
];
const FERRY: &[ActionSpec] = &[
    act("checkpoint.blend_workers"),
    travel("lowsail.levee"),
    act("levee.culvert_path"),
    travel("red_sluice.top"),
    act("top.break_toll"),
    act("world.enter_aftermath"),
    act("return.open_ferry"),
];
const OVERLOAD: &[ActionSpec] = &[
    act("checkpoint.use_stolen_permit"),
    travel("lowsail.levee"),
    act("levee.stolen_path"),
    act("floor.force_wheel"),
    travel("red_sluice.top"),
    act("top.overload"),
    act("world.enter_aftermath"),
    act("return.face_flood"),
];
const LONG_RELIEF_PREFIX: &[ActionSpec] = &[
    act("checkpoint.show_charter"),
    travel("lowsail.docks"),
    act("docks.ring_warning"),
    travel("lowsail.levee"),
    act("levee.relay_warning"),
    act("levee.authority_path"),
    act("floor.open_relief"),
    travel("red_sluice.top"),
    act("top.divert_relief"),
    act("world.enter_aftermath"),
    act("return.move_inland"),
];
const IGNITE_EXTENSION: &[ActionSpec] = &[
    act("return.visit_workshop"),
    act("fume_yards.take_stock"),
    travel(BAY),
    act("fume_yards.take_cask"),
    act("fume_yards.take_fuel"),
    act("fume_yards.prepare_charge"),
    act("fume_yards.fit_wet_screen"),
    act("fume_yards.ignite_batch"),
];
const EARLY_IGNITION: &[ActionSpec] = &[
    act("checkpoint.show_charter"),
    travel("lowsail.levee"),
    travel(WORKSHOP),
    act("fume_yards.take_stock"),
    travel(BAY),
    act("fume_yards.take_cask"),
    act("fume_yards.take_fuel"),
    act("fume_yards.prepare_charge"),
    act("fume_yards.fit_wet_screen"),
    act("fume_yards.ignite_batch"),
];

fn content() -> CompiledContent {
    parse_and_compile_production(SOURCE).expect("batch workshop production pack compiles")
}

fn record(session: &mut Session<'_>, content: &CompiledContent, spec: ActionSpec) {
    let action = enumerate_legal_actions(session.state(), content)
        .unwrap()
        .into_iter()
        .find(|action| {
            action.definition_id == spec.id
                && spec.destination.is_none_or(|destination| {
                    action
                        .parameters
                        .get("destination")
                        .is_some_and(|value| value == destination)
                })
        })
        .unwrap_or_else(|| {
            panic!(
                "missing {} at {} turn {}",
                spec.id,
                session.state().world.current_location,
                session.state().world.time
            )
        });
    let time = session.state().world.time;
    let page = content.action_page(session.state(), 0, usize::MAX).unwrap();
    let view = page
        .actions
        .iter()
        .find(|view| view.action_id == action.action_id)
        .unwrap();
    assert_eq!(
        (view.time_cost.minimum_ticks, view.time_cost.maximum_ticks),
        (1, 1)
    );
    let recorded = session.record(&action).unwrap();
    assert_eq!(recorded.observation.world_time, time + 1);
    assert!(
        recorded.observation.text.split_whitespace().count()
            + recorded
                .observation
                .supplies
                .summary()
                .split_whitespace()
                .count()
            < 100
    );
}

fn record_all(session: &mut Session<'_>, content: &CompiledContent, specs: &[ActionSpec]) {
    for spec in specs {
        record(session, content, *spec);
    }
}

fn assert_absent(session: &Session<'_>, content: &CompiledContent, id: &str) {
    assert!(
        !enumerate_legal_actions(session.state(), content)
            .unwrap()
            .iter()
            .any(|action| action.definition_id == id),
        "{id} returned"
    );
}

fn save_resume<'content>(
    session: &Session<'content>,
    content: &'content CompiledContent,
) -> Session<'content> {
    let player = session.player_trace().unwrap();
    let encoded = player.to_json().unwrap();
    let decoded = PlayerTrace::from_json(&encoded).unwrap();
    resume_player_trace(&decoded, content).expect("recipe save replays from canonical genesis")
}

fn assert_replay(session: &Session<'_>, content: &CompiledContent) {
    let trace = Trace::from_json(&session.trace().to_json().unwrap()).unwrap();
    assert_eq!(verify(&trace, content).unwrap(), *session.state());
    let resumed = save_resume(session, content);
    assert_eq!(resumed.state(), session.state());
    assert_eq!(resumed.trace(), session.trace());
    assert_eq!(
        content.observe(resumed.state()).unwrap(),
        content.observe(session.state()).unwrap()
    );
    assert_eq!(
        content.action_page(resumed.state(), 0, usize::MAX).unwrap(),
        content.action_page(session.state(), 0, usize::MAX).unwrap()
    );
}

fn deadline_specs() -> Vec<ActionSpec> {
    let mut specs = vec![act("wait_tide"); 16];
    specs.extend([act("world.enter_aftermath"), act("return.face_flood")]);
    specs
}

fn owned(session: &Session<'_>, item: &str) -> u32 {
    session
        .state()
        .character
        .inventory
        .get(item)
        .copied()
        .unwrap_or(0)
}

fn resolved(session: &Session<'_>, id: &str, applied: bool) -> usize {
    session.state().event_log.iter().filter(|event| matches!(&event.kind,
        EventKind::ScheduledEventResolved { event_id, applied: actual, .. } if event_id == id && *actual == applied
    )).count()
}

#[test]
fn banking_preserves_the_tide_choice_and_a_freight_commission_that_abandonment_loses() {
    let content = content();
    let mut shared = Session::new_game("ilyan", 71, &content).unwrap();
    record_all(&mut shared, &content, EARLY_IGNITION);
    assert_eq!(shared.state().world.time, 10);
    let mut bank = save_resume(&shared, &content);
    record(&mut bank, &content, act("fume_yards.bank_kiln"));
    assert_eq!(
        (
            owned(&bank, CLAIM),
            owned(&bank, SPOILED),
            owned(&bank, FILTER)
        ),
        (0, 1, 0)
    );
    assert_eq!(resolved(&bank, "fume_yards.batch_ready", false), 1);
    let road = [
        travel(WORKSHOP),
        travel("lowsail.levee"),
        act("levee.authority_path"),
        travel("red_sluice.top"),
    ];
    record_all(&mut bank, &content, &road);
    record(&mut bank, &content, act("top.hold_market"));
    assert_eq!(bank.state().world.time, 16);
    assert!(bank.state().world.flags.contains("flow_locked_market"));
    assert!(!bank.state().world.flags.contains("surge_missed"));
    assert_eq!(resolved(&bank, "fume_yards.batch_spoil", false), 1);
    record_all(
        &mut bank,
        &content,
        &[
            act("world.enter_aftermath"),
            act("return.count_dry_stalls"),
            act("return.visit_workshop"),
            travel(BAY),
            act("fume_yards.load_kiln_freight"),
        ],
    );
    assert_eq!(bank.state().character.resources["coin"], 13);
    assert_eq!(bank.state().character.resources["stamina"], 1);
    assert_absent(&bank, &content, "fume_yards.load_kiln_freight");
    assert!(bank.state().world.npcs[BRANN].remembers("fume_yards.kiln_banked"));
    assert!(
        !bank.state().world.locations[BAY]
            .flags
            .contains("fume_yards.freight_spoiled")
    );
    let mut draw = save_resume(&shared, &content);
    record_all(
        &mut draw,
        &content,
        &[act("wait_tide"), act("fume_yards.draw_filter")],
    );
    record_all(&mut draw, &content, &road);
    assert_eq!(draw.state().world.time, 16);
    assert_eq!(owned(&draw, FILTER), 1);
    assert!(draw.state().world.flags.contains("surge_missed"));
    assert_absent(&draw, &content, "top.hold_market");
    let mut abandon = save_resume(&shared, &content);
    record_all(&mut abandon, &content, &road);
    record(&mut abandon, &content, act("top.hold_market"));
    record_all(
        &mut abandon,
        &content,
        &[
            act("world.enter_aftermath"),
            act("return.count_dry_stalls"),
            act("return.visit_workshop"),
            travel(BAY),
        ],
    );
    assert_eq!(abandon.state().world.time, 19);
    assert!(abandon.state().world.flags.contains("flow_locked_market"));
    assert_eq!(resolved(&abandon, "fume_yards.batch_spoil", true), 1);
    assert!(
        abandon.state().world.locations[BAY]
            .flags
            .contains("fume_yards.freight_spoiled")
    );
    assert_eq!((owned(&abandon, SPOILED), owned(&abandon, FILTER)), (1, 0));
    assert_absent(&abandon, &content, "fume_yards.load_kiln_freight");
    assert_absent(&abandon, &content, "fume_yards.load_filtered_kiln_freight");
    assert!(
        content
            .observe(abandon.state())
            .unwrap()
            .text
            .contains("commission is lost")
    );
    for session in [&bank, &draw, &abandon] {
        assert_replay(session, &content);
    }
}

#[test]
fn every_old_outcome_and_deadline_can_export_one_filter_without_rewriting_water_history() {
    let content = content();
    let deadline = deadline_specs();
    let cases = [
        (
            "ilyan",
            SPLIT,
            "ending_accord",
            "both shores still receive a share",
        ),
        (
            "ilyan",
            HOLD,
            "ending_council",
            "upland works still lack water",
        ),
        ("rook", RELIEF, "ending_relief", "uphill"),
        ("rook", FERRY, "ending_freedom", "free ferry"),
        (
            "rook",
            OVERLOAD,
            "ending_disaster",
            "old market crossing remains lost",
        ),
        (
            "ilyan",
            deadline.as_slice(),
            "ending_disaster",
            "old market crossing remains lost",
        ),
    ];
    for (preset, prefix, ending, context) in cases {
        let mut session = Session::new_game(preset, 71, &content).unwrap();
        record_all(&mut session, &content, prefix);
        assert!(session.state().world.flags.contains(ending));
        let flags = session.state().world.flags.clone();
        let coin = session.state().character.resources["coin"];
        let stamina = session.state().character.resources["stamina"];
        record_all(&mut session, &content, IGNITE_EXTENSION);
        record_all(
            &mut session,
            &content,
            &[
                act("wait_tide"),
                act("fume_yards.draw_filter"),
                act("world.enter_aftermath"),
                act("return.sell_filter"),
            ],
        );
        assert_eq!(session.state().world.flags, flags);
        assert_eq!(
            (
                session.state().character.resources["coin"],
                session.state().character.resources["stamina"]
            ),
            (coin + 4, stamina)
        );
        for item in [
            CLAY, MESH, FUEL, CASK, CHARGE, CLAIM, FILTER, SPOILED, PLUGS,
        ] {
            assert_eq!(owned(&session, item), 0);
        }
        for npc in [NESSA, BRANN, PERA] {
            assert!(session.state().world.npcs[npc].inventory.is_empty());
        }
        assert!(
            content
                .observe(session.state())
                .unwrap()
                .text
                .contains(context)
        );
        assert_eq!(
            session.state().world.npcs["oren_pell"].memories["fume_yards.filter_bought"].provenance,
            KnowledgeProvenance::Witnessed
        );
        assert_absent(&session, &content, "return.sell_filter");
        record_all(
            &mut session,
            &content,
            &[act("return.visit_workshop"), travel(BAY)],
        );
        let revisit = content.observe(session.state()).unwrap().text;
        assert!(revisit.contains("filter left the kiln"));
        assert!(revisit.contains("three coins") && revisit.contains("two stamina"));
        assert!(!revisit.contains("Two clay"));
        assert_absent(&session, &content, "fume_yards.fit_dust_filter");
        assert_eq!(resolved(&session, "fume_yards.batch_ready", true), 1);
        assert_eq!(resolved(&session, "fume_yards.batch_spoil", false), 1);
        assert_replay(&session, &content);
    }
}

#[test]
fn serialized_boundary_forks_match_uninterrupted_ready_draw_bank_and_remote_spoil_histories() {
    let content = content();
    let branches: &[&[ActionSpec]] = &[
        &[
            act("wait_tide"),
            act("fume_yards.draw_filter"),
            act("fume_yards.fit_dust_filter"),
            act("fume_yards.load_filtered_kiln_freight"),
        ],
        &[
            act("fume_yards.bank_kiln"),
            act("fume_yards.load_kiln_freight"),
            act("wait_tide"),
            act("wait_tide"),
        ],
        &[
            travel(WORKSHOP),
            travel("lowsail.levee"),
            travel("lowsail_market"),
            travel("lowsail.docks"),
            travel("lowsail.levee"),
            travel(WORKSHOP),
            travel(BAY),
            act("fume_yards.inspect_spoiled_batch"),
        ],
        &[
            act("wait_tide"),
            act("wait_tide"),
            act("wait_tide"),
            act("fume_yards.draw_filter"),
            act("world.enter_aftermath"),
            act("return.sell_filter"),
        ],
    ];
    for branch in branches {
        let mut specs = HOLD.to_vec();
        specs.extend_from_slice(IGNITE_EXTENSION);
        specs.extend_from_slice(branch);
        let mut uninterrupted = Session::new_game("ilyan", 71, &content).unwrap();
        record_all(&mut uninterrupted, &content, &specs);
        for checkpoint in [12, 13, 14, 15, 16, 18, specs.len()] {
            let mut prefix = Session::new_game("ilyan", 71, &content).unwrap();
            record_all(&mut prefix, &content, &specs[..checkpoint]);
            let mut resumed = save_resume(&prefix, &content);
            assert_eq!(resumed.trace(), prefix.trace());
            record_all(&mut resumed, &content, &specs[checkpoint..]);
            assert_eq!(
                resumed.state(),
                uninterrupted.state(),
                "checkpoint {checkpoint}"
            );
            assert_eq!(
                resumed.trace(),
                uninterrupted.trace(),
                "checkpoint {checkpoint}"
            );
            assert_eq!(
                resumed.player_trace().unwrap(),
                uninterrupted.player_trace().unwrap()
            );
        }
        assert_replay(&uninterrupted, &content);
    }
}

#[test]
fn ready_and_spoil_collide_with_the_surge_without_pausing_production_or_the_deadline() {
    let content = content();
    // Early ignition pre9 normally resolves at11/14. Delaying by2 or5
    // creates a real spoil/surge or ready/surge collision at16.
    for delay in [2, 5] {
        let mut session = Session::new_game("ilyan", 71, &content).unwrap();
        record_all(&mut session, &content, &EARLY_IGNITION[..9]);
        for _ in 0..delay {
            record(&mut session, &content, act("wait_tide"));
        }
        record(&mut session, &content, act("fume_yards.ignite_batch"));
        while session.state().world.time < 16 {
            record(&mut session, &content, act("wait_tide"));
        }
        let boundary = session.trace().steps.last().unwrap();
        assert!(session.state().world.flags.contains("surge_missed"));
        let expected = if delay == 2 {
            "fume_yards.batch_spoil"
        } else {
            "fume_yards.batch_ready"
        };
        for id in [expected, "lowsail.next_surge"] {
            assert!(boundary.events.iter().any(|event| matches!(&event.kind, EventKind::ScheduledEventResolved { event_id, applied: true, .. } if event_id == id)));
        }
        if delay == 2 {
            assert_eq!((owned(&session, CLAIM), owned(&session, SPOILED)), (0, 1));
            assert_absent(&session, &content, "fume_yards.draw_filter");
            assert_absent(&session, &content, "fume_yards.load_kiln_freight");
        } else {
            record_all(
                &mut session,
                &content,
                &[
                    act("fume_yards.draw_filter"),
                    act("fume_yards.fit_dust_filter"),
                    act("fume_yards.load_filtered_kiln_freight"),
                ],
            );
            assert_eq!(
                (
                    owned(&session, FILTER),
                    session.state().character.resources["coin"]
                ),
                (0, 13)
            );
            assert_eq!(resolved(&session, "fume_yards.batch_spoil", false), 1);
        }
        assert_replay(&session, &content);
    }
}

#[test]
fn first_firing_after_the_existing_128_turn_relief_path_uses_its_own_future_window() {
    let content = content();
    let mut specs = LONG_RELIEF_PREFIX.to_vec();
    let roundtrip = [
        travel("red_sluice.top"),
        travel("red_sluice.floor"),
        travel("lowsail.levee"),
        travel("lowsail_market"),
        travel("lowsail.docks"),
        travel("lowsail.levee"),
        act("world.enter_aftermath"),
    ];
    for _ in 0..15 {
        specs.extend(roundtrip);
    }
    specs.extend([act("wait_tide"); 5]);
    specs.extend(roundtrip);
    assert_eq!(specs.len(), 128);
    let mut session = Session::new_game("ilyan", 71, &content).unwrap();
    record_all(&mut session, &content, &specs);
    assert!(!session.trace().steps.iter().any(
        |step| step.observation.location_id == BAY || step.observation.location_id == WORKSHOP
    ));
    let old_flags = session.state().world.flags.clone();
    assert_eq!(session.state().world.npcs[BRANN].inventory[FUEL], 1);
    assert_eq!(session.state().world.npcs[PERA].inventory[CASK], 1);
    record_all(&mut session, &content, IGNITE_EXTENSION);
    assert_eq!(session.state().world.time, 136);
    for (id, due) in [
        ("fume_yards.batch_ready", 137),
        ("fume_yards.batch_spoil", 140),
    ] {
        assert_eq!(
            session
                .state()
                .world
                .scheduled_events
                .iter()
                .find(|event| event.id == id)
                .unwrap()
                .due_time,
            due
        );
    }
    let mut resumed = save_resume(&session, &content);
    let finish = [
        act("wait_tide"),
        act("fume_yards.draw_filter"),
        act("fume_yards.fit_dust_filter"),
        act("fume_yards.load_filtered_kiln_freight"),
    ];
    record_all(&mut session, &content, &finish);
    record_all(&mut resumed, &content, &finish);
    assert_eq!(session.state().world.time, 140);
    assert_eq!(session.state().world.flags, old_flags);
    assert_eq!(session.trace(), resumed.trace());
    assert!(session.state().world.scheduled_events.is_empty());
    assert_eq!(resolved(&session, "fume_yards.batch_ready", true), 1);
    assert_eq!(resolved(&session, "fume_yards.batch_spoil", false), 1);
    assert_replay(&session, &content);
}
