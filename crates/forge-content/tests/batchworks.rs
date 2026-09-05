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
const BAY: &str = "fume_yards.kiln_bay";
const BRANN: &str = "fume_yards.brann_coil";
const PERA: &str = "fume_yards.pera_senn";
const FUEL: &str = "fume_yards.fuel";
const CASK: &str = "fume_yards.water_cask";
const CHARGE: &str = "fume_yards.prepared_charge";
const CLAIM: &str = "fume_yards.batch_claim";
const FILTER: &str = "fume_yards.filter";
const SPOILED: &str = "fume_yards.spoiled_charge";

fn content() -> CompiledContent {
    parse_and_compile_production(SOURCE).expect("batch workshop production pack compiles")
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

fn supply_bay(state: GameState, content: &CompiledContent) -> GameState {
    let state = enter(state, content);
    let state = act(state, content, "fume_yards.take_stock");
    let state = travel(state, content, BAY);
    let state = act(state, content, "fume_yards.take_cask");
    act(state, content, "fume_yards.take_fuel")
}

fn prepare_and_light(state: GameState, content: &CompiledContent) -> GameState {
    let state = act(state, content, "fume_yards.prepare_charge");
    let state = act(state, content, "fume_yards.fit_wet_screen");
    act(state, content, "fume_yards.ignite_batch")
}

fn draw(state: GameState, content: &CompiledContent) -> GameState {
    let state = act(state, content, "wait_tide");
    act(state, content, "fume_yards.draw_filter")
}

fn assert_absent(state: &GameState, content: &CompiledContent, ids: &[&str]) {
    let legal = definitions(state, content);
    for id in ids {
        assert!(
            !legal.contains(*id),
            "{id} returned at {}",
            state.world.time
        );
    }
}

#[test]
fn finite_custody_and_three_competing_material_choices_have_exact_recipe_receipts() {
    let content = content();
    let initial = content.new_game("ilyan", 71).unwrap();
    assert_eq!(initial.world.npcs[BRANN].location, BAY);
    assert_eq!(initial.world.npcs[PERA].location, BAY);
    assert_eq!(initial.world.npcs[BRANN].inventory[FUEL], 1);
    assert_eq!(initial.world.npcs[PERA].inventory[CASK], 1);
    assert!(
        initial
            .world
            .scheduled_events
            .iter()
            .all(|event| !event.id.starts_with("fume_yards."))
    );
    let supplied = supply_bay(initial, &content);
    for npc in [NESSA, BRANN, PERA] {
        assert!(supplied.world.npcs[npc].inventory.is_empty());
    }
    for (npc, memory) in [
        (BRANN, "fume_yards.fuel_handed_over"),
        (PERA, "fume_yards.cask_handed_over"),
    ] {
        assert_eq!(
            supplied.world.npcs[npc].memories[memory].provenance,
            KnowledgeProvenance::Witnessed
        );
    }
    let state = act(supplied.clone(), &content, "fume_yards.prepare_charge");
    assert_eq!(
        (
            owned(&state, CLAY),
            owned(&state, MESH),
            owned(&state, CHARGE)
        ),
        (0, 0, 1)
    );
    assert_recipe(
        &state,
        "fume_yards.prepare_charge",
        &[(CLAY, 2), (MESH, 1)],
        &[(CHARGE, 1)],
    );
    assert_absent(&state, &content, &["fume_yards.ignite_batch"]);
    let workshop = travel(state.clone(), &content, WORKSHOP);
    assert_absent(
        &workshop,
        &content,
        &[
            "fume_yards.press_repair_plugs",
            "fume_yards.pack_catch_screen",
            "fume_yards.take_stock",
        ],
    );
    let wet = act(state, &content, "fume_yards.fit_wet_screen");
    assert_eq!(owned(&wet, CASK), 0);
    assert_recipe(&wet, "fume_yards.fit_wet_screen", &[(CASK, 1)], &[]);
    let ignition_time = wet.world.time;
    let lit = act(wet, &content, "fume_yards.ignite_batch");
    assert_eq!(
        (owned(&lit, FUEL), owned(&lit, CHARGE), owned(&lit, CLAIM)),
        (0, 0, 1)
    );
    assert_recipe(
        &lit,
        "fume_yards.ignite_batch",
        &[(CHARGE, 1), (FUEL, 1)],
        &[(CLAIM, 1)],
    );
    for (id, delay) in [("fume_yards.batch_ready", 2), ("fume_yards.batch_spoil", 5)] {
        assert_eq!(
            lit.world
                .scheduled_events
                .iter()
                .find(|event| event.id == id)
                .unwrap()
                .due_time,
            ignition_time + delay
        );
    }
    assert_absent(
        &lit,
        &content,
        &[
            "fume_yards.draw_filter",
            "fume_yards.reclaim_charge",
            "fume_yards.prepare_charge",
            "fume_yards.ignite_batch",
        ],
    );
    let made = draw(lit, &content);
    assert_eq!((owned(&made, CLAIM), owned(&made, FILTER)), (0, 1));
    assert_recipe(
        &made,
        "fume_yards.draw_filter",
        &[(CLAIM, 1)],
        &[(FILTER, 1)],
    );
    for cold_choice in [
        "fume_yards.press_repair_plugs",
        "fume_yards.pack_catch_screen",
    ] {
        let cold = travel(supplied.clone(), &content, WORKSHOP);
        let cold = act(cold, &content, cold_choice);
        let cold = travel(cold, &content, BAY);
        assert_absent(&cold, &content, &["fume_yards.prepare_charge"]);
        assert_eq!(owned(&cold, CHARGE), 0);
        let unsupplied = enter(content.new_game("ilyan", 71).unwrap(), &content);
        let unsupplied = act(unsupplied, &content, "fume_yards.take_stock");
        let unsupplied = act(unsupplied, &content, cold_choice);
        let unsupplied = travel(unsupplied, &content, BAY);
        assert_absent(
            &unsupplied,
            &content,
            &[
                "fume_yards.take_fuel",
                "fume_yards.take_cask",
                "fume_yards.fit_wet_screen",
                "fume_yards.prepare_charge",
            ],
        );
        assert_eq!(unsupplied.world.npcs[BRANN].inventory[FUEL], 1);
        assert_eq!(unsupplied.world.npcs[PERA].inventory[CASK], 1);
    }
}

#[test]
fn owned_filter_changes_local_work_cost_or_sells_once_and_depleted_revisits_stay_finished() {
    let content = content();
    let supplied = supply_bay(content.new_game("ilyan", 71).unwrap(), &content);
    let made = draw(prepare_and_light(supplied, &content), &content);
    let guidance = content.observe(&made).unwrap().text;
    assert!(guidance.contains("two stamina") && guidance.contains("four coins"));
    let exported = deadline_return(made.clone(), &content);
    let exported = act(exported, &content, "return.sell_filter");
    let exported = act(exported, &content, "return.visit_workshop");
    let exported = travel(exported, &content, BAY);
    let exported_text = content.observe(&exported).unwrap().text;
    assert!(exported_text.contains("filter left the kiln"));
    assert!(exported_text.contains("three coins") && exported_text.contains("two stamina"));
    assert!(!exported_text.contains("Two clay"));
    assert!(definitions(&exported, &content).contains("fume_yards.load_kiln_freight"));
    assert_absent(
        &exported,
        &content,
        &["fume_yards.prepare_charge", "fume_yards.fit_dust_filter"],
    );
    let exported_loaded = act(exported, &content, "fume_yards.load_kiln_freight");
    let exported_loaded = travel(exported_loaded, &content, WORKSHOP);
    let exported_loaded = travel(exported_loaded, &content, BAY);
    assert!(
        content
            .observe(&exported_loaded)
            .unwrap()
            .text
            .contains("freight is finished")
    );
    assert_absent(
        &exported_loaded,
        &content,
        &[
            "fume_yards.load_kiln_freight",
            "fume_yards.load_filtered_kiln_freight",
        ],
    );
    let stale_fit = select(&made, &content, "fume_yards.fit_dust_filter", None);
    let bare = act(made.clone(), &content, "fume_yards.load_kiln_freight");
    assert_eq!(
        (
            bare.character.resources["coin"],
            bare.character.resources["stamina"],
            owned(&bare, FILTER)
        ),
        (13, 1, 1)
    );
    assert_absent(
        &bare,
        &content,
        &[
            "fume_yards.fit_dust_filter",
            "fume_yards.load_kiln_freight",
            "fume_yards.load_filtered_kiln_freight",
        ],
    );
    assert!(step(&bare, &stale_fit, &content, &bare.entropy).is_err());
    assert!(
        content
            .observe(&bare)
            .unwrap()
            .text
            .contains("sell your remaining filter")
    );
    let fitted = act(made, &content, "fume_yards.fit_dust_filter");
    assert_eq!(owned(&fitted, FILTER), 0);
    assert_recipe(&fitted, "fume_yards.fit_dust_filter", &[(FILTER, 1)], &[]);
    let loaded = act(fitted, &content, "fume_yards.load_filtered_kiln_freight");
    assert_eq!(
        (
            loaded.character.resources["coin"],
            loaded.character.resources["stamina"]
        ),
        (13, 3)
    );
    assert_eq!(
        loaded.world.npcs[BRANN].memories["fume_yards.kiln_freight_paid"].provenance,
        KnowledgeProvenance::Witnessed
    );
    let loaded = deadline_return(loaded, &content);
    assert_absent(&loaded, &content, &["return.sell_filter"]);
    let loaded = act(loaded, &content, "return.visit_workshop");
    let loaded = travel(loaded, &content, BAY);
    assert!(
        content
            .observe(&loaded)
            .unwrap()
            .text
            .contains("installed dust filter remains")
    );
    assert_absent(
        &loaded,
        &content,
        &[
            "fume_yards.take_cask",
            "fume_yards.take_fuel",
            "fume_yards.prepare_charge",
            "fume_yards.draw_filter",
            "fume_yards.fit_dust_filter",
            "fume_yards.load_kiln_freight",
            "fume_yards.load_filtered_kiln_freight",
        ],
    );
    let sale = deadline_return(bare, &content);
    assert!(!sale.world.npcs["oren_pell"].remembers("fume_yards.filter_bought"));
    let sale = act(sale, &content, "return.sell_filter");
    assert_eq!(
        (sale.character.resources["coin"], owned(&sale, FILTER)),
        (17, 0)
    );
    assert_recipe(&sale, "fume_yards.sell_filter", &[(FILTER, 1)], &[]);
    assert_eq!(
        sale.world.npcs["oren_pell"].memories["fume_yards.filter_bought"].provenance,
        KnowledgeProvenance::Witnessed
    );
    assert_absent(&sale, &content, &["return.sell_filter"]);
    assert!(!sale.world.npcs[BRANN].remembers("fume_yards.filter_bought"));
}

#[test]
fn reclaim_before_ignition_preserves_fuel_and_uses_existing_repair_work() {
    let content = content();
    let supplied = supply_bay(content.new_game("ilyan", 71).unwrap(), &content);
    let prepared = act(supplied, &content, "fume_yards.prepare_charge");
    let reclaimed = act(prepared, &content, "fume_yards.reclaim_charge");
    assert_eq!(
        (
            owned(&reclaimed, CHARGE),
            owned(&reclaimed, PLUGS),
            owned(&reclaimed, FUEL),
            owned(&reclaimed, CASK)
        ),
        (0, 1, 1, 1)
    );
    assert_recipe(
        &reclaimed,
        "fume_yards.reclaim_charge",
        &[(CHARGE, 1)],
        &[(PLUGS, 1)],
    );
    assert_absent(
        &reclaimed,
        &content,
        &[
            "fume_yards.prepare_charge",
            "fume_yards.ignite_batch",
            "fume_yards.reclaim_charge",
            "fume_yards.fit_wet_screen",
        ],
    );
    assert!(
        reclaimed
            .world
            .scheduled_events
            .iter()
            .all(|event| !event.id.starts_with("fume_yards."))
    );
    let loaded = act(reclaimed.clone(), &content, "fume_yards.load_kiln_freight");
    assert!(
        content
            .observe(&loaded)
            .unwrap()
            .text
            .contains("Patch Stand")
    );
    let repaired = deadline_return(reclaimed, &content);
    let repaired = act(repaired, &content, "return.patch_stand");
    let repaired = act(repaired, &content, "return.sort_dry_goods");
    assert_eq!(
        (
            owned(&repaired, PLUGS),
            repaired.character.resources["coin"]
        ),
        (0, 13)
    );
    assert_eq!((owned(&repaired, FILTER), owned(&repaired, SCREEN)), (0, 0));
}

#[test]
fn spoilage_consumes_the_remote_claim_without_inventing_npc_knowledge() {
    let content = content();
    let supplied = supply_bay(content.new_game("ilyan", 71).unwrap(), &content);
    let lit = prepare_and_light(supplied, &content);
    let due = lit.world.time + 4;
    let mut away = travel(lit, &content, WORKSHOP);
    away = travel(away, &content, "lowsail.levee");
    away = travel(away, &content, "lowsail_market");
    away = travel(away, &content, "lowsail.docks");
    assert_eq!(away.world.time, due);
    assert_eq!(
        (
            owned(&away, CLAIM),
            owned(&away, FILTER),
            owned(&away, SPOILED)
        ),
        (0, 0, 1)
    );
    assert_recipe(
        &away,
        "fume_yards.spoil_batch",
        &[(CLAIM, 1)],
        &[(SPOILED, 1)],
    );
    for npc in [BRANN, PERA, NESSA, "oren_pell"] {
        assert!(!away.world.npcs[npc].knows("fume_yards.batch_spoiled"));
        assert!(!away.world.npcs[npc].remembers("fume_yards.spoil_inspected"));
    }
    let returned = travel(away, &content, "lowsail.levee");
    let returned = travel(returned, &content, WORKSHOP);
    let returned = travel(returned, &content, BAY);
    assert_absent(
        &returned,
        &content,
        &[
            "fume_yards.draw_filter",
            "fume_yards.reclaim_charge",
            "fume_yards.bank_kiln",
            "fume_yards.load_kiln_freight",
            "fume_yards.load_filtered_kiln_freight",
        ],
    );
    let observed = act(returned, &content, "fume_yards.inspect_spoiled_batch");
    assert_eq!(
        observed.world.npcs[BRANN].knowledge["fume_yards.batch_spoiled"].provenance,
        KnowledgeProvenance::Witnessed
    );
    assert_absent(&observed, &content, &["fume_yards.inspect_spoiled_batch"]);
}

#[test]
fn every_custom_start_can_manufacture_and_choose_either_useful_filter_destination() {
    let content = content();
    let creation = content.character_creation().unwrap();
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
        let initial = content.new_custom_game(&selection, 71).unwrap();
        let coin = initial.character.resources["coin"];
        let stamina = initial.character.resources["stamina"];
        let supplied = supply_bay(initial, &content);
        let made = draw(prepare_and_light(supplied, &content), &content);
        let fitted = act(made.clone(), &content, "fume_yards.fit_dust_filter");
        let local = act(fitted, &content, "fume_yards.load_filtered_kiln_freight");
        assert_eq!(
            (
                local.character.resources["coin"],
                local.character.resources["stamina"],
                owned(&local, FILTER)
            ),
            (coin + 3, stamina, 0),
            "start {mask}"
        );
        let sale = deadline_return(made, &content);
        let sale = act(sale, &content, "return.sell_filter");
        assert_eq!(
            (
                sale.character.resources["coin"],
                sale.character.resources["stamina"],
                owned(&sale, FILTER)
            ),
            (coin + 4, stamina, 0),
            "start {mask}"
        );
    }
}
