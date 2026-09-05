use forge_content::parse_and_compile_production;
use forge_kernel::{CompiledContent, EventKind, KnowledgeProvenance, enumerate_legal_actions};
use forge_replay::{PlayerTrace, Session, Trace, resume_player_trace, verify};

const SOURCE: &str = include_str!("../../../content/split-tide.json");
const WORKSHOP: &str = "fume_yards.workshop";
const NESSA: &str = "fume_yards.nessa_tern";
const CLAY: &str = "fume_yards.clay";
const MESH: &str = "fume_yards.mesh";
const PLUGS: &str = "fume_yards.repair_lot";
const SCREEN: &str = "fume_yards.catch_screen";

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
const REPAIR_EXTENSION: &[ActionSpec] = &[
    act("return.visit_workshop"),
    act("fume_yards.take_stock"),
    act("fume_yards.press_repair_plugs"),
    act("fume_yards.load_freight"),
    act("world.enter_aftermath"),
    act("return.patch_stand"),
    act("return.sort_dry_goods"),
    act("return.visit_workshop"),
    act("world.enter_aftermath"),
];
const SCREEN_EXTENSION: &[ActionSpec] = &[
    act("return.visit_workshop"),
    act("fume_yards.take_stock"),
    act("fume_yards.pack_catch_screen"),
    act("fume_yards.fit_catch_screen"),
    act("fume_yards.load_screened_freight"),
    act("world.enter_aftermath"),
    act("return.visit_workshop"),
];

fn content() -> CompiledContent {
    parse_and_compile_production(SOURCE).expect("cold workshop production pack compiles")
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

#[test]
fn cold_repair_extends_all_five_old_outcomes_and_the_missed_deadline_without_changing_their_truth()
{
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
        assert!(
            session.state().world.flags.contains(ending),
            "prefix {ending}"
        );
        let old_flags = session.state().world.flags.clone();
        let old_location_flags = session.state().world.locations["lowsail.return"]
            .flags
            .clone();
        let old_nessa = session.state().world.npcs[NESSA].clone();
        assert_eq!(old_nessa.inventory[CLAY], 2);
        assert_eq!(old_nessa.inventory[MESH], 1);
        assert!(old_nessa.memories.is_empty());
        let old_coin = session.state().character.resources["coin"];
        let old_stamina = session.state().character.resources["stamina"];
        record_all(&mut session, &content, REPAIR_EXTENSION);
        assert_eq!(session.state().world.flags, old_flags);
        assert!(
            old_location_flags.is_subset(&session.state().world.locations["lowsail.return"].flags)
        );
        assert_eq!(session.state().character.resources["coin"], old_coin + 5);
        assert_eq!(
            session.state().character.resources["stamina"],
            old_stamina - 2
        );
        assert!(!session.state().character.inventory.contains_key(PLUGS));
        assert!(!session.state().character.inventory.contains_key(SCREEN));
        assert!(!session.state().character.inventory.contains_key(CLAY));
        assert!(!session.state().character.inventory.contains_key(MESH));
        assert!(session.state().world.npcs[NESSA].inventory.is_empty());
        let observation = content.observe(session.state()).unwrap();
        assert!(observation.text.contains("holds sorted goods"));
        assert!(
            observation.text.contains(context),
            "lost {context}: {}",
            observation.text
        );
        assert_absent(&session, &content, "return.patch_stand");
        assert_absent(&session, &content, "return.sort_dry_goods");
        assert_replay(&session, &content);
    }
}

#[test]
fn save_before_and_after_each_cold_transformation_and_installation_matches_uninterrupted_play() {
    let content = content();
    for extension in [REPAIR_EXTENSION, SCREEN_EXTENSION] {
        let mut specs = SPLIT.to_vec();
        specs.extend_from_slice(extension);
        let mut uninterrupted = Session::new_game("ilyan", 71, &content).unwrap();
        record_all(&mut uninterrupted, &content, &specs);
        // At every new-material boundary, serialize only the canonical player
        // trace and reconstruct state, events, supplies, catalogs and receipts.
        for checkpoint in SPLIT.len()..=specs.len() {
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
            assert_eq!(
                content.action_page(resumed.state(), 0, usize::MAX).unwrap(),
                content
                    .action_page(uninterrupted.state(), 0, usize::MAX)
                    .unwrap()
            );
        }
        assert_replay(&uninterrupted, &content);
    }
}

#[test]
fn late_first_workshop_entry_extends_the_existing_128_turn_relief_revisit_path() {
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
    assert_eq!(session.state().world.time, 128);
    assert!(
        !session
            .trace()
            .steps
            .iter()
            .any(|step| step.observation.location_id == WORKSHOP)
    );
    assert_eq!(session.state().world.npcs[NESSA].inventory[CLAY], 2);
    assert_eq!(session.state().world.npcs[NESSA].inventory[MESH], 1);
    assert!(session.state().world.npcs[NESSA].memories.is_empty());
    let prior_flags = session.state().world.flags.clone();
    let mut resumed = save_resume(&session, &content);
    record_all(&mut session, &content, REPAIR_EXTENSION);
    record_all(&mut resumed, &content, REPAIR_EXTENSION);
    assert_eq!(session.state().world.time, 137);
    assert_eq!(session.state().world.flags, prior_flags);
    assert_eq!(session.trace(), resumed.trace());
    assert_eq!(
        session.state().world.npcs[NESSA].memories["fume_yards.stock_handed_over"].turn,
        129
    );
    assert_eq!(
        session.state().world.npcs["oren_pell"].knowledge["fume_yards.stand_patched"].provenance,
        KnowledgeProvenance::Witnessed
    );
    assert_eq!(session.state().event_log.iter().filter(|event| matches!(&event.kind, EventKind::ScheduledEventResolved { event_id, applied: false, .. } if event_id == "lowsail.next_surge")).count(), 1);
    assert!(session.state().world.scheduled_events.is_empty());
    assert_replay(&session, &content);
}

#[test]
fn recipe_and_surge_on_the_same_step_replay_across_the_deadline_and_remain_usable_afterward() {
    let content = content();
    let mut specs = vec![
        travel("lowsail.levee"),
        travel(WORKSHOP),
        act("fume_yards.take_stock"),
    ];
    specs.extend([act("wait_tide"); 12]);
    specs.extend([
        act("fume_yards.pack_catch_screen"),
        act("fume_yards.fit_catch_screen"),
        act("fume_yards.load_screened_freight"),
        act("world.enter_aftermath"),
        act("return.face_flood"),
        act("return.visit_workshop"),
    ]);
    let mut session = Session::new_game("ilyan", 71, &content).unwrap();
    record_all(&mut session, &content, &specs);
    let at_surge = &session.trace().steps[15];
    assert_eq!(at_surge.observation.world_time, 16);
    assert_eq!(at_surge.observation.location_id, WORKSHOP);
    let recipe_position = at_surge.events.iter().position(|event| matches!(&event.kind, EventKind::RecipeApplied { recipe, .. } if recipe == "fume_yards.pack_catch_screen")).unwrap();
    let surge_position = at_surge.events.iter().position(|event| matches!(&event.kind, EventKind::ScheduledEventResolved { event_id, applied: true, .. } if event_id == "lowsail.next_surge")).unwrap();
    assert!(recipe_position < surge_position);
    for checkpoint in [3, 15, 16, 17, 18, 19] {
        let mut prefix = Session::new_game("ilyan", 71, &content).unwrap();
        record_all(&mut prefix, &content, &specs[..checkpoint]);
        let mut resumed = save_resume(&prefix, &content);
        record_all(&mut resumed, &content, &specs[checkpoint..]);
        assert_eq!(resumed.trace(), session.trace());
    }
    assert_eq!(session.state().character.resources["coin"], 12);
    assert_eq!(session.state().character.resources["stamina"], 3);
    assert!(session.state().world.flags.contains("surge_missed"));
    assert!(session.state().world.flags.contains("ending_disaster"));
    assert_absent(&session, &content, "fume_yards.load_screened_freight");
    assert_replay(&session, &content);
}

#[test]
fn an_early_cold_detour_can_still_split_the_tide_and_deliver_its_goods_without_extending_the_deadline()
 {
    let content = content();
    let specs = [
        act("checkpoint.show_charter"),
        travel("lowsail.levee"),
        travel(WORKSHOP),
        act("fume_yards.take_stock"),
        act("fume_yards.press_repair_plugs"),
        act("fume_yards.load_freight"),
        travel("lowsail.levee"),
        act("levee.authority_path"),
        act("floor.read_harmonics"),
        travel("red_sluice.top"),
        act("top.check_wheels"),
        act("top.split_flow"),
        act("world.enter_aftermath"),
        act("return.share_water"),
        act("return.patch_stand"),
        act("return.sort_dry_goods"),
    ];
    let mut session = Session::new_game("ilyan", 71, &content).unwrap();
    record_all(&mut session, &content, &specs);
    assert_eq!(session.state().world.time, 16);
    assert!(session.state().world.flags.contains("flow_split"));
    assert!(session.state().world.flags.contains("ending_accord"));
    assert!(!session.state().world.flags.contains("surge_missed"));
    assert!(!session.state().world.flags.contains("sluice_failure"));
    assert_eq!(session.state().character.resources["coin"], 15);
    assert_eq!(session.state().character.resources["stamina"], 1);
    assert!(!session.state().character.inventory.contains_key(PLUGS));
    assert!(
        content
            .observe(session.state())
            .unwrap()
            .text
            .contains("both shores still receive a share")
    );
    assert!(session.state().world.scheduled_events.is_empty());
    assert_eq!(session.state().event_log.iter().filter(|event| matches!(&event.kind, EventKind::ScheduledEventResolved { event_id, applied: false, .. } if event_id == "lowsail.next_surge")).count(), 1);
    assert_replay(&session, &content);
}
