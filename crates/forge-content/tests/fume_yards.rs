use std::collections::BTreeSet;

use forge_content::parse_and_compile_production;
use forge_kernel::{
    CanonicalAction, CharacterChoiceSelection, CharacterSelection, CompiledContent, EventKind,
    GameState, KnowledgeProvenance, enumerate_legal_actions, step,
};

const SOURCE: &str = include_str!("../../../content/split-tide.json");
const WORKSHOP: &str = "fume_yards.workshop";
const NESSA: &str = "fume_yards.nessa_tern";
const CLAY: &str = "fume_yards.clay";
const MESH: &str = "fume_yards.mesh";
const PLUGS: &str = "fume_yards.repair_lot";
const SCREEN: &str = "fume_yards.catch_screen";

fn content() -> CompiledContent {
    parse_and_compile_production(SOURCE).expect("cold workshop production pack compiles")
}

fn select(
    state: &GameState,
    content: &CompiledContent,
    id: &str,
    destination: Option<&str>,
) -> CanonicalAction {
    enumerate_legal_actions(state, content)
        .unwrap()
        .into_iter()
        .find(|action| {
            action.definition_id == id
                && destination.is_none_or(|destination| {
                    action
                        .parameters
                        .get("destination")
                        .is_some_and(|value| value == destination)
                })
        })
        .unwrap_or_else(|| {
            panic!(
                "missing {id} ({destination:?}) at {} turn {}",
                state.world.current_location, state.world.time
            )
        })
}

fn apply(
    state: GameState,
    content: &CompiledContent,
    id: &str,
    destination: Option<&str>,
) -> GameState {
    let action = select(&state, content, id, destination);
    let page = content.action_page(&state, 0, usize::MAX).unwrap();
    let view = page
        .actions
        .iter()
        .find(|view| view.action_id == action.action_id)
        .unwrap();
    assert_eq!(view.time_cost.minimum_ticks, 1);
    assert_eq!(view.time_cost.maximum_ticks, 1);
    let transition = step(&state, &action, content, &state.entropy).unwrap();
    let observation = content.observe_after_transition(&transition).unwrap();
    assert!(
        observation.text.split_whitespace().count()
            + observation.supplies.summary().split_whitespace().count()
            < 100
    );
    let next = transition.into_state();
    assert_eq!(next.world.time, state.world.time + 1);
    next
}

fn act(state: GameState, content: &CompiledContent, id: &str) -> GameState {
    apply(state, content, id, None)
}

fn travel(state: GameState, content: &CompiledContent, destination: &str) -> GameState {
    apply(state, content, "travel_adjacent", Some(destination))
}

fn enter(state: GameState, content: &CompiledContent) -> GameState {
    let state = travel(state, content, "lowsail.levee");
    travel(state, content, WORKSHOP)
}

fn definitions(state: &GameState, content: &CompiledContent) -> BTreeSet<String> {
    enumerate_legal_actions(state, content)
        .unwrap()
        .into_iter()
        .map(|action| action.definition_id)
        .collect()
}

fn owned(state: &GameState, item: &str) -> u32 {
    state.character.inventory.get(item).copied().unwrap_or(0)
}

fn deadline_return(mut state: GameState, content: &CompiledContent) -> GameState {
    while state.world.time < 16 {
        state = act(state, content, "wait_tide");
    }
    act(state, content, "world.enter_aftermath")
}

fn assert_recipe(
    state: &GameState,
    recipe_id: &str,
    expected_inputs: &[(&str, u32)],
    expected_outputs: &[(&str, u32)],
) {
    let events: Vec<_> = state
        .event_log
        .iter()
        .filter_map(|event| {
            if let EventKind::RecipeApplied {
                recipe,
                inputs,
                outputs,
            } = &event.kind
            {
                (recipe == recipe_id).then_some((inputs, outputs))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].0,
        &expected_inputs
            .iter()
            .map(|(id, count)| ((*id).to_owned(), *count))
            .collect()
    );
    assert_eq!(
        events[0].1,
        &expected_outputs
            .iter()
            .map(|(id, count)| ((*id).to_owned(), *count))
            .collect()
    );
}

#[test]
fn finite_stock_canonical_recipe_and_external_paid_work_preserve_credible_knowledge() {
    let content = content();
    let state = content.new_game("ilyan", 71).unwrap();
    let state = enter(state, &content);
    let observation = content.observe(&state).unwrap();
    assert!(observation.text.contains("two clay and one mesh"));
    assert!(observation.text.contains("three-coin sorting"));
    assert!(observation.text.contains("two stamina"));
    assert!(observation.text.contains("tide steps"));
    assert!(
        !observation
            .supplies
            .items
            .iter()
            .any(|entry| entry.id == CLAY || entry.id == MESH)
    );
    assert_eq!(state.world.npcs[NESSA].inventory[CLAY], 2);
    assert_eq!(state.world.npcs[NESSA].inventory[MESH], 1);
    let state = act(state, &content, "fume_yards.take_stock");
    let stock_memory = &state.world.npcs[NESSA].memories["fume_yards.stock_handed_over"];
    assert_eq!(stock_memory.turn, 2);
    assert_eq!(stock_memory.provenance, KnowledgeProvenance::Witnessed);
    assert!(state.world.npcs[NESSA].inventory.is_empty());
    assert_eq!((owned(&state, CLAY), owned(&state, MESH)), (2, 1));
    let stale_screen = select(&state, &content, "fume_yards.pack_catch_screen", None);
    let state = act(state, &content, "fume_yards.press_repair_plugs");
    assert_eq!(
        (
            owned(&state, CLAY),
            owned(&state, MESH),
            owned(&state, PLUGS),
            owned(&state, SCREEN)
        ),
        (0, 0, 1, 0)
    );
    assert_recipe(
        &state,
        "fume_yards.press_repair_plugs",
        &[(CLAY, 2), (MESH, 1)],
        &[(PLUGS, 1)],
    );
    let before_stale = state.clone();
    assert!(step(&state, &stale_screen, &content, &state.entropy).is_err());
    assert_eq!(state, before_stale);
    let state = act(state, &content, "fume_yards.load_freight");
    assert_eq!(state.character.resources["coin"], 12);
    assert_eq!(state.character.resources["stamina"], 1);
    assert!(
        content
            .observe(&state)
            .unwrap()
            .text
            .contains("Patch Stand")
    );
    assert!(!state.world.npcs["oren_pell"].knows("fume_yards.stand_patched"));
    assert!(!state.world.npcs["oren_pell"].remembers("fume_yards.repair_plugs_pressed"));
    let state = deadline_return(state, &content);
    let before_patch_time = state.world.time;
    assert_eq!(state.world.npcs["oren_pell"].location, "lowsail.return");
    assert!(!state.world.npcs["oren_pell"].knows("fume_yards.stand_patched"));
    let state = act(state, &content, "return.patch_stand");
    assert_eq!(owned(&state, PLUGS), 0);
    assert_recipe(&state, "fume_yards.patch_stand", &[(PLUGS, 1)], &[]);
    let knowledge = &state.world.npcs["oren_pell"].knowledge["fume_yards.stand_patched"];
    assert_eq!(knowledge.turn, before_patch_time);
    assert_eq!(knowledge.provenance, KnowledgeProvenance::Witnessed);
    assert!(!state.world.npcs[NESSA].knows("fume_yards.stand_patched"));
    assert!(
        content
            .observe(&state)
            .unwrap()
            .text
            .contains("old market crossing remains lost")
    );
    let state = act(state, &content, "return.sort_dry_goods");
    assert_eq!(state.character.resources["coin"], 15);
    assert!(
        content
            .observe(&state)
            .unwrap()
            .text
            .contains("holds sorted goods")
    );
    assert!(state.world.flags.contains("surge_missed"));
    assert!(!definitions(&state, &content).contains("return.patch_stand"));
    assert!(!definitions(&state, &content).contains("return.sort_dry_goods"));
    let state = act(state, &content, "return.visit_workshop");
    let old_npcs = state.world.npcs.clone();
    let state = act(state, &content, "world.enter_aftermath");
    assert_eq!(state.world.npcs, old_npcs);
    let state = act(state, &content, "return.visit_workshop");
    assert!(
        content
            .observe(&state)
            .unwrap()
            .text
            .contains("work remains finished")
    );
    for id in [
        "fume_yards.take_stock",
        "fume_yards.press_repair_plugs",
        "fume_yards.pack_catch_screen",
        "fume_yards.fit_catch_screen",
        "fume_yards.load_freight",
        "fume_yards.load_screened_freight",
    ] {
        assert!(
            !definitions(&state, &content).contains(id),
            "depleted action {id} returned"
        );
    }
    assert!(state.world.npcs[NESSA].inventory.is_empty());
}

#[test]
fn matched_screen_and_repair_jobs_pay_equal_local_wages_but_compete_for_material_and_stamina() {
    let content = content();
    let state = enter(content.new_game("ilyan", 71).unwrap(), &content);
    let shared = act(state, &content, "fume_yards.take_stock");
    let stale_press = select(&shared, &content, "fume_yards.press_repair_plugs", None);
    let repair = act(shared.clone(), &content, "fume_yards.press_repair_plugs");
    let repair = act(repair, &content, "fume_yards.load_freight");
    let screen = act(shared, &content, "fume_yards.pack_catch_screen");
    assert_eq!(owned(&screen, SCREEN), 1);
    assert!(!definitions(&screen, &content).contains("fume_yards.load_freight"));
    assert!(step(&screen, &stale_press, &content, &screen.entropy).is_err());
    let screen = act(screen, &content, "fume_yards.fit_catch_screen");
    assert_eq!(owned(&screen, SCREEN), 0);
    assert_recipe(
        &screen,
        "fume_yards.pack_catch_screen",
        &[(CLAY, 2), (MESH, 1)],
        &[(SCREEN, 1)],
    );
    assert_recipe(&screen, "fume_yards.fit_catch_screen", &[(SCREEN, 1)], &[]);
    let screen = act(screen, &content, "fume_yards.load_screened_freight");
    assert_eq!(
        screen.character.resources["coin"],
        repair.character.resources["coin"]
    );
    assert_eq!(
        screen.character.resources["stamina"],
        repair.character.resources["stamina"] + 2
    );
    assert_eq!(owned(&repair, PLUGS), 1);
    assert_eq!(owned(&screen, PLUGS), 0);
    let screen = deadline_return(screen, &content);
    assert!(!definitions(&screen, &content).contains("return.patch_stand"));
    assert!(!definitions(&screen, &content).contains("return.sort_dry_goods"));
    let screen = act(screen, &content, "return.visit_workshop");
    assert!(
        content
            .observe(&screen)
            .unwrap()
            .text
            .contains("fitted catch screen")
    );
    assert!(!definitions(&screen, &content).contains("fume_yards.load_screened_freight"));
}

#[test]
fn finishing_freight_first_retires_screening_but_keeps_repair_useful() {
    let content = content();
    for take_stock_first in [false, true] {
        let mut state = enter(content.new_game("ilyan", 71).unwrap(), &content);
        if take_stock_first {
            state = act(state, &content, "fume_yards.take_stock");
        }
        state = act(state, &content, "fume_yards.load_freight");
        assert!(
            !content
                .observe(&state)
                .unwrap()
                .text
                .contains("screening saves")
        );
        if !take_stock_first {
            state = act(state, &content, "fume_yards.take_stock");
        }
        let ids = definitions(&state, &content);
        assert!(ids.contains("fume_yards.press_repair_plugs"));
        assert!(!ids.contains("fume_yards.pack_catch_screen"));
        assert!(!ids.contains("fume_yards.load_freight"));
        state = act(state, &content, "fume_yards.press_repair_plugs");
        state = deadline_return(state, &content);
        state = act(state, &content, "return.patch_stand");
        state = act(state, &content, "return.sort_dry_goods");
        assert_eq!(state.character.resources["coin"], 15);
    }
}

#[test]
fn all_64_custom_starts_reach_and_complete_both_useful_cold_choices() {
    let content = content();
    let creation = content.character_creation().unwrap();
    assert_eq!(creation.slots.len(), 6);
    for mask in 0..64 {
        let selection = CharacterSelection {
            name: "Mara Venn".to_owned(),
            choices: creation
                .slots
                .iter()
                .enumerate()
                .map(|(index, slot)| CharacterChoiceSelection {
                    slot_id: slot.id.clone(),
                    choice_id: slot.choices[(mask >> index) & 1].id.clone(),
                })
                .collect(),
        };
        let state = content.new_custom_game(&selection, 71).unwrap();
        let original_coin = state.character.resources["coin"];
        let original_stamina = state.character.resources["stamina"];
        let state = enter(state, &content);
        assert!(definitions(&state, &content).contains("fume_yards.take_stock"));
        assert!(definitions(&state, &content).contains("fume_yards.load_freight"));
        let shared = act(state, &content, "fume_yards.take_stock");
        let repair = act(shared.clone(), &content, "fume_yards.press_repair_plugs");
        let repair = act(repair, &content, "fume_yards.load_freight");
        let repair = deadline_return(repair, &content);
        let repair = act(repair, &content, "return.patch_stand");
        let repair = act(repair, &content, "return.sort_dry_goods");
        assert_eq!(
            repair.character.resources["coin"],
            original_coin + 5,
            "start {mask}"
        );
        assert_eq!(
            repair.character.resources["stamina"],
            original_stamina - 2,
            "start {mask}"
        );
        let screen = act(shared, &content, "fume_yards.pack_catch_screen");
        let screen = act(screen, &content, "fume_yards.fit_catch_screen");
        let screen = act(screen, &content, "fume_yards.load_screened_freight");
        assert_eq!(
            screen.character.resources["coin"],
            original_coin + 2,
            "start {mask}"
        );
        assert_eq!(
            screen.character.resources["stamina"], original_stamina,
            "start {mask}"
        );
    }
}

#[test]
fn crafting_on_the_surge_boundary_does_not_pause_or_replace_the_world_deadline() {
    let content = content();
    let state = enter(content.new_game("ilyan", 71).unwrap(), &content);
    let mut state = act(state, &content, "fume_yards.take_stock");
    while state.world.time < 15 {
        state = act(state, &content, "wait_tide");
    }
    let state = act(state, &content, "fume_yards.press_repair_plugs");
    assert_eq!(state.world.time, 16);
    assert_eq!(state.world.current_location, WORKSHOP);
    assert!(state.world.flags.contains("surge_missed"));
    assert_eq!(owned(&state, PLUGS), 1);
    assert_eq!(state.event_log.iter().filter(|event| matches!(&event.kind, EventKind::ScheduledEventResolved { event_id, applied: true, .. } if event_id == "lowsail.next_surge")).count(), 1);
    let state = act(state, &content, "fume_yards.load_freight");
    let state = act(state, &content, "world.enter_aftermath");
    let state = act(state, &content, "return.patch_stand");
    let state = act(state, &content, "return.sort_dry_goods");
    assert!(state.world.flags.contains("sluice_failure"));
    assert_eq!(state.character.resources["coin"], 15);
}
