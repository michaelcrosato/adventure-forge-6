// Literal reviewed paths and outcomes, independent of reducer branch selection.
const SALVAGE_ASH: &str = "fume_yards.ash_beds";
const SALVAGE_DARO: &str = "fume_yards.daro_venn";
const SALVAGE_BAY: &str = "fume_yards.kiln_bay";
const SALVAGE_FRONT_STEPS: [ScenarioStep; 9] = append_steps(
    HOLD_MARKET_STEPS,
    &[
        action!("return.visit_workshop"),
        action!("travel_adjacent", "destination" => "fume_yards.ash_beds"),
    ],
);
const SALVAGE_SAFE_STEPS: [ScenarioStep; 11] = append_steps(
    &SALVAGE_FRONT_STEPS,
    &[
        action!("fume_yards.brace_rack"),
        action!("fume_yards.recover_braced_filter"),
    ],
);
const SALVAGE_SALE_STEPS: [ScenarioStep; 13] = append_steps(
    &SALVAGE_SAFE_STEPS,
    &[
        action!("world.enter_aftermath"),
        action!("return.sell_filter"),
    ],
);
const SALVAGE_LOCAL_STEPS: [ScenarioStep; 15] = append_steps(
    &SALVAGE_SAFE_STEPS,
    &[
        action!("travel_adjacent", "destination" => "fume_yards.workshop"),
        action!("travel_adjacent", "destination" => "fume_yards.kiln_bay"),
        action!("fume_yards.fit_dust_filter"),
        action!("fume_yards.load_cold_freight"),
    ],
);
const SALVAGE_PULL_STEPS: [ScenarioStep; 10] = append_steps(
    &SALVAGE_FRONT_STEPS,
    &[action!("fume_yards.pull_rack_filter")],
);
const SALVAGE_SKILLED_STEPS: &[ScenarioStep] = &[
    action!("checkpoint.blend_workers"),
    action!("travel_adjacent", "destination" => "lowsail.levee"),
    action!("travel_adjacent", "destination" => "fume_yards.workshop"),
    action!("travel_adjacent", "destination" => "fume_yards.kiln_bay"),
    action!("fume_yards.enter_ash_hatch"),
    action!("fume_yards.thread_rack_filter"),
];
const SALVAGE_PRIOR_STEPS: [ScenarioStep; 19] = append_steps(
    &BATCH_DRAWN_STEPS,
    &[
        action!("fume_yards.enter_ash_hatch"),
        action!("fume_yards.pull_rack_filter"),
    ],
);
const SALVAGE_COMPOSED_STEPS: [ScenarioStep; 23] = append_steps(
    &SALVAGE_SAFE_STEPS,
    &[
        action!("travel_adjacent", "destination" => "fume_yards.workshop"),
        action!("fume_yards.take_stock"),
        action!("travel_adjacent", "destination" => "fume_yards.kiln_bay"),
        action!("fume_yards.fit_dust_filter"),
        action!("fume_yards.take_fuel"),
        action!("fume_yards.prepare_charge"),
        action!("fume_yards.ignite_batch"),
        action!("wait_tide"),
        action!("fume_yards.draw_filter"),
        action!("fume_yards.load_filtered_kiln_freight"),
        action!("world.enter_aftermath"),
        action!("return.sell_filter"),
    ],
);
const SALVAGE_REAR_STEPS: [ScenarioStep; 12] = append_steps(
    HOLD_MARKET_STEPS,
    &[
        action!("return.visit_workshop"),
        action!("travel_adjacent", "destination" => "fume_yards.kiln_bay"),
        action!("fume_yards.enter_ash_hatch"),
        action!("fume_yards.brace_rack"),
        action!("fume_yards.recover_braced_filter"),
    ],
);
const SALVAGE_REPORTED_STEPS: [ScenarioStep; 14] = append_steps(
    &SALVAGE_REAR_STEPS,
    &[action!("fume_yards.report_with_daro"), action!("wait_tide")],
);
const SALVAGE_UNREPORTED_STEPS: [ScenarioStep; 14] = append_steps(
    &SALVAGE_REAR_STEPS,
    &[action!("fume_yards.leave_ash_hatch"), action!("wait_tide")],
);
const SALVAGE_RECOVERY_ACTIONS: &[&str] = &[
    "fume_yards.brace_rack",
    "fume_yards.recover_braced_filter",
    "fume_yards.thread_rack_filter",
    "fume_yards.pull_rack_filter",
];
const SALVAGE_UNSPENT_STOCK: &[ScenarioNpcInventoryExpectation] = &[
    ScenarioNpcInventoryExpectation {
        npc: "fume_yards.nessa_tern",
        item: "fume_yards.clay",
        count: 2,
    },
    ScenarioNpcInventoryExpectation {
        npc: "fume_yards.nessa_tern",
        item: "fume_yards.mesh",
        count: 1,
    },
    ScenarioNpcInventoryExpectation {
        npc: "fume_yards.brann_coil",
        item: "fume_yards.fuel",
        count: 1,
    },
    ScenarioNpcInventoryExpectation {
        npc: "fume_yards.pera_senn",
        item: "fume_yards.water_cask",
        count: 1,
    },
];
const SALVAGE_EMPTY_RACK: &[ScenarioNpcInventoryAbsence] = &[ScenarioNpcInventoryAbsence {
    npc: SALVAGE_DARO,
    item: "fume_yards.filter",
}];
const SALVAGE_REMOTE_UNINFORMED: &[ScenarioNpcKnowledgeAbsence] = &[
    ScenarioNpcKnowledgeAbsence {
        npc: "fume_yards.brann_coil",
        knowledge_id: "fume_yards.rack_cleared",
    },
    ScenarioNpcKnowledgeAbsence {
        npc: "fume_yards.pera_senn",
        knowledge_id: "fume_yards.rack_cleared",
    },
    ScenarioNpcKnowledgeAbsence {
        npc: "fume_yards.nessa_tern",
        knowledge_id: "fume_yards.rack_cleared",
    },
    ScenarioNpcKnowledgeAbsence {
        npc: "oren_pell",
        knowledge_id: "fume_yards.rack_cleared",
    },
];
const fn salvage_knowledge(turn: u64) -> ScenarioNpcKnowledgeExpectation {
    ScenarioNpcKnowledgeExpectation {
        npc: SALVAGE_DARO,
        knowledge_id: "fume_yards.rack_cleared",
        provenance: ScenarioKnowledgeProvenance::Witnessed,
        turn,
    }
}
const SALVAGE_BASE: ScenarioExpectations = ScenarioExpectations {
    final_location: SALVAGE_ASH,
    final_action_definition: "fume_yards.recover_braced_filter",
    final_observation_contains: "You recover the filter intact",
    forbidden_observation_contains: &[],
    final_world_time: Some(11),
    exclusive_after_action: "",
    required_world_flags: &["ending_council", "flow_locked_market"],
    forbidden_world_flags: &["surge_missed", "sluice_failure"],
    required_location_flags: &[
        (SALVAGE_ASH, "fume_yards.rack_braced"),
        (SALVAGE_ASH, "fume_yards.rack_cleared"),
    ],
    forbidden_location_flags: &[
        (SALVAGE_BAY, "fume_yards.dust_filter_fitted"),
        (SALVAGE_BAY, "fume_yards.batch_ignited"),
        (SALVAGE_ASH, "fume_yards.report_paid"),
    ],
    required_deeds: &[],
    required_visited_locations: &["fume_yards.workshop", SALVAGE_ASH],
    required_npc_locations: &[
        ScenarioNpcLocationExpectation {
            npc: SALVAGE_DARO,
            location: SALVAGE_ASH,
        },
        ScenarioNpcLocationExpectation {
            npc: "fume_yards.brann_coil",
            location: SALVAGE_BAY,
        },
    ],
    required_npc_knowledge: &[salvage_knowledge(10)],
    forbidden_npc_knowledge: SALVAGE_REMOTE_UNINFORMED,
    required_npc_memories: &[batch_memory(SALVAGE_DARO, "fume_yards.rack_cleared", 10)],
    required_character_inventory: &[
        ScenarioInventoryExpectation {
            item: "rope",
            count: 1,
        },
        ScenarioInventoryExpectation {
            item: "fume_yards.filter",
            count: 1,
        },
    ],
    required_character_resources: &[
        ScenarioResourceExpectation {
            resource: "coin",
            amount: 10,
        },
        ScenarioResourceExpectation {
            resource: "stamina",
            amount: 1,
        },
    ],
    required_npc_inventory: SALVAGE_UNSPENT_STOCK,
    forbidden_npc_inventory: SALVAGE_EMPTY_RACK,
    forbidden_character_inventory: &[
        "fume_yards.shard",
        "fume_yards.batch_claim",
        "fume_yards.prepared_charge",
    ],
    recipe_events: &[],
    storage_balances: UNTOUCHED_COLLATERAL,
    storage_transfers: &[],
    entropy_draws: &[],
    deferred_events: &[],
    pending_deferred_events: &[],
    required_legal_definitions: &[],
    forbidden_legal_definitions: SALVAGE_RECOVERY_ACTIONS,
};
const SALVAGE_SAFE_SALE_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-salvage-safe-sale",
    claim_id: "milestone-2.salvage.safe-sale",
    start: ScenarioStartSpec::Preset {
        character_preset_id: "ilyan",
    },
    seed: 71,
    steps: &SALVAGE_SALE_STEPS,
    expectations: ScenarioExpectations {
        final_observation_contains: "Oren buys your filter for four coins.",
        final_location: "lowsail.return",
        final_action_definition: "return.sell_filter",
        final_world_time: Some(13),
        required_character_inventory: &[ScenarioInventoryExpectation {
            item: "rope",
            count: 1,
        }],
        forbidden_character_inventory: &[
            "fume_yards.filter",
            "fume_yards.shard",
            "fume_yards.batch_claim",
            "fume_yards.prepared_charge",
        ],
        required_character_resources: &[
            ScenarioResourceExpectation {
                resource: "coin",
                amount: 14,
            },
            ScenarioResourceExpectation {
                resource: "stamina",
                amount: 1,
            },
        ],
        required_location_flags: &[
            (SALVAGE_ASH, "fume_yards.rack_braced"),
            (SALVAGE_ASH, "fume_yards.rack_cleared"),
            ("lowsail.return", "fume_yards.filter_sold"),
        ],
        required_npc_memories: &[
            batch_memory(SALVAGE_DARO, "fume_yards.rack_cleared", 10),
            batch_memory("oren_pell", "fume_yards.filter_bought", 12),
        ],
        recipe_events: &[batch_recipe(
            12,
            "fume_yards.sell_filter",
            &[("fume_yards.filter", 1)],
            &[],
        )],
        forbidden_legal_definitions: &["return.sell_filter"],
        ..SALVAGE_BASE
    },
};
const SALVAGE_SAFE_LOCAL_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-salvage-safe-local",
    claim_id: "milestone-2.salvage.safe-local",
    steps: &SALVAGE_LOCAL_STEPS,
    expectations: ScenarioExpectations {
        final_observation_contains: "cold work closes firing.",
        final_location: SALVAGE_BAY,
        final_action_definition: "fume_yards.load_cold_freight",
        final_world_time: Some(15),
        required_character_inventory: &[ScenarioInventoryExpectation {
            item: "rope",
            count: 1,
        }],
        forbidden_character_inventory: &[
            "fume_yards.filter",
            "fume_yards.shard",
            "fume_yards.batch_claim",
            "fume_yards.prepared_charge",
        ],
        required_character_resources: &[
            ScenarioResourceExpectation {
                resource: "coin",
                amount: 13,
            },
            ScenarioResourceExpectation {
                resource: "stamina",
                amount: 1,
            },
        ],
        required_location_flags: &[
            (SALVAGE_ASH, "fume_yards.rack_braced"),
            (SALVAGE_ASH, "fume_yards.rack_cleared"),
            (SALVAGE_BAY, "fume_yards.dust_filter_fitted"),
            (SALVAGE_BAY, "fume_yards.kiln_closed"),
            (SALVAGE_BAY, "fume_yards.cold_work_chosen"),
            (SALVAGE_BAY, "fume_yards.kiln_freight_loaded"),
        ],
        forbidden_location_flags: &[
            (SALVAGE_BAY, "fume_yards.batch_ignited"),
            ("lowsail.return", "fume_yards.filter_sold"),
            (SALVAGE_ASH, "fume_yards.report_paid"),
        ],
        recipe_events: &[batch_recipe(
            13,
            "fume_yards.fit_dust_filter",
            &[("fume_yards.filter", 1)],
            &[],
        )],
        forbidden_legal_definitions: &[
            "fume_yards.load_cold_freight",
            "fume_yards.load_kiln_freight",
            "fume_yards.load_filtered_kiln_freight",
            "fume_yards.prepare_charge",
            "fume_yards.ignite_batch",
            "fume_yards.fit_dust_filter",
        ],
        ..SALVAGE_BASE
    },
    ..SALVAGE_SAFE_SALE_SPEC
};
const SALVAGE_SKILLED_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-salvage-skilled",
    claim_id: "milestone-2.salvage.skilled",
    start: ScenarioStartSpec::Preset {
        character_preset_id: "rook",
    },
    steps: SALVAGE_SKILLED_STEPS,
    expectations: ScenarioExpectations {
        final_observation_contains: "The filter comes free; you retain the rope.",
        final_action_definition: "fume_yards.thread_rack_filter",
        final_world_time: Some(6),
        required_world_flags: &[],
        required_location_flags: &[
            (SALVAGE_ASH, "fume_yards.rack_rear_open"),
            (SALVAGE_ASH, "fume_yards.rack_cleared"),
        ],
        forbidden_location_flags: &[
            (SALVAGE_ASH, "fume_yards.rack_braced"),
            (SALVAGE_ASH, "fume_yards.report_paid"),
            (SALVAGE_BAY, "fume_yards.batch_ignited"),
        ],
        required_npc_knowledge: &[salvage_knowledge(5)],
        required_npc_memories: &[batch_memory(SALVAGE_DARO, "fume_yards.rack_cleared", 5)],
        required_character_inventory: &[
            ScenarioInventoryExpectation {
                item: "rope",
                count: 1,
            },
            ScenarioInventoryExpectation {
                item: "wire",
                count: 1,
            },
            ScenarioInventoryExpectation {
                item: "fume_yards.filter",
                count: 1,
            },
        ],
        required_character_resources: &[
            ScenarioResourceExpectation {
                resource: "coin",
                amount: 5,
            },
            ScenarioResourceExpectation {
                resource: "stamina",
                amount: 4,
            },
        ],
        ..SALVAGE_BASE
    },
    ..SALVAGE_SAFE_SALE_SPEC
};
const SALVAGE_INTACT_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-salvage-intact",
    claim_id: "milestone-2.salvage.intact",
    seed: 27,
    steps: &SALVAGE_PULL_STEPS,
    expectations: ScenarioExpectations {
        final_observation_contains: "the filter comes free intact.",
        final_action_definition: "fume_yards.pull_rack_filter",
        final_world_time: Some(10),
        required_location_flags: &[(SALVAGE_ASH, "fume_yards.rack_cleared")],
        forbidden_location_flags: &[
            (SALVAGE_ASH, "fume_yards.rack_braced"),
            (SALVAGE_BAY, "fume_yards.dust_filter_fitted"),
            (SALVAGE_BAY, "fume_yards.batch_ignited"),
            (SALVAGE_ASH, "fume_yards.report_paid"),
        ],
        required_npc_knowledge: &[salvage_knowledge(9)],
        required_npc_memories: &[batch_memory(SALVAGE_DARO, "fume_yards.rack_cleared", 9)],
        required_character_resources: &[
            ScenarioResourceExpectation {
                resource: "coin",
                amount: 10,
            },
            ScenarioResourceExpectation {
                resource: "stamina",
                amount: 3,
            },
        ],
        storage_balances: UNTOUCHED_COLLATERAL,
        storage_transfers: &[],
        entropy_draws: &[EntropyExpectation {
            turn: 9,
            algorithm: "splitmix64-v1",
            cursor: 0,
            value: 10902710238276814474,
        }],
        ..SALVAGE_BASE
    },
    ..SALVAGE_SAFE_SALE_SPEC
};
const SALVAGE_BROKEN_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-salvage-broken",
    claim_id: "milestone-2.salvage.broken",
    seed: 123,
    expectations: ScenarioExpectations {
        final_observation_contains: "The rack filter broke",
        required_character_inventory: &[
            ScenarioInventoryExpectation {
                item: "rope",
                count: 1,
            },
            ScenarioInventoryExpectation {
                item: "fume_yards.shard",
                count: 1,
            },
        ],
        forbidden_character_inventory: &[
            "fume_yards.filter",
            "fume_yards.batch_claim",
            "fume_yards.prepared_charge",
        ],
        recipe_events: &[batch_recipe(
            9,
            "fume_yards.break_filter",
            &[("fume_yards.filter", 1)],
            &[("fume_yards.shard", 1)],
        )],
        storage_balances: UNTOUCHED_COLLATERAL,
        storage_transfers: &[],
        entropy_draws: &[EntropyExpectation {
            turn: 9,
            algorithm: "splitmix64-v1",
            cursor: 0,
            value: 13032462758197477675,
        }],
        ..SALVAGE_INTACT_SPEC.expectations
    },
    ..SALVAGE_INTACT_SPEC
};
const SALVAGE_PRIOR_FILTER_BROKEN_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-salvage-prior-filter-broken",
    claim_id: "milestone-2.salvage.prior-filter-broken",
    steps: &SALVAGE_PRIOR_STEPS,
    expectations: ScenarioExpectations {
        final_world_time: Some(19),
        required_location_flags: &[
            (SALVAGE_ASH, "fume_yards.rack_rear_open"),
            (SALVAGE_ASH, "fume_yards.rack_cleared"),
            (SALVAGE_BAY, "fume_yards.batch_drawn"),
        ],
        forbidden_location_flags: &[
            (SALVAGE_ASH, "fume_yards.rack_braced"),
            (SALVAGE_BAY, "fume_yards.dust_filter_fitted"),
            (SALVAGE_BAY, "fume_yards.batch_active"),
            (SALVAGE_BAY, "fume_yards.batch_spoiled"),
            (SALVAGE_ASH, "fume_yards.report_paid"),
        ],
        required_character_inventory: &[
            ScenarioInventoryExpectation {
                item: "rope",
                count: 1,
            },
            ScenarioInventoryExpectation {
                item: "fume_yards.filter",
                count: 1,
            },
            ScenarioInventoryExpectation {
                item: "fume_yards.shard",
                count: 1,
            },
        ],
        forbidden_character_inventory: &[
            "fume_yards.batch_claim",
            "fume_yards.prepared_charge",
            "fume_yards.fuel",
            "fume_yards.water_cask",
        ],
        required_npc_inventory: &[],
        forbidden_npc_inventory: &[
            BATCH_EMPTY_STOCK[0],
            BATCH_EMPTY_STOCK[1],
            BATCH_EMPTY_STOCK[2],
            BATCH_EMPTY_STOCK[3],
            SALVAGE_EMPTY_RACK[0],
        ],
        required_npc_knowledge: &[salvage_knowledge(18)],
        required_npc_memories: &[batch_memory(SALVAGE_DARO, "fume_yards.rack_cleared", 18)],
        recipe_events: &[
            BATCH_BASE_RECIPES[0],
            BATCH_BASE_RECIPES[1],
            BATCH_BASE_RECIPES[2],
            batch_recipe(
                16,
                "fume_yards.draw_filter",
                &[("fume_yards.batch_claim", 1)],
                &[("fume_yards.filter", 1)],
            ),
            batch_recipe(
                18,
                "fume_yards.break_filter",
                &[("fume_yards.filter", 1)],
                &[("fume_yards.shard", 1)],
            ),
        ],
        deferred_events: &[
            batch_schedule(14, "fume_yards.batch_ready", 16),
            batch_schedule(14, "fume_yards.batch_spoil", 19),
            batch_resolved(16, "fume_yards.batch_ready", true),
            batch_resolved(19, "fume_yards.batch_spoil", false),
        ],
        storage_balances: UNTOUCHED_COLLATERAL,
        storage_transfers: &[],
        entropy_draws: &[EntropyExpectation {
            turn: 18,
            algorithm: "splitmix64-v1",
            cursor: 0,
            value: 13032462758197477675,
        }],
        ..SALVAGE_BROKEN_SPEC.expectations
    },
    ..SALVAGE_BROKEN_SPEC
};
const SALVAGE_PROTECTED_PRODUCTION_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-salvage-protected-production",
    claim_id: "milestone-2.salvage.protected-production",
    steps: &SALVAGE_COMPOSED_STEPS,
    expectations: ScenarioExpectations {
        final_world_time: Some(23),
        required_character_resources: &[
            ScenarioResourceExpectation {
                resource: "coin",
                amount: 17,
            },
            ScenarioResourceExpectation {
                resource: "stamina",
                amount: 1,
            },
        ],
        required_location_flags: &[
            (SALVAGE_ASH, "fume_yards.rack_cleared"),
            (SALVAGE_ASH, "fume_yards.rack_braced"),
            (SALVAGE_BAY, "fume_yards.dust_filter_fitted"),
            (SALVAGE_BAY, "fume_yards.batch_drawn"),
            (SALVAGE_BAY, "fume_yards.kiln_freight_loaded"),
            ("lowsail.return", "fume_yards.filter_sold"),
        ],
        forbidden_location_flags: &[
            (SALVAGE_BAY, "fume_yards.wet_screen_fitted"),
            (SALVAGE_BAY, "fume_yards.batch_active"),
            (SALVAGE_BAY, "fume_yards.batch_spoiled"),
            (SALVAGE_BAY, "fume_yards.cold_work_chosen"),
            (SALVAGE_ASH, "fume_yards.report_paid"),
        ],
        required_npc_inventory: &[SALVAGE_UNSPENT_STOCK[3]],
        forbidden_npc_inventory: &[
            BATCH_EMPTY_STOCK[0],
            BATCH_EMPTY_STOCK[1],
            BATCH_EMPTY_STOCK[2],
            SALVAGE_EMPTY_RACK[0],
        ],
        required_npc_memories: &[
            batch_memory(SALVAGE_DARO, "fume_yards.rack_cleared", 10),
            batch_memory("oren_pell", "fume_yards.filter_bought", 22),
        ],
        recipe_events: &[
            batch_recipe(
                14,
                "fume_yards.fit_dust_filter",
                &[("fume_yards.filter", 1)],
                &[],
            ),
            batch_recipe(
                16,
                "fume_yards.prepare_charge",
                PILOT_INPUTS,
                &[("fume_yards.prepared_charge", 1)],
            ),
            batch_recipe(
                17,
                "fume_yards.ignite_batch",
                &[("fume_yards.fuel", 1), ("fume_yards.prepared_charge", 1)],
                &[("fume_yards.batch_claim", 1)],
            ),
            batch_recipe(
                19,
                "fume_yards.draw_filter",
                &[("fume_yards.batch_claim", 1)],
                &[("fume_yards.filter", 1)],
            ),
            batch_recipe(
                22,
                "fume_yards.sell_filter",
                &[("fume_yards.filter", 1)],
                &[],
            ),
        ],
        deferred_events: &[
            batch_schedule(17, "fume_yards.batch_ready", 19),
            batch_schedule(17, "fume_yards.batch_spoil", 22),
            batch_resolved(19, "fume_yards.batch_ready", true),
            batch_resolved(22, "fume_yards.batch_spoil", false),
        ],
        ..SALVAGE_SAFE_SALE_SPEC.expectations
    },
    ..SALVAGE_SAFE_SALE_SPEC
};
const SALVAGE_UNREPORTED_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-salvage-unreported",
    claim_id: "milestone-2.salvage.unreported",
    steps: &SALVAGE_UNREPORTED_STEPS,
    expectations: ScenarioExpectations {
        final_observation_contains: "Fit your filter here: loading saves two stamina;",
        final_location: SALVAGE_BAY,
        final_action_definition: "wait_tide",
        final_world_time: Some(14),
        required_location_flags: &[
            (SALVAGE_ASH, "fume_yards.rack_braced"),
            (SALVAGE_ASH, "fume_yards.rack_cleared"),
            (SALVAGE_ASH, "fume_yards.rack_rear_open"),
        ],
        required_npc_knowledge: &[salvage_knowledge(11)],
        required_npc_memories: &[batch_memory(SALVAGE_DARO, "fume_yards.rack_cleared", 11)],
        ..SALVAGE_BASE
    },
    ..SALVAGE_SAFE_SALE_SPEC
};
const SALVAGE_REPORTED_SPEC: ScenarioSpec = ScenarioSpec {
    id: "m2-fume-salvage-reported",
    claim_id: "milestone-2.salvage.reported",
    steps: &SALVAGE_REPORTED_STEPS,
    expectations: ScenarioExpectations {
        required_location_flags: &[
            (SALVAGE_ASH, "fume_yards.rack_braced"),
            (SALVAGE_ASH, "fume_yards.rack_cleared"),
            (SALVAGE_ASH, "fume_yards.rack_rear_open"),
            (SALVAGE_ASH, "fume_yards.report_paid"),
        ],
        forbidden_location_flags: &[
            (SALVAGE_BAY, "fume_yards.dust_filter_fitted"),
            (SALVAGE_BAY, "fume_yards.batch_ignited"),
        ],
        required_npc_locations: &[
            ScenarioNpcLocationExpectation {
                npc: SALVAGE_DARO,
                location: SALVAGE_BAY,
            },
            ScenarioNpcLocationExpectation {
                npc: "fume_yards.brann_coil",
                location: SALVAGE_BAY,
            },
        ],
        required_npc_knowledge: &[
            salvage_knowledge(11),
            ScenarioNpcKnowledgeExpectation {
                npc: "fume_yards.brann_coil",
                knowledge_id: "fume_yards.rack_cleared",
                provenance: ScenarioKnowledgeProvenance::Told { by: SALVAGE_DARO },
                turn: 12,
            },
        ],
        forbidden_npc_knowledge: &[
            SALVAGE_REMOTE_UNINFORMED[1],
            SALVAGE_REMOTE_UNINFORMED[2],
            SALVAGE_REMOTE_UNINFORMED[3],
        ],
        required_npc_memories: &[
            batch_memory(SALVAGE_DARO, "fume_yards.rack_cleared", 11),
            batch_memory(SALVAGE_DARO, "fume_yards.rack_reported", 12),
            batch_memory("fume_yards.brann_coil", "fume_yards.rack_report_paid", 12),
        ],
        required_character_resources: &[
            ScenarioResourceExpectation {
                resource: "coin",
                amount: 11,
            },
            ScenarioResourceExpectation {
                resource: "stamina",
                amount: 1,
            },
        ],
        forbidden_legal_definitions: &["fume_yards.report_with_daro"],
        ..SALVAGE_UNREPORTED_SPEC.expectations
    },
    ..SALVAGE_UNREPORTED_SPEC
};

#[cfg(test)]
mod salvage_oracle_tests {
    use super::*;

    #[test]
    fn salvage_literal_paths_and_outcomes_replay() {
        let content = crate::load_content().unwrap();
        for spec in [
            SALVAGE_SAFE_SALE_SPEC,
            SALVAGE_SAFE_LOCAL_SPEC,
            SALVAGE_SKILLED_SPEC,
            SALVAGE_INTACT_SPEC,
            SALVAGE_BROKEN_SPEC,
            SALVAGE_PRIOR_FILTER_BROKEN_SPEC,
            SALVAGE_PROTECTED_PRODUCTION_SPEC,
            SALVAGE_REPORTED_SPEC,
            SALVAGE_UNREPORTED_SPEC,
        ] {
            validate_specs(&[spec]).unwrap();
            run(&spec, &content).unwrap_or_else(|error| panic!("{}: {error}", spec.id));
        }
    }

    #[test]
    fn salvage_report_oracle_rejects_wrong_source_time_remote_leak_and_payment() {
        let content = crate::load_content().unwrap();
        let reported = run(&SALVAGE_REPORTED_SPEC, &content).unwrap();
        let uninformed = run(&SALVAGE_UNREPORTED_SPEC, &content).unwrap();
        let expected = SALVAGE_REPORTED_SPEC.expectations;
        for mutation in 0..4 {
            let mut state = reported.state().clone();
            let knowledge = state
                .world
                .npcs
                .get_mut("fume_yards.brann_coil")
                .unwrap()
                .knowledge
                .get_mut("fume_yards.rack_cleared")
                .unwrap();
            match mutation {
                0 => knowledge.turn -= 1,
                1 => knowledge.provenance = forge_kernel::KnowledgeProvenance::Witnessed,
                2 => {
                    knowledge.provenance = forge_kernel::KnowledgeProvenance::Told {
                        by: "fume_yards.pera_senn".to_owned(),
                    }
                }
                _ => {
                    state
                        .world
                        .npcs
                        .get_mut(SALVAGE_DARO)
                        .unwrap()
                        .knowledge
                        .get_mut("fume_yards.rack_cleared")
                        .unwrap()
                        .turn = 12
                }
            }
            assert!(
                validate_npc_knowledge(
                    &state,
                    expected.required_npc_knowledge,
                    expected.forbidden_npc_knowledge
                )
                .is_err()
            );
        }
        let mut leaked = uninformed.state().clone();
        leaked.world.npcs.get_mut("fume_yards.brann_coil").unwrap().knowledge.insert(
            "fume_yards.rack_cleared".to_owned(), reported.state().world.npcs["fume_yards.brann_coil"].knowledge["fume_yards.rack_cleared"].clone());
        assert!(
            validate_npc_knowledge(
                &leaked,
                SALVAGE_UNREPORTED_SPEC.expectations.required_npc_knowledge,
                SALVAGE_REMOTE_UNINFORMED
            )
            .is_err()
        );
        assert!(
            validate_character_resources(uninformed.state(), expected.required_character_resources)
                .is_err()
        );
        assert!(
            validate_npc_locations(uninformed.state(), expected.required_npc_locations).is_err()
        );
        assert!(validate_npc_memories(uninformed.state(), expected.required_npc_memories).is_err());
    }

    #[test]
    fn salvage_chance_outcome_and_composed_stock_oracles_are_independent() {
        let content = crate::load_content().unwrap();
        let intact = run(&SALVAGE_INTACT_SPEC, &content).unwrap();
        let broken = run(&SALVAGE_BROKEN_SPEC, &content).unwrap();
        assert!(
            validate_inventory(
                intact.state(),
                SALVAGE_BROKEN_SPEC
                    .expectations
                    .required_character_inventory,
                SALVAGE_EMPTY_RACK
            )
            .is_err()
        );
        assert!(
            validate_recipe_events(
                intact.state(),
                SALVAGE_BROKEN_SPEC.expectations.recipe_events
            )
            .is_err()
        );
        assert!(
            validate_inventory(
                broken.state(),
                SALVAGE_INTACT_SPEC
                    .expectations
                    .required_character_inventory,
                SALVAGE_EMPTY_RACK
            )
            .is_err()
        );
        assert!(
            validate_entropy_history(
                broken.state(),
                SALVAGE_INTACT_SPEC.expectations.entropy_draws
            )
            .is_err()
        );
        let composed = run(&SALVAGE_PROTECTED_PRODUCTION_SPEC, &content).unwrap();
        let mut duplicate = composed.state().clone();
        duplicate
            .world
            .npcs
            .get_mut(SALVAGE_DARO)
            .unwrap()
            .inventory
            .insert("fume_yards.filter".to_owned(), 1);
        assert!(
            validate_inventory(
                &duplicate,
                SALVAGE_PROTECTED_PRODUCTION_SPEC
                    .expectations
                    .required_character_inventory,
                SALVAGE_PROTECTED_PRODUCTION_SPEC
                    .expectations
                    .forbidden_npc_inventory
            )
            .is_err()
        );
        assert!(
            validate_character_resources(
                composed.state(),
                SALVAGE_SAFE_SALE_SPEC
                    .expectations
                    .required_character_resources
            )
            .is_err()
        );
    }
}
