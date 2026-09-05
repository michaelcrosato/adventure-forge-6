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
const DARO: &str = "fume_yards.daro_venn";
const BRANN: &str = "fume_yards.brann_coil";
const PERA: &str = "fume_yards.pera_senn";
const NESSA: &str = "fume_yards.nessa_tern";
const FILTER: &str = "fume_yards.filter";
const SHARD: &str = "fume_yards.shard";
const FACT: &str = "fume_yards.rack_cleared";
const RECOVERIES: &[&str] = &[
    "fume_yards.brace_rack",
    "fume_yards.recover_braced_filter",
    "fume_yards.thread_rack_filter",
    "fume_yards.pull_rack_filter",
];
const JOBS: &[&str] = &[
    "fume_yards.load_cold_freight",
    "fume_yards.load_kiln_freight",
    "fume_yards.load_filtered_kiln_freight",
];

fn content() -> CompiledContent {
    parse_and_compile_production(SOURCE).expect("salvage production compiles below 100 words")
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
                "missing {id} {destination:?} at {} turn {}",
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
    assert_eq!(
        (view.time_cost.minimum_ticks, view.time_cost.maximum_ticks),
        (1, 1)
    );
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
    apply(state, content, id, None)
}
fn travel(state: GameState, content: &CompiledContent, destination: &str) -> GameState {
    apply(state, content, "travel_adjacent", Some(destination))
}
fn legal(state: &GameState, content: &CompiledContent) -> BTreeSet<String> {
    enumerate_legal_actions(state, content)
        .unwrap()
        .into_iter()
        .map(|action| action.definition_id)
        .collect()
}
fn absent(state: &GameState, content: &CompiledContent, ids: &[&str]) {
    let actions = legal(state, content);
    for id in ids {
        assert!(
            !actions.contains(*id),
            "unexpected {id} at {}",
            state.world.time
        );
    }
}
fn owned(state: &GameState, item: &str) -> u32 {
    state.character.inventory.get(item).copied().unwrap_or(0)
}
fn flag(state: &GameState, location: &str, flag: &str) -> bool {
    state.world.locations[location].flags.contains(flag)
}
fn front(state: GameState, content: &CompiledContent) -> GameState {
    let state = travel(state, content, "lowsail.levee");
    let state = travel(state, content, WORKSHOP);
    travel(state, content, ASH)
}
fn safe(state: GameState, content: &CompiledContent) -> GameState {
    let state = act(state, content, "fume_yards.brace_rack");
    act(state, content, "fume_yards.recover_braced_filter")
}
fn rear_from_front(state: GameState, content: &CompiledContent) -> GameState {
    let state = travel(state, content, WORKSHOP);
    let state = travel(state, content, BAY);
    act(state, content, "fume_yards.enter_ash_hatch")
}
fn hold(content: &CompiledContent, seed: u64) -> GameState {
    let mut state = content.new_game("ilyan", seed).unwrap();
    for (id, destination) in [
        ("checkpoint.show_charter", None),
        ("travel_adjacent", Some("lowsail.levee")),
        ("levee.authority_path", None),
        ("travel_adjacent", Some("red_sluice.top")),
        ("top.hold_market", None),
        ("world.enter_aftermath", None),
        ("return.count_dry_stalls", None),
    ] {
        state = apply(state, content, id, destination);
    }
    assert_eq!(state.world.time, 7);
    state
}
fn hold_front(content: &CompiledContent, seed: u64) -> GameState {
    let state = act(hold(content, seed), content, "return.visit_workshop");
    travel(state, content, ASH)
}
fn manufactured(content: &CompiledContent, seed: u64) -> GameState {
    let state = act(hold(content, seed), content, "return.visit_workshop");
    let state = act(state, content, "fume_yards.take_stock");
    let state = travel(state, content, BAY);
    let mut state = state;
    for id in [
        "fume_yards.take_cask",
        "fume_yards.take_fuel",
        "fume_yards.prepare_charge",
        "fume_yards.fit_wet_screen",
        "fume_yards.ignite_batch",
        "wait_tide",
        "fume_yards.draw_filter",
    ] {
        state = act(state, content, id);
    }
    assert_eq!(state.world.time, 17);
    state
}
fn custom(content: &CompiledContent, mask: usize) -> GameState {
    let selection = CharacterSelection {
        name: "Mara Venn".to_owned(),
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
    content.new_custom_game(&selection, 71).unwrap()
}

#[test]
fn safe_front_recovery_has_finite_custody_and_credible_escorted_report() {
    let content = content();
    let initial = hold_front(&content, 71);
    assert_eq!(initial.world.npcs[DARO].inventory.len(), 1);
    assert_eq!(initial.world.npcs[DARO].inventory[FILTER], 1);
    absent(
        &initial,
        &content,
        &[
            "fume_yards.leave_ash_hatch",
            "fume_yards.report_with_daro",
            "fume_yards.thread_rack_filter",
        ],
    );
    let recovered = safe(initial, &content);
    assert_eq!(
        (
            recovered.world.time,
            owned(&recovered, FILTER),
            recovered.character.resources["stamina"]
        ),
        (11, 1, 1)
    );
    assert!(recovered.world.npcs[DARO].inventory.is_empty());
    assert!(flag(&recovered, ASH, FACT));
    assert_eq!(
        recovered.world.npcs[DARO].knowledge[FACT].provenance,
        KnowledgeProvenance::Witnessed
    );
    assert_eq!(
        recovered.world.npcs[DARO].memories[FACT].provenance,
        KnowledgeProvenance::Witnessed
    );
    for npc in [BRANN, PERA, NESSA, "oren_pell"] {
        assert!(!recovered.world.npcs[npc].knows(FACT));
    }
    assert!(
        content
            .observe(&recovered)
            .unwrap()
            .text
            .contains("Via Workshop, use Kiln Bay's Ash Hatch")
    );
    absent(&recovered, &content, RECOVERIES);
    absent(&recovered, &content, &["fume_yards.report_with_daro"]);
    let rear = rear_from_front(recovered.clone(), &content);
    let reported = act(rear, &content, "fume_yards.report_with_daro");
    assert_eq!(
        (reported.world.time, reported.character.resources["coin"]),
        (15, 11)
    );
    assert_eq!(reported.world.current_location, BAY);
    assert_eq!(reported.world.npcs[DARO].location, BAY);
    assert!(!reported.world.locations[ASH].entities.contains(DARO));
    assert!(reported.world.locations[BAY].entities.contains(DARO));
    assert_eq!(
        reported.world.npcs[BRANN].knowledge[FACT].provenance,
        KnowledgeProvenance::Told {
            by: DARO.to_owned()
        }
    );
    assert_eq!(
        reported.world.npcs[DARO].knowledge[FACT],
        recovered.world.npcs[DARO].knowledge[FACT]
    );
    for (npc, memory) in [
        (BRANN, "fume_yards.rack_report_paid"),
        (DARO, "fume_yards.rack_reported"),
    ] {
        assert_eq!(
            reported.world.npcs[npc].memories[memory].provenance,
            KnowledgeProvenance::Witnessed
        );
    }
    let revisit = act(reported, &content, "fume_yards.enter_ash_hatch");
    absent(&revisit, &content, RECOVERIES);
    absent(&revisit, &content, &["fume_yards.report_with_daro"]);
    assert_eq!(revisit.character.resources["coin"], 11);
    assert!(
        content
            .observe(&revisit)
            .unwrap()
            .text
            .contains("rack stays empty")
    );
}

#[test]
fn all_64_starts_can_recover_safely_and_choose_local_cold_work_or_export() {
    let content = content();
    for mask in 0..64 {
        let initial = custom(&content, mask);
        let coin = initial.character.resources["coin"];
        let stamina = initial.character.resources["stamina"];
        let recovered = safe(front(initial, &content), &content);
        assert_eq!(recovered.world.time, 5);
        assert_eq!(recovered.character.resources["stamina"], stamina - 2);
        assert_eq!(recovered.entropy.cursor, 0);
        let local = travel(recovered.clone(), &content, WORKSHOP);
        let local = travel(local, &content, BAY);
        let local = act(local, &content, "fume_yards.fit_dust_filter");
        absent(
            &local,
            &content,
            &[
                "fume_yards.take_cask",
                "fume_yards.fit_wet_screen",
                "fume_yards.load_filtered_kiln_freight",
            ],
        );
        assert!(
            content
                .observe(&local)
                .unwrap()
                .text
                .contains("close firing")
        );
        let local = act(local, &content, "fume_yards.load_cold_freight");
        assert_eq!(
            (
                owned(&local, FILTER),
                local.character.resources["coin"],
                local.character.resources["stamina"]
            ),
            (0, coin + 3, stamina - 2),
            "start {mask}"
        );
        assert!(flag(&local, BAY, "fume_yards.kiln_closed"));
        assert!(flag(&local, BAY, "fume_yards.cold_work_chosen"));
        assert_eq!(local.world.npcs[NESSA].inventory["fume_yards.clay"], 2);
        assert_eq!(local.world.npcs[NESSA].inventory["fume_yards.mesh"], 1);
        assert_eq!(local.world.npcs[BRANN].inventory["fume_yards.fuel"], 1);
        assert_eq!(local.world.npcs[PERA].inventory["fume_yards.water_cask"], 1);
        absent(&local, &content, JOBS);
        absent(
            &local,
            &content,
            &[
                "fume_yards.prepare_charge",
                "fume_yards.take_fuel",
                "fume_yards.ignite_batch",
                "fume_yards.fit_dust_filter",
            ],
        );
        let mut sale = recovered;
        while sale.world.time < 16 {
            sale = act(sale, &content, "wait_tide");
        }
        let sale = act(sale, &content, "world.enter_aftermath");
        let sale = act(sale, &content, "return.sell_filter");
        assert_eq!(
            (
                owned(&sale, FILTER),
                sale.character.resources["coin"],
                sale.character.resources["stamina"]
            ),
            (0, coin + 4, stamina - 2),
            "start {mask}"
        );
        assert!(!flag(&sale, BAY, "fume_yards.dust_filter_fitted"));
        absent(&sale, &content, &["return.sell_filter"]);
    }
}

#[test]
fn rear_rope_method_combines_lineage_and_calling_without_entropy_or_consumption() {
    let content = content();
    for (mask, expected) in [(0, false), (1, false), (4, false), (5, true)] {
        let initial = custom(&content, mask);
        let stamina = initial.character.resources["stamina"];
        let front = front(initial, &content);
        assert!(legal(&front, &content).contains("fume_yards.pull_rack_filter"));
        absent(&front, &content, &["fume_yards.thread_rack_filter"]);
        let rear = rear_from_front(front, &content);
        assert_eq!(
            legal(&rear, &content).contains("fume_yards.thread_rack_filter"),
            expected,
            "mask {mask}"
        );
        assert!(legal(&rear, &content).contains("fume_yards.brace_rack"));
        if expected {
            absent(&rear, &content, &["fume_yards.pull_rack_filter"]);
            let recovered = act(rear, &content, "fume_yards.thread_rack_filter");
            assert_eq!(
                (
                    owned(&recovered, FILTER),
                    owned(&recovered, "rope"),
                    recovered.character.resources["stamina"],
                    recovered.entropy.cursor
                ),
                (1, 1, stamina, 0)
            );
            assert!(recovered.world.npcs[DARO].remembers("fume_yards.rack_threaded"));
            absent(&recovered, &content, RECOVERIES);
        }
    }
}

#[test]
fn risky_recovery_has_one_draw_at_the_exact_75_percent_boundary_and_reports_both_results() {
    let content = content();
    for (seed, bucket, filters, shards) in [(27, 74, 1, 0), (123, 75, 0, 1)] {
        let start = hold_front(&content, seed);
        let action = select(&start, &content, "fume_yards.pull_rack_filter", None);
        let transition = step(&start, &action, &content, &start.entropy).unwrap();
        let text = content.observe_after_transition(&transition).unwrap().text;
        let recovered = transition.into_state();
        assert_eq!(
            (
                owned(&recovered, FILTER),
                owned(&recovered, SHARD),
                recovered.character.resources["stamina"]
            ),
            (filters, shards, 3)
        );
        let draws: Vec<_> = recovered
            .event_log
            .iter()
            .filter_map(|event| {
                if let EventKind::RandomDraw { value, .. } = event.kind {
                    Some(value)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(draws.len(), 1);
        assert_eq!(draws[0] % 100, bucket);
        assert_eq!(recovered.entropy.cursor, 1);
        assert!(text.contains(if shards == 1 {
            "The rack filter broke"
        } else {
            "filter comes free intact"
        }));
        assert!(recovered.world.npcs[DARO].inventory.is_empty());
        assert!(!recovered.world.npcs[BRANN].knows(FACT));
        let reported = act(
            rear_from_front(recovered, &content),
            &content,
            "fume_yards.report_with_daro",
        );
        assert_eq!(reported.character.resources["coin"], 11);
        assert_eq!(
            reported.world.npcs[BRANN].knowledge[FACT].provenance,
            KnowledgeProvenance::Told {
                by: DARO.to_owned()
            }
        );
    }
}

#[test]
fn broken_rack_result_does_not_mistake_an_existing_manufactured_filter_for_success() {
    let content = content();
    let made = manufactured(&content, 123);
    assert_eq!(owned(&made, FILTER), 1);
    let rack = act(made, &content, "fume_yards.enter_ash_hatch");
    let pull = select(&rack, &content, "fume_yards.pull_rack_filter", None);
    let transition = step(&rack, &pull, &content, &rack.entropy).unwrap();
    assert!(
        content
            .observe_after_transition(&transition)
            .unwrap()
            .text
            .starts_with("The rack filter broke")
    );
    let broken = transition.into_state();
    assert_eq!(
        (
            broken.world.time,
            owned(&broken, FILTER),
            owned(&broken, SHARD)
        ),
        (19, 1, 1)
    );
    assert_eq!(broken.entropy.cursor, 1);
    assert!(broken.world.npcs[DARO].inventory.is_empty());
    let receipts: Vec<_> = broken
        .event_log
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::RecipeApplied {
                recipe,
                inputs,
                outputs,
            } if recipe == "fume_yards.break_filter" => Some((inputs, outputs)),
            _ => None,
        })
        .collect();
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].0.get(FILTER), Some(&1));
    assert_eq!(receipts[0].1.get(SHARD), Some(&1));
}

#[test]
fn salvage_protection_preserves_cask_and_still_supports_original_manufacture_and_export() {
    let content = content();
    let start = safe(hold_front(&content, 71), &content);
    let state = act(
        rear_from_front(start, &content),
        &content,
        "fume_yards.report_with_daro",
    );
    let state = act(state, &content, "fume_yards.fit_dust_filter");
    let state = act(state, &content, "fume_yards.take_fuel");
    let state = travel(state, &content, WORKSHOP);
    let state = act(state, &content, "fume_yards.take_stock");
    let state = travel(state, &content, BAY);
    let state = act(state, &content, "fume_yards.prepare_charge");
    assert!(
        content
            .observe(&state)
            .unwrap()
            .text
            .contains("fire it or reclaim repair plugs")
    );
    absent(
        &state,
        &content,
        &[
            "fume_yards.load_cold_freight",
            "fume_yards.fit_wet_screen",
            "fume_yards.take_cask",
        ],
    );
    let state = act(state, &content, "fume_yards.ignite_batch");
    assert!(!flag(&state, BAY, "fume_yards.wet_screen_fitted"));
    assert_eq!(state.world.npcs[PERA].inventory["fume_yards.water_cask"], 1);
    assert!(
        content
            .observe(&state)
            .unwrap()
            .text
            .contains("Wait for the filter")
    );
    let state = act(state, &content, "wait_tide");
    assert!(
        content
            .observe(&state)
            .unwrap()
            .text
            .contains("Draw your filter")
    );
    let state = act(state, &content, "fume_yards.draw_filter");
    let state = act(state, &content, "fume_yards.load_filtered_kiln_freight");
    assert_eq!(
        (
            state.character.resources["coin"],
            state.character.resources["stamina"],
            owned(&state, FILTER)
        ),
        (14, 1, 1)
    );
    absent(&state, &content, JOBS);
    let state = act(state, &content, "world.enter_aftermath");
    let state = act(state, &content, "return.sell_filter");
    assert_eq!(
        (
            state.character.resources["coin"],
            state.character.resources["stamina"],
            owned(&state, FILTER)
        ),
        (18, 1, 0)
    );
    assert_eq!(state.world.npcs[PERA].inventory["fume_yards.water_cask"], 1);
    let state = act(state, &content, "return.visit_workshop");
    let state = travel(state, &content, ASH);
    absent(&state, &content, RECOVERIES);
}

#[test]
fn recovery_retires_when_owned_filters_cover_remaining_consumers() {
    let content = content();
    let made = manufactured(&content, 27);
    let two_slots = act(made.clone(), &content, "fume_yards.enter_ash_hatch");
    assert!(legal(&two_slots, &content).contains("fume_yards.pull_rack_filter"));
    let two = act(two_slots, &content, "fume_yards.pull_rack_filter");
    assert_eq!(owned(&two, FILTER), 2);
    absent(&two, &content, RECOVERIES);
    let loaded = act(made, &content, "fume_yards.load_kiln_freight");
    let rack = act(loaded, &content, "fume_yards.enter_ash_hatch");
    assert_eq!(owned(&rack, FILTER), 1);
    assert_eq!(rack.world.npcs[DARO].inventory[FILTER], 1);
    absent(&rack, &content, RECOVERIES);
    assert!(
        content
            .observe(&rack)
            .unwrap()
            .text
            .contains("needs are covered")
    );
    let sale = act(rack, &content, "world.enter_aftermath");
    let sale = act(sale, &content, "return.sell_filter");
    let rack = act(sale, &content, "return.visit_workshop");
    let rack = travel(rack, &content, ASH);
    assert_eq!(owned(&rack, FILTER), 0);
    assert_eq!(rack.world.npcs[DARO].inventory[FILTER], 1);
    absent(&rack, &content, RECOVERIES);
}

#[test]
fn owned_salvage_filter_cannot_install_during_live_or_spoiled_firing() {
    let content = content();
    let recovered = safe(hold_front(&content, 71), &content);
    let state = travel(recovered, &content, WORKSHOP);
    let state = act(state, &content, "fume_yards.take_stock");
    let mut state = travel(state, &content, BAY);
    for id in [
        "fume_yards.take_fuel",
        "fume_yards.take_cask",
        "fume_yards.prepare_charge",
        "fume_yards.fit_wet_screen",
        "fume_yards.ignite_batch",
    ] {
        state = act(state, &content, id);
    }
    absent(&state, &content, &["fume_yards.fit_dust_filter"]);
    for _ in 0..4 {
        state = act(state, &content, "wait_tide");
    }
    assert!(flag(&state, BAY, "fume_yards.freight_spoiled"));
    let text = content.observe(&state).unwrap().text;
    assert!(text.contains("Spoilage ruined Brann's freight commission"));
    assert!(text.contains("sell your surviving filter to Oren"));
    assert!(!text.contains("Fit your filter here"));
    assert_eq!(owned(&state, FILTER), 1);
    absent(&state, &content, &["fume_yards.fit_dust_filter"]);
    absent(&state, &content, JOBS);
    let sale = act(state, &content, "world.enter_aftermath");
    let sale = act(sale, &content, "return.sell_filter");
    assert_eq!(sale.character.resources["coin"], 14);
}

#[test]
fn deadline_at_recovery_and_late_first_entry_preserve_stock_and_shared_aftermath() {
    let content = content();
    let mut state = front(content.new_game("ilyan", 71).unwrap(), &content);
    while state.world.time < 14 {
        state = act(state, &content, "wait_tide");
    }
    let state = safe(state, &content);
    assert_eq!(state.world.time, 16);
    assert_eq!(owned(&state, FILTER), 1);
    assert!(state.world.flags.contains("sluice_outcome_chosen"));
    let returned = act(state, &content, "world.enter_aftermath");
    assert_eq!(returned.world.current_location, "lowsail.return");
    assert_eq!(returned.world.npcs[DARO].location, ASH);
    let mut late = hold(&content, 71);
    while late.world.time < 129 {
        late = act(late, &content, "wait_tide");
    }
    let late = act(late, &content, "return.visit_workshop");
    let late = travel(late, &content, ASH);
    assert_eq!(late.world.npcs[DARO].inventory[FILTER], 1);
    let late = safe(late, &content);
    assert_eq!(
        (late.world.time, owned(&late, FILTER), late.entropy.cursor),
        (133, 1, 0)
    );
    assert!(late.world.npcs[DARO].inventory.is_empty());
}

#[test]
fn installed_salvage_filter_does_not_hide_spoilage_or_offer_lost_freight() {
    let content = content();
    let recovered = safe(hold_front(&content, 71), &content);
    let state = travel(recovered, &content, WORKSHOP);
    let state = act(state, &content, "fume_yards.take_stock");
    let state = travel(state, &content, BAY);
    let mut state = act(state, &content, "fume_yards.fit_dust_filter");
    for id in [
        "fume_yards.take_fuel",
        "fume_yards.prepare_charge",
        "fume_yards.ignite_batch",
        "wait_tide",
        "wait_tide",
        "wait_tide",
        "wait_tide",
    ] {
        state = act(state, &content, id);
    }
    let text = content.observe(&state).unwrap().text;
    assert!(text.contains("dust filter remains installed"));
    assert!(text.contains("three-coin freight commission is lost"));
    assert!(!text.contains("loading kiln freight pays"));
    assert_eq!(state.world.npcs[PERA].inventory["fume_yards.water_cask"], 1);
    assert!(flag(&state, BAY, "fume_yards.dust_filter_fitted"));
    assert!(flag(&state, BAY, "fume_yards.freight_spoiled"));
    absent(&state, &content, JOBS);
    let state = travel(state, &content, WORKSHOP);
    let state = travel(state, &content, BAY);
    assert_eq!(content.observe(&state).unwrap().text, text);
    absent(&state, &content, JOBS);
}

#[test]
fn salvaged_protection_after_cold_crafting_promises_only_the_available_cold_job() {
    let content = content();
    let state = act(hold(&content, 71), &content, "return.visit_workshop");
    let state = act(state, &content, "fume_yards.take_stock");
    let state = act(state, &content, "fume_yards.press_repair_plugs");
    let state = safe(travel(state, &content, ASH), &content);
    let state = travel(state, &content, WORKSHOP);
    let state = travel(state, &content, BAY);
    let action = select(&state, &content, "fume_yards.fit_dust_filter", None);
    let transition = step(&state, &action, &content, &state.entropy).unwrap();
    let text = content.observe_after_transition(&transition).unwrap().text;
    assert!(text.contains("unload for three coins without spending stamina"));
    assert!(!text.contains("preparing"));
    assert!(!text.contains("prepare a batch"));
    let state = transition.into_state();
    absent(
        &state,
        &content,
        &[
            "fume_yards.prepare_charge",
            "fume_yards.take_fuel",
            "fume_yards.take_cask",
        ],
    );
    assert_eq!(owned(&state, "fume_yards.repair_lot"), 1);
    let state = act(state, &content, "fume_yards.load_cold_freight");
    assert_eq!(state.character.resources["coin"], 13);
}

#[test]
fn salvage_sale_and_revisit_extend_every_reviewed_outcome_and_missed_deadline() {
    // Literal reviewed prefixes from the retained cold-pilot replay suite.
    // Each outcome/cast snapshot is taken before the salvage extension.
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
    let content = content();
    let mut deadline: Vec<Spec> = vec![("wait_tide", None); 16];
    deadline.extend([("world.enter_aftermath", None), ("return.face_flood", None)]);
    for (preset, prefix, ending, context) in [
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
    ] {
        let mut state = content.new_game(preset, 71).unwrap();
        for (id, destination) in prefix {
            state = apply(state, &content, id, *destination);
        }
        assert!(state.world.flags.contains(ending));
        let old_flags = state.world.flags.clone();
        let old_return = state.world.locations["lowsail.return"].flags.clone();
        let old_coin = state.character.resources["coin"];
        let old_stamina = state.character.resources["stamina"];
        let old_npcs = state.world.npcs.clone();
        let state = act(state, &content, "return.visit_workshop");
        let state = safe(travel(state, &content, ASH), &content);
        let state = act(state, &content, "world.enter_aftermath");
        let state = act(state, &content, "return.sell_filter");
        let state = act(state, &content, "return.visit_workshop");
        let state = travel(state, &content, ASH);
        absent(&state, &content, RECOVERIES);
        let state = act(state, &content, "world.enter_aftermath");
        assert_eq!(state.world.flags, old_flags);
        assert!(old_return.is_subset(&state.world.locations["lowsail.return"].flags));
        assert_eq!(
            (
                state.character.resources["coin"],
                state.character.resources["stamina"],
                owned(&state, FILTER)
            ),
            (old_coin + 4, old_stamina - 2, 0)
        );
        for (npc, before) in old_npcs {
            assert_eq!(
                state.world.npcs[&npc].location, before.location,
                "cast moved: {npc}"
            );
            if npc != DARO && npc != "oren_pell" {
                assert_eq!(state.world.npcs[&npc], before);
            }
        }
        assert!(content.observe(&state).unwrap().text.contains(context));
        assert!(state.world.npcs["oren_pell"].remembers("fume_yards.filter_bought"));
    }
}
