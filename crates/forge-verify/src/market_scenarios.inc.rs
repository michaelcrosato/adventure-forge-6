// Reviewed literal tradeoffs. No expected quantity or record is derived from a run.
const MARKET_CAGE: &str = "fume_yards.collateral_cage";
const MARKET_RETURN: &str = "lowsail.return";
const MARKET_WORKSHOP: &str = "fume_yards.workshop";
const MARKET_PERA: &str = "fume_yards.pera_senn";
const MARKET_BRANN: &str = "fume_yards.brann_coil";
const MARKET_NESSA: &str = "fume_yards.nessa_tern";

const MARKET_PURCHASE_STEPS: [ScenarioStep; 10] = append_steps(
    &SALVAGE_FRONT_STEPS,
    &[action!("fume_yards.buy_collateral_filter")],
);
const MARKET_FUEL_STEPS: [ScenarioStep; 13] = append_steps(
    HOLD_MARKET_STEPS,
    &[
        action!("return.visit_workshop"),
        action!("travel_adjacent", "destination" => "fume_yards.kiln_bay"),
        action!("fume_yards.take_fuel"),
        action!("fume_yards.enter_ash_hatch"),
        action!("fume_yards.read_collateral_docket"),
        action!("fume_yards.settle_collateral_fuel"),
    ],
);
const MARKET_COMMON_STEPS: [ScenarioStep; 17] = append_steps(
    &MARKET_PURCHASE_STEPS,
    &[
        action!("travel_adjacent", "destination" => "fume_yards.workshop"),
        action!("fume_yards.take_stock"),
        action!("fume_yards.press_repair_plugs"),
        action!("fume_yards.load_freight"),
        action!("world.enter_aftermath"),
        action!("return.patch_stand"),
        action!("return.order_water_stand"),
    ],
);
const MARKET_LOCAL_STEPS: [ScenarioStep; 21] = append_steps(
    &MARKET_COMMON_STEPS,
    &[
        action!("return.visit_workshop"),
        action!("travel_adjacent", "destination" => "fume_yards.kiln_bay"),
        action!("fume_yards.fit_dust_filter"),
        action!("fume_yards.load_cold_freight"),
    ],
);
const MARKET_SALE_STEPS: [ScenarioStep; 18] =
    append_steps(&MARKET_COMMON_STEPS, &[action!("return.sell_filter")]);
const MARKET_CASK_PREFIX: [ScenarioStep; 21] = append_steps(
    &MARKET_COMMON_STEPS,
    &[
        action!("return.fit_market_filter"),
        action!("return.visit_workshop"),
        action!("travel_adjacent", "destination" => "fume_yards.kiln_bay"),
        action!("fume_yards.take_market_cask"),
    ],
);
const MARKET_DELIVERED_STEPS: [ScenarioStep; 22] = append_steps(
    &MARKET_CASK_PREFIX,
    &[action!("fume_yards.escort_market_cask")],
);
const MARKET_UNDELIVERED_STEPS: [ScenarioStep; 22] =
    append_steps(&MARKET_CASK_PREFIX, &[action!("world.enter_aftermath")]);
const MARKET_WATER_STEPS: [ScenarioStep; 24] = append_steps(
    &MARKET_DELIVERED_STEPS,
    &[
        action!("return.install_market_cask"),
        action!("return.draw_clean_water"),
    ],
);
const MARKET_COMPOSED_STEPS: [ScenarioStep; 30] = append_steps(
    HOLD_MARKET_STEPS,
    &[
        action!("return.visit_workshop"),
        action!("fume_yards.take_stock"),
        action!("fume_yards.press_repair_plugs"),
        action!("travel_adjacent", "destination" => "fume_yards.ash_beds"),
        action!("fume_yards.brace_rack"),
        action!("fume_yards.recover_braced_filter"),
        action!("travel_adjacent", "destination" => "fume_yards.workshop"),
        action!("travel_adjacent", "destination" => "fume_yards.kiln_bay"),
        action!("fume_yards.fit_dust_filter"),
        action!("fume_yards.load_cold_freight"),
        action!("world.enter_aftermath"),
        action!("return.patch_stand"),
        action!("return.order_water_stand"),
        action!("return.visit_workshop"),
        action!("travel_adjacent", "destination" => "fume_yards.ash_beds"),
        action!("fume_yards.buy_collateral_filter"),
        action!("travel_adjacent", "destination" => "fume_yards.workshop"),
        action!("travel_adjacent", "destination" => "fume_yards.kiln_bay"),
        action!("fume_yards.take_market_cask"),
        action!("fume_yards.escort_market_cask"),
        action!("return.fit_market_filter"),
        action!("return.install_market_cask"),
        action!("return.draw_clean_water"),
    ],
);
const MARKET_AFTER_REPORT_STEPS: [ScenarioStep; 16] = append_steps(
    &SALVAGE_REPORTED_STEPS,
    &[
        action!("fume_yards.return_to_cage"),
        action!("fume_yards.buy_collateral_filter"),
    ],
);

const fn market_append<T: Copy, const N: usize>(left: &[T], right: &[T]) -> [T; N] {
    assert!(left.len() + right.len() == N && !left.is_empty());
    let mut output = [left[0]; N];
    let mut index = 0;
    while index < left.len() {
        output[index] = left[index];
        index += 1;
    }
    while index < N {
        output[index] = right[index - left.len()];
        index += 1;
    }
    output
}
const fn market_resource(resource: &'static str, amount: i64) -> ScenarioResourceExpectation {
    ScenarioResourceExpectation { resource, amount }
}
const fn market_item(item: &'static str, count: u32) -> ScenarioInventoryExpectation {
    ScenarioInventoryExpectation { item, count }
}
const fn market_stock(
    npc: &'static str,
    item: &'static str,
    count: u32,
) -> ScenarioNpcInventoryExpectation {
    ScenarioNpcInventoryExpectation { npc, item, count }
}
const fn market_position(
    npc: &'static str,
    location: &'static str,
) -> ScenarioNpcLocationExpectation {
    ScenarioNpcLocationExpectation { npc, location }
}
const fn market_knowledge(
    npc: &'static str,
    knowledge_id: &'static str,
    turn: u64,
) -> ScenarioNpcKnowledgeExpectation {
    ScenarioNpcKnowledgeExpectation {
        npc,
        knowledge_id,
        provenance: ScenarioKnowledgeProvenance::Witnessed,
        turn,
    }
}
const fn market_absent_knowledge(
    npc: &'static str,
    knowledge_id: &'static str,
) -> ScenarioNpcKnowledgeAbsence {
    ScenarioNpcKnowledgeAbsence { npc, knowledge_id }
}
const fn market_transfer(
    turn: u64,
    direction: StorageTransferDirection,
    item: &'static str,
) -> StorageTransferExpectation {
    StorageTransferExpectation {
        turn,
        direction,
        storage: MARKET_CAGE,
        item,
        count: 1,
    }
}
const MARKET_EMPTY_CAGE: &[StorageBalanceExpectation] = &[StorageBalanceExpectation {
    storage: MARKET_CAGE,
    inventory: &[],
}];
const MARKET_PURCHASE_TRANSFER: &[StorageTransferExpectation] = &[market_transfer(
    9,
    StorageTransferDirection::ToCharacter,
    "fume_yards.filter",
)];
const MARKET_NO_INTERMEDIATES: &[&str] = &[
    "fume_yards.clay",
    "fume_yards.mesh",
    "fume_yards.repair_lot",
    "fume_yards.catch_screen",
    "fume_yards.prepared_charge",
    "fume_yards.batch_claim",
    "fume_yards.spoiled_charge",
    "fume_yards.shard",
    "fume_yards.fuel",
];
const MARKET_NO_FIRE: &[(&str, &str)] = &[
    (SALVAGE_BAY, "fume_yards.batch_ignited"),
    (SALVAGE_BAY, "fume_yards.batch_active"),
    (SALVAGE_BAY, "fume_yards.batch_spoiled"),
    (SALVAGE_BAY, "fume_yards.freight_spoiled"),
    (SALVAGE_BAY, "fume_yards.wet_screen_fitted"),
];
const MARKET_CUSTODIANS: &[ScenarioNpcLocationExpectation] = &[
    market_position(SALVAGE_DARO, SALVAGE_ASH),
    market_position(MARKET_NESSA, MARKET_WORKSHOP),
    market_position(MARKET_BRANN, SALVAGE_BAY),
    market_position("oren_pell", MARKET_RETURN),
];
const MARKET_ALL_STOCK: &[ScenarioNpcInventoryExpectation] = &[
    market_stock(SALVAGE_DARO, "fume_yards.filter", 1),
    market_stock(MARKET_NESSA, "fume_yards.clay", 2),
    market_stock(MARKET_NESSA, "fume_yards.mesh", 1),
    market_stock(MARKET_BRANN, "fume_yards.fuel", 1),
    market_stock(MARKET_PERA, "fume_yards.water_cask", 1),
];
const MARKET_EMPTY_NESSA: &[ScenarioNpcInventoryAbsence] = &[
    ScenarioNpcInventoryAbsence {
        npc: MARKET_NESSA,
        item: "fume_yards.clay",
    },
    ScenarioNpcInventoryAbsence {
        npc: MARKET_NESSA,
        item: "fume_yards.mesh",
    },
];
const MARKET_NO_REMOTE_TRADE: &[ScenarioNpcKnowledgeAbsence] = &[
    market_absent_knowledge(MARKET_BRANN, "fume_yards.collateral_settled"),
    market_absent_knowledge(MARKET_PERA, "fume_yards.collateral_settled"),
    market_absent_knowledge(MARKET_NESSA, "fume_yards.collateral_settled"),
    market_absent_knowledge("oren_pell", "fume_yards.collateral_settled"),
];
const MARKET_COMMON_KNOWLEDGE: &[ScenarioNpcKnowledgeExpectation] = &[
    market_knowledge(SALVAGE_DARO, "fume_yards.collateral_settled", 9),
    market_knowledge("oren_pell", "fume_yards.stand_patched", 15),
    market_knowledge("oren_pell", "fume_yards.market_water_ordered", 16),
];
const MARKET_COMMON_MEMORIES: &[ScenarioNpcMemoryExpectation] = &[
    batch_memory(SALVAGE_DARO, "fume_yards.collateral_paid", 9),
    batch_memory(MARKET_NESSA, "fume_yards.stock_handed_over", 11),
    batch_memory(MARKET_NESSA, "fume_yards.repair_plugs_pressed", 12),
    batch_memory(MARKET_NESSA, "fume_yards.freight_paid", 13),
    batch_memory("oren_pell", "fume_yards.stand_patched", 15),
    batch_memory("oren_pell", "fume_yards.market_water_ordered", 16),
];
const MARKET_COMMON_RECIPES: &[ScenarioRecipeExpectation] = &[
    batch_recipe(
        12,
        "fume_yards.press_repair_plugs",
        PILOT_INPUTS,
        &[("fume_yards.repair_lot", 1)],
    ),
    batch_recipe(
        15,
        "fume_yards.patch_stand",
        &[("fume_yards.repair_lot", 1)],
        &[],
    ),
];
const MARKET_COMMON_FLAGS: &[(&str, &str)] = &[
    (SALVAGE_ASH, "fume_yards.collateral_settled"),
    (MARKET_WORKSHOP, "fume_yards.stock_given"),
    (MARKET_WORKSHOP, "fume_yards.freight_loaded"),
    (MARKET_RETURN, "fume_yards.stand_patched"),
    (MARKET_RETURN, "fume_yards.market_water_ordered"),
];
const MARKET_CLOSED_TRADE: &[&str] = &[
    "fume_yards.buy_collateral_filter",
    "fume_yards.settle_collateral_fuel",
    "fume_yards.read_collateral_docket",
];

const MARKET_BASE: ScenarioExpectations = ScenarioExpectations {
    final_location: SALVAGE_ASH,
    final_action_definition: "fume_yards.buy_collateral_filter",
    final_observation_contains: "You pay Daro four coins and take the caged filter.",
    forbidden_observation_contains: &[],
    final_world_time: Some(10),
    exclusive_after_action: "",
    required_world_flags: &["ending_council", "flow_locked_market"],
    forbidden_world_flags: &["surge_missed", "sluice_failure"],
    required_location_flags: &[(SALVAGE_ASH, "fume_yards.collateral_settled")],
    forbidden_location_flags: &market_append::<_, 14>(
        MARKET_NO_FIRE,
        &[
            (SALVAGE_ASH, "fume_yards.rack_cleared"),
            (SALVAGE_ASH, "fume_yards.rack_braced"),
            (SALVAGE_BAY, "fume_yards.fuel_settled"),
            (SALVAGE_BAY, "fume_yards.dust_filter_fitted"),
            (MARKET_RETURN, "fume_yards.market_water_ordered"),
            (MARKET_RETURN, "fume_yards.market_filter_fitted"),
            (MARKET_RETURN, "fume_yards.market_cask_delivered"),
            (MARKET_RETURN, "fume_yards.market_cask_installed"),
            (MARKET_RETURN, "fume_yards.clean_water_drawn"),
        ],
    ),
    required_deeds: &[],
    required_visited_locations: &[MARKET_WORKSHOP, SALVAGE_ASH],
    required_npc_locations: &market_append::<_, 5>(
        MARKET_CUSTODIANS,
        &[market_position(MARKET_PERA, SALVAGE_BAY)],
    ),
    required_npc_knowledge: &[market_knowledge(
        SALVAGE_DARO,
        "fume_yards.collateral_settled",
        9,
    )],
    forbidden_npc_knowledge: &market_append::<_, 7>(
        MARKET_NO_REMOTE_TRADE,
        &[
            market_absent_knowledge(SALVAGE_DARO, "fume_yards.rack_cleared"),
            market_absent_knowledge(MARKET_PERA, "fume_yards.market_cask"),
            market_absent_knowledge("oren_pell", "fume_yards.market_cask"),
        ],
    ),
    required_npc_memories: &[batch_memory(SALVAGE_DARO, "fume_yards.collateral_paid", 9)],
    required_character_inventory: &[market_item("rope", 1), market_item("fume_yards.filter", 1)],
    required_character_resources: &[market_resource("coin", 6), market_resource("stamina", 3)],
    required_npc_inventory: MARKET_ALL_STOCK,
    forbidden_npc_inventory: &[],
    forbidden_character_inventory: &market_append::<_, 10>(
        MARKET_NO_INTERMEDIATES,
        &["fume_yards.water_cask"],
    ),
    recipe_events: &[],
    entropy_draws: &[],
    deferred_events: &[],
    pending_deferred_events: &[],
    staffing_history: None,
    cold_shift_history: None,
    storage_balances: MARKET_EMPTY_CAGE,
    storage_transfers: MARKET_PURCHASE_TRANSFER,
    required_legal_definitions: &["fume_yards.brace_rack", "fume_yards.pull_rack_filter"],
    forbidden_legal_definitions: MARKET_CLOSED_TRADE,
};
const MARKET_PURCHASE_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-collateral-purchase",
    claim_id: "milestone-2.market.collateral-purchase",
    start: ScenarioStartSpec::Preset {
        character_preset_id: "ilyan",
    },
    seed: 71,
    steps: &MARKET_PURCHASE_STEPS,
    expectations: MARKET_BASE,
};
const MARKET_FUEL_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-collateral-fuel",
    claim_id: "milestone-2.market.collateral-fuel",
    steps: &MARKET_FUEL_STEPS,
    expectations: ScenarioExpectations {
        final_action_definition: "fume_yards.settle_collateral_fuel",
        final_world_time: Some(13),
        final_observation_contains: "You leave one fuel in the cage and take its filter.",
        required_character_resources: &[market_resource("coin", 10), market_resource("stamina", 3)],
        required_deeds: &["fume_yards.read_collateral_docket"],
        required_location_flags: &[
            (SALVAGE_ASH, "fume_yards.collateral_settled"),
            (SALVAGE_ASH, "fume_yards.rack_rear_open"),
            (SALVAGE_BAY, "fume_yards.fuel_taken"),
            (SALVAGE_BAY, "fume_yards.fuel_settled"),
        ],
        forbidden_location_flags: &market_append::<_, 11>(
            MARKET_NO_FIRE,
            &[
                (SALVAGE_ASH, "fume_yards.rack_cleared"),
                (SALVAGE_ASH, "fume_yards.rack_braced"),
                (SALVAGE_BAY, "fume_yards.dust_filter_fitted"),
                (MARKET_RETURN, "fume_yards.market_water_ordered"),
                (MARKET_RETURN, "fume_yards.market_filter_fitted"),
                (MARKET_RETURN, "fume_yards.clean_water_drawn"),
            ],
        ),
        required_npc_knowledge: &[
            ScenarioNpcKnowledgeExpectation {
                npc: SALVAGE_DARO,
                knowledge_id: "fume_yards.collateral_terms",
                provenance: ScenarioKnowledgeProvenance::Read {
                    source: "fume_yards.collateral_docket",
                },
                turn: 11,
            },
            market_knowledge(SALVAGE_DARO, "fume_yards.collateral_settled", 12),
        ],
        required_npc_memories: &[
            batch_memory(MARKET_BRANN, "fume_yards.fuel_handed_over", 9),
            batch_memory(SALVAGE_DARO, "fume_yards.collateral_docket_read", 11),
            batch_memory(SALVAGE_DARO, "fume_yards.collateral_fuel_received", 12),
        ],
        required_npc_inventory: &[
            MARKET_ALL_STOCK[0],
            MARKET_ALL_STOCK[1],
            MARKET_ALL_STOCK[2],
            MARKET_ALL_STOCK[4],
        ],
        forbidden_npc_inventory: &[ScenarioNpcInventoryAbsence {
            npc: MARKET_BRANN,
            item: "fume_yards.fuel",
        }],
        storage_balances: &[StorageBalanceExpectation {
            storage: MARKET_CAGE,
            inventory: &[("fume_yards.fuel", 1)],
        }],
        storage_transfers: &[
            market_transfer(
                12,
                StorageTransferDirection::FromCharacter,
                "fume_yards.fuel",
            ),
            market_transfer(
                12,
                StorageTransferDirection::ToCharacter,
                "fume_yards.filter",
            ),
        ],
        ..MARKET_BASE
    },
    ..MARKET_PURCHASE_SPEC
};

const MARKET_COMMON: ScenarioExpectations = ScenarioExpectations {
    final_location: MARKET_RETURN,
    final_action_definition: "return.order_water_stand",
    final_world_time: Some(17),
    final_observation_contains: "Oren orders a filter and one stored cask.",
    required_location_flags: MARKET_COMMON_FLAGS,
    forbidden_location_flags: &market_append::<_, 13>(
        MARKET_NO_FIRE,
        &[
            (SALVAGE_ASH, "fume_yards.rack_cleared"),
            (SALVAGE_BAY, "fume_yards.dust_filter_fitted"),
            (MARKET_RETURN, "fume_yards.filter_sold"),
            (MARKET_RETURN, "fume_yards.market_filter_fitted"),
            (MARKET_RETURN, "fume_yards.market_cask_delivered"),
            (MARKET_RETURN, "fume_yards.market_cask_installed"),
            (MARKET_RETURN, "fume_yards.clean_water_drawn"),
            (MARKET_RETURN, "fume_yards.dry_goods_sorted"),
        ],
    ),
    required_character_resources: &[market_resource("coin", 8), market_resource("stamina", 1)],
    required_npc_knowledge: MARKET_COMMON_KNOWLEDGE,
    required_npc_memories: MARKET_COMMON_MEMORIES,
    required_npc_inventory: &[
        MARKET_ALL_STOCK[0],
        MARKET_ALL_STOCK[3],
        MARKET_ALL_STOCK[4],
    ],
    forbidden_npc_inventory: MARKET_EMPTY_NESSA,
    recipe_events: MARKET_COMMON_RECIPES,
    required_legal_definitions: &[
        "return.fit_market_filter",
        "return.sell_filter",
        "return.sort_dry_goods",
        "return.visit_workshop",
    ],
    forbidden_legal_definitions: &[
        "return.order_water_stand",
        "return.patch_stand",
        "return.draw_clean_water",
    ],
    ..MARKET_BASE
};
const MARKET_LOCAL_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-collateral-local",
    claim_id: "milestone-2.market.collateral-local",
    steps: &MARKET_LOCAL_STEPS,
    expectations: ScenarioExpectations {
        final_location: SALVAGE_BAY,
        final_action_definition: "fume_yards.load_cold_freight",
        final_world_time: Some(21),
        final_observation_contains: "cold work closes firing.",
        required_location_flags: &market_append::<_, 9>(
            MARKET_COMMON_FLAGS,
            &[
                (SALVAGE_BAY, "fume_yards.dust_filter_fitted"),
                (SALVAGE_BAY, "fume_yards.kiln_closed"),
                (SALVAGE_BAY, "fume_yards.cold_work_chosen"),
                (SALVAGE_BAY, "fume_yards.kiln_freight_loaded"),
            ],
        ),
        forbidden_location_flags: &market_append::<_, 11>(
            MARKET_NO_FIRE,
            &[
                (SALVAGE_ASH, "fume_yards.rack_cleared"),
                (MARKET_RETURN, "fume_yards.filter_sold"),
                (MARKET_RETURN, "fume_yards.market_filter_fitted"),
                (MARKET_RETURN, "fume_yards.market_cask_delivered"),
                (MARKET_RETURN, "fume_yards.market_cask_installed"),
                (MARKET_RETURN, "fume_yards.clean_water_drawn"),
            ],
        ),
        required_character_inventory: &[market_item("rope", 1)],
        forbidden_character_inventory: &market_append::<_, 11>(
            MARKET_NO_INTERMEDIATES,
            &["fume_yards.filter", "fume_yards.water_cask"],
        ),
        required_character_resources: &[market_resource("coin", 11), market_resource("stamina", 1)],
        required_npc_memories: &market_append::<_, 9>(
            MARKET_COMMON_MEMORIES,
            &[
                batch_memory(MARKET_BRANN, "fume_yards.dust_filter_fitted", 19),
                batch_memory(MARKET_BRANN, "fume_yards.kiln_freight_paid", 20),
                batch_memory(MARKET_BRANN, "fume_yards.cold_work_chosen", 20),
            ],
        ),
        recipe_events: &market_append::<_, 3>(
            MARKET_COMMON_RECIPES,
            &[batch_recipe(
                19,
                "fume_yards.fit_dust_filter",
                &[("fume_yards.filter", 1)],
                &[],
            )],
        ),
        required_legal_definitions: &["world.enter_aftermath"],
        forbidden_legal_definitions: &[
            "fume_yards.fit_dust_filter",
            "fume_yards.load_cold_freight",
            "fume_yards.ignite_batch",
            "fume_yards.prepare_charge",
            "fume_yards.load_kiln_freight",
            "fume_yards.load_filtered_kiln_freight",
        ],
        ..MARKET_COMMON
    },
    ..MARKET_PURCHASE_SPEC
};
const MARKET_SALE_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-collateral-sale",
    claim_id: "milestone-2.market.collateral-sale",
    steps: &MARKET_SALE_STEPS,
    expectations: ScenarioExpectations {
        final_action_definition: "return.sell_filter",
        final_world_time: Some(18),
        final_observation_contains: "Oren buys your filter for four coins.",
        required_location_flags: &market_append::<_, 6>(
            MARKET_COMMON_FLAGS,
            &[(MARKET_RETURN, "fume_yards.filter_sold")],
        ),
        forbidden_location_flags: &market_append::<_, 11>(
            MARKET_NO_FIRE,
            &[
                (SALVAGE_ASH, "fume_yards.rack_cleared"),
                (SALVAGE_BAY, "fume_yards.dust_filter_fitted"),
                (MARKET_RETURN, "fume_yards.market_filter_fitted"),
                (MARKET_RETURN, "fume_yards.market_cask_delivered"),
                (MARKET_RETURN, "fume_yards.market_cask_installed"),
                (MARKET_RETURN, "fume_yards.clean_water_drawn"),
            ],
        ),
        required_character_inventory: &[market_item("rope", 1)],
        forbidden_character_inventory: &market_append::<_, 11>(
            MARKET_NO_INTERMEDIATES,
            &["fume_yards.filter", "fume_yards.water_cask"],
        ),
        required_character_resources: &[market_resource("coin", 12), market_resource("stamina", 1)],
        required_npc_memories: &market_append::<_, 7>(
            MARKET_COMMON_MEMORIES,
            &[batch_memory("oren_pell", "fume_yards.filter_bought", 17)],
        ),
        recipe_events: &market_append::<_, 3>(
            MARKET_COMMON_RECIPES,
            &[batch_recipe(
                17,
                "fume_yards.sell_filter",
                &[("fume_yards.filter", 1)],
                &[],
            )],
        ),
        required_legal_definitions: &["return.visit_workshop", "return.sort_dry_goods"],
        forbidden_legal_definitions: &[
            "return.sell_filter",
            "return.fit_market_filter",
            "return.draw_clean_water",
            "return.install_market_cask",
        ],
        ..MARKET_COMMON
    },
    ..MARKET_PURCHASE_SPEC
};

const MARKET_FILTER_RECIPES: &[ScenarioRecipeExpectation] = &market_append::<_, 3>(
    MARKET_COMMON_RECIPES,
    &[batch_recipe(
        17,
        "fume_yards.fit_market_filter",
        &[("fume_yards.filter", 1)],
        &[],
    )],
);
const MARKET_CASK_KNOWLEDGE: &[ScenarioNpcKnowledgeExpectation] = &market_append::<_, 5>(
    MARKET_COMMON_KNOWLEDGE,
    &[
        market_knowledge(MARKET_PERA, "fume_yards.market_cask", 21),
        ScenarioNpcKnowledgeExpectation {
            npc: "oren_pell",
            knowledge_id: "fume_yards.market_cask",
            turn: 21,
            provenance: ScenarioKnowledgeProvenance::Told { by: MARKET_PERA },
        },
    ],
);
const MARKET_CASK_MEMORIES: &[ScenarioNpcMemoryExpectation] = &market_append::<_, 10>(
    MARKET_COMMON_MEMORIES,
    &[
        batch_memory("oren_pell", "fume_yards.market_filter_fitted", 17),
        batch_memory(MARKET_PERA, "fume_yards.market_cask_handed_over", 20),
        batch_memory(MARKET_PERA, "fume_yards.market_cask_escorted", 21),
        batch_memory("oren_pell", "fume_yards.market_cask_arrived", 21),
    ],
);
const MARKET_CASK_FLAGS: &[(&str, &str)] = &market_append::<_, 8>(
    MARKET_COMMON_FLAGS,
    &[
        (MARKET_RETURN, "fume_yards.market_filter_fitted"),
        (MARKET_RETURN, "fume_yards.market_cask_delivered"),
        (SALVAGE_BAY, "fume_yards.cask_taken"),
    ],
);
const MARKET_EMPTY_CASK: &[ScenarioNpcInventoryAbsence] = &market_append::<_, 3>(
    MARKET_EMPTY_NESSA,
    &[ScenarioNpcInventoryAbsence {
        npc: MARKET_PERA,
        item: "fume_yards.water_cask",
    }],
);
const MARKET_CASK_DELIVERED_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-market-cask-delivered",
    claim_id: "milestone-2.market.cask-delivered",
    steps: &MARKET_DELIVERED_STEPS,
    expectations: ScenarioExpectations {
        final_world_time: Some(22),
        final_action_definition: "fume_yards.escort_market_cask",
        final_observation_contains: "Pera accompanies your cask to Lowsail and identifies it to Oren.",
        required_location_flags: MARKET_CASK_FLAGS,
        forbidden_location_flags: &market_append::<_, 10>(
            MARKET_NO_FIRE,
            &[
                (SALVAGE_ASH, "fume_yards.rack_cleared"),
                (SALVAGE_BAY, "fume_yards.dust_filter_fitted"),
                (MARKET_RETURN, "fume_yards.filter_sold"),
                (MARKET_RETURN, "fume_yards.market_cask_installed"),
                (MARKET_RETURN, "fume_yards.clean_water_drawn"),
            ],
        ),
        required_character_inventory: &[
            market_item("rope", 1),
            market_item("fume_yards.water_cask", 1),
        ],
        forbidden_character_inventory: &market_append::<_, 10>(
            MARKET_NO_INTERMEDIATES,
            &["fume_yards.filter"],
        ),
        required_npc_inventory: &[MARKET_ALL_STOCK[0], MARKET_ALL_STOCK[3]],
        forbidden_npc_inventory: MARKET_EMPTY_CASK,
        required_npc_locations: &market_append::<_, 5>(
            MARKET_CUSTODIANS,
            &[market_position(MARKET_PERA, MARKET_RETURN)],
        ),
        required_npc_knowledge: MARKET_CASK_KNOWLEDGE,
        forbidden_npc_knowledge: &market_append::<_, 8>(
            MARKET_NO_REMOTE_TRADE,
            &[
                market_absent_knowledge(SALVAGE_DARO, "fume_yards.rack_cleared"),
                market_absent_knowledge(SALVAGE_DARO, "fume_yards.market_cask"),
                market_absent_knowledge(MARKET_BRANN, "fume_yards.market_cask"),
                market_absent_knowledge(MARKET_NESSA, "fume_yards.market_cask"),
            ],
        ),
        required_npc_memories: MARKET_CASK_MEMORIES,
        recipe_events: MARKET_FILTER_RECIPES,
        required_legal_definitions: &["return.install_market_cask", "return.sort_dry_goods"],
        forbidden_legal_definitions: &[
            "return.fit_market_filter",
            "return.draw_clean_water",
            "return.sell_filter",
            "fume_yards.escort_market_cask",
        ],
        ..MARKET_COMMON
    },
    ..MARKET_PURCHASE_SPEC
};
const MARKET_CASK_UNDELIVERED_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-market-cask-undelivered",
    claim_id: "milestone-2.market.cask-undelivered",
    steps: &MARKET_UNDELIVERED_STEPS,
    expectations: ScenarioExpectations {
        final_action_definition: "world.enter_aftermath",
        final_observation_contains: "Bring Pera from Kiln Bay to identify your cask",
        forbidden_observation_contains: &[
            "The fitted filter needs a cask",
            "The stand needs a filter and cask",
        ],
        required_location_flags: &market_append::<_, 7>(
            MARKET_COMMON_FLAGS,
            &[
                (MARKET_RETURN, "fume_yards.market_filter_fitted"),
                (SALVAGE_BAY, "fume_yards.cask_taken"),
            ],
        ),
        forbidden_location_flags: &market_append::<_, 11>(
            MARKET_NO_FIRE,
            &[
                (SALVAGE_ASH, "fume_yards.rack_cleared"),
                (SALVAGE_BAY, "fume_yards.dust_filter_fitted"),
                (MARKET_RETURN, "fume_yards.filter_sold"),
                (MARKET_RETURN, "fume_yards.market_cask_delivered"),
                (MARKET_RETURN, "fume_yards.market_cask_installed"),
                (MARKET_RETURN, "fume_yards.clean_water_drawn"),
            ],
        ),
        required_npc_locations: MARKET_BASE.required_npc_locations,
        required_npc_knowledge: MARKET_COMMON_KNOWLEDGE,
        forbidden_npc_knowledge: MARKET_BASE.forbidden_npc_knowledge,
        required_npc_memories: &market_append::<_, 8>(
            MARKET_COMMON_MEMORIES,
            &[
                batch_memory("oren_pell", "fume_yards.market_filter_fitted", 17),
                batch_memory(MARKET_PERA, "fume_yards.market_cask_handed_over", 20),
            ],
        ),
        required_legal_definitions: &["return.visit_workshop", "return.sort_dry_goods"],
        forbidden_legal_definitions: &[
            "return.install_market_cask",
            "return.draw_clean_water",
            "return.fit_market_filter",
            "return.sell_filter",
        ],
        ..MARKET_CASK_DELIVERED_SPEC.expectations
    },
    ..MARKET_PURCHASE_SPEC
};
const MARKET_WATER_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-market-water",
    claim_id: "milestone-2.market.water",
    steps: &MARKET_WATER_STEPS,
    expectations: ScenarioExpectations {
        final_world_time: Some(24),
        final_action_definition: "return.draw_clean_water",
        final_observation_contains: "The installed cask has no second ration.",
        required_location_flags: &market_append::<_, 10>(
            MARKET_CASK_FLAGS,
            &[
                (MARKET_RETURN, "fume_yards.market_cask_installed"),
                (MARKET_RETURN, "fume_yards.clean_water_drawn"),
            ],
        ),
        forbidden_location_flags: &market_append::<_, 9>(
            MARKET_NO_FIRE,
            &[
                (SALVAGE_ASH, "fume_yards.rack_cleared"),
                (SALVAGE_BAY, "fume_yards.dust_filter_fitted"),
                (MARKET_RETURN, "fume_yards.filter_sold"),
                (MARKET_RETURN, "fume_yards.dry_goods_sorted"),
            ],
        ),
        required_character_inventory: &[market_item("rope", 1)],
        forbidden_character_inventory: &market_append::<_, 11>(
            MARKET_NO_INTERMEDIATES,
            &["fume_yards.filter", "fume_yards.water_cask"],
        ),
        required_character_resources: &[market_resource("coin", 8), market_resource("stamina", 3)],
        required_npc_knowledge: &market_append::<_, 6>(
            MARKET_CASK_KNOWLEDGE,
            &[market_knowledge(
                "oren_pell",
                "fume_yards.clean_water_supplied",
                23,
            )],
        ),
        required_npc_memories: &market_append::<_, 12>(
            MARKET_CASK_MEMORIES,
            &[
                batch_memory("oren_pell", "fume_yards.market_cask_installed", 22),
                batch_memory("oren_pell", "fume_yards.clean_water_supplied", 23),
            ],
        ),
        recipe_events: &market_append::<_, 4>(
            MARKET_FILTER_RECIPES,
            &[batch_recipe(
                22,
                "fume_yards.install_market_cask",
                &[("fume_yards.water_cask", 1)],
                &[],
            )],
        ),
        required_legal_definitions: &["return.sort_dry_goods", "return.visit_workshop"],
        forbidden_legal_definitions: &[
            "return.draw_clean_water",
            "return.install_market_cask",
            "return.fit_market_filter",
            "return.sell_filter",
            "return.order_water_stand",
        ],
        ..MARKET_CASK_DELIVERED_SPEC.expectations
    },
    ..MARKET_PURCHASE_SPEC
};
const MARKET_COMPOSED_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-market-water-composed",
    claim_id: "milestone-2.market.water-composed",
    steps: &MARKET_COMPOSED_STEPS,
    expectations: ScenarioExpectations {
        final_world_time: Some(30),
        required_character_resources: &[market_resource("coin", 9), market_resource("stamina", 3)],
        required_location_flags: &[
            (SALVAGE_ASH, "fume_yards.collateral_settled"),
            (SALVAGE_ASH, "fume_yards.rack_braced"),
            (SALVAGE_ASH, "fume_yards.rack_cleared"),
            (MARKET_WORKSHOP, "fume_yards.stock_given"),
            (SALVAGE_BAY, "fume_yards.dust_filter_fitted"),
            (SALVAGE_BAY, "fume_yards.kiln_closed"),
            (SALVAGE_BAY, "fume_yards.cold_work_chosen"),
            (SALVAGE_BAY, "fume_yards.kiln_freight_loaded"),
            (SALVAGE_BAY, "fume_yards.cask_taken"),
            (MARKET_RETURN, "fume_yards.stand_patched"),
            (MARKET_RETURN, "fume_yards.market_water_ordered"),
            (MARKET_RETURN, "fume_yards.market_filter_fitted"),
            (MARKET_RETURN, "fume_yards.market_cask_delivered"),
            (MARKET_RETURN, "fume_yards.market_cask_installed"),
            (MARKET_RETURN, "fume_yards.clean_water_drawn"),
        ],
        forbidden_location_flags: &market_append::<_, 10>(
            MARKET_NO_FIRE,
            &[
                (SALVAGE_ASH, "fume_yards.report_paid"),
                (SALVAGE_BAY, "fume_yards.fuel_taken"),
                (MARKET_WORKSHOP, "fume_yards.freight_loaded"),
                (MARKET_RETURN, "fume_yards.filter_sold"),
                (MARKET_RETURN, "fume_yards.dry_goods_sorted"),
            ],
        ),
        required_npc_inventory: &[MARKET_ALL_STOCK[3]],
        forbidden_npc_inventory: &market_append::<_, 4>(MARKET_EMPTY_CASK, SALVAGE_EMPTY_RACK),
        required_npc_knowledge: &[
            salvage_knowledge(12),
            market_knowledge(SALVAGE_DARO, "fume_yards.collateral_settled", 22),
            market_knowledge("oren_pell", "fume_yards.stand_patched", 18),
            market_knowledge("oren_pell", "fume_yards.market_water_ordered", 19),
            market_knowledge(MARKET_PERA, "fume_yards.market_cask", 26),
            ScenarioNpcKnowledgeExpectation {
                npc: "oren_pell",
                knowledge_id: "fume_yards.market_cask",
                turn: 26,
                provenance: ScenarioKnowledgeProvenance::Told { by: MARKET_PERA },
            },
            market_knowledge("oren_pell", "fume_yards.clean_water_supplied", 29),
        ],
        forbidden_npc_knowledge: &market_append::<_, 11>(
            MARKET_NO_REMOTE_TRADE,
            &market_append::<_, 7>(
                SALVAGE_REMOTE_UNINFORMED,
                &[
                    market_absent_knowledge(SALVAGE_DARO, "fume_yards.market_cask"),
                    market_absent_knowledge(MARKET_BRANN, "fume_yards.market_cask"),
                    market_absent_knowledge(MARKET_NESSA, "fume_yards.market_cask"),
                ],
            ),
        ),
        required_npc_memories: &[
            batch_memory(MARKET_NESSA, "fume_yards.stock_handed_over", 8),
            batch_memory(MARKET_NESSA, "fume_yards.repair_plugs_pressed", 9),
            batch_memory(SALVAGE_DARO, "fume_yards.rack_cleared", 12),
            batch_memory(SALVAGE_DARO, "fume_yards.collateral_paid", 22),
            batch_memory(MARKET_BRANN, "fume_yards.dust_filter_fitted", 15),
            batch_memory(MARKET_BRANN, "fume_yards.kiln_freight_paid", 16),
            batch_memory(MARKET_BRANN, "fume_yards.cold_work_chosen", 16),
            batch_memory("oren_pell", "fume_yards.stand_patched", 18),
            batch_memory("oren_pell", "fume_yards.market_water_ordered", 19),
            batch_memory(MARKET_PERA, "fume_yards.market_cask_handed_over", 25),
            batch_memory(MARKET_PERA, "fume_yards.market_cask_escorted", 26),
            batch_memory("oren_pell", "fume_yards.market_cask_arrived", 26),
            batch_memory("oren_pell", "fume_yards.market_filter_fitted", 27),
            batch_memory("oren_pell", "fume_yards.market_cask_installed", 28),
            batch_memory("oren_pell", "fume_yards.clean_water_supplied", 29),
        ],
        storage_transfers: &[market_transfer(
            22,
            StorageTransferDirection::ToCharacter,
            "fume_yards.filter",
        )],
        recipe_events: &[
            batch_recipe(
                9,
                "fume_yards.press_repair_plugs",
                PILOT_INPUTS,
                &[("fume_yards.repair_lot", 1)],
            ),
            batch_recipe(
                15,
                "fume_yards.fit_dust_filter",
                &[("fume_yards.filter", 1)],
                &[],
            ),
            batch_recipe(
                18,
                "fume_yards.patch_stand",
                &[("fume_yards.repair_lot", 1)],
                &[],
            ),
            batch_recipe(
                27,
                "fume_yards.fit_market_filter",
                &[("fume_yards.filter", 1)],
                &[],
            ),
            batch_recipe(
                28,
                "fume_yards.install_market_cask",
                &[("fume_yards.water_cask", 1)],
                &[],
            ),
        ],
        ..MARKET_WATER_SPEC.expectations
    },
    ..MARKET_PURCHASE_SPEC
};
const MARKET_AFTER_REPORT_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-collateral-after-report",
    claim_id: "milestone-2.market.collateral-after-report",
    steps: &MARKET_AFTER_REPORT_STEPS,
    expectations: ScenarioExpectations {
        final_world_time: Some(16),
        required_character_resources: &[market_resource("coin", 7), market_resource("stamina", 1)],
        required_character_inventory: &[
            market_item("rope", 1),
            market_item("fume_yards.filter", 2),
        ],
        required_location_flags: &[
            (SALVAGE_ASH, "fume_yards.collateral_settled"),
            (SALVAGE_ASH, "fume_yards.daro_returned_to_cage"),
            (SALVAGE_ASH, "fume_yards.rack_braced"),
            (SALVAGE_ASH, "fume_yards.rack_cleared"),
            (SALVAGE_ASH, "fume_yards.rack_rear_open"),
            (SALVAGE_ASH, "fume_yards.report_paid"),
        ],
        forbidden_location_flags: &market_append::<_, 11>(
            MARKET_NO_FIRE,
            &[
                (SALVAGE_BAY, "fume_yards.dust_filter_fitted"),
                (MARKET_RETURN, "fume_yards.market_water_ordered"),
                (MARKET_RETURN, "fume_yards.market_filter_fitted"),
                (MARKET_RETURN, "fume_yards.market_cask_delivered"),
                (MARKET_RETURN, "fume_yards.market_cask_installed"),
                (MARKET_RETURN, "fume_yards.clean_water_drawn"),
            ],
        ),
        required_npc_inventory: SALVAGE_UNSPENT_STOCK,
        forbidden_npc_inventory: SALVAGE_EMPTY_RACK,
        required_npc_knowledge: &[
            salvage_knowledge(11),
            market_knowledge(SALVAGE_DARO, "fume_yards.collateral_settled", 15),
            ScenarioNpcKnowledgeExpectation {
                npc: MARKET_BRANN,
                knowledge_id: "fume_yards.rack_cleared",
                turn: 12,
                provenance: ScenarioKnowledgeProvenance::Told { by: SALVAGE_DARO },
            },
        ],
        forbidden_npc_knowledge: &market_append::<_, 7>(
            MARKET_NO_REMOTE_TRADE,
            &[
                SALVAGE_REMOTE_UNINFORMED[1],
                SALVAGE_REMOTE_UNINFORMED[2],
                SALVAGE_REMOTE_UNINFORMED[3],
            ],
        ),
        required_npc_memories: &[
            batch_memory(SALVAGE_DARO, "fume_yards.rack_cleared", 11),
            batch_memory(SALVAGE_DARO, "fume_yards.rack_reported", 12),
            batch_memory(MARKET_BRANN, "fume_yards.rack_report_paid", 12),
            batch_memory(SALVAGE_DARO, "fume_yards.returned_to_cage", 14),
            batch_memory(SALVAGE_DARO, "fume_yards.collateral_paid", 15),
        ],
        storage_transfers: &[market_transfer(
            15,
            StorageTransferDirection::ToCharacter,
            "fume_yards.filter",
        )],
        required_legal_definitions: &["fume_yards.leave_ash_hatch"],
        forbidden_legal_definitions: &market_append::<_, 7>(
            MARKET_CLOSED_TRADE,
            SALVAGE_RECOVERY_ACTIONS,
        ),
        ..MARKET_BASE
    },
    ..MARKET_PURCHASE_SPEC
};

#[cfg(test)]
mod market_oracle_tests {
    use super::*;
    use forge_kernel::EventKind;

    #[test]
    fn market_nine_literal_paths_and_quantities_replay() {
        let content = crate::load_content().unwrap();
        for spec in [
            MARKET_PURCHASE_SPEC,
            MARKET_FUEL_SPEC,
            MARKET_LOCAL_SPEC,
            MARKET_SALE_SPEC,
            MARKET_WATER_SPEC,
            MARKET_COMPOSED_SPEC,
            MARKET_CASK_DELIVERED_SPEC,
            MARKET_CASK_UNDELIVERED_SPEC,
            MARKET_AFTER_REPORT_SPEC,
        ] {
            validate_specs(&[spec]).unwrap_or_else(|error| panic!("{}: {error}", spec.id));
            run(&spec, &content).unwrap_or_else(|error| panic!("{}: {error}", spec.id));
        }
    }

    #[test]
    fn market_storage_oracle_rejects_wrong_balance_direction_order_and_missing_records() {
        let content = crate::load_content().unwrap();
        let session = run(&MARKET_FUEL_SPEC, &content).unwrap();
        let expected = MARKET_FUEL_SPEC.expectations;
        validate_storage_history(
            session.state(),
            expected.storage_balances,
            expected.storage_transfers,
        )
        .unwrap();
        let indices = session
            .state()
            .event_log
            .iter()
            .enumerate()
            .filter_map(|(index, event)| {
                matches!(
                    event.kind,
                    EventKind::CharacterItemTransferredToStorage { .. }
                        | EventKind::StorageItemTransferredToCharacter { .. }
                )
                .then_some(index)
            })
            .collect::<Vec<_>>();
        assert_eq!(indices.len(), 2);
        for mutation in 0..11 {
            let mut state = session.state().clone();
            match mutation {
                0 => {
                    state.world.storages.remove(MARKET_CAGE);
                }
                1 => {
                    state
                        .world
                        .storages
                        .get_mut(MARKET_CAGE)
                        .unwrap()
                        .inventory
                        .clear();
                }
                2 => {
                    state
                        .world
                        .storages
                        .get_mut(MARKET_CAGE)
                        .unwrap()
                        .inventory
                        .insert("fume_yards.filter".to_owned(), 1);
                }
                3 => {
                    state
                        .world
                        .storages
                        .get_mut(MARKET_CAGE)
                        .unwrap()
                        .inventory
                        .insert("fume_yards.fuel".to_owned(), 2);
                }
                4 => {
                    state.event_log[indices[0]].kind =
                        EventKind::StorageItemTransferredToCharacter {
                            storage: MARKET_CAGE.to_owned(),
                            item: "fume_yards.fuel".to_owned(),
                            count: 1,
                        }
                }
                5 => state.event_log[indices[0]].turn = 11,
                6 => {
                    state.event_log[indices[0]].kind =
                        EventKind::CharacterItemTransferredToStorage {
                            storage: MARKET_CAGE.to_owned(),
                            item: "fume_yards.fuel".to_owned(),
                            count: 2,
                        }
                }
                7 => {
                    state.event_log[indices[0]].kind =
                        EventKind::CharacterItemTransferredToStorage {
                            storage: "fume_yards.other_cage".to_owned(),
                            item: "fume_yards.fuel".to_owned(),
                            count: 1,
                        }
                }
                8 => state.event_log.swap(indices[0], indices[1]),
                9 => {
                    state.event_log.remove(indices[0]);
                }
                _ => {
                    let duplicate = state.event_log[indices[1]].clone();
                    state.event_log.push(duplicate);
                }
            }
            assert!(
                validate_storage_history(
                    &state,
                    expected.storage_balances,
                    expected.storage_transfers
                )
                .is_err(),
                "mutation {mutation}"
            );
        }
    }

    #[test]
    fn market_price_fuel_and_rack_resurrection_claims_are_independent() {
        let content = crate::load_content().unwrap();
        let purchased = run(&MARKET_PURCHASE_SPEC, &content).unwrap();
        let fueled = run(&MARKET_FUEL_SPEC, &content).unwrap();
        assert_eq!(purchased.state().character.resources["coin"], 6);
        assert_eq!(fueled.state().character.resources["coin"], 10);
        assert_eq!(
            purchased.state().world.npcs[SALVAGE_DARO].inventory["fume_yards.filter"],
            1
        );
        assert_eq!(
            fueled.state().world.npcs[SALVAGE_DARO].inventory["fume_yards.filter"],
            1
        );
        assert!(
            !fueled
                .state()
                .character
                .inventory
                .contains_key("fume_yards.fuel")
        );
        assert!(
            validate_character_resources(
                purchased.state(),
                MARKET_FUEL_SPEC.expectations.required_character_resources
            )
            .is_err()
        );
        assert!(
            validate_storage_history(fueled.state(), MARKET_EMPTY_CAGE, MARKET_PURCHASE_TRANSFER)
                .is_err()
        );
        let mut free = purchased.state().clone();
        free.character.resources.insert("coin".to_owned(), 10);
        assert!(
            validate_character_resources(&free, MARKET_BASE.required_character_resources).is_err()
        );
        let mut no_fuel_cost = fueled.state().clone();
        no_fuel_cost
            .world
            .npcs
            .get_mut(MARKET_BRANN)
            .unwrap()
            .inventory
            .insert("fume_yards.fuel".to_owned(), 1);
        assert!(
            validate_inventory(
                &no_fuel_cost,
                MARKET_FUEL_SPEC.expectations.required_character_inventory,
                MARKET_FUEL_SPEC.expectations.forbidden_npc_inventory
            )
            .is_err()
        );
        let reported = run(&MARKET_AFTER_REPORT_SPEC, &content).unwrap();
        let mut resurrected = reported.state().clone();
        resurrected
            .world
            .npcs
            .get_mut(SALVAGE_DARO)
            .unwrap()
            .inventory
            .insert("fume_yards.filter".to_owned(), 1);
        assert!(
            validate_inventory(
                &resurrected,
                MARKET_AFTER_REPORT_SPEC
                    .expectations
                    .required_character_inventory,
                MARKET_AFTER_REPORT_SPEC
                    .expectations
                    .forbidden_npc_inventory
            )
            .is_err()
        );
        assert_eq!(reported.state().character.inventory["fume_yards.filter"], 2);
        assert_eq!(
            reported.state().world.npcs[MARKET_BRANN].knowledge["fume_yards.rack_cleared"].turn,
            12
        );
    }

    #[test]
    fn market_three_destinations_bind_distinct_consumption_and_exact_single_water_gain() {
        let content = crate::load_content().unwrap();
        let local = run(&MARKET_LOCAL_SPEC, &content).unwrap();
        let sale = run(&MARKET_SALE_SPEC, &content).unwrap();
        let water = run(&MARKET_WATER_SPEC, &content).unwrap();
        assert_eq!(
            (
                local.state().character.resources["coin"],
                local.state().character.resources["stamina"]
            ),
            (11, 1)
        );
        assert_eq!(
            (
                sale.state().character.resources["coin"],
                sale.state().character.resources["stamina"]
            ),
            (12, 1)
        );
        assert_eq!(
            (
                water.state().character.resources["coin"],
                water.state().character.resources["stamina"]
            ),
            (8, 3)
        );
        for (session, spec) in [(&local, MARKET_LOCAL_SPEC), (&sale, MARKET_SALE_SPEC)] {
            assert!(
                validate_recipe_events(
                    session.state(),
                    MARKET_WATER_SPEC.expectations.recipe_events
                )
                .is_err()
            );
            let mut false_consumer = spec;
            false_consumer.expectations.required_location_flags =
                &[(MARKET_RETURN, "fume_yards.market_filter_fitted")];
            assert!(validate_session(&false_consumer, session, &content).is_err());
        }
        let mut missing_filter = water.state().clone();
        missing_filter.event_log.retain(|event| !matches!(&event.kind, EventKind::RecipeApplied { recipe, .. } if recipe == "fume_yards.fit_market_filter"));
        assert!(
            validate_recipe_events(
                &missing_filter,
                MARKET_WATER_SPEC.expectations.recipe_events
            )
            .is_err()
        );
        let mut no_water = MARKET_WATER_SPEC;
        no_water.expectations.forbidden_location_flags =
            &[(MARKET_RETURN, "fume_yards.clean_water_drawn")];
        assert!(validate_session(&no_water, &water, &content).is_err());
        let gains = water
            .state()
            .event_log
            .iter()
            .filter(|event| {
                matches!(&event.kind,
            EventKind::ResourceAdjusted { resource, amount: 2 } if resource == "stamina")
            })
            .collect::<Vec<_>>();
        assert_eq!(gains.len(), 1);
        assert_eq!(gains[0].turn, 23);
        assert!(
            enumerate_legal_actions(water.state(), &content)
                .unwrap()
                .iter()
                .all(|action| action.definition_id != "return.draw_clean_water")
        );
        let composed = run(&MARKET_COMPOSED_SPEC, &content).unwrap();
        assert!(
            composed.state().world.npcs[SALVAGE_DARO]
                .inventory
                .is_empty()
        );
        assert_eq!(
            composed.state().world.npcs[MARKET_BRANN].inventory["fume_yards.fuel"],
            1
        );
        assert!(
            composed.state().world.npcs[MARKET_PERA]
                .inventory
                .is_empty()
        );
        assert!(
            composed.state().world.locations[SALVAGE_BAY]
                .flags
                .contains("fume_yards.cold_work_chosen")
        );
        assert!(
            !composed.state().world.locations[SALVAGE_BAY]
                .flags
                .contains("fume_yards.batch_ignited")
        );
    }

    #[test]
    fn market_cask_report_requires_physical_source_and_preserves_its_original_time() {
        let content = crate::load_content().unwrap();
        let delivered = run(&MARKET_CASK_DELIVERED_SPEC, &content).unwrap();
        let undelivered = run(&MARKET_CASK_UNDELIVERED_SPEC, &content).unwrap();
        let expected = MARKET_CASK_DELIVERED_SPEC.expectations;
        assert_eq!(delivered.state().world.time, 22);
        assert_eq!(undelivered.state().world.time, 22);
        assert_eq!(
            delivered.state().world.current_location,
            undelivered.state().world.current_location
        );
        assert_eq!(
            delivered.state().character.inventory,
            undelivered.state().character.inventory
        );
        assert_eq!(
            delivered.state().character.resources,
            undelivered.state().character.resources
        );
        assert!(
            validate_npc_locations(undelivered.state(), expected.required_npc_locations).is_err()
        );
        assert!(
            validate_npc_knowledge(
                undelivered.state(),
                expected.required_npc_knowledge,
                expected.forbidden_npc_knowledge
            )
            .is_err()
        );
        assert!(
            validate_npc_memories(undelivered.state(), expected.required_npc_memories).is_err()
        );
        for mutation in 0..3 {
            let mut changed = delivered.state().clone();
            match mutation {
                0 => {
                    changed
                        .world
                        .npcs
                        .get_mut(MARKET_PERA)
                        .unwrap()
                        .knowledge
                        .get_mut("fume_yards.market_cask")
                        .unwrap()
                        .turn = 20
                }
                1 => {
                    changed
                        .world
                        .npcs
                        .get_mut("oren_pell")
                        .unwrap()
                        .knowledge
                        .get_mut("fume_yards.market_cask")
                        .unwrap()
                        .provenance = forge_kernel::KnowledgeProvenance::Witnessed
                }
                _ => {
                    changed
                        .world
                        .npcs
                        .get_mut("oren_pell")
                        .unwrap()
                        .knowledge
                        .get_mut("fume_yards.market_cask")
                        .unwrap()
                        .provenance = forge_kernel::KnowledgeProvenance::Told {
                        by: SALVAGE_DARO.to_owned(),
                    }
                }
            }
            assert!(
                validate_npc_knowledge(
                    &changed,
                    expected.required_npc_knowledge,
                    expected.forbidden_npc_knowledge
                )
                .is_err()
            );
        }
        let mut remote_leak = undelivered.state().clone();
        remote_leak
            .world
            .npcs
            .get_mut("oren_pell")
            .unwrap()
            .knowledge
            .insert(
                "fume_yards.market_cask".to_owned(),
                delivered.state().world.npcs["oren_pell"].knowledge["fume_yards.market_cask"]
                    .clone(),
            );
        assert!(
            validate_npc_knowledge(
                &remote_leak,
                MARKET_CASK_UNDELIVERED_SPEC
                    .expectations
                    .required_npc_knowledge,
                MARKET_CASK_UNDELIVERED_SPEC
                    .expectations
                    .forbidden_npc_knowledge
            )
            .is_err()
        );
        let water = run(&MARKET_WATER_SPEC, &content).unwrap();
        for npc in [MARKET_PERA, "oren_pell"] {
            assert_eq!(
                water.state().world.npcs[npc].knowledge["fume_yards.market_cask"],
                delivered.state().world.npcs[npc].knowledge["fume_yards.market_cask"]
            );
        }
    }
}
