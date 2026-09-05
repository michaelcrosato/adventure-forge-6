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
const FACT: &str = "fume_yards.market_cask";
const CAGE: &str = "fume_yards.collateral_cage";
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
    parse_and_compile_production(SOURCE).expect("market water production compiles below 100 words")
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

fn cage(state: &GameState, item: &str) -> u32 {
    state.world.storages[CAGE]
        .inventory
        .get(item)
        .copied()
        .unwrap_or(0)
}
fn full_catalog(state: &GameState, content: &CompiledContent) {
    let expected: Vec<_> = enumerate_legal_actions(state, content)
        .unwrap()
        .into_iter()
        .map(|a| a.action_id)
        .collect();
    let mut actual = Vec::new();
    let mut offset = 0;
    loop {
        let page = content.action_page(state, offset, 7).unwrap();
        assert_eq!(page.total, expected.len());
        actual.extend(page.actions.into_iter().map(|a| a.action_id));
        if let Some(next) = page.next_offset {
            offset = next;
        } else {
            break;
        }
    }
    assert_eq!(actual, expected);
}
fn finish_water(mut state: GameState, content: &CompiledContent) -> GameState {
    for id in [
        "fume_yards.take_market_cask",
        "fume_yards.escort_market_cask",
        "return.fit_market_filter",
        "return.install_market_cask",
        "return.draw_clean_water",
    ] {
        state = act(state, content, id);
    }
    state
}
fn water_from_return(state: GameState, content: &CompiledContent) -> GameState {
    let state = act(state, content, "return.visit_workshop");
    let state = act(state, content, "fume_yards.take_stock");
    let state = act(state, content, "fume_yards.press_repair_plugs");
    let state = act(state, content, "world.enter_aftermath");
    let state = act(state, content, "return.patch_stand");
    let state = act(state, content, "return.order_water_stand");
    let state = act(state, content, "return.visit_workshop");
    let state = travel(state, content, ASH);
    let state = act(state, content, "fume_yards.buy_collateral_filter");
    let state = travel(state, content, WORKSHOP);
    let state = travel(state, content, BAY);
    finish_water(state, content)
}

#[test]
fn purchase_depletes_separate_cage_and_preserves_rack_with_exact_coin_boundary() {
    let content = content();
    let start = hold_front(&content, 71);
    assert_eq!(cage(&start, FILTER), 1);
    assert_eq!(start.world.npcs[DARO].inventory[FILTER], 1);
    assert!(!start.world.locations[ASH].entities.contains(CAGE));
    let action = select(&start, &content, "fume_yards.buy_collateral_filter", None);
    let transition = step(&start, &action, &content, &start.entropy).unwrap();
    assert!(transition.events().iter().any(|event| matches!(&event.kind, EventKind::StorageItemTransferredToCharacter { storage, item, count } if storage == CAGE && item == FILTER && *count == 1)));
    let bought = transition.into_state();
    assert_eq!(
        (
            bought.world.time,
            bought.character.resources["coin"],
            bought.character.resources["stamina"],
            owned(&bought, FILTER),
            cage(&bought, FILTER)
        ),
        (10, 6, 3, 1, 0)
    );
    assert!(bought.world.storages[CAGE].inventory.is_empty());
    assert_eq!(bought.world.npcs[DARO].inventory[FILTER], 1);
    assert_eq!(
        bought.world.npcs[DARO].memories["fume_yards.collateral_paid"].provenance,
        KnowledgeProvenance::Witnessed
    );
    absent(
        &bought,
        &content,
        &[
            "fume_yards.buy_collateral_filter",
            "fume_yards.settle_collateral_fuel",
            "fume_yards.read_collateral_docket",
        ],
    );
    full_catalog(&bought, &content);
    assert!(step(&bought, &action, &content, &bought.entropy).is_err());
    let visit = travel(bought, &content, WORKSHOP);
    let visit = travel(visit, &content, ASH);
    assert_eq!(cage(&visit, FILTER), 0);

    // Two real routes isolate exact cash thresholds without fabricated state.
    let state = travel(
        content.new_game("rook", 71).unwrap(),
        &content,
        "lowsail.docks",
    );
    let state = act(state, &content, "docks.rig_towline");
    let state = travel(state, &content, WORKSHOP);
    let four = act(state.clone(), &content, "fume_yards.load_freight");
    let four = travel(four, &content, ASH);
    assert_eq!(four.character.resources["coin"], 4);
    let paid = act(four, &content, "fume_yards.buy_collateral_filter");
    assert_eq!(paid.character.resources["coin"], 0);
    let three = safe(travel(state, &content, ASH), &content);
    let three = act(
        rear_from_front(three, &content),
        &content,
        "fume_yards.report_with_daro",
    );
    let three = act(three, &content, "fume_yards.return_to_cage");
    assert_eq!(
        (
            three.character.resources["coin"],
            owned(&three, FILTER),
            cage(&three, FILTER)
        ),
        (3, 1, 1)
    );
    absent(&three, &content, &["fume_yards.buy_collateral_filter"]);
    assert_eq!(three.world.npcs[DARO].location, ASH);
    assert!(three.world.npcs[DARO].inventory.is_empty());
}

#[test]
fn fuel_settlement_requires_calling_and_read_docket_and_moves_the_exact_owned_lot() {
    let content = content();
    for (mask, clerk) in [(0, true), (4, false)] {
        let state = travel(custom(&content, mask), &content, "lowsail.levee");
        let state = travel(state, &content, WORKSHOP);
        let state = travel(state, &content, BAY);
        let state = act(state, &content, "fume_yards.take_cask");
        let state = act(state, &content, "fume_yards.take_fuel");
        let state = act(state, &content, "fume_yards.enter_ash_hatch");
        absent(&state, &content, &["fume_yards.settle_collateral_fuel"]);
        assert!(!state.world.npcs[DARO].knows("fume_yards.collateral_terms"));
        let read = act(state, &content, "fume_yards.read_collateral_docket");
        assert_eq!(
            read.world.npcs[DARO].knowledge["fume_yards.collateral_terms"].provenance,
            KnowledgeProvenance::Read {
                source: "fume_yards.collateral_docket".to_owned()
            }
        );
        assert_eq!(
            legal(&read, &content).contains("fume_yards.settle_collateral_fuel"),
            clerk
        );
        full_catalog(&read, &content);
        if clerk {
            let coin = read.character.resources["coin"];
            let action = select(&read, &content, "fume_yards.settle_collateral_fuel", None);
            let transition = step(&read, &action, &content, &read.entropy).unwrap();
            let transfers: Vec<_> = transition
                .events()
                .iter()
                .filter(|event| {
                    matches!(
                        event.kind,
                        EventKind::CharacterItemTransferredToStorage { .. }
                            | EventKind::StorageItemTransferredToCharacter { .. }
                    )
                })
                .collect();
            assert_eq!(transfers.len(), 2);
            assert!(
                matches!(&transfers[0].kind, EventKind::CharacterItemTransferredToStorage { storage, item, count } if storage == CAGE && item == "fume_yards.fuel" && *count == 1)
            );
            assert!(
                matches!(&transfers[1].kind, EventKind::StorageItemTransferredToCharacter { storage, item, count } if storage == CAGE && item == FILTER && *count == 1)
            );
            let settled = transition.into_state();
            assert_eq!(
                (
                    settled.character.resources["coin"],
                    owned(&settled, "fume_yards.fuel"),
                    owned(&settled, FILTER),
                    cage(&settled, "fume_yards.fuel"),
                    cage(&settled, FILTER)
                ),
                (coin, 0, 1, 1, 0)
            );
            assert!(settled.world.npcs[BRANN].inventory.is_empty());
            assert_eq!(settled.world.npcs[DARO].inventory[FILTER], 1);
            absent(
                &settled,
                &content,
                &[
                    "fume_yards.settle_collateral_fuel",
                    "fume_yards.buy_collateral_filter",
                ],
            );
            let bay = act(settled, &content, "fume_yards.leave_ash_hatch");
            assert_eq!(owned(&bay, "fume_yards.water_cask"), 1);
            let fitted = act(bay.clone(), &content, "fume_yards.fit_dust_filter");
            assert!(
                content
                    .observe(&fitted)
                    .unwrap()
                    .text
                    .contains("cold loading")
            );
            assert!(
                !content
                    .observe(&fitted)
                    .unwrap()
                    .text
                    .contains("prepare a batch")
            );
            absent(
                &bay,
                &content,
                &[
                    "fume_yards.take_fuel",
                    "fume_yards.ignite_batch",
                    "fume_yards.take_cask",
                    "fume_yards.fit_wet_screen",
                    "fume_yards.prepare_charge",
                ],
            );
        }
    }
}

#[test]
fn old_report_can_be_followed_by_actual_daro_return_and_trade() {
    let content = content();
    let state = safe(hold_front(&content, 71), &content);
    let state = act(
        rear_from_front(state, &content),
        &content,
        "fume_yards.report_with_daro",
    );
    let original = state.world.npcs[BRANN].knowledge["fume_yards.rack_cleared"].clone();
    let away = act(state, &content, "fume_yards.enter_ash_hatch");
    absent(
        &away,
        &content,
        &[
            "fume_yards.buy_collateral_filter",
            "fume_yards.read_collateral_docket",
        ],
    );
    let bay = act(away, &content, "fume_yards.leave_ash_hatch");
    let back = act(bay, &content, "fume_yards.return_to_cage");
    assert_eq!(back.world.npcs[DARO].location, ASH);
    assert!(back.world.locations[ASH].entities.contains(DARO));
    assert!(!back.world.locations[BAY].entities.contains(DARO));
    assert!(
        content
            .observe(&back)
            .unwrap()
            .text
            .contains("Daro has returned to his cage")
    );
    assert!(back.world.npcs[DARO].inventory.is_empty());
    let bought = act(back, &content, "fume_yards.buy_collateral_filter");
    assert_eq!(owned(&bought, FILTER), 2);
    assert_eq!(
        bought.world.npcs[BRANN].knowledge["fume_yards.rack_cleared"],
        original
    );
    assert_eq!(bought.character.resources["coin"], 7);
}

#[test]
fn every_custom_start_can_compose_safe_local_work_repair_purchase_and_one_water_ration() {
    let content = content();
    for mask in 0..64 {
        let initial = custom(&content, mask);
        let coin = initial.character.resources["coin"];
        let stamina = initial.character.resources["stamina"];
        let state = travel(initial, &content, "lowsail.levee");
        let state = travel(state, &content, WORKSHOP);
        let state = act(state, &content, "fume_yards.take_stock");
        let state = act(state, &content, "fume_yards.press_repair_plugs");
        let state = safe(travel(state, &content, ASH), &content);
        let state = travel(state, &content, WORKSHOP);
        let state = travel(state, &content, BAY);
        let state = act(state, &content, "fume_yards.fit_dust_filter");
        let mut state = act(state, &content, "fume_yards.load_cold_freight");
        assert_eq!(state.world.time, 11);
        assert_eq!(state.world.npcs[PERA].inventory["fume_yards.water_cask"], 1);
        while state.world.time < 16 {
            state = act(state, &content, "wait_tide");
        }
        let state = act(state, &content, "world.enter_aftermath");
        let state = act(state, &content, "return.patch_stand");
        let state = act(state, &content, "return.order_water_stand");
        let state = act(state, &content, "return.visit_workshop");
        let state = travel(state, &content, ASH);
        let state = act(state, &content, "fume_yards.buy_collateral_filter");
        let state = travel(state, &content, WORKSHOP);
        let state = travel(state, &content, BAY);
        absent(
            &state,
            &content,
            &["fume_yards.take_cask", "fume_yards.fit_wet_screen"],
        );
        let state = finish_water(state, &content);
        assert_eq!(state.world.time, 29);
        assert_eq!(
            (
                state.character.resources["coin"],
                state.character.resources["stamina"]
            ),
            (coin - 1, stamina),
            "start {mask}"
        );
        for item in [
            FILTER,
            "fume_yards.clay",
            "fume_yards.mesh",
            "fume_yards.repair_lot",
            "fume_yards.water_cask",
        ] {
            assert_eq!(owned(&state, item), 0);
        }
        for npc in [NESSA, DARO, PERA] {
            assert!(state.world.npcs[npc].inventory.is_empty());
        }
        assert_eq!(state.world.npcs[BRANN].inventory["fume_yards.fuel"], 1);
        assert!(state.world.storages[CAGE].inventory.is_empty());
        assert!(flag(&state, BAY, "fume_yards.dust_filter_fitted"));
        assert!(flag(
            &state,
            "lowsail.return",
            "fume_yards.market_filter_fitted"
        ));
        assert!(flag(
            &state,
            "lowsail.return",
            "fume_yards.market_cask_installed"
        ));
        absent(
            &state,
            &content,
            &[
                "return.draw_clean_water",
                "return.install_market_cask",
                "return.fit_market_filter",
                "return.order_water_stand",
            ],
        );
        let state = act(state, &content, "return.visit_workshop");
        let state = travel(state, &content, BAY);
        assert!(
            content
                .observe(&state)
                .unwrap()
                .text
                .contains("Pera has reached Lowsail")
        );
        absent(&state, &content, JOBS);
        absent(
            &state,
            &content,
            &[
                "fume_yards.take_market_cask",
                "fume_yards.escort_market_cask",
            ],
        );
    }
}

#[test]
fn escort_binds_owned_cask_witness_then_movement_then_told_provenance_without_remote_leak() {
    let content = content();
    let state = act(hold(&content, 71), &content, "return.visit_workshop");
    let state = act(state, &content, "fume_yards.take_stock");
    let state = act(state, &content, "fume_yards.press_repair_plugs");
    let state = act(state, &content, "world.enter_aftermath");
    let state = act(state, &content, "return.patch_stand");
    let state = act(state, &content, "return.order_water_stand");
    let state = act(state, &content, "return.visit_workshop");
    let state = travel(state, &content, ASH);
    let state = act(state, &content, "fume_yards.buy_collateral_filter");
    let state = travel(state, &content, WORKSHOP);
    let state = travel(state, &content, BAY);
    let loaded = act(state, &content, "fume_yards.take_market_cask");
    assert_eq!(owned(&loaded, "fume_yards.water_cask"), 1);
    assert!(loaded.world.npcs[PERA].inventory.is_empty());
    assert!(!loaded.world.npcs[PERA].knows(FACT));
    assert!(!loaded.world.npcs["oren_pell"].knows(FACT));
    let action = select(&loaded, &content, "fume_yards.escort_market_cask", None);
    let transition = step(&loaded, &action, &content, &loaded.entropy).unwrap();
    let events = transition.events();
    let source = events.iter().position(|e| matches!(&e.kind, EventKind::NpcKnowledgeAdded { npc, knowledge } if npc == PERA && knowledge == FACT)).unwrap();
    let movement = events
        .iter()
        .position(|e| matches!(&e.kind, EventKind::NpcMoved { npc, .. } if npc == PERA))
        .unwrap();
    let told = events.iter().position(|e| matches!(&e.kind, EventKind::NpcKnowledgeAdded { npc, knowledge } if npc == "oren_pell" && knowledge == FACT)).unwrap();
    assert!(source < movement && movement < told);
    let arrived = transition.into_state();
    assert_eq!(owned(&arrived, "fume_yards.water_cask"), 1);
    assert_eq!(arrived.world.current_location, "lowsail.return");
    assert_eq!(arrived.world.npcs[PERA].location, "lowsail.return");
    assert!(!arrived.world.locations[BAY].entities.contains(PERA));
    assert!(
        arrived.world.locations["lowsail.return"]
            .entities
            .contains(PERA)
    );
    assert_eq!(
        arrived.world.npcs[PERA].knowledge[FACT].turn,
        loaded.world.time
    );
    assert_eq!(
        arrived.world.npcs["oren_pell"].knowledge[FACT].turn,
        loaded.world.time
    );
    assert_eq!(
        arrived.world.npcs[PERA].knowledge[FACT].provenance,
        KnowledgeProvenance::Witnessed
    );
    assert_eq!(
        arrived.world.npcs["oren_pell"].knowledge[FACT].provenance,
        KnowledgeProvenance::Told {
            by: PERA.to_owned()
        }
    );
    for npc in [DARO, NESSA, BRANN] {
        assert!(!arrived.world.npcs[npc].knows(FACT));
    }
    full_catalog(&arrived, &content);
    for cask_first in [false, true] {
        let (first, second) = if cask_first {
            ("return.install_market_cask", "return.fit_market_filter")
        } else {
            ("return.fit_market_filter", "return.install_market_cask")
        };
        let half = act(arrived.clone(), &content, first);
        absent(&half, &content, &["return.draw_clean_water"]);
        let ready = act(half, &content, second);
        assert_eq!(
            (
                owned(&ready, FILTER),
                owned(&ready, "fume_yards.water_cask")
            ),
            (0, 0)
        );
        assert!(
            content
                .observe(&ready)
                .unwrap()
                .text
                .contains("Draw clean water for two stamina")
        );
        let water = act(ready, &content, "return.draw_clean_water");
        assert_eq!(water.character.resources["stamina"], 5);
        assert_eq!(
            water.world.npcs[PERA].knowledge[FACT],
            arrived.world.npcs[PERA].knowledge[FACT]
        );
        assert!(
            content
                .observe(&water)
                .unwrap()
                .text
                .contains("stored ration is spent")
        );
        absent(&water, &content, &["return.draw_clean_water"]);
        full_catalog(&water, &content);
    }
}

#[test]
fn an_unused_cask_from_the_old_handover_can_be_escorted_without_a_second_transfer() {
    let content = content();
    let state = act(hold(&content, 71), &content, "return.visit_workshop");
    let state = travel(state, &content, BAY);
    let state = act(state, &content, "fume_yards.take_cask");
    let original = state.world.npcs[PERA].memories["fume_yards.cask_handed_over"].clone();
    let state = travel(state, &content, WORKSHOP);
    let state = act(state, &content, "fume_yards.take_stock");
    let state = act(state, &content, "fume_yards.press_repair_plugs");
    let state = act(state, &content, "world.enter_aftermath");
    let state = act(state, &content, "return.patch_stand");
    let state = act(state, &content, "return.order_water_stand");
    let state = act(state, &content, "return.visit_workshop");
    let state = travel(state, &content, ASH);
    let state = act(state, &content, "fume_yards.buy_collateral_filter");
    let state = travel(state, &content, WORKSHOP);
    let state = travel(state, &content, BAY);
    absent(
        &state,
        &content,
        &["fume_yards.take_market_cask", "fume_yards.take_cask"],
    );
    let state = act(state, &content, "fume_yards.escort_market_cask");
    assert_eq!(owned(&state, "fume_yards.water_cask"), 1);
    assert_eq!(
        state.world.npcs[PERA].memories["fume_yards.cask_handed_over"],
        original
    );
    assert!(!state.world.npcs[PERA].remembers("fume_yards.market_cask_handed_over"));
}

#[test]
fn market_order_widens_recovery_only_after_real_repair_and_retains_capacity_limits() {
    let content = content();
    let state = act(hold(&content, 27), &content, "return.visit_workshop");
    let state = act(state, &content, "fume_yards.take_stock");
    let state = travel(state, &content, BAY);
    let state = act(state, &content, "fume_yards.prepare_charge");
    let state = act(state, &content, "fume_yards.reclaim_charge");
    let state = act(state, &content, "fume_yards.load_kiln_freight");
    let state = act(state, &content, "fume_yards.enter_ash_hatch");
    let before = act(state, &content, "fume_yards.buy_collateral_filter");
    assert_eq!(owned(&before, FILTER), 1);
    absent(&before, &content, RECOVERIES);
    let state = act(before, &content, "world.enter_aftermath");
    absent(&state, &content, &["return.order_water_stand"]);
    let state = act(state, &content, "return.patch_stand");
    let state = act(state, &content, "return.order_water_stand");
    let state = act(state, &content, "return.visit_workshop");
    let state = travel(state, &content, ASH);
    assert!(legal(&state, &content).contains("fume_yards.pull_rack_filter"));
    let state = act(state, &content, "fume_yards.pull_rack_filter");
    assert_eq!(owned(&state, FILTER), 2);
    absent(&state, &content, RECOVERIES);
    let state = act(state, &content, "world.enter_aftermath");
    let state = act(state, &content, "return.fit_market_filter");
    let state = act(state, &content, "return.sell_filter");
    assert_eq!(owned(&state, FILTER), 0);
    assert!(flag(
        &state,
        "lowsail.return",
        "fume_yards.market_filter_fitted"
    ));
    assert!(flag(&state, "lowsail.return", "fume_yards.filter_sold"));
}

#[test]
fn spent_cask_or_no_attainable_filter_prevents_an_empty_water_order() {
    let content = content();
    let state = act(hold(&content, 71), &content, "return.visit_workshop");
    let state = act(state, &content, "fume_yards.take_stock");
    let state = travel(state, &content, BAY);
    let state = act(state, &content, "fume_yards.take_cask");
    let state = act(state, &content, "fume_yards.fit_wet_screen");
    let state = act(state, &content, "fume_yards.prepare_charge");
    let state = act(state, &content, "fume_yards.reclaim_charge");
    let state = act(state, &content, "world.enter_aftermath");
    let state = act(state, &content, "return.patch_stand");
    assert_eq!(owned(&state, "fume_yards.water_cask"), 0);
    assert!(state.world.npcs[PERA].inventory.is_empty());
    absent(
        &state,
        &content,
        &["return.order_water_stand", "return.fit_market_filter"],
    );

    let state = act(hold(&content, 123), &content, "return.visit_workshop");
    let state = act(state, &content, "fume_yards.take_stock");
    let state = act(state, &content, "fume_yards.press_repair_plugs");
    let state = travel(state, &content, ASH);
    let state = act(state, &content, "fume_yards.buy_collateral_filter");
    let state = travel(state, &content, WORKSHOP);
    let state = travel(state, &content, BAY);
    let state = act(state, &content, "fume_yards.fit_dust_filter");
    let state = act(state, &content, "fume_yards.load_cold_freight");
    let state = act(state, &content, "fume_yards.enter_ash_hatch");
    let state = act(state, &content, "fume_yards.pull_rack_filter");
    assert_eq!(owned(&state, SHARD), 1);
    let state = act(state, &content, "world.enter_aftermath");
    let state = act(state, &content, "return.patch_stand");
    assert_eq!(owned(&state, FILTER), 0);
    assert_eq!(state.world.npcs[PERA].inventory["fume_yards.water_cask"], 1);
    absent(&state, &content, &["return.order_water_stand"]);
}

#[test]
fn market_water_extends_every_reviewed_outcome_and_missed_deadline() {
    // Literal reviewed prefixes from the retained cold-pilot replay suite.
    // Each outcome/cast snapshot is taken before the water extension.
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
        let state = water_from_return(state, &content);
        assert_eq!(state.world.flags, old_flags);
        assert!(old_return.is_subset(&state.world.locations["lowsail.return"].flags));
        assert_eq!(
            (
                state.character.resources["coin"],
                state.character.resources["stamina"],
                owned(&state, FILTER)
            ),
            (old_coin - 4, old_stamina + 2, 0)
        );
        for (npc, before) in old_npcs {
            if npc == PERA {
                assert_eq!(state.world.npcs[&npc].location, "lowsail.return");
            } else {
                assert_eq!(state.world.npcs[&npc].location, before.location);
            }
            if ![NESSA, DARO, PERA, "oren_pell"].contains(&npc.as_str()) {
                assert_eq!(state.world.npcs[&npc], before);
            }
        }
        assert!(content.observe(&state).unwrap().text.contains(context));
        assert!(state.world.npcs["oren_pell"].remembers("fume_yards.clean_water_supplied"));
    }
}

#[test]
fn fuel_settlement_preserves_reclamation_of_a_legitimate_prepared_charge() {
    let content = content();
    let state = act(hold(&content, 71), &content, "return.visit_workshop");
    let state = act(state, &content, "fume_yards.take_stock");
    let state = travel(state, &content, BAY);
    let state = act(state, &content, "fume_yards.take_fuel");
    let state = act(state, &content, "fume_yards.prepare_charge");
    let state = act(state, &content, "fume_yards.enter_ash_hatch");
    let state = act(state, &content, "fume_yards.read_collateral_docket");
    let state = act(state, &content, "fume_yards.settle_collateral_fuel");
    let state = act(state, &content, "fume_yards.leave_ash_hatch");
    assert!(
        content
            .observe(&state)
            .unwrap()
            .text
            .contains("reclaim the unfired charge")
    );
    assert_eq!(owned(&state, "fume_yards.prepared_charge"), 1);
    absent(
        &state,
        &content,
        &["fume_yards.ignite_batch", "fume_yards.take_fuel"],
    );
    let state = act(state, &content, "fume_yards.reclaim_charge");
    assert_eq!(
        (
            owned(&state, "fume_yards.prepared_charge"),
            owned(&state, "fume_yards.repair_lot"),
            cage(&state, "fume_yards.fuel")
        ),
        (0, 1, 1)
    );
    let state = act(state, &content, "fume_yards.fit_dust_filter");
    let state = act(state, &content, "fume_yards.load_filtered_kiln_freight");
    assert_eq!(state.character.resources["coin"], 13);
    let state = act(state, &content, "world.enter_aftermath");
    let state = act(state, &content, "return.patch_stand");
    let state = act(state, &content, "return.order_water_stand");
    let state = act(state, &content, "return.visit_workshop");
    let state = travel(state, &content, BAY);
    absent(
        &state,
        &content,
        &["fume_yards.take_cask", "fume_yards.prepare_charge"],
    );
    let state = act(state, &content, "fume_yards.take_market_cask");
    assert_eq!(owned(&state, "fume_yards.water_cask"), 1);
    assert!(state.world.npcs[PERA].inventory.is_empty());
    absent(&state, &content, &["fume_yards.fit_wet_screen"]);
}

#[test]
fn purchase_at_tide_deadline_and_first_market_work_after_128_do_not_expire_stock() {
    let content = content();
    let mut state = front(content.new_game("ilyan", 71).unwrap(), &content);
    while state.world.time < 15 {
        state = act(state, &content, "wait_tide");
    }
    let state = act(state, &content, "fume_yards.buy_collateral_filter");
    assert_eq!(state.world.time, 16);
    assert!(state.world.flags.contains("surge_missed"));
    assert_eq!(owned(&state, FILTER), 1);
    assert_eq!(cage(&state, FILTER), 0);
    assert_eq!(state.event_log.iter().filter(|event| matches!(&event.kind, EventKind::ScheduledEventResolved { event_id, applied: true, .. } if event_id == "lowsail.next_surge")).count(), 1);
    let state = act(state, &content, "world.enter_aftermath");
    assert_eq!(state.world.npcs[DARO].location, ASH);
    assert_eq!(state.world.npcs[PERA].location, BAY);
    let mut late = hold(&content, 71);
    while late.world.time < 129 {
        late = act(late, &content, "wait_tide");
    }
    assert_eq!(cage(&late, FILTER), 1);
    let late = water_from_return(late, &content);
    assert_eq!(
        (
            late.world.time,
            late.character.resources["coin"],
            late.character.resources["stamina"]
        ),
        (145, 6, 5)
    );
    assert_eq!(cage(&late, FILTER), 0);
    absent(&late, &content, &["return.draw_clean_water"]);
}

#[test]
fn owned_undelivered_cask_names_pera_and_the_actual_return_route() {
    const GUIDANCE: &str = "Bring Pera from Kiln Bay to identify your cask";
    let content = content();
    for filter_first in [false, true] {
        let mut state = hold(&content, 71);
        for (id, destination) in [
            ("return.visit_workshop", None),
            ("fume_yards.take_stock", None),
            ("fume_yards.press_repair_plugs", None),
            ("world.enter_aftermath", None),
            ("return.patch_stand", None),
            ("return.order_water_stand", None),
            ("return.visit_workshop", None),
            ("travel_adjacent", Some(ASH)),
            ("fume_yards.buy_collateral_filter", None),
            ("world.enter_aftermath", None),
        ] {
            state = apply(state, &content, id, destination);
        }
        if filter_first {
            state = act(state, &content, "return.fit_market_filter");
        }
        assert!(!content.observe(&state).unwrap().text.contains(GUIDANCE));
        state = act(state, &content, "return.visit_workshop");
        state = travel(state, &content, BAY);
        state = act(state, &content, "fume_yards.take_market_cask");
        let inventory = state.character.inventory.clone();
        let resources = state.character.resources.clone();
        state = act(state, &content, "world.enter_aftermath");
        assert_eq!(state.character.inventory, inventory);
        assert_eq!(state.character.resources, resources);
        assert_eq!(owned(&state, "fume_yards.water_cask"), 1);
        assert_eq!(state.world.npcs[PERA].location, BAY);
        assert!(!state.world.npcs["oren_pell"].knows(FACT));
        let text = content.observe(&state).unwrap().text;
        assert!(
            text.contains(GUIDANCE),
            "owned cask omitted escort guidance: {text}"
        );
        assert!(text.contains("upland works still lack water"));
        assert!(!text.contains("needs a cask"));
        absent(
            &state,
            &content,
            &["return.install_market_cask", "return.draw_clean_water"],
        );
        assert!(legal(&state, &content).contains("return.visit_workshop"));
        state = act(state, &content, "return.visit_workshop");
        state = travel(state, &content, BAY);
        state = act(state, &content, "fume_yards.escort_market_cask");
        assert!(!content.observe(&state).unwrap().text.contains(GUIDANCE));
        assert!(legal(&state, &content).contains("return.install_market_cask"));
        state = act(state, &content, "return.install_market_cask");
        assert_eq!(owned(&state, "fume_yards.water_cask"), 0);
        assert!(!content.observe(&state).unwrap().text.contains(GUIDANCE));
    }
}
