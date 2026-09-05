// These are reviewed action recipes and literal semantic expectations. They do
// not derive expected deadlines, stock, or outputs from the content under test.
const BATCH_IGNITION_EXTENSION: &[ScenarioStep] = &[
    action!("return.visit_workshop"),
    action!("fume_yards.take_stock"),
    action!("travel_adjacent", "destination" => "fume_yards.kiln_bay"),
    action!("fume_yards.take_cask"),
    action!("fume_yards.take_fuel"),
    action!("fume_yards.prepare_charge"),
    action!("fume_yards.fit_wet_screen"),
    action!("fume_yards.ignite_batch"),
];
const BATCH_IGNITED_STEPS: [ScenarioStep; 15] =
    append_steps(HOLD_MARKET_STEPS, BATCH_IGNITION_EXTENSION);
const BATCH_READY_STEPS: [ScenarioStep; 16] =
    append_steps(&BATCH_IGNITED_STEPS, &[action!("wait_tide")]);
const BATCH_DRAWN_STEPS: [ScenarioStep; 17] =
    append_steps(&BATCH_READY_STEPS, &[action!("fume_yards.draw_filter")]);
const BATCH_LOCAL_STEPS: [ScenarioStep; 19] = append_steps(
    &BATCH_DRAWN_STEPS,
    &[
        action!("fume_yards.fit_dust_filter"),
        action!("fume_yards.load_filtered_kiln_freight"),
    ],
);
const BATCH_SALE_STEPS: [ScenarioStep; 19] = append_steps(
    &BATCH_DRAWN_STEPS,
    &[
        action!("world.enter_aftermath"),
        action!("return.sell_filter"),
    ],
);
const BATCH_BARE_STEPS: [ScenarioStep; 18] = append_steps(
    &BATCH_DRAWN_STEPS,
    &[action!("fume_yards.load_kiln_freight")],
);
const BATCH_REMOTE_STEPS: [ScenarioStep; 19] = append_steps(
    &BATCH_IGNITED_STEPS,
    &[
        action!("travel_adjacent", "destination" => "fume_yards.workshop"),
        action!("travel_adjacent", "destination" => "lowsail.levee"),
        action!("travel_adjacent", "destination" => "lowsail_market"),
        action!("travel_adjacent", "destination" => "lowsail.docks"),
    ],
);
const BATCH_EARLY_IGNITED_STEPS: &[ScenarioStep] = &[
    action!("checkpoint.show_charter"),
    action!("travel_adjacent", "destination" => "lowsail.levee"),
    action!("travel_adjacent", "destination" => "fume_yards.workshop"),
    action!("fume_yards.take_stock"),
    action!("travel_adjacent", "destination" => "fume_yards.kiln_bay"),
    action!("fume_yards.take_cask"),
    action!("fume_yards.take_fuel"),
    action!("fume_yards.prepare_charge"),
    action!("fume_yards.fit_wet_screen"),
    action!("fume_yards.ignite_batch"),
];
const BATCH_BANK_STEPS: [ScenarioStep; 21] = append_steps(
    BATCH_EARLY_IGNITED_STEPS,
    &[
        action!("fume_yards.bank_kiln"),
        action!("travel_adjacent", "destination" => "fume_yards.workshop"),
        action!("travel_adjacent", "destination" => "lowsail.levee"),
        action!("levee.authority_path"),
        action!("travel_adjacent", "destination" => "red_sluice.top"),
        action!("top.hold_market"),
        action!("world.enter_aftermath"),
        action!("return.count_dry_stalls"),
        action!("return.visit_workshop"),
        action!("travel_adjacent", "destination" => "fume_yards.kiln_bay"),
        action!("fume_yards.load_kiln_freight"),
    ],
);
const BATCH_MISSED_STEPS: [ScenarioStep; 19] = append_steps(
    BATCH_EARLY_IGNITED_STEPS,
    &[
        action!("wait_tide"),
        action!("fume_yards.draw_filter"),
        action!("travel_adjacent", "destination" => "fume_yards.workshop"),
        action!("travel_adjacent", "destination" => "lowsail.levee"),
        action!("levee.authority_path"),
        action!("travel_adjacent", "destination" => "red_sluice.top"),
        action!("world.enter_aftermath"),
        action!("return.face_flood"),
        action!("return.sell_filter"),
    ],
);
const BATCH_LATE_IGNITED_STEPS: [ScenarioStep; 136] =
    append_steps(&PILOT_LATE_PREFIX, BATCH_IGNITION_EXTENSION);
const BATCH_LATE_STEPS: [ScenarioStep; 140] = append_steps(
    &BATCH_LATE_IGNITED_STEPS,
    &[
        action!("wait_tide"),
        action!("fume_yards.draw_filter"),
        action!("fume_yards.fit_dust_filter"),
        action!("fume_yards.load_filtered_kiln_freight"),
    ],
);
const BATCH_RECLAIM_STEPS: [ScenarioStep; 16] = append_steps(
    HOLD_MARKET_STEPS,
    &[
        action!("return.visit_workshop"),
        action!("fume_yards.take_stock"),
        action!("travel_adjacent", "destination" => "fume_yards.kiln_bay"),
        action!("fume_yards.take_fuel"),
        action!("fume_yards.prepare_charge"),
        action!("fume_yards.reclaim_charge"),
        action!("world.enter_aftermath"),
        action!("return.patch_stand"),
        action!("return.sort_dry_goods"),
    ],
);

const BATCH_NPC_LOCATIONS: &[ScenarioNpcLocationExpectation] = &[
    AFTERMATH_NPC_LOCATIONS[0],
    AFTERMATH_NPC_LOCATIONS[1],
    AFTERMATH_NPC_LOCATIONS[2],
    AFTERMATH_NPC_LOCATIONS[3],
    AFTERMATH_NPC_LOCATIONS[4],
    ScenarioNpcLocationExpectation {
        npc: "fume_yards.nessa_tern",
        location: "fume_yards.workshop",
    },
    ScenarioNpcLocationExpectation {
        npc: "fume_yards.brann_coil",
        location: "fume_yards.kiln_bay",
    },
    ScenarioNpcLocationExpectation {
        npc: "fume_yards.pera_senn",
        location: "fume_yards.kiln_bay",
    },
];
const BATCH_EMPTY_STOCK: &[ScenarioNpcInventoryAbsence] = &[
    PILOT_EMPTY_NESSA[0],
    PILOT_EMPTY_NESSA[1],
    ScenarioNpcInventoryAbsence {
        npc: "fume_yards.brann_coil",
        item: "fume_yards.fuel",
    },
    ScenarioNpcInventoryAbsence {
        npc: "fume_yards.pera_senn",
        item: "fume_yards.water_cask",
    },
];
const BATCH_NO_SPOIL_KNOWLEDGE: &[ScenarioNpcKnowledgeAbsence] = &[
    ScenarioNpcKnowledgeAbsence {
        npc: "fume_yards.brann_coil",
        knowledge_id: "fume_yards.batch_spoiled",
    },
    ScenarioNpcKnowledgeAbsence {
        npc: "fume_yards.pera_senn",
        knowledge_id: "fume_yards.batch_spoiled",
    },
    ScenarioNpcKnowledgeAbsence {
        npc: "fume_yards.nessa_tern",
        knowledge_id: "fume_yards.batch_spoiled",
    },
    ScenarioNpcKnowledgeAbsence {
        npc: "oren_pell",
        knowledge_id: "fume_yards.batch_spoiled",
    },
];
const fn batch_memory(
    npc: &'static str,
    memory_id: &'static str,
    turn: u64,
) -> ScenarioNpcMemoryExpectation {
    ScenarioNpcMemoryExpectation {
        npc,
        memory_id,
        turn,
        provenance: ScenarioKnowledgeProvenance::Witnessed,
    }
}
const fn batch_recipe(
    turn: u64,
    recipe: &'static str,
    inputs: &'static [(&'static str, u32)],
    outputs: &'static [(&'static str, u32)],
) -> ScenarioRecipeExpectation {
    ScenarioRecipeExpectation {
        turn,
        recipe,
        inputs,
        outputs,
    }
}
const fn batch_schedule(
    turn: u64,
    event_id: &'static str,
    due_time: u64,
) -> DeferredEventExpectation {
    DeferredEventExpectation::Scheduled {
        turn,
        event_id,
        event_kind: "production",
        due_time,
    }
}
const fn batch_resolved(
    turn: u64,
    event_id: &'static str,
    applied: bool,
) -> DeferredEventExpectation {
    DeferredEventExpectation::Resolved {
        turn,
        event_id,
        event_kind: "production",
        applied,
    }
}
const BATCH_BASE_RECIPES: &[ScenarioRecipeExpectation] = &[
    batch_recipe(
        12,
        "fume_yards.prepare_charge",
        PILOT_INPUTS,
        &[("fume_yards.prepared_charge", 1)],
    ),
    batch_recipe(
        13,
        "fume_yards.fit_wet_screen",
        &[("fume_yards.water_cask", 1)],
        &[],
    ),
    batch_recipe(
        14,
        "fume_yards.ignite_batch",
        &[("fume_yards.fuel", 1), ("fume_yards.prepared_charge", 1)],
        &[("fume_yards.batch_claim", 1)],
    ),
];
const BATCH_EARLY_RECIPES: &[ScenarioRecipeExpectation] = &[
    batch_recipe(
        7,
        "fume_yards.prepare_charge",
        PILOT_INPUTS,
        &[("fume_yards.prepared_charge", 1)],
    ),
    batch_recipe(
        8,
        "fume_yards.fit_wet_screen",
        &[("fume_yards.water_cask", 1)],
        &[],
    ),
    batch_recipe(
        9,
        "fume_yards.ignite_batch",
        &[("fume_yards.fuel", 1), ("fume_yards.prepared_charge", 1)],
        &[("fume_yards.batch_claim", 1)],
    ),
];
const BATCH_READY_HISTORY: &[DeferredEventExpectation] = &[
    batch_schedule(14, "fume_yards.batch_ready", 16),
    batch_schedule(14, "fume_yards.batch_spoil", 19),
    batch_resolved(16, "fume_yards.batch_ready", true),
];
const BATCH_DRAWN_HISTORY: &[DeferredEventExpectation] = &[
    BATCH_READY_HISTORY[0],
    BATCH_READY_HISTORY[1],
    BATCH_READY_HISTORY[2],
    batch_resolved(19, "fume_yards.batch_spoil", false),
];
const BATCH_PENDING_SPOIL: &[PendingEventExpectation] = &[PendingEventExpectation {
    event_id: "fume_yards.batch_spoil",
    event_kind: "production",
    due_time: 19,
}];

macro_rules! batch_lit_flags {
    ($(($location:literal, $flag:literal)),* $(,)?) => { &[
        ("fume_yards.workshop", "fume_yards.stock_given"),
        ("fume_yards.kiln_bay", "fume_yards.fuel_taken"),
        ("fume_yards.kiln_bay", "fume_yards.cask_taken"),
        ("fume_yards.kiln_bay", "fume_yards.charge_prepared"),
        ("fume_yards.kiln_bay", "fume_yards.wet_screen_fitted"),
        ("fume_yards.kiln_bay", "fume_yards.batch_ignited"),
        $(($location, $flag)),*
    ] };
}
macro_rules! batch_hold_flags {
    ($($extra:tt)*) => { batch_lit_flags!(
        ("lowsail_market", "market_permit"),
        ("red_sluice.floor", "authorized_entry"),
        ("lowsail.return", "market_stable"),
        ("lowsail.return", "upland_dry"),
        $($extra)*
    ) };
}
macro_rules! batch_spent_items {
    ($($extra:literal),* $(,)?) => { &[
        "fume_yards.clay", "fume_yards.mesh", "fume_yards.prepared_charge", "fume_yards.fuel",
        "fume_yards.water_cask", "fume_yards.repair_lot", "fume_yards.catch_screen", $($extra),*
    ] };
}
macro_rules! batch_closed_flags {
    ($(($location:literal, $flag:literal)),* $(,)?) => { &[
        ("fume_yards.kiln_bay", "fume_yards.batch_active"),
        ("fume_yards.kiln_bay", "fume_yards.batch_ready"),
        ("fume_yards.workshop", "fume_yards.screen_fitted"),
        ("fume_yards.workshop", "fume_yards.freight_loaded"),
        $(($location, $flag)),*
    ] };
}
macro_rules! batch_memories {
    ($stock:literal, $cask:literal, $fuel:literal, $prepare:literal, $wet:literal, $ignite:literal; $($extra:expr),* $(,)?) => { &[
        batch_memory("fume_yards.nessa_tern", "fume_yards.stock_handed_over", $stock),
        batch_memory("fume_yards.pera_senn", "fume_yards.cask_handed_over", $cask),
        batch_memory("fume_yards.brann_coil", "fume_yards.fuel_handed_over", $fuel),
        batch_memory("fume_yards.brann_coil", "fume_yards.charge_prepared", $prepare),
        batch_memory("fume_yards.brann_coil", "fume_yards.wet_screen_fitted", $wet),
        batch_memory("fume_yards.pera_senn", "fume_yards.water_spent", $wet),
        batch_memory("fume_yards.brann_coil", "fume_yards.batch_ignited", $ignite),
        $($extra),*
    ] };
}

const fn batch_expectations(
    time: u64,
    location: &'static str,
    action: &'static str,
    text: &'static str,
) -> ScenarioExpectations {
    ScenarioExpectations {
        final_location: location,
        final_action_definition: action,
        final_observation_contains: text,
        forbidden_observation_contains: &[],
        final_world_time: Some(time),
        exclusive_after_action: "",
        required_world_flags: &[
            "council_route",
            "sluice_outcome_chosen",
            "flow_locked_market",
            "ending_council",
        ],
        forbidden_world_flags: &[
            "flow_split",
            "old_channel_open",
            "flow_relief",
            "sluice_failure",
            "surge_missed",
            "ending_accord",
            "ending_freedom",
            "ending_relief",
            "ending_disaster",
        ],
        required_location_flags: &[],
        forbidden_location_flags: &[],
        required_deeds: &["backed_council", "accepted_council"],
        required_visited_locations: &[
            "lowsail_market",
            "lowsail.levee",
            "red_sluice.floor",
            "red_sluice.top",
            "lowsail.return",
            "fume_yards.workshop",
            "fume_yards.kiln_bay",
        ],
        required_npc_locations: BATCH_NPC_LOCATIONS,
        required_npc_knowledge: &[],
        forbidden_npc_knowledge: BATCH_NO_SPOIL_KNOWLEDGE,
        required_npc_memories: &[],
        required_character_inventory: &[ScenarioInventoryExpectation {
            item: "rope",
            count: 1,
        }],
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
        required_npc_inventory: &[],
        forbidden_npc_inventory: BATCH_EMPTY_STOCK,
        forbidden_character_inventory: batch_spent_items!(
            "fume_yards.batch_claim",
            "fume_yards.filter",
            "fume_yards.spoiled_charge"
        ),
        recipe_events: &[],
        staffing_history: None,
        cold_shift_history: None,
        storage_balances: UNTOUCHED_COLLATERAL,
        storage_transfers: &[],
        entropy_draws: &[],
        deferred_events: &[],
        pending_deferred_events: &[],
        required_legal_definitions: &["wait_tide"],
        forbidden_legal_definitions: OUTCOME_DEFINITIONS,
    }
}
const fn batch_spec(
    id: &'static str,
    claim_id: &'static str,
    steps: &'static [ScenarioStep],
    expectations: ScenarioExpectations,
) -> ScenarioSpec {
    ScenarioSpec {
        id,
        claim_id,
        start: ScenarioStartSpec::Preset {
            character_preset_id: "ilyan",
        },
        seed: 71,
        steps,
        expectations,
    }
}

const BATCH_LOCAL_RECIPE_EVENTS: &[ScenarioRecipeExpectation] = &[
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
        17,
        "fume_yards.fit_dust_filter",
        &[("fume_yards.filter", 1)],
        &[],
    ),
];
const BATCH_SALE_RECIPE_EVENTS: &[ScenarioRecipeExpectation] = &[
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
        "fume_yards.sell_filter",
        &[("fume_yards.filter", 1)],
        &[],
    ),
];
const BATCH_BARE_RECIPE_EVENTS: &[ScenarioRecipeExpectation] = &[
    BATCH_BASE_RECIPES[0],
    BATCH_BASE_RECIPES[1],
    BATCH_BASE_RECIPES[2],
    batch_recipe(
        16,
        "fume_yards.draw_filter",
        &[("fume_yards.batch_claim", 1)],
        &[("fume_yards.filter", 1)],
    ),
];
const BATCH_REMOTE_RECIPE_EVENTS: &[ScenarioRecipeExpectation] = &[
    BATCH_BASE_RECIPES[0],
    BATCH_BASE_RECIPES[1],
    BATCH_BASE_RECIPES[2],
    batch_recipe(
        19,
        "fume_yards.spoil_batch",
        &[("fume_yards.batch_claim", 1)],
        &[("fume_yards.spoiled_charge", 1)],
    ),
];
const BATCH_REMOTE_DEFERRED_EVENTS: &[DeferredEventExpectation] = &[
    BATCH_READY_HISTORY[0],
    BATCH_READY_HISTORY[1],
    BATCH_READY_HISTORY[2],
    batch_resolved(19, "fume_yards.batch_spoil", true),
];
const BATCH_BANK_RECIPE_EVENTS: &[ScenarioRecipeExpectation] = &[
    BATCH_EARLY_RECIPES[0],
    BATCH_EARLY_RECIPES[1],
    BATCH_EARLY_RECIPES[2],
    batch_recipe(
        10,
        "fume_yards.bank_batch",
        &[("fume_yards.batch_claim", 1)],
        &[("fume_yards.spoiled_charge", 1)],
    ),
];
const BATCH_MISSED_RECIPE_EVENTS: &[ScenarioRecipeExpectation] = &[
    BATCH_EARLY_RECIPES[0],
    BATCH_EARLY_RECIPES[1],
    BATCH_EARLY_RECIPES[2],
    batch_recipe(
        11,
        "fume_yards.draw_filter",
        &[("fume_yards.batch_claim", 1)],
        &[("fume_yards.filter", 1)],
    ),
    batch_recipe(
        18,
        "fume_yards.sell_filter",
        &[("fume_yards.filter", 1)],
        &[],
    ),
];
const BATCH_RECLAIM_FORBIDDEN_NPC_INVENTORY: &[ScenarioNpcInventoryAbsence] = &[
    BATCH_EMPTY_STOCK[0],
    BATCH_EMPTY_STOCK[1],
    BATCH_EMPTY_STOCK[2],
];

const BATCH_READY_SPEC: ScenarioSpec = batch_spec(
    "m2-fume-batch-ready",
    "fume-yards.batch.ready",
    &BATCH_READY_STEPS,
    ScenarioExpectations {
        required_location_flags: batch_hold_flags!(
            ("fume_yards.kiln_bay", "fume_yards.batch_active"),
            ("fume_yards.kiln_bay", "fume_yards.batch_ready")
        ),
        forbidden_location_flags: &[
            ("fume_yards.kiln_bay", "fume_yards.batch_spoiled"),
            ("fume_yards.kiln_bay", "fume_yards.freight_spoiled"),
            ("fume_yards.kiln_bay", "fume_yards.batch_drawn"),
            ("fume_yards.kiln_bay", "fume_yards.kiln_closed"),
            ("fume_yards.kiln_bay", "fume_yards.kiln_banked"),
            ("fume_yards.kiln_bay", "fume_yards.kiln_reclaimed"),
            ("fume_yards.kiln_bay", "fume_yards.dust_filter_fitted"),
        ],
        required_character_inventory: &[
            ScenarioInventoryExpectation {
                item: "rope",
                count: 1,
            },
            ScenarioInventoryExpectation {
                item: "fume_yards.batch_claim",
                count: 1,
            },
        ],
        forbidden_character_inventory: batch_spent_items!(
            "fume_yards.filter",
            "fume_yards.spoiled_charge"
        ),
        required_npc_memories: batch_memories!(8, 10, 11, 12, 13, 14;),
        recipe_events: BATCH_BASE_RECIPES,
        storage_balances: UNTOUCHED_COLLATERAL,
        storage_transfers: &[],
        entropy_draws: &[],
        deferred_events: BATCH_READY_HISTORY,
        pending_deferred_events: BATCH_PENDING_SPOIL,
        required_legal_definitions: &[
            "fume_yards.draw_filter",
            "fume_yards.bank_kiln",
            "wait_tide",
        ],
        forbidden_legal_definitions: &[
            "fume_yards.take_cask",
            "fume_yards.take_fuel",
            "fume_yards.prepare_charge",
            "fume_yards.ignite_batch",
            "fume_yards.reclaim_charge",
            "fume_yards.fit_dust_filter",
        ],
        ..batch_expectations(16, "fume_yards.kiln_bay", "wait_tide", "Batch ready.")
    },
);

const BATCH_LOCAL_SPEC: ScenarioSpec = batch_spec(
    "m2-fume-manufacture-local",
    "fume-yards.manufacture.local",
    &BATCH_LOCAL_STEPS,
    ScenarioExpectations {
        required_location_flags: batch_hold_flags!(
            ("fume_yards.kiln_bay", "fume_yards.batch_drawn"),
            ("fume_yards.kiln_bay", "fume_yards.kiln_closed"),
            ("fume_yards.kiln_bay", "fume_yards.dust_filter_fitted"),
            ("fume_yards.kiln_bay", "fume_yards.kiln_freight_loaded")
        ),
        forbidden_location_flags: batch_closed_flags!(
            ("fume_yards.kiln_bay", "fume_yards.batch_spoiled"),
            ("fume_yards.kiln_bay", "fume_yards.freight_spoiled"),
            ("fume_yards.kiln_bay", "fume_yards.kiln_banked"),
            ("fume_yards.kiln_bay", "fume_yards.kiln_reclaimed"),
            ("lowsail.return", "fume_yards.filter_sold")
        ),
        required_character_resources: &[
            ScenarioResourceExpectation {
                resource: "coin",
                amount: 13,
            },
            ScenarioResourceExpectation {
                resource: "stamina",
                amount: 3,
            },
        ],
        required_npc_memories: batch_memories!(8, 10, 11, 12, 13, 14; batch_memory("fume_yards.brann_coil", "fume_yards.filter_drawn", 16), batch_memory("fume_yards.brann_coil", "fume_yards.dust_filter_fitted", 17), batch_memory("fume_yards.brann_coil", "fume_yards.kiln_freight_paid", 18)),
        recipe_events: BATCH_LOCAL_RECIPE_EVENTS,
        storage_balances: UNTOUCHED_COLLATERAL,
        storage_transfers: &[],
        entropy_draws: &[],
        deferred_events: BATCH_DRAWN_HISTORY,
        forbidden_legal_definitions: &[
            "fume_yards.draw_filter",
            "fume_yards.ignite_batch",
            "fume_yards.bank_kiln",
            "fume_yards.fit_dust_filter",
            "fume_yards.load_kiln_freight",
            "fume_yards.load_filtered_kiln_freight",
        ],
        ..batch_expectations(
            19,
            "fume_yards.kiln_bay",
            "fume_yards.load_filtered_kiln_freight",
            "Your installed filter catches the loading dust.",
        )
    },
);

const BATCH_SALE_SPEC: ScenarioSpec = batch_spec(
    "m2-fume-manufacture-sale",
    "fume-yards.manufacture.sale",
    &BATCH_SALE_STEPS,
    ScenarioExpectations {
        required_location_flags: batch_hold_flags!(
            ("fume_yards.kiln_bay", "fume_yards.batch_drawn"),
            ("fume_yards.kiln_bay", "fume_yards.kiln_closed"),
            ("lowsail.return", "fume_yards.filter_sold")
        ),
        forbidden_location_flags: batch_closed_flags!(
            ("fume_yards.kiln_bay", "fume_yards.batch_spoiled"),
            ("fume_yards.kiln_bay", "fume_yards.freight_spoiled"),
            ("fume_yards.kiln_bay", "fume_yards.kiln_banked"),
            ("fume_yards.kiln_bay", "fume_yards.kiln_reclaimed"),
            ("fume_yards.kiln_bay", "fume_yards.dust_filter_fitted"),
            ("fume_yards.kiln_bay", "fume_yards.kiln_freight_loaded")
        ),
        required_character_resources: &[
            ScenarioResourceExpectation {
                resource: "coin",
                amount: 14,
            },
            ScenarioResourceExpectation {
                resource: "stamina",
                amount: 3,
            },
        ],
        required_npc_memories: batch_memories!(8, 10, 11, 12, 13, 14; batch_memory("fume_yards.brann_coil", "fume_yards.filter_drawn", 16), batch_memory("oren_pell", "fume_yards.filter_bought", 18)),
        recipe_events: BATCH_SALE_RECIPE_EVENTS,
        storage_balances: UNTOUCHED_COLLATERAL,
        storage_transfers: &[],
        entropy_draws: &[],
        deferred_events: BATCH_DRAWN_HISTORY,
        forbidden_legal_definitions: &[
            "return.sell_filter",
            "return.patch_stand",
            "return.sort_dry_goods",
            "fume_yards.fit_dust_filter",
        ],
        ..batch_expectations(
            19,
            "lowsail.return",
            "return.sell_filter",
            "Oren buys your filter for four coins.",
        )
    },
);

const BATCH_BARE_SPEC: ScenarioSpec = batch_spec(
    "m2-fume-manufacture-bare",
    "fume-yards.manufacture.bare",
    &BATCH_BARE_STEPS,
    ScenarioExpectations {
        required_location_flags: batch_hold_flags!(
            ("fume_yards.kiln_bay", "fume_yards.batch_drawn"),
            ("fume_yards.kiln_bay", "fume_yards.kiln_closed"),
            ("fume_yards.kiln_bay", "fume_yards.kiln_freight_loaded")
        ),
        forbidden_location_flags: batch_closed_flags!(
            ("fume_yards.kiln_bay", "fume_yards.batch_spoiled"),
            ("fume_yards.kiln_bay", "fume_yards.freight_spoiled"),
            ("fume_yards.kiln_bay", "fume_yards.kiln_banked"),
            ("fume_yards.kiln_bay", "fume_yards.kiln_reclaimed"),
            ("fume_yards.kiln_bay", "fume_yards.dust_filter_fitted"),
            ("lowsail.return", "fume_yards.filter_sold")
        ),
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
                amount: 13,
            },
            ScenarioResourceExpectation {
                resource: "stamina",
                amount: 1,
            },
        ],
        forbidden_character_inventory: batch_spent_items!(
            "fume_yards.batch_claim",
            "fume_yards.spoiled_charge"
        ),
        required_npc_memories: batch_memories!(8, 10, 11, 12, 13, 14; batch_memory("fume_yards.brann_coil", "fume_yards.filter_drawn", 16), batch_memory("fume_yards.brann_coil", "fume_yards.kiln_freight_paid", 17)),
        recipe_events: BATCH_BARE_RECIPE_EVENTS,
        storage_balances: UNTOUCHED_COLLATERAL,
        storage_transfers: &[],
        entropy_draws: &[],
        deferred_events: BATCH_READY_HISTORY,
        pending_deferred_events: BATCH_PENDING_SPOIL,
        required_legal_definitions: &["wait_tide"],
        forbidden_legal_definitions: &[
            "fume_yards.draw_filter",
            "fume_yards.ignite_batch",
            "fume_yards.bank_kiln",
            "fume_yards.fit_dust_filter",
            "fume_yards.load_kiln_freight",
            "fume_yards.load_filtered_kiln_freight",
        ],
        ..batch_expectations(
            18,
            "fume_yards.kiln_bay",
            "fume_yards.load_kiln_freight",
            "spending two stamina.",
        )
    },
);

const BATCH_REMOTE_SPEC: ScenarioSpec = batch_spec(
    "m2-fume-remote-spoil",
    "fume-yards.batch.remote-spoil",
    &BATCH_REMOTE_STEPS,
    ScenarioExpectations {
        required_location_flags: batch_hold_flags!(
            ("fume_yards.kiln_bay", "fume_yards.batch_spoiled"),
            ("fume_yards.kiln_bay", "fume_yards.freight_spoiled"),
            ("fume_yards.kiln_bay", "fume_yards.kiln_closed")
        ),
        forbidden_location_flags: batch_closed_flags!(
            ("fume_yards.kiln_bay", "fume_yards.batch_drawn"),
            ("fume_yards.kiln_bay", "fume_yards.kiln_banked"),
            ("fume_yards.kiln_bay", "fume_yards.kiln_reclaimed"),
            ("fume_yards.kiln_bay", "fume_yards.dust_filter_fitted"),
            ("fume_yards.kiln_bay", "fume_yards.kiln_freight_loaded"),
            ("lowsail.return", "fume_yards.filter_sold")
        ),
        required_character_inventory: &[
            ScenarioInventoryExpectation {
                item: "rope",
                count: 1,
            },
            ScenarioInventoryExpectation {
                item: "fume_yards.spoiled_charge",
                count: 1,
            },
        ],
        forbidden_character_inventory: batch_spent_items!(
            "fume_yards.batch_claim",
            "fume_yards.filter"
        ),
        required_npc_memories: batch_memories!(8, 10, 11, 12, 13, 14;),
        recipe_events: BATCH_REMOTE_RECIPE_EVENTS,
        storage_balances: UNTOUCHED_COLLATERAL,
        storage_transfers: &[],
        entropy_draws: &[],
        deferred_events: BATCH_REMOTE_DEFERRED_EVENTS,
        ..batch_expectations(19, "lowsail.docks", "travel_adjacent", "Batch spoiled.")
    },
);

const BATCH_BANK_SPEC: ScenarioSpec = batch_spec(
    "m2-fume-bank-save-tide",
    "fume-yards.batch.bank-save-tide",
    &BATCH_BANK_STEPS,
    ScenarioExpectations {
        required_location_flags: batch_hold_flags!(
            ("fume_yards.kiln_bay", "fume_yards.kiln_banked"),
            ("fume_yards.kiln_bay", "fume_yards.kiln_closed"),
            ("fume_yards.kiln_bay", "fume_yards.kiln_freight_loaded")
        ),
        forbidden_location_flags: batch_closed_flags!(
            ("fume_yards.kiln_bay", "fume_yards.batch_drawn"),
            ("fume_yards.kiln_bay", "fume_yards.batch_spoiled"),
            ("fume_yards.kiln_bay", "fume_yards.freight_spoiled"),
            ("fume_yards.kiln_bay", "fume_yards.kiln_reclaimed"),
            ("fume_yards.kiln_bay", "fume_yards.dust_filter_fitted"),
            ("lowsail.return", "fume_yards.filter_sold")
        ),
        required_character_inventory: &[
            ScenarioInventoryExpectation {
                item: "rope",
                count: 1,
            },
            ScenarioInventoryExpectation {
                item: "fume_yards.spoiled_charge",
                count: 1,
            },
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
        forbidden_character_inventory: batch_spent_items!(
            "fume_yards.batch_claim",
            "fume_yards.filter"
        ),
        required_npc_memories: batch_memories!(3, 5, 6, 7, 8, 9; batch_memory("fume_yards.brann_coil", "fume_yards.kiln_banked", 10), batch_memory("fume_yards.brann_coil", "fume_yards.kiln_freight_paid", 20)),
        recipe_events: BATCH_BANK_RECIPE_EVENTS,
        storage_balances: UNTOUCHED_COLLATERAL,
        storage_transfers: &[],
        entropy_draws: &[],
        deferred_events: &[
            batch_schedule(9, "fume_yards.batch_ready", 11),
            batch_schedule(9, "fume_yards.batch_spoil", 14),
            batch_resolved(11, "fume_yards.batch_ready", false),
            batch_resolved(14, "fume_yards.batch_spoil", false),
        ],
        forbidden_legal_definitions: &[
            "fume_yards.draw_filter",
            "fume_yards.ignite_batch",
            "fume_yards.bank_kiln",
            "fume_yards.fit_dust_filter",
            "fume_yards.load_kiln_freight",
            "fume_yards.load_filtered_kiln_freight",
        ],
        ..batch_expectations(
            21,
            "fume_yards.kiln_bay",
            "fume_yards.load_kiln_freight",
            "Brann pays three coins;",
        )
    },
);

const BATCH_MISSED_SPEC: ScenarioSpec = batch_spec(
    "m2-fume-draw-miss-tide",
    "fume-yards.batch.draw-miss-tide",
    &BATCH_MISSED_STEPS,
    ScenarioExpectations {
        required_world_flags: &[
            "council_route",
            "sluice_outcome_chosen",
            "sluice_failure",
            "surge_missed",
            "ending_disaster",
        ],
        forbidden_world_flags: &[
            "flow_split",
            "flow_locked_market",
            "flow_relief",
            "old_channel_open",
            "ending_accord",
            "ending_council",
            "ending_relief",
            "ending_freedom",
        ],
        required_deeds: &["faced_flood"],
        required_location_flags: batch_lit_flags!(
            ("fume_yards.kiln_bay", "fume_yards.batch_drawn"),
            ("fume_yards.kiln_bay", "fume_yards.kiln_closed"),
            ("lowsail.return", "market_flooded"),
            ("lowsail.return", "fume_yards.filter_sold")
        ),
        forbidden_location_flags: batch_closed_flags!(
            ("fume_yards.kiln_bay", "fume_yards.batch_spoiled"),
            ("fume_yards.kiln_bay", "fume_yards.freight_spoiled"),
            ("fume_yards.kiln_bay", "fume_yards.kiln_banked"),
            ("fume_yards.kiln_bay", "fume_yards.kiln_reclaimed"),
            ("fume_yards.kiln_bay", "fume_yards.dust_filter_fitted"),
            ("fume_yards.kiln_bay", "fume_yards.kiln_freight_loaded")
        ),
        required_character_resources: &[
            ScenarioResourceExpectation {
                resource: "coin",
                amount: 14,
            },
            ScenarioResourceExpectation {
                resource: "stamina",
                amount: 3,
            },
        ],
        required_npc_memories: batch_memories!(3, 5, 6, 7, 8, 9; batch_memory("fume_yards.brann_coil", "fume_yards.filter_drawn", 11), batch_memory("oren_pell", "fume_yards.filter_bought", 18)),
        recipe_events: BATCH_MISSED_RECIPE_EVENTS,
        storage_balances: UNTOUCHED_COLLATERAL,
        storage_transfers: &[],
        entropy_draws: &[],
        deferred_events: &[
            batch_schedule(9, "fume_yards.batch_ready", 11),
            batch_schedule(9, "fume_yards.batch_spoil", 14),
            batch_resolved(11, "fume_yards.batch_ready", true),
            batch_resolved(14, "fume_yards.batch_spoil", false),
        ],
        forbidden_legal_definitions: &[
            "top.hold_market",
            "top.split_flow",
            "top.divert_relief",
            "top.break_toll",
            "top.overload",
            "return.sell_filter",
        ],
        ..batch_expectations(
            19,
            "lowsail.return",
            "return.sell_filter",
            "Oren buys your filter for four coins.",
        )
    },
);

const BATCH_LATE_SPEC: ScenarioSpec = ScenarioSpec {
    start: ScenarioStartSpec::Preset {
        character_preset_id: "rook",
    },
    ..batch_spec(
        "m2-fume-late-manufacture",
        "fume-yards.batch.late-manufacture",
        &BATCH_LATE_STEPS,
        ScenarioExpectations {
            required_world_flags: &[
                "culvert_revealed",
                "market_warned",
                "relief_channel_open",
                "sluice_outcome_chosen",
                "flow_relief",
                "ending_relief",
            ],
            forbidden_world_flags: &[
                "tide_key_offered",
                "flow_split",
                "flow_locked_market",
                "old_channel_open",
                "sluice_failure",
                "surge_missed",
                "ending_accord",
                "ending_council",
                "ending_freedom",
                "ending_disaster",
            ],
            required_deeds: &["rigged_towline", "opened_relief", "accepted_relocation"],
            required_location_flags: batch_lit_flags!(
                ("fume_yards.kiln_bay", "fume_yards.batch_drawn"),
                ("fume_yards.kiln_bay", "fume_yards.kiln_closed"),
                ("fume_yards.kiln_bay", "fume_yards.dust_filter_fitted"),
                ("fume_yards.kiln_bay", "fume_yards.kiln_freight_loaded"),
                ("red_sluice.floor", "culvert_entry"),
                ("red_sluice.top", "relief_ready"),
                ("lowsail.return", "market_moved")
            ),
            forbidden_location_flags: batch_closed_flags!(
                ("fume_yards.kiln_bay", "fume_yards.batch_spoiled"),
                ("fume_yards.kiln_bay", "fume_yards.freight_spoiled"),
                ("fume_yards.kiln_bay", "fume_yards.kiln_banked"),
                ("fume_yards.kiln_bay", "fume_yards.kiln_reclaimed"),
                ("lowsail.return", "fume_yards.filter_sold")
            ),
            required_character_inventory: &[
                ScenarioInventoryExpectation {
                    item: "rope",
                    count: 1,
                },
                ScenarioInventoryExpectation {
                    item: "wire",
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
            required_npc_knowledge: PAID_TOWLINE_REQUIRED_NPC_KNOWLEDGE,
            required_npc_memories: batch_memories!(129, 131, 132, 133, 134, 135; batch_memory("fume_yards.brann_coil", "fume_yards.filter_drawn", 137), batch_memory("fume_yards.brann_coil", "fume_yards.dust_filter_fitted", 138), batch_memory("fume_yards.brann_coil", "fume_yards.kiln_freight_paid", 139)),
            recipe_events: &[
                batch_recipe(
                    133,
                    "fume_yards.prepare_charge",
                    PILOT_INPUTS,
                    &[("fume_yards.prepared_charge", 1)],
                ),
                batch_recipe(
                    134,
                    "fume_yards.fit_wet_screen",
                    &[("fume_yards.water_cask", 1)],
                    &[],
                ),
                batch_recipe(
                    135,
                    "fume_yards.ignite_batch",
                    &[("fume_yards.fuel", 1), ("fume_yards.prepared_charge", 1)],
                    &[("fume_yards.batch_claim", 1)],
                ),
                batch_recipe(
                    137,
                    "fume_yards.draw_filter",
                    &[("fume_yards.batch_claim", 1)],
                    &[("fume_yards.filter", 1)],
                ),
                batch_recipe(
                    138,
                    "fume_yards.fit_dust_filter",
                    &[("fume_yards.filter", 1)],
                    &[],
                ),
            ],
            storage_balances: UNTOUCHED_COLLATERAL,
            storage_transfers: &[],
            entropy_draws: &[],
            deferred_events: &[
                batch_schedule(135, "fume_yards.batch_ready", 137),
                batch_schedule(135, "fume_yards.batch_spoil", 140),
                batch_resolved(137, "fume_yards.batch_ready", true),
                batch_resolved(140, "fume_yards.batch_spoil", false),
            ],
            forbidden_legal_definitions: &[
                "fume_yards.draw_filter",
                "fume_yards.ignite_batch",
                "fume_yards.bank_kiln",
                "fume_yards.fit_dust_filter",
                "fume_yards.load_kiln_freight",
                "fume_yards.load_filtered_kiln_freight",
            ],
            ..batch_expectations(
                140,
                "fume_yards.kiln_bay",
                "fume_yards.load_filtered_kiln_freight",
                "Your installed filter catches the loading dust.",
            )
        },
    )
};

const BATCH_RECLAIM_SPEC: ScenarioSpec = batch_spec(
    "m2-fume-reclaim-charge",
    "fume-yards.batch.reclaim-charge",
    &BATCH_RECLAIM_STEPS,
    ScenarioExpectations {
        required_location_flags: &[
            ("lowsail_market", "market_permit"),
            ("red_sluice.floor", "authorized_entry"),
            ("lowsail.return", "market_stable"),
            ("lowsail.return", "upland_dry"),
            ("fume_yards.workshop", "fume_yards.stock_given"),
            ("fume_yards.kiln_bay", "fume_yards.fuel_taken"),
            ("fume_yards.kiln_bay", "fume_yards.charge_prepared"),
            ("fume_yards.kiln_bay", "fume_yards.kiln_reclaimed"),
            ("fume_yards.kiln_bay", "fume_yards.kiln_closed"),
            ("lowsail.return", "fume_yards.stand_patched"),
            ("lowsail.return", "fume_yards.dry_goods_sorted"),
        ],
        forbidden_location_flags: batch_closed_flags!(
            ("fume_yards.kiln_bay", "fume_yards.cask_taken"),
            ("fume_yards.kiln_bay", "fume_yards.wet_screen_fitted"),
            ("fume_yards.kiln_bay", "fume_yards.batch_ignited"),
            ("fume_yards.kiln_bay", "fume_yards.batch_drawn"),
            ("fume_yards.kiln_bay", "fume_yards.batch_spoiled"),
            ("fume_yards.kiln_bay", "fume_yards.freight_spoiled"),
            ("fume_yards.kiln_bay", "fume_yards.kiln_banked"),
            ("fume_yards.kiln_bay", "fume_yards.dust_filter_fitted"),
            ("fume_yards.kiln_bay", "fume_yards.kiln_freight_loaded"),
            ("lowsail.return", "fume_yards.filter_sold")
        ),
        required_character_inventory: &[
            ScenarioInventoryExpectation {
                item: "rope",
                count: 1,
            },
            ScenarioInventoryExpectation {
                item: "fume_yards.fuel",
                count: 1,
            },
        ],
        required_character_resources: &[
            ScenarioResourceExpectation {
                resource: "coin",
                amount: 13,
            },
            ScenarioResourceExpectation {
                resource: "stamina",
                amount: 3,
            },
        ],
        required_npc_inventory: &[ScenarioNpcInventoryExpectation {
            npc: "fume_yards.pera_senn",
            item: "fume_yards.water_cask",
            count: 1,
        }],
        forbidden_npc_inventory: BATCH_RECLAIM_FORBIDDEN_NPC_INVENTORY,
        forbidden_character_inventory: &[
            "fume_yards.clay",
            "fume_yards.mesh",
            "fume_yards.prepared_charge",
            "fume_yards.water_cask",
            "fume_yards.batch_claim",
            "fume_yards.filter",
            "fume_yards.spoiled_charge",
            "fume_yards.repair_lot",
            "fume_yards.catch_screen",
        ],
        required_npc_memories: &[
            batch_memory("fume_yards.nessa_tern", "fume_yards.stock_handed_over", 8),
            batch_memory("fume_yards.brann_coil", "fume_yards.fuel_handed_over", 10),
            batch_memory("fume_yards.brann_coil", "fume_yards.charge_prepared", 11),
            batch_memory("fume_yards.brann_coil", "fume_yards.charge_reclaimed", 12),
            batch_memory("oren_pell", "fume_yards.stand_patched", 14),
            batch_memory("oren_pell", "fume_yards.dry_goods_paid", 15),
        ],
        required_npc_knowledge: &[ScenarioNpcKnowledgeExpectation {
            npc: "oren_pell",
            knowledge_id: "fume_yards.stand_patched",
            provenance: ScenarioKnowledgeProvenance::Witnessed,
            turn: 14,
        }],
        recipe_events: &[
            batch_recipe(
                11,
                "fume_yards.prepare_charge",
                PILOT_INPUTS,
                &[("fume_yards.prepared_charge", 1)],
            ),
            batch_recipe(
                12,
                "fume_yards.reclaim_charge",
                &[("fume_yards.prepared_charge", 1)],
                &[("fume_yards.repair_lot", 1)],
            ),
            batch_recipe(
                14,
                "fume_yards.patch_stand",
                &[("fume_yards.repair_lot", 1)],
                &[],
            ),
        ],
        forbidden_legal_definitions: &[
            "return.patch_stand",
            "return.sort_dry_goods",
            "return.sell_filter",
        ],
        ..batch_expectations(
            16,
            "lowsail.return",
            "return.sort_dry_goods",
            "Oren pays three coins; the sorting job is finished.",
        )
    },
);

#[cfg(test)]
mod batch_tests {
    use super::*;
    use forge_kernel::{EventKind, GameState};

    const BATCH_SPECS: &[ScenarioSpec] = &[
        BATCH_READY_SPEC,
        BATCH_LOCAL_SPEC,
        BATCH_SALE_SPEC,
        BATCH_BARE_SPEC,
        BATCH_REMOTE_SPEC,
        BATCH_BANK_SPEC,
        BATCH_MISSED_SPEC,
        BATCH_LATE_SPEC,
        BATCH_RECLAIM_SPEC,
    ];

    #[test]
    fn all_nine_batch_production_scenarios_meet_literal_claims() {
        let content = crate::load_content().unwrap();
        validate_specs(BATCH_SPECS).unwrap();
        for spec in BATCH_SPECS {
            let session =
                run(spec, &content).unwrap_or_else(|error| panic!("{}: {error}", spec.id));
            assert_complete_timed_history(spec, session.state());
        }
    }

    fn prefix<'a>(spec: &ScenarioSpec, count: usize, content: &'a CompiledContent) -> Session<'a> {
        let ScenarioStartSpec::Preset {
            character_preset_id,
        } = spec.start
        else {
            panic!("batch witnesses use authored presets");
        };
        let mut session = Session::new_game(character_preset_id, spec.seed, content).unwrap();
        for step in &spec.steps[..count] {
            record_step(&mut session, content, step).unwrap();
        }
        session
    }

    fn assert_history_rejected(state: &GameState, spec: &ScenarioSpec, mutation: &str) {
        assert!(
            validate_deferred_history(
                state,
                spec.expectations.deferred_events,
                spec.expectations.pending_deferred_events
            )
            .is_err(),
            "accepted {mutation}"
        );
    }

    #[test]
    fn batch_deferred_oracle_rejects_schedule_and_resolution_field_corruption() {
        let content = crate::load_content().unwrap();
        let session = run(&BATCH_REMOTE_SPEC, &content).unwrap();
        let original = session.state();
        let scheduled = original
            .event_log
            .iter()
            .position(|e| matches!(&e.kind, EventKind::EventScheduled { .. }))
            .unwrap();
        let resolved = original
            .event_log
            .iter()
            .position(|e| {
                matches!(&e.kind, EventKind::ScheduledEventResolved { event_id, .. }
                if event_id == "fume_yards.batch_spoil")
            })
            .unwrap();
        for field in ["turn", "id", "kind", "due", "variant"] {
            let mut bad = original.clone();
            let event = &mut bad.event_log[scheduled];
            if field == "turn" {
                event.turn += 1;
            }
            let EventKind::EventScheduled {
                event_id,
                event_kind,
                due_time,
            } = &mut event.kind
            else {
                unreachable!()
            };
            match field {
                "id" => *event_id = "unreviewed.batch_ready".to_owned(),
                "kind" => *event_kind = "deadline".to_owned(),
                "due" => *due_time += 1,
                "variant" => {
                    event.kind = EventKind::ScheduledEventResolved {
                        event_id: "fume_yards.batch_ready".to_owned(),
                        event_kind: "production".to_owned(),
                        applied: true,
                    }
                }
                _ => {}
            }
            assert_history_rejected(&bad, &BATCH_REMOTE_SPEC, field);
        }
        for field in ["turn", "id", "kind", "applied"] {
            let mut bad = original.clone();
            let event = &mut bad.event_log[resolved];
            if field == "turn" {
                event.turn -= 1;
            }
            let EventKind::ScheduledEventResolved {
                event_id,
                event_kind,
                applied,
            } = &mut event.kind
            else {
                unreachable!()
            };
            match field {
                "id" => *event_id = "unreviewed.batch_spoil".to_owned(),
                "kind" => *event_kind = "deadline".to_owned(),
                "applied" => *applied = false,
                _ => {}
            }
            assert_history_rejected(&bad, &BATCH_REMOTE_SPEC, field);
        }
        let mut swapped = original.clone();
        swapped.event_log.swap(scheduled, resolved);
        assert_history_rejected(&swapped, &BATCH_REMOTE_SPEC, "history order");
        let mut missing = original.clone();
        missing.event_log.remove(resolved);
        assert_history_rejected(&missing, &BATCH_REMOTE_SPEC, "missing resolution");
        let mut duplicate = original.clone();
        duplicate
            .event_log
            .push(original.event_log[scheduled].clone());
        assert_history_rejected(&duplicate, &BATCH_REMOTE_SPEC, "duplicate schedule");
        let mut missing_schedule = original.clone();
        missing_schedule.event_log.remove(scheduled);
        assert_history_rejected(&missing_schedule, &BATCH_REMOTE_SPEC, "missing schedule");
        let mut duplicate_resolution = original.clone();
        duplicate_resolution
            .event_log
            .push(original.event_log[resolved].clone());
        assert_history_rejected(
            &duplicate_resolution,
            &BATCH_REMOTE_SPEC,
            "duplicate resolution",
        );
    }

    #[test]
    fn batch_deferred_oracle_rejects_pending_queue_corruption() {
        let content = crate::load_content().unwrap();
        let session = prefix(&BATCH_READY_SPEC, 15, &content);
        let expected = &BATCH_READY_HISTORY[..2];
        let pending = &[
            PendingEventExpectation {
                event_id: "fume_yards.batch_ready",
                event_kind: "production",
                due_time: 16,
            },
            PendingEventExpectation {
                event_id: "fume_yards.batch_spoil",
                event_kind: "production",
                due_time: 19,
            },
        ];
        validate_deferred_history(session.state(), expected, pending).unwrap();
        let ready = session
            .state()
            .world
            .scheduled_events
            .iter()
            .position(|event| event.id == "fume_yards.batch_ready")
            .unwrap();
        let spoil = session
            .state()
            .world
            .scheduled_events
            .iter()
            .position(|event| event.id == "fume_yards.batch_spoil")
            .unwrap();
        for mutation in ["id", "kind", "due", "order", "missing", "duplicate"] {
            let mut bad = session.state().clone();
            let queue = &mut bad.world.scheduled_events;
            match mutation {
                "id" => queue[ready].id = "unreviewed.batch_ready".to_owned(),
                "kind" => queue[ready].event_kind = "deadline".to_owned(),
                "due" => queue[ready].due_time += 1,
                "order" => queue.swap(ready, spoil),
                "missing" => {
                    queue.remove(ready);
                }
                "duplicate" => queue.push(queue[ready].clone()),
                _ => unreachable!(),
            }
            assert!(
                validate_deferred_history(&bad, expected, pending).is_err(),
                "accepted pending {mutation}"
            );
        }
    }

    #[test]
    fn batch_recipe_oracle_rejects_exact_quantity_and_order_corruption() {
        let content = crate::load_content().unwrap();
        let session = run(&BATCH_REMOTE_SPEC, &content).unwrap();
        let first = session
            .state()
            .event_log
            .iter()
            .position(|event| matches!(event.kind, EventKind::RecipeApplied { .. }))
            .unwrap();
        let last = session
            .state()
            .event_log
            .iter()
            .rposition(|event| matches!(event.kind, EventKind::RecipeApplied { .. }))
            .unwrap();
        for mutation in [
            "turn",
            "id",
            "input",
            "output",
            "extra input",
            "extra output",
            "order",
            "missing",
            "duplicate",
        ] {
            let mut bad = session.state().clone();
            let event = &mut bad.event_log[last];
            if mutation == "turn" {
                event.turn -= 1;
            }
            let EventKind::RecipeApplied {
                recipe,
                inputs,
                outputs,
            } = &mut event.kind
            else {
                unreachable!()
            };
            match mutation {
                "id" => *recipe = "fume_yards.bank_batch".to_owned(),
                "input" => {
                    inputs.insert("fume_yards.batch_claim".to_owned(), 2);
                }
                "output" => {
                    outputs.insert("fume_yards.spoiled_charge".to_owned(), 2);
                }
                "extra input" => {
                    inputs.insert("fume_yards.fuel".to_owned(), 1);
                }
                "extra output" => {
                    outputs.insert("fume_yards.filter".to_owned(), 1);
                }
                "order" => bad.event_log.swap(first, last),
                "missing" => {
                    bad.event_log.remove(last);
                }
                "duplicate" => bad.event_log.push(bad.event_log[last].clone()),
                _ => {}
            }
            assert!(
                validate_recipe_events(&bad, BATCH_REMOTE_SPEC.expectations.recipe_events).is_err(),
                "accepted recipe {mutation}"
            );
        }
    }

    fn rejected_claim(
        original: &ScenarioSpec,
        changed: &ScenarioSpec,
        session: &Session<'_>,
        content: &CompiledContent,
    ) {
        assert_ne!(binding(original).unwrap(), binding(changed).unwrap());
        assert!(validate_session(changed, session, content).is_err());
    }

    #[test]
    fn batch_semantic_claims_bind_stock_flags_memories_resources_and_deadlines() {
        let content = crate::load_content().unwrap();
        let remote = run(&BATCH_REMOTE_SPEC, &content).unwrap();
        let mut wrong = BATCH_REMOTE_SPEC;
        wrong.expectations.required_character_inventory = &[ScenarioInventoryExpectation {
            item: "fume_yards.spoiled_charge",
            count: 2,
        }];
        rejected_claim(&BATCH_REMOTE_SPEC, &wrong, &remote, &content);
        wrong = BATCH_REMOTE_SPEC;
        wrong.expectations.required_character_resources = &[ScenarioResourceExpectation {
            resource: "coin",
            amount: 13,
        }];
        rejected_claim(&BATCH_REMOTE_SPEC, &wrong, &remote, &content);
        wrong = BATCH_REMOTE_SPEC;
        wrong.expectations.forbidden_location_flags =
            &[("fume_yards.kiln_bay", "fume_yards.freight_spoiled")];
        rejected_claim(&BATCH_REMOTE_SPEC, &wrong, &remote, &content);
        wrong = BATCH_REMOTE_SPEC;
        wrong.expectations.required_location_flags =
            &[("fume_yards.kiln_bay", "fume_yards.batch_drawn")];
        rejected_claim(&BATCH_REMOTE_SPEC, &wrong, &remote, &content);
        wrong = BATCH_REMOTE_SPEC;
        wrong.expectations.required_npc_knowledge = &[ScenarioNpcKnowledgeExpectation {
            npc: "fume_yards.brann_coil",
            knowledge_id: "fume_yards.batch_spoiled",
            provenance: ScenarioKnowledgeProvenance::Witnessed,
            turn: 19,
        }];
        rejected_claim(&BATCH_REMOTE_SPEC, &wrong, &remote, &content);
        wrong = BATCH_REMOTE_SPEC;
        const WRONG_MEMORY: &[ScenarioNpcMemoryExpectation] = &[batch_memory(
            "fume_yards.brann_coil",
            "fume_yards.batch_ignited",
            15,
        )];
        wrong.expectations.required_npc_memories = WRONG_MEMORY;
        rejected_claim(&BATCH_REMOTE_SPEC, &wrong, &remote, &content);
        wrong = BATCH_REMOTE_SPEC;
        wrong.expectations.deferred_events = BATCH_DRAWN_HISTORY;
        rejected_claim(&BATCH_REMOTE_SPEC, &wrong, &remote, &content);
        wrong = BATCH_REMOTE_SPEC;
        wrong.expectations.pending_deferred_events = BATCH_PENDING_SPOIL;
        rejected_claim(&BATCH_REMOTE_SPEC, &wrong, &remote, &content);
        wrong = BATCH_REMOTE_SPEC;
        wrong.expectations.recipe_events = BATCH_BASE_RECIPES;
        rejected_claim(&BATCH_REMOTE_SPEC, &wrong, &remote, &content);

        let reclaim = run(&BATCH_RECLAIM_SPEC, &content).unwrap();
        let mut wrong = BATCH_RECLAIM_SPEC;
        wrong.expectations.required_npc_inventory = &[ScenarioNpcInventoryExpectation {
            npc: "fume_yards.pera_senn",
            item: "fume_yards.water_cask",
            count: 2,
        }];
        rejected_claim(&BATCH_RECLAIM_SPEC, &wrong, &reclaim, &content);
        wrong = BATCH_RECLAIM_SPEC;
        wrong.expectations.forbidden_character_inventory = &["fume_yards.fuel"];
        rejected_claim(&BATCH_RECLAIM_SPEC, &wrong, &reclaim, &content);
    }

    fn owned(state: &GameState, item: &str) -> u32 {
        state.character.inventory.get(item).copied().unwrap_or(0)
    }

    type TimedRecord<'a> = (u64, &'a str, &'a str, Option<u64>, Option<bool>);

    fn assert_complete_timed_history(spec: &ScenarioSpec, state: &GameState) {
        // Include the old absolute surge in this oracle to bind its ordering
        // against the new relative events at the shared time-16 boundary.
        let actual: Vec<TimedRecord<'_>> = state
            .event_log
            .iter()
            .filter_map(|event| match &event.kind {
                EventKind::EventScheduled {
                    event_id,
                    event_kind,
                    due_time,
                } => Some((
                    event.turn,
                    event_id.as_str(),
                    event_kind.as_str(),
                    Some(*due_time),
                    None,
                )),
                EventKind::ScheduledEventResolved {
                    event_id,
                    event_kind,
                    applied,
                } => Some((
                    event.turn,
                    event_id.as_str(),
                    event_kind.as_str(),
                    None,
                    Some(*applied),
                )),
                _ => None,
            })
            .collect();
        const STANDARD: &[TimedRecord<'_>] = &[
            (14, "fume_yards.batch_ready", "production", Some(16), None),
            (14, "fume_yards.batch_spoil", "production", Some(19), None),
            (16, "fume_yards.batch_ready", "production", None, Some(true)),
            (16, "lowsail.next_surge", "deadline", None, Some(false)),
        ];
        let expected: Vec<TimedRecord<'_>> = match spec.id {
            "m2-fume-batch-ready" | "m2-fume-manufacture-bare" => STANDARD.to_vec(),
            "m2-fume-manufacture-local" | "m2-fume-manufacture-sale" => [
                STANDARD,
                &[(
                    19,
                    "fume_yards.batch_spoil",
                    "production",
                    None,
                    Some(false),
                )],
            ]
            .concat(),
            "m2-fume-remote-spoil" => [
                STANDARD,
                &[(19, "fume_yards.batch_spoil", "production", None, Some(true))],
            ]
            .concat(),
            "m2-fume-bank-save-tide" => vec![
                (9, "fume_yards.batch_ready", "production", Some(11), None),
                (9, "fume_yards.batch_spoil", "production", Some(14), None),
                (
                    11,
                    "fume_yards.batch_ready",
                    "production",
                    None,
                    Some(false),
                ),
                (
                    14,
                    "fume_yards.batch_spoil",
                    "production",
                    None,
                    Some(false),
                ),
                (16, "lowsail.next_surge", "deadline", None, Some(false)),
            ],
            "m2-fume-draw-miss-tide" => vec![
                (9, "fume_yards.batch_ready", "production", Some(11), None),
                (9, "fume_yards.batch_spoil", "production", Some(14), None),
                (11, "fume_yards.batch_ready", "production", None, Some(true)),
                (
                    14,
                    "fume_yards.batch_spoil",
                    "production",
                    None,
                    Some(false),
                ),
                (16, "lowsail.next_surge", "deadline", None, Some(true)),
            ],
            "m2-fume-late-manufacture" => vec![
                (16, "lowsail.next_surge", "deadline", None, Some(false)),
                (135, "fume_yards.batch_ready", "production", Some(137), None),
                (135, "fume_yards.batch_spoil", "production", Some(140), None),
                (
                    137,
                    "fume_yards.batch_ready",
                    "production",
                    None,
                    Some(true),
                ),
                (
                    140,
                    "fume_yards.batch_spoil",
                    "production",
                    None,
                    Some(false),
                ),
            ],
            "m2-fume-reclaim-charge" => {
                vec![(16, "lowsail.next_surge", "deadline", None, Some(false))]
            }
            _ => panic!("unreviewed batch timeline"),
        };
        assert_eq!(actual, expected, "{} common timeline", spec.id);
    }

    fn surge_history(state: &GameState) -> Vec<(u64, bool)> {
        state
            .event_log
            .iter()
            .filter_map(|event| match &event.kind {
                EventKind::ScheduledEventResolved {
                    event_id,
                    event_kind,
                    applied,
                } if event_id == "lowsail.next_surge" => {
                    assert_eq!(event_kind, "deadline");
                    Some((event.turn, *applied))
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn early_bank_preserves_tide_choice_and_freight_while_wait_and_draw_misses() {
        let content = crate::load_content().unwrap();
        for (bank, draw) in BATCH_BANK_STEPS[..10].iter().zip(&BATCH_MISSED_STEPS[..10]) {
            assert_eq!(bank.definition_id, draw.definition_id);
            assert_eq!(bank.parameters, draw.parameters);
        }
        let mut bank = prefix(&BATCH_BANK_SPEC, 15, &content);
        let mut missed = prefix(&BATCH_MISSED_SPEC, 16, &content);
        assert_eq!(bank.state().world.current_location, "red_sluice.top");
        assert_eq!(missed.state().world.current_location, "red_sluice.top");
        assert_eq!(bank.state().world.time, 15);
        assert_eq!(missed.state().world.time, 16);
        validate_legal_set(bank.state(), &content, &["top.hold_market"], &[]).unwrap();
        validate_legal_set(missed.state(), &content, &[], OUTCOME_DEFINITIONS).unwrap();
        assert!(surge_history(bank.state()).is_empty());
        assert_eq!(surge_history(missed.state()), vec![(16, true)]);
        assert_eq!(
            (
                owned(bank.state(), "fume_yards.spoiled_charge"),
                owned(bank.state(), "fume_yards.filter")
            ),
            (1, 0)
        );
        assert_eq!(
            (
                owned(missed.state(), "fume_yards.spoiled_charge"),
                owned(missed.state(), "fume_yards.filter")
            ),
            (0, 1)
        );
        for step in &BATCH_BANK_STEPS[15..] {
            record_step(&mut bank, &content, step).unwrap();
        }
        for step in &BATCH_MISSED_STEPS[16..] {
            record_step(&mut missed, &content, step).unwrap();
        }
        validate_session(&BATCH_BANK_SPEC, &bank, &content).unwrap();
        validate_session(&BATCH_MISSED_SPEC, &missed, &content).unwrap();
        assert_eq!(surge_history(bank.state()), vec![(16, false)]);
        assert_eq!(surge_history(missed.state()), vec![(16, true)]);
        assert_eq!(
            (
                bank.state().character.resources["coin"],
                bank.state().character.resources["stamina"]
            ),
            (13, 1)
        );
        assert_eq!(
            (
                missed.state().character.resources["coin"],
                missed.state().character.resources["stamina"]
            ),
            (14, 3)
        );
    }

    #[test]
    fn late_batch_uses_new_deadlines_and_preserves_paid_relief_resources() {
        let content = crate::load_content().unwrap();
        let mut session = prefix(&BATCH_LATE_SPEC, 128, &content);
        assert_eq!(session.state().world.time, 128);
        assert_eq!(
            (
                session.state().character.resources["coin"],
                session.state().character.resources["stamina"]
            ),
            (2, 4)
        );
        validate_deferred_history(session.state(), &[], &[]).unwrap();
        assert_eq!(surge_history(session.state()), vec![(16, false)]);
        let save = session.player_trace().unwrap();
        let resumed = forge_replay::resume_player_trace(&save, &content).unwrap();
        assert_eq!(resumed.state(), session.state());
        assert_eq!(resumed.trace(), session.trace());
        for step in &BATCH_LATE_STEPS[128..] {
            record_step(&mut session, &content, step).unwrap();
        }
        validate_session(&BATCH_LATE_SPEC, &session, &content).unwrap();
        assert_eq!(
            (
                session.state().character.resources["coin"],
                session.state().character.resources["stamina"]
            ),
            (5, 4)
        );
        assert_eq!(surge_history(session.state()), vec![(16, false)]);
        validate_npc_memories(session.state(), PAID_TOWLINE_REQUIRED_NPC_MEMORIES).unwrap();
    }

    #[test]
    fn remote_spoil_leaves_cast_uninformed_and_retires_freight_on_revisit() {
        let content = crate::load_content().unwrap();
        let mut session = run(&BATCH_REMOTE_SPEC, &content).unwrap();
        for npc in session.state().world.npcs.values() {
            assert!(!npc.knowledge.contains_key("fume_yards.batch_spoiled"));
            assert!(!npc.memories.contains_key("fume_yards.spoil_inspected"));
        }
        for step in [
            action!("world.enter_aftermath"),
            action!("return.visit_workshop"),
            action!("travel_adjacent", "destination" => "fume_yards.kiln_bay"),
        ] {
            record_step(&mut session, &content, &step).unwrap();
        }
        validate_legal_set(
            session.state(),
            &content,
            &["fume_yards.inspect_spoiled_batch"],
            &[
                "fume_yards.load_kiln_freight",
                "fume_yards.load_filtered_kiln_freight",
                "fume_yards.ignite_batch",
                "fume_yards.draw_filter",
            ],
        )
        .unwrap();
        validate_npc_knowledge(session.state(), &[], BATCH_NO_SPOIL_KNOWLEDGE).unwrap();
        validate_deferred_history(
            session.state(),
            BATCH_REMOTE_SPEC.expectations.deferred_events,
            &[],
        )
        .unwrap();
    }
}
