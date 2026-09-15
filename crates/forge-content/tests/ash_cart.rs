use std::collections::BTreeSet;

use forge_content::parse_and_compile_production;
use forge_kernel::{
    CanonicalAction, CharacterChoiceSelection, CharacterSelection, CompiledContent, EventKind,
    GameState, KnowledgeProvenance, enumerate_legal_actions, step,
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

fn hold_market_for(content: &CompiledContent, character: &str, seed: u64) -> GameState {
    let mut state = content.new_game(character, seed).unwrap();
    if character == "ilyan" {
        state = act(state, content, "checkpoint.show_charter");
        state = travel(state, content, "lowsail.levee");
        state = act(state, content, "levee.authority_path");
    } else {
        state = act(state, content, "checkpoint.read_flag");
        state = act(state, content, "checkpoint.ask_sava");
        state = travel(state, content, "lowsail.docks");
        state = act(state, content, "docks.ask_oren");
        state = travel(state, content, "lowsail.levee");
        state = act(state, content, "levee.culvert_path");
    }
    state = travel(state, content, "red_sluice.top");
    state = if character == "ilyan" {
        act(state, content, "top.hold_market")
    } else {
        act(state, content, "top.break_toll")
    };
    state = act(state, content, "world.enter_aftermath");
    if character == "ilyan" {
        state = act(state, content, "return.count_dry_stalls");
        assert_eq!(state.world.time, 7);
    }
    assert_eq!(state.world.current_location, RETURN);
    state
}

fn hold_market(content: &CompiledContent, seed: u64) -> GameState {
    hold_market_for(content, "ilyan", seed)
}

fn banked_with_filter(content: &CompiledContent, seed: u64, take_cask: bool) -> GameState {
    banked_with_filter_for(content, "ilyan", seed, take_cask)
}

fn banked_with_filter_for(
    content: &CompiledContent,
    character: &str,
    seed: u64,
    take_cask: bool,
) -> GameState {
    banked_with_filter_from_return(
        content,
        hold_market_for(content, character, seed),
        take_cask,
    )
}

fn banked_with_filter_from_return(
    content: &CompiledContent,
    mut state: GameState,
    take_cask: bool,
) -> GameState {
    state = act(state, content, "return.visit_workshop");
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

fn custom_banked_with_filter(content: &CompiledContent, mask: usize, seed: u64) -> GameState {
    let selection = CharacterSelection {
        name: "Ash route comparison".into(),
        choices: content
            .character_creation()
            .unwrap()
            .slots
            .iter()
            .enumerate()
            .map(|(index, slot)| CharacterChoiceSelection {
                slot_id: slot.id.clone(),
                choice_id: slot.choices[(mask >> index) & 1].id.clone(),
            })
            .collect(),
    };
    let mut state = content.new_custom_game(&selection, seed).unwrap();
    for (id, destination) in [
        ("checkpoint.read_flag", None),
        ("checkpoint.ask_sava", None),
        ("travel_adjacent", Some("lowsail.docks")),
        ("docks.ask_oren", None),
        ("travel_adjacent", Some("lowsail.levee")),
        ("levee.culvert_path", None),
        ("travel_adjacent", Some("red_sluice.top")),
        ("top.break_toll", None),
        ("world.enter_aftermath", None),
        ("return.visit_workshop", None),
    ] {
        state = match destination {
            Some(destination) => travel(state, content, destination),
            None => act(state, content, id),
        };
    }
    state = act(state, content, "fume_yards.take_stock");
    state = travel(state, content, BAY);
    state = act(state, content, "fume_yards.prepare_charge");
    state = travel(state, content, WORKSHOP);
    state = travel(state, content, ASH);
    state = act(state, content, "fume_yards.buy_collateral_filter");
    state = travel(state, content, WORKSHOP);
    state = travel(state, content, BAY);
    for id in [
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

fn market_filter_ready_for_ash(content: &CompiledContent, seed: u64) -> GameState {
    let mut state = hold_market(content, seed);
    state = act(state, content, "return.visit_workshop");
    state = act(state, content, "fume_yards.take_stock");
    state = travel(state, content, BAY);
    state = act(state, content, "fume_yards.prepare_charge");
    state = act(state, content, "fume_yards.reclaim_charge");
    state = travel(state, content, WORKSHOP);
    state = travel(state, content, ASH);
    state = act(state, content, "fume_yards.buy_collateral_filter");
    state = act(state, content, "world.enter_aftermath");
    state = act(state, content, "return.patch_stand");
    state = act(state, content, "return.order_water_stand");
    state = act(state, content, "return.fit_market_filter");
    assert!(
        state.world.locations[RETURN]
            .flags
            .contains("fume_yards.market_filter_fitted")
    );
    state = act(state, content, "return.visit_workshop");
    travel(state, content, BAY)
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

#[test]
fn settled_post_delivery_lane_can_send_pera_home_without_moving_the_player() {
    let content = content();
    let mut state = banked_with_filter(&content, 71, false);
    state = act(state, &content, "fume_yards.bring_pera_to_ash");
    state = act(state, &content, "fume_yards.load_spoiled_ash");
    state = act(state, &content, "fume_yards.prepare_dry_ash_freight");
    state = act(state, &content, "fume_yards.escort_ash_freight");
    state = act(state, &content, "return.unload_dirty_ash_freight");
    state = act(state, &content, "return.send_pera_home");
    state = act(state, &content, "return.visit_workshop");
    state = travel(state, &content, BAY);
    assert!(!definitions(&state, &content).contains("fume_yards.return_pera_after_ash_cleanup"));

    state = act(state, &content, "fume_yards.bring_pera_back_to_ash");
    state = act(state, &content, "fume_yards.settle_ash_lane");
    assert!(definitions(&state, &content).contains("fume_yards.return_pera_after_ash_cleanup"));
    let stale = select(&state, &content, "fume_yards.return_pera_after_ash_cleanup");

    state = act(state, &content, "fume_yards.return_pera_after_ash_cleanup");
    assert_eq!(state.world.current_location, ASH);
    assert_eq!(state.world.npcs[PERA].location, BAY);
    assert!(
        state.world.npcs[PERA]
            .memories
            .contains_key("fume_yards.returned_after_ash_cleanup")
    );
    assert_eq!(state.character.resources["coin"], 9);
    assert_eq!(state.character.inventory.get(CASK), None);
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
    assert!(!definitions(&state, &content).contains("fume_yards.return_pera_after_ash_cleanup"));
}

#[test]
fn ledger_clerk_can_file_a_witnessed_ash_manifest_before_unload() {
    let content = content();
    let mut state = banked_with_filter(&content, 71, false);
    state = act(state, &content, "fume_yards.bring_pera_to_ash");
    state = act(state, &content, "fume_yards.load_spoiled_ash");
    state = act(state, &content, "fume_yards.prepare_dry_ash_freight");
    assert!(definitions(&state, &content).contains("fume_yards.audit_ash_manifest"));
    let stale_audit = select(&state, &content, "fume_yards.audit_ash_manifest");

    state = act(state, &content, "fume_yards.audit_ash_manifest");
    assert!(
        state.world.locations[ASH]
            .flags
            .contains("fume_yards.ash_manifest_audited")
    );
    assert!(
        state
            .character
            .deeds
            .contains("fume_yards.ash_manifest_audited")
    );
    assert_eq!(
        state.world.npcs[PERA].memories["fume_yards.ash_manifest_audited"].provenance,
        KnowledgeProvenance::Witnessed
    );
    assert!(
        !state.world.npcs[OREN]
            .knowledge
            .contains_key("fume_yards.ash_manifest_filed")
    );
    assert!(step(&state, &stale_audit, &content, &state.entropy).is_err());

    state = act(state, &content, "fume_yards.escort_ash_freight");
    assert!(definitions(&state, &content).contains("return.file_ash_manifest"));
    assert!(definitions(&state, &content).contains("return.unload_dirty_ash_freight"));
    let stale_file = select(&state, &content, "return.file_ash_manifest");

    state = act(state, &content, "return.file_ash_manifest");
    assert_eq!(state.world.current_location, RETURN);
    assert_eq!(state.world.npcs[PERA].location, BAY);
    assert_eq!(state.character.inventory.get(FREIGHT), Some(&1));
    assert_eq!(
        state.world.npcs[OREN].knowledge["fume_yards.ash_manifest_filed"].provenance,
        KnowledgeProvenance::Witnessed
    );
    assert!(
        state.world.npcs[OREN]
            .memories
            .contains_key("fume_yards.ash_manifest_filed")
    );
    assert!(
        state.world.npcs[PERA]
            .memories
            .contains_key("fume_yards.ash_manifest_filed")
    );
    assert!(
        state.world.locations[RETURN]
            .flags
            .contains("fume_yards.ash_manifest_filed")
    );
    assert!(!definitions(&state, &content).contains("return.send_pera_home"));
    assert!(definitions(&state, &content).contains("return.unload_dirty_ash_freight"));

    state = act(state, &content, "return.unload_dirty_ash_freight");
    assert_eq!(state.character.resources["coin"], 9);
    assert_eq!(state.character.resources["stamina"], 1);
    assert_eq!(state.character.inventory.get(FREIGHT), None);
    assert!(
        state.world.npcs[OREN]
            .memories
            .contains_key("fume_yards.ash_freight_paid")
    );
    assert!(step(&state, &stale_file, &content, &state.entropy).is_err());

    let mut rook = banked_with_filter_for(&content, "rook", 71, false);
    rook = act(rook, &content, "fume_yards.bring_pera_to_ash");
    rook = act(rook, &content, "fume_yards.load_spoiled_ash");
    rook = act(rook, &content, "fume_yards.prepare_dry_ash_freight");
    assert!(!definitions(&rook, &content).contains("fume_yards.audit_ash_manifest"));
}

#[test]
fn filed_manifest_releases_pera_before_clean_unload() {
    let content = content();
    let mut state = banked_with_filter(&content, 71, true);
    state = act(state, &content, "fume_yards.bring_pera_to_ash");
    state = act(state, &content, "fume_yards.load_spoiled_ash");
    state = act(state, &content, "fume_yards.prepare_wet_ash_freight");
    state = act(state, &content, "fume_yards.audit_ash_manifest");
    state = act(state, &content, "fume_yards.escort_ash_freight");
    let stale_file = select(&state, &content, "return.file_ash_manifest");

    state = act(state, &content, "return.file_ash_manifest");
    assert_eq!(state.world.npcs[PERA].location, BAY);
    assert_eq!(state.world.npcs[PERA].inventory.get(CASK), None);
    assert_eq!(state.character.inventory.get(FREIGHT), Some(&1));
    assert!(definitions(&state, &content).contains("return.unload_clean_ash_freight"));
    assert!(!definitions(&state, &content).contains("return.unload_dirty_ash_freight"));
    assert!(!definitions(&state, &content).contains("return.unload_filtered_ash_freight"));

    let stamina = state.character.resources["stamina"];
    state = act(state, &content, "return.unload_clean_ash_freight");
    assert_eq!(state.character.resources["coin"], 9);
    assert_eq!(state.character.resources["stamina"], stamina);
    assert_eq!(state.character.inventory.get(FREIGHT), None);
    assert!(
        state.world.locations[ASH]
            .flags
            .contains("fume_yards.ash_contained")
    );
    assert!(
        state.world.locations[RETURN]
            .flags
            .contains("fume_yards.ash_manifest_filed")
    );
    assert!(
        state.world.locations[RETURN]
            .flags
            .contains("fume_yards.ash_freight_unloaded")
    );
    assert!(
        state.world.npcs[OREN]
            .memories
            .contains_key("fume_yards.ash_freight_paid")
    );
    assert!(step(&state, &stale_file, &content, &state.entropy).is_err());
    assert!(!definitions(&state, &content).contains("return.send_pera_home"));
}

#[test]
fn filed_manifest_preserves_filtered_unload_and_post_delivery_cleanup() {
    let content = content();
    let mut state = market_filter_ready_for_ash(&content, 123);
    state = act(state, &content, "fume_yards.enter_ash_hatch");
    state = act(state, &content, "fume_yards.pull_rack_filter");
    assert_eq!(state.character.inventory.get(SHARD), Some(&1));
    state = act(state, &content, "fume_yards.leave_ash_hatch");
    state = act(state, &content, "fume_yards.bring_pera_to_ash");
    state = act(state, &content, "fume_yards.load_broken_ash");
    state = act(state, &content, "fume_yards.prepare_dry_ash_freight");
    state = act(state, &content, "fume_yards.audit_ash_manifest");
    state = act(state, &content, "fume_yards.escort_ash_freight");
    let stale_file = select(&state, &content, "return.file_ash_manifest");

    state = act(state, &content, "return.file_ash_manifest");
    assert_eq!(state.world.npcs[PERA].location, BAY);
    assert_eq!(state.world.npcs[PERA].inventory[CASK], 1);
    assert!(definitions(&state, &content).contains("return.unload_filtered_ash_freight"));
    assert!(!definitions(&state, &content).contains("return.unload_dirty_ash_freight"));
    assert!(!definitions(&state, &content).contains("return.unload_clean_ash_freight"));

    let stamina = state.character.resources["stamina"];
    state = act(state, &content, "return.unload_filtered_ash_freight");
    assert_eq!(state.character.resources["coin"], 9);
    assert_eq!(state.character.resources["stamina"], stamina);
    assert_eq!(state.character.inventory.get(FREIGHT), None);
    assert!(
        state.world.locations[ASH]
            .flags
            .contains("fume_yards.ash_dirty")
    );
    assert!(
        state.world.locations[ASH]
            .flags
            .contains("fume_yards.ash_freight_dirty")
    );
    assert!(step(&state, &stale_file, &content, &state.entropy).is_err());
    state = act(state, &content, "return.visit_workshop");
    state = travel(state, &content, BAY);
    assert!(definitions(&state, &content).contains("fume_yards.bring_pera_back_to_ash"));

    state = act(state, &content, "fume_yards.bring_pera_back_to_ash");
    assert_eq!(state.world.npcs[PERA].location, ASH);
    state = act(state, &content, "fume_yards.settle_ash_lane");
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
    assert_eq!(state.character.inventory.get(CASK), None);
    assert!(definitions(&state, &content).contains("fume_yards.return_pera_after_ash_cleanup"));
    state = act(state, &content, "fume_yards.return_pera_after_ash_cleanup");
    assert_eq!(state.world.npcs[PERA].location, BAY);
    assert_eq!(state.world.current_location, ASH);
    assert!(
        state.world.locations[RETURN]
            .flags
            .contains("fume_yards.ash_manifest_filed")
    );
    assert!(
        state.world.locations[RETURN]
            .flags
            .contains("fume_yards.ash_freight_unloaded")
    );
    assert!(
        state.world.locations[ASH]
            .flags
            .contains("fume_yards.ash_lane_settled")
    );
    assert!(!definitions(&state, &content).contains("return.send_pera_home"));
}

#[test]
fn rook_can_complete_ordinary_dirty_delivery_without_audited_release() {
    let content = content();
    let mut state = banked_with_filter_for(&content, "rook", 71, false);
    state = act(state, &content, "fume_yards.bring_pera_to_ash");
    state = act(state, &content, "fume_yards.load_spoiled_ash");
    state = act(state, &content, "fume_yards.prepare_dry_ash_freight");
    assert!(!definitions(&state, &content).contains("fume_yards.audit_ash_manifest"));
    state = act(state, &content, "fume_yards.escort_ash_freight");
    assert!(definitions(&state, &content).contains("return.unload_dirty_ash_freight"));
    assert!(!definitions(&state, &content).contains("return.file_ash_manifest"));
    assert_eq!(
        state.world.npcs[OREN].knowledge["fume_yards.ash_freight_condition"].provenance,
        KnowledgeProvenance::Told { by: PERA.into() }
    );

    state = act(state, &content, "return.unload_dirty_ash_freight");
    assert_eq!(state.character.resources["coin"], 4);
    assert_eq!(state.character.resources["stamina"], 2);
    assert_eq!(state.world.npcs[PERA].inventory[CASK], 1);
    assert_eq!(state.character.inventory.get(FREIGHT), None);
    assert!(
        state.world.locations[RETURN]
            .flags
            .contains("fume_yards.ash_freight_unloaded")
    );
    assert!(
        !state.world.locations[RETURN]
            .flags
            .contains("fume_yards.ash_manifest_filed")
    );
    assert!(
        state.world.npcs[OREN]
            .memories
            .contains_key("fume_yards.ash_freight_paid")
    );
    assert!(definitions(&state, &content).contains("return.send_pera_home"));
    state = act(state, &content, "return.send_pera_home");
    assert_eq!(state.world.npcs[PERA].location, BAY);
}

#[test]
fn oren_learns_only_peras_ash_condition_after_physical_escort() {
    let content = content();
    let mut state = banked_with_filter(&content, 71, false);
    state = act(state, &content, "fume_yards.bring_pera_to_ash");
    state = act(state, &content, "fume_yards.load_spoiled_ash");
    state = act(state, &content, "fume_yards.prepare_dry_ash_freight");

    assert!(
        !state.world.npcs[OREN]
            .knowledge
            .contains_key("fume_yards.ash_freight_condition")
    );
    assert!(
        !state.world.npcs[OREN]
            .knowledge
            .contains_key("fume_yards.ash_manifest_filed")
    );
    assert!(state.world.npcs[OREN].inventory.is_empty());

    let escort_turn = state.world.time;
    state = act(state, &content, "fume_yards.escort_ash_freight");
    let condition = &state.world.npcs[OREN].knowledge["fume_yards.ash_freight_condition"];
    assert_eq!(condition.turn, escort_turn);
    assert_eq!(
        condition.provenance,
        KnowledgeProvenance::Told { by: PERA.into() }
    );
    assert!(
        !state.world.npcs[OREN]
            .knowledge
            .contains_key("fume_yards.ash_manifest_filed")
    );
    assert!(state.world.npcs[OREN].inventory.is_empty());
}

#[test]
fn matched_custom_callings_separate_manifest_release_from_ordinary_delivery() {
    let content = content();
    let mut clerk = custom_banked_with_filter(&content, 8, 71);
    clerk = act(clerk, &content, "fume_yards.bring_pera_to_ash");
    clerk = act(clerk, &content, "fume_yards.load_spoiled_ash");
    clerk = act(clerk, &content, "fume_yards.prepare_dry_ash_freight");
    assert!(definitions(&clerk, &content).contains("fume_yards.audit_ash_manifest"));
    clerk = act(clerk, &content, "fume_yards.audit_ash_manifest");
    clerk = act(clerk, &content, "fume_yards.escort_ash_freight");
    assert!(definitions(&clerk, &content).contains("return.file_ash_manifest"));
    clerk = act(clerk, &content, "return.file_ash_manifest");
    assert_eq!(clerk.world.npcs[PERA].location, BAY);
    assert!(!definitions(&clerk, &content).contains("return.send_pera_home"));
    let clerk_coin = clerk.character.resources["coin"];
    let clerk_stamina = clerk.character.resources["stamina"];
    clerk = act(clerk, &content, "return.unload_dirty_ash_freight");
    assert_eq!(clerk.character.resources["coin"], clerk_coin + 3);
    assert_eq!(clerk.character.resources["stamina"], clerk_stamina - 2);

    let mut runner = custom_banked_with_filter(&content, 63, 71);
    runner = act(runner, &content, "fume_yards.bring_pera_to_ash");
    runner = act(runner, &content, "fume_yards.load_spoiled_ash");
    runner = act(runner, &content, "fume_yards.prepare_dry_ash_freight");
    assert!(!definitions(&runner, &content).contains("fume_yards.audit_ash_manifest"));
    runner = act(runner, &content, "fume_yards.escort_ash_freight");
    assert!(!definitions(&runner, &content).contains("return.file_ash_manifest"));
    let runner_coin = runner.character.resources["coin"];
    let runner_stamina = runner.character.resources["stamina"];
    runner = act(runner, &content, "return.unload_dirty_ash_freight");
    assert_eq!(runner.character.resources["coin"], runner_coin + 3);
    assert_eq!(runner.character.resources["stamina"], runner_stamina - 2);
    runner = act(runner, &content, "return.send_pera_home");
    assert_eq!(runner.world.npcs[PERA].location, BAY);
}

#[test]
fn every_custom_combination_can_complete_ordinary_dirty_delivery() {
    let content = content();
    for mask in 0..64 {
        let mut state = custom_banked_with_filter(&content, mask, 71);
        let coin = state.character.resources["coin"];
        let stamina = state.character.resources["stamina"];
        state = act(state, &content, "fume_yards.bring_pera_to_ash");
        state = act(state, &content, "fume_yards.load_spoiled_ash");
        state = act(state, &content, "fume_yards.prepare_dry_ash_freight");
        assert!(
            definitions(&state, &content).contains("fume_yards.escort_ash_freight"),
            "mask {mask} at {} turn {} with Pera at {}: {:?}",
            state.world.current_location,
            state.world.time,
            state.world.npcs[PERA].location,
            definitions(&state, &content)
        );
        state = act(state, &content, "fume_yards.escort_ash_freight");
        assert!(definitions(&state, &content).contains("return.unload_dirty_ash_freight"));

        state = act(state, &content, "return.unload_dirty_ash_freight");
        assert_eq!(state.character.resources["coin"], coin + 3, "mask {mask}");
        assert_eq!(
            state.character.resources["stamina"],
            stamina - 2,
            "mask {mask}"
        );
        assert_eq!(state.world.npcs[PERA].inventory[CASK], 1, "mask {mask}");
        assert_eq!(state.character.inventory.get(FREIGHT), None, "mask {mask}");
        assert_eq!(
            state.world.npcs[OREN].knowledge["fume_yards.ash_freight_condition"].provenance,
            KnowledgeProvenance::Told { by: PERA.into() },
            "mask {mask}"
        );
        assert!(definitions(&state, &content).contains("return.send_pera_home"));
        state = act(state, &content, "return.send_pera_home");
        assert_eq!(state.world.npcs[PERA].location, BAY, "mask {mask}");
    }
}

#[test]
fn ordinary_dirty_delivery_preserves_each_reviewed_tide_context() {
    type Spec = (&'static str, Option<&'static str>);
    const SPLIT: &[Spec] = &[
        ("checkpoint.show_charter", None),
        ("travel_adjacent", Some("lowsail.levee")),
        ("levee.authority_path", None),
        ("floor.read_harmonics", None),
        ("travel_adjacent", Some("red_sluice.top")),
        ("top.check_wheels", None),
        ("top.split_flow", None),
        ("world.enter_aftermath", None),
        ("return.share_water", None),
    ];
    const HOLD: &[Spec] = &[
        ("checkpoint.show_charter", None),
        ("travel_adjacent", Some("lowsail.levee")),
        ("levee.authority_path", None),
        ("travel_adjacent", Some("red_sluice.top")),
        ("top.hold_market", None),
        ("world.enter_aftermath", None),
        ("return.count_dry_stalls", None),
    ];
    const RELIEF: &[Spec] = &[
        ("travel_adjacent", Some("lowsail.docks")),
        ("docks.ring_warning", None),
        ("docks.ask_oren", None),
        ("travel_adjacent", Some("lowsail.levee")),
        ("levee.relay_warning", None),
        ("levee.culvert_path", None),
        ("floor.open_relief", None),
        ("travel_adjacent", Some("red_sluice.top")),
        ("top.divert_relief", None),
        ("world.enter_aftermath", None),
        ("return.move_inland", None),
    ];
    const FERRY: &[Spec] = &[
        ("checkpoint.blend_workers", None),
        ("travel_adjacent", Some("lowsail.levee")),
        ("levee.culvert_path", None),
        ("travel_adjacent", Some("red_sluice.top")),
        ("top.break_toll", None),
        ("world.enter_aftermath", None),
        ("return.open_ferry", None),
    ];
    const OVERLOAD: &[Spec] = &[
        ("checkpoint.use_stolen_permit", None),
        ("travel_adjacent", Some("lowsail.levee")),
        ("levee.stolen_path", None),
        ("floor.force_wheel", None),
        ("travel_adjacent", Some("red_sluice.top")),
        ("top.overload", None),
        ("world.enter_aftermath", None),
        ("return.face_flood", None),
    ];
    let mut deadline = vec![("wait_tide", None); 16];
    deadline.extend([("world.enter_aftermath", None), ("return.face_flood", None)]);
    let cases = [
        ("ilyan", SPLIT, "ending_accord"),
        ("ilyan", HOLD, "ending_council"),
        ("rook", RELIEF, "ending_relief"),
        ("rook", FERRY, "ending_freedom"),
        ("rook", OVERLOAD, "ending_disaster"),
        ("ilyan", deadline.as_slice(), "ending_disaster"),
    ];

    let content = content();
    for (character, prefix, ending) in cases {
        let mut state = content.new_game(character, 71).unwrap();
        for (id, destination) in prefix {
            state = match destination {
                Some(destination) => travel(state, &content, destination),
                None => act(state, &content, id),
            };
        }
        assert!(state.world.flags.contains(ending));
        let flags = state.world.flags.clone();
        state = banked_with_filter_from_return(&content, state, false);
        let coin = state.character.resources["coin"];
        let stamina = state.character.resources["stamina"];
        state = act(state, &content, "fume_yards.bring_pera_to_ash");
        state = act(state, &content, "fume_yards.load_spoiled_ash");
        state = act(state, &content, "fume_yards.prepare_dry_ash_freight");
        state = act(state, &content, "fume_yards.escort_ash_freight");
        assert!(definitions(&state, &content).contains("return.unload_dirty_ash_freight"));
        state = act(state, &content, "return.unload_dirty_ash_freight");
        assert_eq!(state.character.resources["coin"], coin + 3);
        assert_eq!(state.character.resources["stamina"], stamina - 2);
        assert_eq!(state.world.flags, flags);
        assert_eq!(state.world.npcs[PERA].inventory[CASK], 1);
        assert_eq!(state.character.inventory.get(FREIGHT), None);
        assert_eq!(
            state.world.npcs[OREN].knowledge["fume_yards.ash_freight_condition"].provenance,
            KnowledgeProvenance::Told { by: PERA.into() }
        );
        state = act(state, &content, "return.send_pera_home");
        assert_eq!(state.world.npcs[PERA].location, BAY);
    }
}

#[test]
fn first_ash_delivery_after_long_old_world_traversal_preserves_tide_and_custody() {
    let content = content();
    let mut state = hold_market(&content, 71);
    let tide_flags = state.world.flags.clone();
    while state.world.time < 129 {
        state = act(state, &content, "wait_tide");
    }
    assert_eq!(state.world.current_location, RETURN);
    assert_eq!(state.world.time, 129);
    assert_eq!(state.world.npcs[PERA].location, BAY);

    state = banked_with_filter_from_return(&content, state, false);
    let coin = state.character.resources["coin"];
    let stamina = state.character.resources["stamina"];
    state = act(state, &content, "fume_yards.bring_pera_to_ash");
    state = act(state, &content, "fume_yards.load_spoiled_ash");
    state = act(state, &content, "fume_yards.prepare_dry_ash_freight");
    state = act(state, &content, "fume_yards.escort_ash_freight");
    state = act(state, &content, "return.unload_dirty_ash_freight");
    assert_eq!(state.world.flags, tide_flags);
    assert_eq!(state.character.resources["coin"], coin + 3);
    assert_eq!(state.character.resources["stamina"], stamina - 2);
    assert_eq!(state.world.npcs[PERA].inventory[CASK], 1);
    assert_eq!(state.character.inventory.get(FREIGHT), None);
    assert_eq!(state.world.npcs[PERA].location, RETURN);
    assert_eq!(
        state.world.npcs[OREN].knowledge["fume_yards.ash_freight_condition"].provenance,
        KnowledgeProvenance::Told { by: PERA.into() }
    );
    state = act(state, &content, "return.send_pera_home");
    assert_eq!(state.world.npcs[PERA].location, BAY);
}
