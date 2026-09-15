use std::collections::BTreeSet;

use forge_content::parse_and_compile_production;
use forge_kernel::{
    CanonicalAction, CompiledContent, EventKind, GameState, KnowledgeProvenance,
    enumerate_legal_actions, step,
};

const SOURCE: &str = include_str!("../../../content/split-tide.json");
const ASH: &str = "fume_yards.ash_beds";
const BAY: &str = "fume_yards.kiln_bay";
const WORKSHOP: &str = "fume_yards.workshop";
const RETURN: &str = "lowsail.return";
const PERA: &str = "fume_yards.pera_senn";
const OREN: &str = "oren_pell";
const CASK: &str = "fume_yards.water_cask";
const SPOILED: &str = "fume_yards.spoiled_charge";
const SHARD: &str = "fume_yards.shard";
const FEED: &str = "fume_yards.ash_feed";
const FREIGHT: &str = "fume_yards.ash_freight";

fn content() -> CompiledContent {
    parse_and_compile_production(SOURCE).expect("ash-cart production compiles below 100 words")
}

fn select(state: &GameState, content: &CompiledContent, id: &str) -> CanonicalAction {
    enumerate_legal_actions(state, content)
        .unwrap()
        .into_iter()
        .find(|action| action.definition_id == id)
        .unwrap_or_else(|| {
            panic!(
                "missing {id} at {} turn {}\n{}",
                state.world.current_location,
                state.world.time,
                content.observe(state).unwrap().text
            )
        })
}

fn apply(state: GameState, content: &CompiledContent, id: &str) -> GameState {
    let action = select(&state, content, id);
    let view = content
        .action_page(&state, 0, usize::MAX)
        .unwrap()
        .actions
        .into_iter()
        .find(|view| view.action_id == action.action_id)
        .unwrap();
    assert_eq!(view.time_cost.minimum_ticks, 1);
    assert_eq!(view.time_cost.maximum_ticks, 1);
    let transition = step(&state, &action, content, &state.entropy).unwrap();
    let observation = content.observe_after_transition(&transition).unwrap();
    assert!(
        observation.text.split_whitespace().count()
            + observation.supplies.summary().split_whitespace().count()
            < 100,
        "{}",
        observation.text
    );
    let next = transition.into_state();
    assert_eq!(next.world.time, state.world.time + 1);
    next
}

fn act(state: GameState, content: &CompiledContent, id: &str) -> GameState {
    apply(state, content, id)
}

fn travel(state: GameState, content: &CompiledContent, destination: &str) -> GameState {
    let action = enumerate_legal_actions(&state, content)
        .unwrap()
        .into_iter()
        .find(|action| {
            action.definition_id == "travel_adjacent"
                && action.parameters.get("destination") == Some(&destination.to_owned())
        })
        .unwrap_or_else(|| panic!("cannot travel to {destination} at {}", state.world.time));
    let transition = step(&state, &action, content, &state.entropy).unwrap();
    transition.into_state()
}

fn hold_market(content: &CompiledContent, seed: u64) -> GameState {
    let mut state = content.new_game("ilyan", seed).unwrap();
    state = act(state, content, "checkpoint.show_charter");
    state = travel(state, content, "lowsail.levee");
    state = act(state, content, "levee.authority_path");
    state = travel(state, content, "red_sluice.top");
    state = act(state, content, "top.hold_market");
    state = act(state, content, "world.enter_aftermath");
    state = act(state, content, "return.count_dry_stalls");
    assert_eq!(state.world.time, 7);
    assert_eq!(state.world.current_location, RETURN);
    state
}

fn banked_with_filter(content: &CompiledContent, seed: u64, take_cask: bool) -> GameState {
    let mut state = act(hold_market(content, seed), content, "return.visit_workshop");
    state = act(state, content, "fume_yards.take_stock");
    state = travel(state, content, BAY);
    if take_cask {
        state = act(state, content, "fume_yards.take_cask");
    }
    state = travel(state, content, WORKSHOP);
    state = travel(state, content, ASH);
    state = act(state, content, "fume_yards.buy_collateral_filter");
    state = travel(state, content, WORKSHOP);
    state = travel(state, content, BAY);
    for id in [
        "fume_yards.prepare_charge",
        "fume_yards.fit_dust_filter",
        "fume_yards.take_fuel",
        "fume_yards.ignite_batch",
        "fume_yards.bank_kiln",
    ] {
        state = act(state, content, id);
    }
    assert_eq!(state.character.inventory.get(SPOILED), Some(&1));
    state
}

fn definitions(state: &GameState, content: &CompiledContent) -> BTreeSet<String> {
    enumerate_legal_actions(state, content)
        .unwrap()
        .into_iter()
        .map(|action| action.definition_id)
        .collect()
}

fn assert_recipe(state: &GameState, recipe: &str, input: (&str, u32), output: Option<(&str, u32)>) {
    let events: Vec<_> = state
        .event_log
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::RecipeApplied {
                recipe: actual,
                inputs,
                outputs,
            } if actual == recipe => Some((event.turn, inputs.clone(), outputs.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].1.get(input.0), Some(&input.1));
    match output {
        Some((item, count)) => assert_eq!(events[0].2.get(item), Some(&count)),
        None => assert!(events[0].2.is_empty()),
    }
}

#[test]
fn wet_and_dry_ash_freight_use_real_waste_and_compete_for_the_cask() {
    let content = content();
    let mut wet = banked_with_filter(&content, 71, true);
    wet = act(wet, &content, "fume_yards.bring_pera_to_ash");
    wet = act(wet, &content, "fume_yards.load_spoiled_ash");
    assert_eq!(wet.character.inventory.get(FEED), Some(&1));
    assert_eq!(wet.character.inventory.get(SPOILED), None);
    assert_eq!(wet.world.npcs[PERA].location, ASH);
    assert_eq!(
        wet.world.npcs[PERA].knowledge["fume_yards.ash_feed_loaded"].turn,
        wet.world.time - 1
    );
    wet = act(wet, &content, "fume_yards.prepare_wet_ash_freight");
    assert_eq!(wet.character.inventory.get(FREIGHT), Some(&1));
    assert_eq!(wet.character.inventory.get(CASK), None);
    assert!(
        wet.world.locations[ASH]
            .flags
            .contains("fume_yards.ash_contained")
    );
    assert_recipe(
        &wet,
        "fume_yards.prepare_wet_ash_freight",
        (FEED, 1),
        Some((FREIGHT, 1)),
    );
    wet = act(wet, &content, "fume_yards.escort_ash_freight");
    assert_eq!(wet.world.current_location, RETURN);
    assert_eq!(wet.world.npcs[PERA].location, RETURN);
    assert_eq!(
        wet.world.npcs[OREN].knowledge["fume_yards.ash_freight_condition"].provenance,
        KnowledgeProvenance::Told { by: PERA.into() }
    );
    assert!(!definitions(&wet, &content).contains("return.unload_dirty_ash_freight"));
    wet = act(wet, &content, "return.unload_clean_ash_freight");
    assert_eq!(wet.character.resources["coin"], 9);
    assert_eq!(wet.character.inventory.get(FREIGHT), None);
    assert!(!definitions(&wet, &content).contains("return.unload_clean_ash_freight"));
    let wet = act(wet, &content, "return.send_pera_home");
    assert_eq!(wet.world.npcs[PERA].location, BAY);
    assert_eq!(
        wet.world.npcs[PERA]
            .inventory
            .get(CASK)
            .copied()
            .unwrap_or_default(),
        0
    );

    let mut dry = banked_with_filter(&content, 71, false);
    dry = act(dry, &content, "fume_yards.bring_pera_to_ash");
    dry = act(dry, &content, "fume_yards.load_spoiled_ash");
    dry = act(dry, &content, "fume_yards.prepare_dry_ash_freight");
    assert_eq!(dry.world.npcs[PERA].inventory[CASK], 1);
    assert!(
        dry.world.locations[ASH]
            .flags
            .contains("fume_yards.ash_dirty")
    );
    dry = act(dry, &content, "fume_yards.escort_ash_freight");
    let stale_dirty = select(&dry, &content, "return.unload_dirty_ash_freight");
    assert!(!definitions(&dry, &content).contains("return.unload_clean_ash_freight"));
    dry = act(dry, &content, "return.unload_dirty_ash_freight");
    assert_eq!(dry.character.resources["coin"], 9);
    assert_eq!(dry.character.resources["stamina"], 1);
    assert_eq!(dry.world.npcs[PERA].inventory[CASK], 1);
    assert!(step(&dry, &stale_dirty, &content, &dry.entropy).is_err());
    let dry = act(dry, &content, "return.send_pera_home");
    assert_eq!(dry.world.npcs[PERA].location, BAY);
}

#[test]
fn broken_shard_can_be_loaded_then_cancelled_without_remote_delivery() {
    let content = content();
    let mut state = hold_market(&content, 123);
    state = act(state, &content, "return.visit_workshop");
    state = travel(state, &content, BAY);
    state = act(state, &content, "fume_yards.enter_ash_hatch");
    state = act(state, &content, "fume_yards.pull_rack_filter");
    assert_eq!(state.character.inventory.get(SHARD), Some(&1));
    state = act(state, &content, "fume_yards.leave_ash_hatch");
    state = act(state, &content, "fume_yards.bring_pera_to_ash");
    state = act(state, &content, "fume_yards.load_broken_ash");
    assert_eq!(state.character.inventory.get(SHARD), None);
    assert_eq!(state.character.inventory.get(FEED), Some(&1));
    assert!(
        !state.world.npcs[OREN]
            .knowledge
            .contains_key("fume_yards.ash_feed_loaded")
    );
    state = act(state, &content, "fume_yards.return_pera_from_ash");
    assert_eq!(state.world.npcs[PERA].location, BAY);
    assert_eq!(state.world.current_location, ASH);
    assert!(definitions(&state, &content).contains("travel_adjacent"));
    assert!(!definitions(&state, &content).contains("fume_yards.load_broken_ash"));
    let state = travel(travel(state, &content, WORKSHOP), &content, BAY);
    assert!(definitions(&state, &content).contains("fume_yards.bring_pera_to_ash"));
}

#[test]
fn installed_market_filter_removes_dirty_ash_unloading_cost() {
    let content = content();
    let mut state = hold_market(&content, 123);
    state = act(state, &content, "return.visit_workshop");
    state = act(state, &content, "fume_yards.take_stock");
    state = travel(state, &content, BAY);
    state = act(state, &content, "fume_yards.prepare_charge");
    state = act(state, &content, "fume_yards.reclaim_charge");
    state = travel(state, &content, WORKSHOP);
    state = travel(state, &content, ASH);
    state = act(state, &content, "fume_yards.buy_collateral_filter");
    state = act(state, &content, "world.enter_aftermath");
    state = act(state, &content, "return.patch_stand");
    state = act(state, &content, "return.order_water_stand");
    state = act(state, &content, "return.fit_market_filter");
    assert!(
        state.world.locations[RETURN]
            .flags
            .contains("fume_yards.market_filter_fitted")
    );
    state = act(state, &content, "return.visit_workshop");
    state = travel(state, &content, BAY);
    state = act(state, &content, "fume_yards.enter_ash_hatch");
    state = act(state, &content, "fume_yards.pull_rack_filter");
    assert_eq!(state.character.inventory.get(SHARD), Some(&1));
    state = act(state, &content, "fume_yards.leave_ash_hatch");
    state = act(state, &content, "fume_yards.bring_pera_to_ash");
    state = act(state, &content, "fume_yards.load_broken_ash");
    state = act(state, &content, "fume_yards.prepare_dry_ash_freight");
    state = act(state, &content, "fume_yards.escort_ash_freight");
    assert!(definitions(&state, &content).contains("return.unload_filtered_ash_freight"));
    assert!(!definitions(&state, &content).contains("return.unload_dirty_ash_freight"));
    let stamina = state.character.resources["stamina"];
    state = act(state, &content, "return.unload_filtered_ash_freight");
    assert_eq!(state.character.resources["stamina"], stamina);
    assert_eq!(state.character.resources["coin"], 9);
    assert_eq!(state.character.inventory.get(FREIGHT), None);
}

#[test]
fn settling_the_lane_spends_peras_real_cask_without_retroactively_cleaning_freight() {
    let content = content();
    let mut no_cask = banked_with_filter(&content, 71, true);
    no_cask = act(no_cask, &content, "fume_yards.bring_pera_to_ash");
    no_cask = act(no_cask, &content, "fume_yards.load_spoiled_ash");
    no_cask = act(no_cask, &content, "fume_yards.prepare_dry_ash_freight");
    assert!(!definitions(&no_cask, &content).contains("fume_yards.settle_ash_lane"));

    let mut state = banked_with_filter(&content, 71, false);
    state = act(state, &content, "fume_yards.bring_pera_to_ash");
    state = act(state, &content, "fume_yards.load_spoiled_ash");
    state = act(state, &content, "fume_yards.prepare_dry_ash_freight");
    assert!(definitions(&state, &content).contains("fume_yards.settle_ash_lane"));
    let stale = select(&state, &content, "fume_yards.settle_ash_lane");

    state = act(state, &content, "fume_yards.settle_ash_lane");
    assert_eq!(state.character.inventory.get(CASK), None);
    assert_eq!(state.world.npcs[PERA].inventory.get(CASK), None);
    assert!(
        !state.world.locations[ASH]
            .flags
            .contains("fume_yards.ash_dirty")
    );
    assert!(
        state.world.locations[ASH]
            .flags
            .contains("fume_yards.ash_freight_dirty")
    );
    assert!(
        state.world.locations[ASH]
            .flags
            .contains("fume_yards.ash_lane_settled")
    );
    assert_eq!(
        state.world.npcs[PERA].knowledge["fume_yards.ash_lane_settled"].provenance,
        KnowledgeProvenance::Witnessed
    );
    assert_recipe(&state, "fume_yards.settle_ash_lane", (CASK, 1), None);
    assert!(step(&state, &stale, &content, &state.entropy).is_err());
    assert!(!definitions(&state, &content).contains("return.unload_clean_ash_freight"));

    state = act(state, &content, "fume_yards.escort_ash_freight");
    let stamina = state.character.resources["stamina"];
    state = act(state, &content, "return.unload_dirty_ash_freight");
    assert_eq!(state.character.resources["stamina"], stamina - 2);
    state = act(state, &content, "return.send_pera_home");
    state = act(state, &content, "return.visit_workshop");
    state = travel(state, &content, ASH);
    assert!(
        !state.world.locations[ASH]
            .flags
            .contains("fume_yards.ash_dirty")
    );
    assert!(
        state.world.locations[ASH]
            .flags
            .contains("fume_yards.ash_lane_settled")
    );
}

#[test]
fn dirty_lane_can_be_settled_after_paid_delivery_with_a_physical_pera_return() {
    let content = content();
    let mut state = banked_with_filter(&content, 71, false);
    state = act(state, &content, "fume_yards.bring_pera_to_ash");
    state = act(state, &content, "fume_yards.load_spoiled_ash");
    state = act(state, &content, "fume_yards.prepare_dry_ash_freight");
    state = act(state, &content, "fume_yards.escort_ash_freight");
    state = act(state, &content, "return.unload_dirty_ash_freight");
    state = act(state, &content, "return.send_pera_home");
    assert_eq!(state.character.resources["coin"], 9);
    assert_eq!(state.world.npcs[PERA].location, BAY);
    assert!(
        state.world.locations[ASH]
            .flags
            .contains("fume_yards.ash_dirty")
    );
    assert!(
        state.world.locations[RETURN]
            .flags
            .contains("fume_yards.ash_freight_unloaded")
    );

    state = act(state, &content, "return.visit_workshop");
    state = travel(state, &content, BAY);
    assert!(definitions(&state, &content).contains("fume_yards.bring_pera_back_to_ash"));
    assert!(!definitions(&state, &content).contains("fume_yards.settle_ash_lane"));
    state = act(state, &content, "fume_yards.bring_pera_back_to_ash");
    assert_eq!(state.world.current_location, ASH);
    assert_eq!(state.world.npcs[PERA].location, ASH);
    assert!(
        state.world.npcs[PERA]
            .memories
            .contains_key("fume_yards.ash_lane_returned")
    );
    let stale = select(&state, &content, "fume_yards.settle_ash_lane");
    state = act(state, &content, "fume_yards.settle_ash_lane");
    assert_eq!(state.character.inventory.get(CASK), None);
    assert!(
        !state.world.locations[ASH]
            .flags
            .contains("fume_yards.ash_dirty")
    );
    assert!(
        state.world.locations[ASH]
            .flags
            .contains("fume_yards.ash_lane_settled")
    );
    assert!(
        state.world.locations[RETURN]
            .flags
            .contains("fume_yards.ash_freight_unloaded")
    );
    assert!(step(&state, &stale, &content, &state.entropy).is_err());

    let mut no_cask = banked_with_filter(&content, 71, true);
    no_cask = act(no_cask, &content, "fume_yards.bring_pera_to_ash");
    no_cask = act(no_cask, &content, "fume_yards.load_spoiled_ash");
    no_cask = act(no_cask, &content, "fume_yards.prepare_dry_ash_freight");
    no_cask = act(no_cask, &content, "fume_yards.escort_ash_freight");
    no_cask = act(no_cask, &content, "return.unload_dirty_ash_freight");
    no_cask = act(no_cask, &content, "return.send_pera_home");
    no_cask = act(no_cask, &content, "return.visit_workshop");
    no_cask = travel(no_cask, &content, BAY);
    assert!(!definitions(&no_cask, &content).contains("fume_yards.bring_pera_back_to_ash"));
}
