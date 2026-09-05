use forge_kernel::{
    CharacterChoiceSelection, CharacterSelection, CompiledContent, enumerate_legal_actions,
    sha256_json,
};
use forge_replay::{Session, TraceStart};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

use crate::deferred_evidence::{
    DeferredEventExpectation, PendingEventExpectation, validate_deferred_expectations,
    validate_deferred_history,
};
use crate::entropy_evidence::{
    EntropyExpectation, validate_entropy_expectations, validate_entropy_history,
};
use crate::storage_evidence::{
    StorageBalanceExpectation, StorageTransferDirection, StorageTransferExpectation,
    validate_storage_expectations, validate_storage_history,
};
use crate::{VerifyError, replay_error};

const SCENARIO_BINDING_FORMAT: &str = "forge-scenario-spec-v1";
const RECIPE_BINDING_FORMAT: &str = "forge-scenario-recipe-v1";

const REQUIRED_SCENARIO_IDS: &[&str] = &[
    "m2-fume-crew-staffed",
    "m2-fume-crew-ordinary",
    "m2-fume-crew-other-history",
    "m2-fume-crew-no-account",
    "m2-fume-crew-no-help",
    "m2-fume-crew-walked-away",
    "m2-fume-crew-cancelled",
    "m2-fume-crew-water-composed",
    "m2-fume-collateral-purchase",
    "m2-fume-collateral-fuel",
    "m2-fume-collateral-local",
    "m2-fume-collateral-sale",
    "m2-fume-market-water",
    "m2-fume-market-water-composed",
    "m2-fume-market-cask-delivered",
    "m2-fume-market-cask-undelivered",
    "m2-fume-collateral-after-report",
    "m2-fume-salvage-safe-sale",
    "m2-fume-salvage-safe-local",
    "m2-fume-salvage-skilled",
    "m2-fume-salvage-intact",
    "m2-fume-salvage-broken",
    "m2-fume-salvage-prior-filter-broken",
    "m2-fume-salvage-protected-production",
    "m2-fume-salvage-reported",
    "m2-fume-salvage-unreported",
    "m2-fume-batch-ready",
    "m2-fume-manufacture-local",
    "m2-fume-manufacture-sale",
    "m2-fume-manufacture-bare",
    "m2-fume-remote-spoil",
    "m2-fume-bank-save-tide",
    "m2-fume-draw-miss-tide",
    "m2-fume-late-manufacture",
    "m2-fume-reclaim-charge",
    "m2-fume-cold-repair",
    "m2-fume-cold-screen",
    "m2-fume-unscreened-freight",
    "m2-fume-late-repair",
    "m0-ilyan",
    "m0-rook",
    "m1-custom-cross-current",
    "m1-custom-unlikely-ally",
    "m1-deadline-missed-surge",
    "m1-outcome-split-flow",
    "m1-outcome-hold-market",
    "m1-outcome-relief-channel",
    "m1-outcome-break-toll",
    "m1-outcome-overload-disaster",
    "m1-area-lowsail-market",
    "m1-area-red-sluice",
    "m1-tide-key-split-flow",
    "m1-warning-unrelayed",
    "m1-warning-relayed",
    "m1-paid-towline-relief",
];

const OUTCOME_DEFINITIONS: &[&str] = &[
    "top.split_flow",
    "top.hold_market",
    "top.divert_relief",
    "top.break_toll",
    "top.overload",
];

#[derive(Clone, Copy, Debug, Serialize)]
pub(super) struct ScenarioStep {
    pub definition_id: &'static str,
    pub parameters: &'static [(&'static str, &'static str)],
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ScenarioKnowledgeProvenance {
    Witnessed,
    Read { source: &'static str },
    Told { by: &'static str },
    Rumor { from: &'static str },
}

#[derive(Clone, Copy, Debug, Serialize)]
struct ScenarioNpcKnowledgeExpectation {
    npc: &'static str,
    knowledge_id: &'static str,
    provenance: ScenarioKnowledgeProvenance,
    turn: u64,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct ScenarioNpcKnowledgeAbsence {
    npc: &'static str,
    knowledge_id: &'static str,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct ScenarioNpcMemoryExpectation {
    npc: &'static str,
    memory_id: &'static str,
    provenance: ScenarioKnowledgeProvenance,
    turn: u64,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct ScenarioNpcLocationExpectation {
    npc: &'static str,
    location: &'static str,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct ScenarioInventoryExpectation {
    item: &'static str,
    count: u32,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct ScenarioResourceExpectation {
    resource: &'static str,
    amount: i64,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct ScenarioNpcInventoryExpectation {
    npc: &'static str,
    item: &'static str,
    count: u32,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct ScenarioNpcInventoryAbsence {
    npc: &'static str,
    item: &'static str,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct ScenarioRecipeExpectation {
    turn: u64,
    recipe: &'static str,
    inputs: &'static [(&'static str, u32)],
    outputs: &'static [(&'static str, u32)],
}

#[derive(Clone, Copy, Debug, Serialize)]
pub(super) struct ScenarioExpectations {
    #[serde(skip_serializing_if = "Option::is_none")]
    staffing_history: Option<&'static StaffingHistoryExpectation>,
    final_location: &'static str,
    final_action_definition: &'static str,
    final_observation_contains: &'static str,
    forbidden_observation_contains: &'static [&'static str],
    final_world_time: Option<u64>,
    exclusive_after_action: &'static str,
    required_world_flags: &'static [&'static str],
    forbidden_world_flags: &'static [&'static str],
    required_location_flags: &'static [(&'static str, &'static str)],
    forbidden_location_flags: &'static [(&'static str, &'static str)],
    required_deeds: &'static [&'static str],
    required_visited_locations: &'static [&'static str],
    required_npc_locations: &'static [ScenarioNpcLocationExpectation],
    required_npc_knowledge: &'static [ScenarioNpcKnowledgeExpectation],
    forbidden_npc_knowledge: &'static [ScenarioNpcKnowledgeAbsence],
    required_npc_memories: &'static [ScenarioNpcMemoryExpectation],
    required_character_inventory: &'static [ScenarioInventoryExpectation],
    required_character_resources: &'static [ScenarioResourceExpectation],
    required_npc_inventory: &'static [ScenarioNpcInventoryExpectation],
    forbidden_npc_inventory: &'static [ScenarioNpcInventoryAbsence],
    forbidden_character_inventory: &'static [&'static str],
    recipe_events: &'static [ScenarioRecipeExpectation],
    storage_balances: &'static [StorageBalanceExpectation],
    storage_transfers: &'static [StorageTransferExpectation],
    entropy_draws: &'static [EntropyExpectation],
    deferred_events: &'static [DeferredEventExpectation],
    pending_deferred_events: &'static [PendingEventExpectation],
    required_legal_definitions: &'static [&'static str],
    forbidden_legal_definitions: &'static [&'static str],
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ScenarioStartSpec {
    Preset {
        character_preset_id: &'static str,
    },
    Custom {
        name: &'static str,
        choices: &'static [(&'static str, &'static str)],
    },
}

#[derive(Clone, Copy, Debug, Serialize)]
pub(super) struct ScenarioSpec {
    pub id: &'static str,
    claim_id: &'static str,
    start: ScenarioStartSpec,
    seed: u64,
    pub steps: &'static [ScenarioStep],
    expectations: ScenarioExpectations,
}

macro_rules! action {
    ($definition:literal) => {
        ScenarioStep {
            definition_id: $definition,
            parameters: &[],
        }
    };
    ($definition:literal, $name:literal => $value:literal) => {
        ScenarioStep {
            definition_id: $definition,
            parameters: &[($name, $value)],
        }
    };
}

const M0_ILYAN_STEPS: &[ScenarioStep] = &[
    action!("checkpoint.audit_order"),
    action!("travel_adjacent", "destination" => "lowsail.levee"),
];

const M0_ROOK_STEPS: &[ScenarioStep] = &[
    action!("checkpoint.blend_workers"),
    action!("travel_adjacent", "destination" => "lowsail.levee"),
];

const CUSTOM_CROSS_CURRENT_CHOICES: &[(&str, &str)] = &[
    ("lineage", "fenborn"),
    ("origin", "red-sluice"),
    ("calling", "ledger-clerk"),
    ("value", "order"),
    ("burden", "wanted"),
    ("history", "stole-permit"),
];

const CUSTOM_CROSS_CURRENT_STEPS: &[ScenarioStep] = &[
    action!("checkpoint.audit_order"),
    action!("checkpoint.use_stolen_permit"),
    action!("travel_adjacent", "destination" => "lowsail.levee"),
];

const CUSTOM_UNLIKELY_ALLY_CHOICES: &[(&str, &str)] = &[
    ("lineage", "kilnborn"),
    ("origin", "lowsail"),
    ("calling", "lock-runner"),
    ("value", "order"),
    ("burden", "indebted"),
    ("history", "saved-worker"),
];

const CUSTOM_UNLIKELY_ALLY_STEPS: &[ScenarioStep] = &[
    action!("checkpoint.recall_worker"),
    action!("travel_adjacent", "destination" => "lowsail.levee"),
];

const DEADLINE_MISSED_STEPS: &[ScenarioStep] = &[
    action!("wait_tide"),
    action!("wait_tide"),
    action!("wait_tide"),
    action!("wait_tide"),
    action!("wait_tide"),
    action!("wait_tide"),
    action!("wait_tide"),
    action!("wait_tide"),
    action!("wait_tide"),
    action!("wait_tide"),
    action!("wait_tide"),
    action!("wait_tide"),
    action!("wait_tide"),
    action!("wait_tide"),
    action!("wait_tide"),
    action!("wait_tide"),
    action!("world.enter_aftermath"),
    action!("return.face_flood"),
];

const SPLIT_FLOW_STEPS: &[ScenarioStep] = &[
    action!("checkpoint.show_charter"),
    action!("travel_adjacent", "destination" => "lowsail.levee"),
    action!("levee.authority_path"),
    action!("floor.read_harmonics"),
    action!("travel_adjacent", "destination" => "red_sluice.top"),
    action!("top.check_wheels"),
    action!("top.split_flow"),
    action!("world.enter_aftermath"),
    action!("return.share_water"),
];

const HOLD_MARKET_STEPS: &[ScenarioStep] = &[
    action!("checkpoint.show_charter"),
    action!("travel_adjacent", "destination" => "lowsail.levee"),
    action!("levee.authority_path"),
    action!("travel_adjacent", "destination" => "red_sluice.top"),
    action!("top.hold_market"),
    action!("world.enter_aftermath"),
    action!("return.count_dry_stalls"),
];

const WARNING_UNRELAYED_STEPS: &[ScenarioStep] = &[
    action!("checkpoint.show_charter"),
    action!("travel_adjacent", "destination" => "lowsail.docks"),
    action!("docks.ring_warning"),
    action!("travel_adjacent", "destination" => "lowsail.levee"),
    action!("wait_tide"),
    action!("levee.authority_path"),
];

const WARNING_RELAYED_STEPS: &[ScenarioStep] = &[
    action!("checkpoint.show_charter"),
    action!("travel_adjacent", "destination" => "lowsail.docks"),
    action!("docks.ring_warning"),
    action!("travel_adjacent", "destination" => "lowsail.levee"),
    action!("levee.relay_warning"),
    action!("levee.authority_path"),
];

const OREN_MARKET_WARNING_KNOWLEDGE: &[ScenarioNpcKnowledgeExpectation] =
    &[ScenarioNpcKnowledgeExpectation {
        npc: "oren_pell",
        knowledge_id: "market_warned",
        provenance: ScenarioKnowledgeProvenance::Witnessed,
        turn: 2,
    }];

const UNRELAYED_FORBIDDEN_NPC_KNOWLEDGE: &[ScenarioNpcKnowledgeAbsence] =
    &[ScenarioNpcKnowledgeAbsence {
        npc: "edrik_voss",
        knowledge_id: "market_warned",
    }];

const RELAYED_REQUIRED_NPC_KNOWLEDGE: &[ScenarioNpcKnowledgeExpectation] = &[
    ScenarioNpcKnowledgeExpectation {
        npc: "oren_pell",
        knowledge_id: "market_warned",
        provenance: ScenarioKnowledgeProvenance::Witnessed,
        turn: 2,
    },
    ScenarioNpcKnowledgeExpectation {
        npc: "edrik_voss",
        knowledge_id: "market_warned",
        provenance: ScenarioKnowledgeProvenance::Rumor { from: "oren_pell" },
        turn: 4,
    },
];

const AFTERMATH_NPC_LOCATIONS: &[ScenarioNpcLocationExpectation] = &[
    ScenarioNpcLocationExpectation {
        npc: "oren_pell",
        location: "lowsail.return",
    },
    ScenarioNpcLocationExpectation {
        npc: "sava_rusk",
        location: "lowsail.return",
    },
    ScenarioNpcLocationExpectation {
        npc: "mira_kett",
        location: "lowsail.return",
    },
    ScenarioNpcLocationExpectation {
        npc: "yara_dene",
        location: "lowsail.docks",
    },
    ScenarioNpcLocationExpectation {
        npc: "edrik_voss",
        location: "red_sluice.floor",
    },
];

const RELIEF_CHANNEL_STEPS: &[ScenarioStep] = &[
    action!("travel_adjacent", "destination" => "lowsail.docks"),
    action!("docks.ring_warning"),
    action!("docks.ask_oren"),
    action!("travel_adjacent", "destination" => "lowsail.levee"),
    action!("levee.relay_warning"),
    action!("levee.culvert_path"),
    action!("floor.open_relief"),
    action!("travel_adjacent", "destination" => "red_sluice.top"),
    action!("top.divert_relief"),
    action!("world.enter_aftermath"),
    action!("return.move_inland"),
];

const PAID_TOWLINE_RELIEF_STEPS: &[ScenarioStep] = &[
    action!("travel_adjacent", "destination" => "lowsail.docks"),
    action!("docks.ring_warning"),
    action!("docks.rig_towline"),
    action!("levee.relay_warning"),
    action!("levee.culvert_path"),
    action!("floor.open_relief"),
    action!("travel_adjacent", "destination" => "red_sluice.top"),
    action!("top.check_wheels"),
    action!("top.divert_relief"),
    action!("world.enter_aftermath"),
    action!("return.move_inland"),
];

const BREAK_TOLL_STEPS: &[ScenarioStep] = &[
    action!("checkpoint.blend_workers"),
    action!("travel_adjacent", "destination" => "lowsail.levee"),
    action!("levee.culvert_path"),
    action!("travel_adjacent", "destination" => "red_sluice.top"),
    action!("top.break_toll"),
    action!("world.enter_aftermath"),
    action!("return.open_ferry"),
];

const OVERLOAD_STEPS: &[ScenarioStep] = &[
    action!("checkpoint.use_stolen_permit"),
    action!("travel_adjacent", "destination" => "lowsail.levee"),
    action!("levee.stolen_path"),
    action!("floor.force_wheel"),
    action!("travel_adjacent", "destination" => "red_sluice.top"),
    action!("top.overload"),
    action!("world.enter_aftermath"),
    action!("return.face_flood"),
];

const LOWSAIL_AREA_STEPS: &[ScenarioStep] = &[
    action!("checkpoint.blend_workers"),
    action!("travel_adjacent", "destination" => "lowsail.docks"),
    action!("docks.press_yara"),
    action!("docks.ring_warning"),
    action!("travel_adjacent", "destination" => "lowsail.levee"),
    action!("levee.relay_warning"),
    action!("levee.culvert_path"),
    action!("floor.open_relief"),
    action!("travel_adjacent", "destination" => "red_sluice.top"),
    action!("top.divert_relief"),
    action!("world.enter_aftermath"),
    action!("return.move_inland"),
];

const RED_SLUICE_AREA_STEPS: &[ScenarioStep] = &[
    action!("checkpoint.audit_order"),
    action!("checkpoint.show_charter"),
    action!("travel_adjacent", "destination" => "lowsail.levee"),
    action!("levee.inspect_damage"),
    action!("levee.send_report"),
    action!("levee.authority_path"),
    action!("floor.test_pressure"),
    action!("floor.stabilize_gauge"),
    action!("floor.read_harmonics"),
    action!("travel_adjacent", "destination" => "red_sluice.top"),
    action!("top.rescue_worker"),
    action!("top.check_wheels"),
    action!("top.split_flow"),
    action!("world.enter_aftermath"),
    action!("return.share_water"),
];

const TIDE_KEY_SPLIT_FLOW_STEPS: &[ScenarioStep] = &[
    action!("travel_adjacent", "destination" => "lowsail.docks"),
    action!("docks.press_yara"),
    action!("docks.ask_oren"),
    action!("travel_adjacent", "destination" => "lowsail.levee"),
    action!("levee.culvert_path"),
    action!("floor.key_calibration"),
    action!("travel_adjacent", "destination" => "red_sluice.top"),
    action!("top.check_wheels"),
    action!("top.split_flow"),
    action!("world.enter_aftermath"),
    action!("return.share_water"),
];

const TIDE_KEY_CHARACTER_INVENTORY: &[ScenarioInventoryExpectation] =
    &[ScenarioInventoryExpectation {
        item: "split_tide.tide_key",
        count: 1,
    }];

const TIDE_KEY_YARA_INVENTORY_ABSENCE: &[ScenarioNpcInventoryAbsence] =
    &[ScenarioNpcInventoryAbsence {
        npc: "yara_dene",
        item: "split_tide.tide_key",
    }];

const PAID_TOWLINE_REQUIRED_NPC_MEMORIES: &[ScenarioNpcMemoryExpectation] =
    &[ScenarioNpcMemoryExpectation {
        npc: "oren_pell",
        memory_id: "oren_saw_towline",
        provenance: ScenarioKnowledgeProvenance::Witnessed,
        turn: 2,
    }];

const PAID_TOWLINE_REQUIRED_NPC_KNOWLEDGE: &[ScenarioNpcKnowledgeExpectation] = &[
    ScenarioNpcKnowledgeExpectation {
        npc: "oren_pell",
        knowledge_id: "market_warned",
        provenance: ScenarioKnowledgeProvenance::Witnessed,
        turn: 1,
    },
    ScenarioNpcKnowledgeExpectation {
        npc: "edrik_voss",
        knowledge_id: "market_warned",
        provenance: ScenarioKnowledgeProvenance::Rumor { from: "oren_pell" },
        turn: 3,
    },
];

const PAID_TOWLINE_CHARACTER_INVENTORY: &[ScenarioInventoryExpectation] = &[
    ScenarioInventoryExpectation {
        item: "rope",
        count: 1,
    },
    ScenarioInventoryExpectation {
        item: "wire",
        count: 1,
    },
];

const PAID_TOWLINE_CHARACTER_RESOURCES: &[ScenarioResourceExpectation] =
    &[ScenarioResourceExpectation {
        resource: "coin",
        amount: 2,
    }];

#[cfg(test)]
const WRONG_TIDE_KEY_CHARACTER_INVENTORY: &[ScenarioInventoryExpectation] =
    &[ScenarioInventoryExpectation {
        item: "split_tide.tide_key",
        count: 2,
    }];

#[cfg(test)]
const EMPTY_NPC_INVENTORY_ABSENCE: &[ScenarioNpcInventoryAbsence] = &[];

const fn append_steps<const N: usize>(
    first: &[ScenarioStep],
    second: &[ScenarioStep],
) -> [ScenarioStep; N] {
    assert!(N == first.len() + second.len());
    let mut result = [action!("wait_tide"); N];
    let mut i = 0;
    while i < first.len() {
        result[i] = first[i];
        i += 1;
    }
    let mut j = 0;
    while j < second.len() {
        result[i + j] = second[j];
        j += 1;
    }
    result
}
const PILOT_REPAIR_EXTENSION: &[ScenarioStep] = &[
    action!("return.visit_workshop"),
    action!("fume_yards.take_stock"),
    action!("fume_yards.press_repair_plugs"),
    action!("fume_yards.load_freight"),
    action!("world.enter_aftermath"),
    action!("return.patch_stand"),
    action!("return.sort_dry_goods"),
    action!("return.visit_workshop"),
];
const PILOT_REPAIR_STEPS: [ScenarioStep; 15] =
    append_steps(HOLD_MARKET_STEPS, PILOT_REPAIR_EXTENSION);
const PILOT_SCREEN_STEPS: [ScenarioStep; 14] = append_steps(
    HOLD_MARKET_STEPS,
    &[
        action!("return.visit_workshop"),
        action!("fume_yards.take_stock"),
        action!("fume_yards.pack_catch_screen"),
        action!("fume_yards.fit_catch_screen"),
        action!("fume_yards.load_screened_freight"),
        action!("world.enter_aftermath"),
        action!("return.visit_workshop"),
    ],
);
const PILOT_UNSCREENED_STEPS: [ScenarioStep; 12] = append_steps(
    HOLD_MARKET_STEPS,
    &[
        action!("return.visit_workshop"),
        action!("fume_yards.take_stock"),
        action!("fume_yards.load_freight"),
        action!("world.enter_aftermath"),
        action!("return.visit_workshop"),
    ],
);
const PILOT_LATE_PREFIX: [ScenarioStep; 128] =
    append_steps(PAID_TOWLINE_RELIEF_STEPS, &[action!("wait_tide"); 117]);
const PILOT_LATE_STEPS: [ScenarioStep; 136] =
    append_steps(&PILOT_LATE_PREFIX, PILOT_REPAIR_EXTENSION);
const PILOT_INPUTS: &[(&str, u32)] = &[("fume_yards.clay", 2), ("fume_yards.mesh", 1)];
const PILOT_REPAIR_EVENTS: &[ScenarioRecipeExpectation] = &[
    ScenarioRecipeExpectation {
        turn: 9,
        recipe: "fume_yards.press_repair_plugs",
        inputs: PILOT_INPUTS,
        outputs: &[("fume_yards.repair_lot", 1)],
    },
    ScenarioRecipeExpectation {
        turn: 12,
        recipe: "fume_yards.patch_stand",
        inputs: &[("fume_yards.repair_lot", 1)],
        outputs: &[],
    },
];
const PILOT_SCREEN_EVENTS: &[ScenarioRecipeExpectation] = &[
    ScenarioRecipeExpectation {
        turn: 9,
        recipe: "fume_yards.pack_catch_screen",
        inputs: PILOT_INPUTS,
        outputs: &[("fume_yards.catch_screen", 1)],
    },
    ScenarioRecipeExpectation {
        turn: 10,
        recipe: "fume_yards.fit_catch_screen",
        inputs: &[("fume_yards.catch_screen", 1)],
        outputs: &[],
    },
];
const PILOT_LATE_EVENTS: &[ScenarioRecipeExpectation] = &[
    ScenarioRecipeExpectation {
        turn: 130,
        recipe: "fume_yards.press_repair_plugs",
        inputs: PILOT_INPUTS,
        outputs: &[("fume_yards.repair_lot", 1)],
    },
    ScenarioRecipeExpectation {
        turn: 133,
        recipe: "fume_yards.patch_stand",
        inputs: &[("fume_yards.repair_lot", 1)],
        outputs: &[],
    },
];
const PILOT_SPENT_ITEMS: &[&str] = &[
    "fume_yards.clay",
    "fume_yards.mesh",
    "fume_yards.repair_lot",
    "fume_yards.catch_screen",
];
const PILOT_EMPTY_NESSA: &[ScenarioNpcInventoryAbsence] = &[
    ScenarioNpcInventoryAbsence {
        npc: "fume_yards.nessa_tern",
        item: "fume_yards.clay",
    },
    ScenarioNpcInventoryAbsence {
        npc: "fume_yards.nessa_tern",
        item: "fume_yards.mesh",
    },
];

const UNTOUCHED_COLLATERAL: &[StorageBalanceExpectation] = &[StorageBalanceExpectation {
    storage: "fume_yards.collateral_cage",
    inventory: &[("fume_yards.filter", 1)],
}];

include!("batch_scenarios.inc.rs");
include!("salvage_scenarios.inc.rs");
include!("market_scenarios.inc.rs");
include!("staffing_scenarios.inc.rs");

const SCENARIOS: &[ScenarioSpec] = &[
    ScenarioSpec {
        id: "m0-ilyan",
        claim_id: "milestone-0.character-path.ilyan",
        start: ScenarioStartSpec::Preset {
            character_preset_id: "ilyan",
        },
        seed: 71,
        steps: M0_ILYAN_STEPS,
        expectations: ScenarioExpectations {
            staffing_history: None,
            final_location: "lowsail.levee",
            final_action_definition: "travel_adjacent",
            final_observation_contains: "",
            forbidden_observation_contains: &[],
            final_world_time: None,
            exclusive_after_action: "",
            required_world_flags: &["forged_order_found"],
            forbidden_world_flags: &[],
            forbidden_location_flags: &[],
            required_location_flags: &[("lowsail_market", "order_audited")],
            required_deeds: &["read_forged_order"],
            required_visited_locations: &["lowsail_market", "lowsail.levee"],
            required_npc_locations: &[],
            required_npc_knowledge: &[],
            forbidden_npc_knowledge: &[],
            required_npc_memories: &[],
            required_character_inventory: &[],
            required_character_resources: &[],
            required_npc_inventory: &[],
            forbidden_npc_inventory: &[],
            forbidden_character_inventory: &[],
            recipe_events: &[],
            storage_balances: UNTOUCHED_COLLATERAL,
            storage_transfers: &[],
            entropy_draws: &[],
            deferred_events: &[],
            pending_deferred_events: &[],
            required_legal_definitions: &[],
            forbidden_legal_definitions: &[],
        },
    },
    ScenarioSpec {
        id: "m0-rook",
        claim_id: "milestone-0.character-path.rook",
        start: ScenarioStartSpec::Preset {
            character_preset_id: "rook",
        },
        seed: 71,
        steps: M0_ROOK_STEPS,
        expectations: ScenarioExpectations {
            staffing_history: None,
            final_location: "lowsail.levee",
            final_action_definition: "travel_adjacent",
            final_observation_contains: "",
            forbidden_observation_contains: &[],
            final_world_time: None,
            exclusive_after_action: "",
            required_world_flags: &["culvert_revealed"],
            forbidden_world_flags: &[],
            forbidden_location_flags: &[],
            required_location_flags: &[("lowsail_market", "worker_cover")],
            required_deeds: &["found_worker_cover"],
            required_visited_locations: &["lowsail_market", "lowsail.levee"],
            required_npc_locations: &[],
            required_npc_knowledge: &[],
            forbidden_npc_knowledge: &[],
            required_npc_memories: &[],
            required_character_inventory: &[],
            required_character_resources: &[],
            required_npc_inventory: &[],
            forbidden_npc_inventory: &[],
            forbidden_character_inventory: &[],
            recipe_events: &[],
            storage_balances: UNTOUCHED_COLLATERAL,
            storage_transfers: &[],
            entropy_draws: &[],
            deferred_events: &[],
            pending_deferred_events: &[],
            required_legal_definitions: &[],
            forbidden_legal_definitions: &[],
        },
    },
    ScenarioSpec {
        id: "m1-custom-cross-current",
        claim_id: "milestone-1.character-creation.cross-current",
        start: ScenarioStartSpec::Custom {
            name: "Mara Venn",
            choices: CUSTOM_CROSS_CURRENT_CHOICES,
        },
        seed: 71,
        steps: CUSTOM_CROSS_CURRENT_STEPS,
        expectations: ScenarioExpectations {
            staffing_history: None,
            final_location: "lowsail.levee",
            final_action_definition: "travel_adjacent",
            final_observation_contains: "",
            forbidden_observation_contains: &[],
            final_world_time: None,
            exclusive_after_action: "",
            required_world_flags: &["forged_order_found", "stolen_route"],
            forbidden_world_flags: &[],
            forbidden_location_flags: &[],
            required_location_flags: &[("lowsail_market", "order_audited")],
            required_deeds: &["stole_permit", "read_forged_order"],
            required_visited_locations: &["lowsail_market", "lowsail.levee"],
            required_npc_locations: &[],
            required_npc_knowledge: &[],
            forbidden_npc_knowledge: &[],
            required_npc_memories: &[],
            required_character_inventory: &[],
            required_character_resources: &[],
            required_npc_inventory: &[],
            forbidden_npc_inventory: &[],
            forbidden_character_inventory: &[],
            recipe_events: &[],
            storage_balances: UNTOUCHED_COLLATERAL,
            storage_transfers: &[],
            entropy_draws: &[],
            deferred_events: &[],
            pending_deferred_events: &[],
            required_legal_definitions: &[],
            forbidden_legal_definitions: &[],
        },
    },
    ScenarioSpec {
        id: "m1-custom-unlikely-ally",
        claim_id: "milestone-1.character-creation.unlikely-ally",
        start: ScenarioStartSpec::Custom {
            name: "Tarin Holt",
            choices: CUSTOM_UNLIKELY_ALLY_CHOICES,
        },
        seed: 71,
        steps: CUSTOM_UNLIKELY_ALLY_STEPS,
        expectations: ScenarioExpectations {
            staffing_history: None,
            final_location: "lowsail.levee",
            final_action_definition: "travel_adjacent",
            final_observation_contains: "",
            forbidden_observation_contains: &[],
            final_world_time: None,
            exclusive_after_action: "",
            required_world_flags: &["worker_credit"],
            forbidden_world_flags: &[],
            forbidden_location_flags: &[],
            required_location_flags: &[],
            required_deeds: &["saved_worker"],
            required_visited_locations: &["lowsail_market", "lowsail.levee"],
            required_npc_locations: &[],
            required_npc_knowledge: &[],
            forbidden_npc_knowledge: &[],
            required_npc_memories: &[],
            required_character_inventory: &[],
            required_character_resources: &[],
            required_npc_inventory: &[],
            forbidden_npc_inventory: &[],
            forbidden_character_inventory: &[],
            recipe_events: &[],
            storage_balances: UNTOUCHED_COLLATERAL,
            storage_transfers: &[],
            entropy_draws: &[],
            deferred_events: &[],
            pending_deferred_events: &[],
            required_legal_definitions: &[],
            forbidden_legal_definitions: &[],
        },
    },
    ScenarioSpec {
        id: "m1-deadline-missed-surge",
        claim_id: "split-tide.deadline.missed-surge",
        start: ScenarioStartSpec::Preset {
            character_preset_id: "ilyan",
        },
        seed: 71,
        steps: DEADLINE_MISSED_STEPS,
        expectations: ScenarioExpectations {
            staffing_history: None,
            final_location: "lowsail.return",
            final_action_definition: "return.face_flood",
            final_observation_contains: "You face the flooded market and answer for the broken gates.",
            forbidden_observation_contains: &[],
            final_world_time: None,
            exclusive_after_action: "",
            required_world_flags: &[
                "sluice_outcome_chosen",
                "surge_missed",
                "sluice_failure",
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
            forbidden_location_flags: &[],
            required_location_flags: &[("lowsail.return", "market_flooded")],
            required_deeds: &["faced_flood"],
            required_visited_locations: &["lowsail_market", "lowsail.return"],
            required_npc_locations: AFTERMATH_NPC_LOCATIONS,
            required_npc_knowledge: &[],
            forbidden_npc_knowledge: &[],
            required_npc_memories: &[],
            required_character_inventory: &[],
            required_character_resources: &[],
            required_npc_inventory: &[],
            forbidden_npc_inventory: &[],
            forbidden_character_inventory: &[],
            recipe_events: &[],
            storage_balances: UNTOUCHED_COLLATERAL,
            storage_transfers: &[],
            entropy_draws: &[],
            deferred_events: &[],
            pending_deferred_events: &[],
            required_legal_definitions: &[],
            forbidden_legal_definitions: &[],
        },
    },
    ScenarioSpec {
        id: "m1-outcome-split-flow",
        claim_id: "split-tide.outcome.split-flow",
        start: ScenarioStartSpec::Preset {
            character_preset_id: "ilyan",
        },
        seed: 71,
        steps: SPLIT_FLOW_STEPS,
        expectations: ScenarioExpectations {
            staffing_history: None,
            final_location: "lowsail.return",
            final_action_definition: "return.share_water",
            final_observation_contains: "Mira records the shared flow as a new market charter.",
            forbidden_observation_contains: &[],
            final_world_time: None,
            exclusive_after_action: "top.split_flow",
            required_world_flags: &[
                "sluice_calibrated",
                "sluice_outcome_chosen",
                "flow_split",
                "ending_accord",
            ],
            forbidden_world_flags: &[
                "flow_locked_market",
                "flow_relief",
                "old_channel_open",
                "sluice_failure",
                "ending_council",
                "ending_relief",
                "ending_freedom",
                "ending_disaster",
            ],
            forbidden_location_flags: &[],
            required_location_flags: &[
                ("red_sluice.floor", "authorized_entry"),
                ("red_sluice.top", "wheels_checked"),
                ("lowsail.return", "market_stable"),
            ],
            required_deeds: &["tuned_sluice", "shared_water", "returned_for_accord"],
            required_visited_locations: &[
                "lowsail_market",
                "lowsail.levee",
                "red_sluice.floor",
                "red_sluice.top",
                "lowsail.return",
            ],
            required_npc_locations: AFTERMATH_NPC_LOCATIONS,
            required_npc_knowledge: &[],
            forbidden_npc_knowledge: &[],
            required_npc_memories: &[],
            required_character_inventory: &[],
            required_character_resources: &[],
            required_npc_inventory: &[],
            forbidden_npc_inventory: &[],
            forbidden_character_inventory: &[],
            recipe_events: &[],
            storage_balances: UNTOUCHED_COLLATERAL,
            storage_transfers: &[],
            entropy_draws: &[],
            deferred_events: &[],
            pending_deferred_events: &[],
            required_legal_definitions: &[],
            forbidden_legal_definitions: OUTCOME_DEFINITIONS,
        },
    },
    ScenarioSpec {
        id: "m1-outcome-hold-market",
        claim_id: "split-tide.outcome.hold-market",
        start: ScenarioStartSpec::Preset {
            character_preset_id: "ilyan",
        },
        seed: 71,
        steps: HOLD_MARKET_STEPS,
        expectations: ScenarioExpectations {
            staffing_history: None,
            final_location: "lowsail.return",
            final_action_definition: "return.count_dry_stalls",
            final_observation_contains: "You enforce council control while the upland works absorb the loss.",
            forbidden_observation_contains: &[],
            final_world_time: None,
            exclusive_after_action: "top.hold_market",
            required_world_flags: &[
                "council_route",
                "sluice_outcome_chosen",
                "flow_locked_market",
                "ending_council",
            ],
            forbidden_world_flags: &[
                "flow_split",
                "flow_relief",
                "old_channel_open",
                "sluice_failure",
                "ending_accord",
                "ending_relief",
                "ending_freedom",
                "ending_disaster",
            ],
            forbidden_location_flags: &[],
            required_location_flags: &[
                ("lowsail_market", "market_permit"),
                ("red_sluice.floor", "authorized_entry"),
                ("lowsail.return", "market_stable"),
                ("lowsail.return", "upland_dry"),
            ],
            required_deeds: &["backed_council", "accepted_council"],
            required_visited_locations: &[
                "lowsail_market",
                "lowsail.levee",
                "red_sluice.floor",
                "red_sluice.top",
                "lowsail.return",
            ],
            required_npc_locations: AFTERMATH_NPC_LOCATIONS,
            required_npc_knowledge: &[],
            forbidden_npc_knowledge: &[],
            required_npc_memories: &[],
            required_character_inventory: &[],
            required_character_resources: &[],
            required_npc_inventory: &[],
            forbidden_npc_inventory: &[],
            forbidden_character_inventory: &[],
            recipe_events: &[],
            storage_balances: UNTOUCHED_COLLATERAL,
            storage_transfers: &[],
            entropy_draws: &[],
            deferred_events: &[],
            pending_deferred_events: &[],
            required_legal_definitions: &[],
            forbidden_legal_definitions: OUTCOME_DEFINITIONS,
        },
    },
    ScenarioSpec {
        id: "m1-warning-unrelayed",
        claim_id: "split-tide.warning.unrelayed",
        start: ScenarioStartSpec::Preset {
            character_preset_id: "ilyan",
        },
        seed: 71,
        steps: WARNING_UNRELAYED_STEPS,
        expectations: ScenarioExpectations {
            staffing_history: None,
            final_location: "red_sluice.floor",
            final_action_definition: "levee.authority_path",
            final_observation_contains: "",
            forbidden_observation_contains: &["Edrik knows Lowsail has been warned"],
            final_world_time: Some(6),
            exclusive_after_action: "",
            required_world_flags: &["market_warned"],
            forbidden_world_flags: &[],
            forbidden_location_flags: &[],
            required_location_flags: &[("red_sluice.floor", "authorized_entry")],
            required_deeds: &[],
            required_visited_locations: &[
                "lowsail_market",
                "lowsail.docks",
                "lowsail.levee",
                "red_sluice.floor",
            ],
            required_npc_locations: &[],
            required_npc_knowledge: OREN_MARKET_WARNING_KNOWLEDGE,
            forbidden_npc_knowledge: UNRELAYED_FORBIDDEN_NPC_KNOWLEDGE,
            required_npc_memories: &[],
            required_character_inventory: &[],
            required_character_resources: &[],
            required_npc_inventory: &[],
            forbidden_npc_inventory: &[],
            forbidden_character_inventory: &[],
            recipe_events: &[],
            storage_balances: UNTOUCHED_COLLATERAL,
            storage_transfers: &[],
            entropy_draws: &[],
            deferred_events: &[],
            pending_deferred_events: &[],
            required_legal_definitions: &[],
            forbidden_legal_definitions: &["floor.open_relief"],
        },
    },
    ScenarioSpec {
        id: "m1-warning-relayed",
        claim_id: "split-tide.warning.relayed",
        start: ScenarioStartSpec::Preset {
            character_preset_id: "ilyan",
        },
        seed: 71,
        steps: WARNING_RELAYED_STEPS,
        expectations: ScenarioExpectations {
            staffing_history: None,
            final_location: "red_sluice.floor",
            final_action_definition: "levee.authority_path",
            final_observation_contains: "Edrik knows Lowsail has been warned",
            forbidden_observation_contains: &[],
            final_world_time: Some(6),
            exclusive_after_action: "",
            required_world_flags: &["market_warned"],
            forbidden_world_flags: &[],
            forbidden_location_flags: &[],
            required_location_flags: &[
                ("red_sluice.floor", "authorized_entry"),
                ("red_sluice.floor", "warning_received"),
            ],
            required_deeds: &[],
            required_visited_locations: &[
                "lowsail_market",
                "lowsail.docks",
                "lowsail.levee",
                "red_sluice.floor",
            ],
            required_npc_locations: &[],
            required_npc_knowledge: RELAYED_REQUIRED_NPC_KNOWLEDGE,
            forbidden_npc_knowledge: &[],
            required_npc_memories: &[],
            required_character_inventory: &[],
            required_character_resources: &[],
            required_npc_inventory: &[],
            forbidden_npc_inventory: &[],
            forbidden_character_inventory: &[],
            recipe_events: &[],
            storage_balances: UNTOUCHED_COLLATERAL,
            storage_transfers: &[],
            entropy_draws: &[],
            deferred_events: &[],
            pending_deferred_events: &[],
            required_legal_definitions: &["floor.open_relief"],
            forbidden_legal_definitions: &[],
        },
    },
    ScenarioSpec {
        id: "m1-outcome-relief-channel",
        claim_id: "split-tide.outcome.relief-channel",
        start: ScenarioStartSpec::Preset {
            character_preset_id: "ilyan",
        },
        seed: 71,
        steps: RELIEF_CHANNEL_STEPS,
        expectations: ScenarioExpectations {
            staffing_history: None,
            final_location: "lowsail.return",
            final_action_definition: "return.move_inland",
            final_observation_contains: "You help families open a higher market beyond the next surge.",
            forbidden_observation_contains: &[],
            final_world_time: None,
            exclusive_after_action: "top.divert_relief",
            required_world_flags: &[
                "market_warned",
                "relief_channel_open",
                "sluice_outcome_chosen",
                "flow_relief",
                "ending_relief",
            ],
            forbidden_world_flags: &[
                "flow_split",
                "flow_locked_market",
                "old_channel_open",
                "sluice_failure",
                "ending_accord",
                "ending_council",
                "ending_freedom",
                "ending_disaster",
            ],
            forbidden_location_flags: &[],
            required_location_flags: &[
                ("red_sluice.floor", "culvert_entry"),
                ("red_sluice.top", "relief_ready"),
                ("lowsail.return", "market_moved"),
            ],
            required_deeds: &["opened_relief", "accepted_relocation"],
            required_visited_locations: &[
                "lowsail_market",
                "lowsail.docks",
                "lowsail.levee",
                "red_sluice.floor",
                "red_sluice.top",
                "lowsail.return",
            ],
            required_npc_locations: AFTERMATH_NPC_LOCATIONS,
            required_npc_knowledge: &[],
            forbidden_npc_knowledge: &[],
            required_npc_memories: &[],
            required_character_inventory: &[],
            required_character_resources: &[],
            required_npc_inventory: &[],
            forbidden_npc_inventory: &[],
            forbidden_character_inventory: &[],
            recipe_events: &[],
            storage_balances: UNTOUCHED_COLLATERAL,
            storage_transfers: &[],
            entropy_draws: &[],
            deferred_events: &[],
            pending_deferred_events: &[],
            required_legal_definitions: &[],
            forbidden_legal_definitions: OUTCOME_DEFINITIONS,
        },
    },
    ScenarioSpec {
        id: "m1-outcome-break-toll",
        claim_id: "split-tide.outcome.break-toll",
        start: ScenarioStartSpec::Preset {
            character_preset_id: "rook",
        },
        seed: 71,
        steps: BREAK_TOLL_STEPS,
        expectations: ScenarioExpectations {
            staffing_history: None,
            final_location: "lowsail.return",
            final_action_definition: "return.open_ferry",
            final_observation_contains: "You abolish the toll and launch a free ferry between both shores.",
            forbidden_observation_contains: &[],
            final_world_time: None,
            exclusive_after_action: "top.break_toll",
            required_world_flags: &[
                "culvert_revealed",
                "sluice_outcome_chosen",
                "old_channel_open",
                "ending_freedom",
            ],
            forbidden_world_flags: &[
                "flow_split",
                "flow_locked_market",
                "flow_relief",
                "sluice_failure",
                "ending_accord",
                "ending_council",
                "ending_relief",
                "ending_disaster",
            ],
            forbidden_location_flags: &[],
            required_location_flags: &[
                ("red_sluice.floor", "culvert_entry"),
                ("lowsail.return", "ferry_free"),
            ],
            required_deeds: &["freed_ferry", "opened_free_ferry"],
            required_visited_locations: &[
                "lowsail_market",
                "lowsail.levee",
                "red_sluice.floor",
                "red_sluice.top",
                "lowsail.return",
            ],
            required_npc_locations: AFTERMATH_NPC_LOCATIONS,
            required_npc_knowledge: &[],
            forbidden_npc_knowledge: &[],
            required_npc_memories: &[],
            required_character_inventory: &[],
            required_character_resources: &[],
            required_npc_inventory: &[],
            forbidden_npc_inventory: &[],
            forbidden_character_inventory: &[],
            recipe_events: &[],
            storage_balances: UNTOUCHED_COLLATERAL,
            storage_transfers: &[],
            entropy_draws: &[],
            deferred_events: &[],
            pending_deferred_events: &[],
            required_legal_definitions: &[],
            forbidden_legal_definitions: OUTCOME_DEFINITIONS,
        },
    },
    ScenarioSpec {
        id: "m1-outcome-overload-disaster",
        claim_id: "split-tide.outcome.overload-disaster",
        start: ScenarioStartSpec::Preset {
            character_preset_id: "rook",
        },
        seed: 71,
        steps: OVERLOAD_STEPS,
        expectations: ScenarioExpectations {
            staffing_history: None,
            final_location: "lowsail.return",
            final_action_definition: "return.face_flood",
            final_observation_contains: "You face the flooded market and answer for the broken gates.",
            forbidden_observation_contains: &[],
            final_world_time: None,
            exclusive_after_action: "top.overload",
            required_world_flags: &[
                "stolen_route",
                "sluice_breached",
                "sluice_outcome_chosen",
                "sluice_failure",
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
            forbidden_location_flags: &[],
            required_location_flags: &[
                ("red_sluice.floor", "forged_entry"),
                ("red_sluice.top", "worker_rescue_needed"),
                ("lowsail.return", "market_flooded"),
            ],
            required_deeds: &["forced_wheel", "overloaded_gates", "faced_flood"],
            required_visited_locations: &[
                "lowsail_market",
                "lowsail.levee",
                "red_sluice.floor",
                "red_sluice.top",
                "lowsail.return",
            ],
            required_npc_locations: AFTERMATH_NPC_LOCATIONS,
            required_npc_knowledge: &[],
            forbidden_npc_knowledge: &[],
            required_npc_memories: &[],
            required_character_inventory: &[],
            required_character_resources: &[],
            required_npc_inventory: &[],
            forbidden_npc_inventory: &[],
            forbidden_character_inventory: &[],
            recipe_events: &[],
            storage_balances: UNTOUCHED_COLLATERAL,
            storage_transfers: &[],
            entropy_draws: &[],
            deferred_events: &[],
            pending_deferred_events: &[],
            required_legal_definitions: &[],
            forbidden_legal_definitions: OUTCOME_DEFINITIONS,
        },
    },
    ScenarioSpec {
        id: "m1-area-lowsail-market",
        claim_id: "split-tide.area.lowsail-market",
        start: ScenarioStartSpec::Preset {
            character_preset_id: "rook",
        },
        seed: 71,
        steps: LOWSAIL_AREA_STEPS,
        expectations: ScenarioExpectations {
            staffing_history: None,
            final_location: "lowsail.return",
            final_action_definition: "return.move_inland",
            final_observation_contains: "You help families open a higher market beyond the next surge.",
            forbidden_observation_contains: &[],
            final_world_time: None,
            exclusive_after_action: "top.divert_relief",
            required_world_flags: &[
                "culvert_revealed",
                "tide_key_offered",
                "market_warned",
                "relief_channel_open",
                "sluice_outcome_chosen",
                "flow_relief",
                "ending_relief",
            ],
            forbidden_world_flags: &[
                "flow_split",
                "flow_locked_market",
                "old_channel_open",
                "sluice_failure",
                "ending_accord",
                "ending_council",
                "ending_freedom",
                "ending_disaster",
            ],
            forbidden_location_flags: &[],
            required_location_flags: &[
                ("lowsail_market", "worker_cover"),
                ("red_sluice.floor", "culvert_entry"),
                ("red_sluice.top", "relief_ready"),
                ("lowsail.return", "market_moved"),
            ],
            required_deeds: &[
                "found_worker_cover",
                "won_tide_key",
                "opened_relief",
                "accepted_relocation",
            ],
            required_visited_locations: &[
                "lowsail_market",
                "lowsail.docks",
                "lowsail.levee",
                "red_sluice.floor",
                "red_sluice.top",
                "lowsail.return",
            ],
            required_npc_locations: AFTERMATH_NPC_LOCATIONS,
            required_npc_knowledge: &[],
            forbidden_npc_knowledge: &[],
            required_npc_memories: &[],
            required_character_inventory: &[],
            required_character_resources: &[],
            required_npc_inventory: &[],
            forbidden_npc_inventory: &[],
            forbidden_character_inventory: &[],
            recipe_events: &[],
            storage_balances: UNTOUCHED_COLLATERAL,
            storage_transfers: &[],
            entropy_draws: &[],
            deferred_events: &[],
            pending_deferred_events: &[],
            required_legal_definitions: &[],
            forbidden_legal_definitions: OUTCOME_DEFINITIONS,
        },
    },
    ScenarioSpec {
        id: "m1-area-red-sluice",
        claim_id: "split-tide.area.red-sluice",
        start: ScenarioStartSpec::Preset {
            character_preset_id: "ilyan",
        },
        seed: 71,
        steps: RED_SLUICE_AREA_STEPS,
        expectations: ScenarioExpectations {
            staffing_history: None,
            final_location: "lowsail.return",
            final_action_definition: "return.share_water",
            final_observation_contains: "Mira records the shared flow as a new market charter.",
            forbidden_observation_contains: &[],
            final_world_time: None,
            exclusive_after_action: "top.split_flow",
            required_world_flags: &[
                "forged_order_found",
                "report_sent",
                "sluice_calibrated",
                "worker_rescued",
                "sluice_outcome_chosen",
                "flow_split",
                "ending_accord",
            ],
            forbidden_world_flags: &[
                "flow_locked_market",
                "flow_relief",
                "old_channel_open",
                "sluice_failure",
                "ending_council",
                "ending_relief",
                "ending_freedom",
                "ending_disaster",
            ],
            forbidden_location_flags: &[],
            required_location_flags: &[
                ("lowsail.levee", "damage_seen"),
                ("red_sluice.floor", "authorized_entry"),
                ("red_sluice.floor", "pressure_read"),
                ("red_sluice.floor", "gauge_stable"),
                ("red_sluice.floor", "harmonics_read"),
                ("red_sluice.top", "wheels_checked"),
                ("lowsail.return", "market_stable"),
            ],
            required_deeds: &[
                "read_forged_order",
                "sent_sluice_report",
                "tuned_sluice",
                "rescued_worker",
                "shared_water",
                "returned_for_accord",
            ],
            required_visited_locations: &[
                "lowsail_market",
                "lowsail.levee",
                "red_sluice.floor",
                "red_sluice.top",
                "lowsail.return",
            ],
            required_npc_locations: AFTERMATH_NPC_LOCATIONS,
            required_npc_knowledge: &[],
            forbidden_npc_knowledge: &[],
            required_npc_memories: &[],
            required_character_inventory: &[],
            required_character_resources: &[],
            required_npc_inventory: &[],
            forbidden_npc_inventory: &[],
            forbidden_character_inventory: &[],
            recipe_events: &[],
            storage_balances: UNTOUCHED_COLLATERAL,
            storage_transfers: &[],
            entropy_draws: &[],
            deferred_events: &[],
            pending_deferred_events: &[],
            required_legal_definitions: &[],
            forbidden_legal_definitions: OUTCOME_DEFINITIONS,
        },
    },
    ScenarioSpec {
        id: "m1-tide-key-split-flow",
        claim_id: "split-tide.witness.tide-key-split-flow",
        start: ScenarioStartSpec::Preset {
            character_preset_id: "rook",
        },
        seed: 71,
        steps: TIDE_KEY_SPLIT_FLOW_STEPS,
        expectations: ScenarioExpectations {
            staffing_history: None,
            final_location: "lowsail.return",
            final_action_definition: "return.share_water",
            final_observation_contains: "Mira records the shared flow as a new market charter.",
            forbidden_observation_contains: &[],
            final_world_time: Some(11),
            exclusive_after_action: "top.split_flow",
            required_world_flags: &[
                "tide_key_offered",
                "sluice_calibrated",
                "sluice_outcome_chosen",
                "flow_split",
                "ending_accord",
            ],
            forbidden_world_flags: &[
                "flow_locked_market",
                "flow_relief",
                "old_channel_open",
                "sluice_failure",
                "ending_council",
                "ending_relief",
                "ending_freedom",
                "ending_disaster",
            ],
            forbidden_location_flags: &[],
            required_location_flags: &[
                ("red_sluice.floor", "culvert_entry"),
                ("red_sluice.top", "wheels_checked"),
                ("lowsail.return", "market_stable"),
            ],
            required_deeds: &[
                "won_tide_key",
                "calibrated_with_tide_key",
                "shared_water",
                "returned_for_accord",
            ],
            required_visited_locations: &[
                "lowsail_market",
                "lowsail.docks",
                "lowsail.levee",
                "red_sluice.floor",
                "red_sluice.top",
                "lowsail.return",
            ],
            required_npc_locations: AFTERMATH_NPC_LOCATIONS,
            required_npc_knowledge: &[],
            forbidden_npc_knowledge: &[],
            required_npc_memories: &[],
            required_character_inventory: TIDE_KEY_CHARACTER_INVENTORY,
            required_character_resources: &[],
            required_npc_inventory: &[],
            forbidden_npc_inventory: TIDE_KEY_YARA_INVENTORY_ABSENCE,
            forbidden_character_inventory: &[],
            recipe_events: &[],
            storage_balances: UNTOUCHED_COLLATERAL,
            storage_transfers: &[],
            entropy_draws: &[],
            deferred_events: &[],
            pending_deferred_events: &[],
            required_legal_definitions: &[],
            forbidden_legal_definitions: OUTCOME_DEFINITIONS,
        },
    },
    ScenarioSpec {
        id: "m1-paid-towline-relief",
        claim_id: "split-tide.witness.paid-towline-relief",
        start: ScenarioStartSpec::Preset {
            character_preset_id: "rook",
        },
        seed: 71,
        steps: PAID_TOWLINE_RELIEF_STEPS,
        expectations: ScenarioExpectations {
            staffing_history: None,
            final_location: "lowsail.return",
            final_action_definition: "return.move_inland",
            final_observation_contains: "You help families open a higher market beyond the next surge.",
            forbidden_observation_contains: &[],
            final_world_time: Some(11),
            exclusive_after_action: "top.divert_relief",
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
                "ending_accord",
                "ending_council",
                "ending_freedom",
                "ending_disaster",
            ],
            forbidden_location_flags: &[],
            required_location_flags: &[
                ("red_sluice.floor", "culvert_entry"),
                ("red_sluice.top", "relief_ready"),
                ("lowsail.return", "market_moved"),
            ],
            required_deeds: &["rigged_towline", "opened_relief", "accepted_relocation"],
            required_visited_locations: &[
                "lowsail_market",
                "lowsail.docks",
                "lowsail.levee",
                "red_sluice.floor",
                "red_sluice.top",
                "lowsail.return",
            ],
            required_npc_locations: AFTERMATH_NPC_LOCATIONS,
            required_npc_knowledge: PAID_TOWLINE_REQUIRED_NPC_KNOWLEDGE,
            forbidden_npc_knowledge: &[],
            required_npc_memories: PAID_TOWLINE_REQUIRED_NPC_MEMORIES,
            required_character_inventory: PAID_TOWLINE_CHARACTER_INVENTORY,
            required_character_resources: PAID_TOWLINE_CHARACTER_RESOURCES,
            required_npc_inventory: &[],
            forbidden_npc_inventory: &[],
            forbidden_character_inventory: &[],
            recipe_events: &[],
            storage_balances: UNTOUCHED_COLLATERAL,
            storage_transfers: &[],
            entropy_draws: &[],
            deferred_events: &[],
            pending_deferred_events: &[],
            required_legal_definitions: &[],
            forbidden_legal_definitions: OUTCOME_DEFINITIONS,
        },
    },
    ScenarioSpec {
        id: "m2-fume-cold-repair",
        claim_id: "fume-yards.pilot.cold-repair",
        start: ScenarioStartSpec::Preset {
            character_preset_id: "ilyan",
        },
        seed: 71,
        steps: &PILOT_REPAIR_STEPS,
        expectations: ScenarioExpectations {
            staffing_history: None,
            final_location: "fume_yards.workshop",
            final_action_definition: "return.visit_workshop",
            final_observation_contains: "spent",
            forbidden_observation_contains: &[],
            final_world_time: Some(15),
            exclusive_after_action: "",
            required_world_flags: &[
                "sluice_outcome_chosen",
                "flow_locked_market",
                "ending_council",
            ],
            forbidden_world_flags: &[
                "flow_split",
                "old_channel_open",
                "sluice_failure",
                "surge_missed",
                "flow_relief",
                "ending_relief",
            ],
            forbidden_location_flags: &[],
            required_location_flags: &[
                ("fume_yards.workshop", "fume_yards.stock_given"),
                ("fume_yards.workshop", "fume_yards.freight_loaded"),
                ("lowsail.return", "fume_yards.stand_patched"),
                ("lowsail.return", "fume_yards.dry_goods_sorted"),
            ],
            required_deeds: &[],
            required_visited_locations: &[
                "lowsail.levee",
                "red_sluice.top",
                "lowsail.return",
                "fume_yards.workshop",
            ],
            required_npc_locations: AFTERMATH_NPC_LOCATIONS,
            required_npc_knowledge: &[ScenarioNpcKnowledgeExpectation {
                npc: "oren_pell",
                knowledge_id: "fume_yards.stand_patched",
                provenance: ScenarioKnowledgeProvenance::Witnessed,
                turn: 12,
            }],
            forbidden_npc_knowledge: &[],
            required_npc_memories: &[
                ScenarioNpcMemoryExpectation {
                    npc: "fume_yards.nessa_tern",
                    memory_id: "fume_yards.stock_handed_over",
                    provenance: ScenarioKnowledgeProvenance::Witnessed,
                    turn: 8,
                },
                ScenarioNpcMemoryExpectation {
                    npc: "oren_pell",
                    memory_id: "fume_yards.dry_goods_paid",
                    provenance: ScenarioKnowledgeProvenance::Witnessed,
                    turn: 13,
                },
            ],
            required_character_inventory: &[],
            required_character_resources: &[
                ScenarioResourceExpectation {
                    resource: "coin",
                    amount: 15,
                },
                ScenarioResourceExpectation {
                    resource: "stamina",
                    amount: 1,
                },
            ],
            required_npc_inventory: &[],
            forbidden_npc_inventory: PILOT_EMPTY_NESSA,
            forbidden_character_inventory: PILOT_SPENT_ITEMS,
            recipe_events: PILOT_REPAIR_EVENTS,
            storage_balances: UNTOUCHED_COLLATERAL,
            storage_transfers: &[],
            entropy_draws: &[],
            deferred_events: &[],
            pending_deferred_events: &[],
            required_legal_definitions: &[],
            forbidden_legal_definitions: &[
                "fume_yards.take_stock",
                "fume_yards.pack_catch_screen",
                "fume_yards.fit_catch_screen",
                "fume_yards.load_freight",
                "fume_yards.load_screened_freight",
                "return.patch_stand",
                "return.sort_dry_goods",
                "fume_yards.press_repair_plugs",
            ],
        },
    },
    ScenarioSpec {
        id: "m2-fume-cold-screen",
        claim_id: "fume-yards.pilot.cold-screen",
        start: ScenarioStartSpec::Preset {
            character_preset_id: "ilyan",
        },
        seed: 71,
        steps: &PILOT_SCREEN_STEPS,
        expectations: ScenarioExpectations {
            staffing_history: None,
            final_location: "fume_yards.workshop",
            final_action_definition: "return.visit_workshop",
            final_observation_contains: "screen",
            forbidden_observation_contains: &[],
            final_world_time: Some(14),
            exclusive_after_action: "",
            required_world_flags: &[
                "sluice_outcome_chosen",
                "flow_locked_market",
                "ending_council",
            ],
            forbidden_world_flags: &[
                "flow_split",
                "old_channel_open",
                "sluice_failure",
                "surge_missed",
                "flow_relief",
                "ending_relief",
            ],
            forbidden_location_flags: &[],
            required_location_flags: &[
                ("fume_yards.workshop", "fume_yards.stock_given"),
                ("fume_yards.workshop", "fume_yards.freight_loaded"),
                ("fume_yards.workshop", "fume_yards.screen_fitted"),
            ],
            required_deeds: &[],
            required_visited_locations: &[
                "lowsail.levee",
                "red_sluice.top",
                "lowsail.return",
                "fume_yards.workshop",
            ],
            required_npc_locations: AFTERMATH_NPC_LOCATIONS,
            required_npc_knowledge: &[],
            forbidden_npc_knowledge: &[ScenarioNpcKnowledgeAbsence {
                npc: "oren_pell",
                knowledge_id: "fume_yards.stand_patched",
            }],
            required_npc_memories: &[ScenarioNpcMemoryExpectation {
                npc: "fume_yards.nessa_tern",
                memory_id: "fume_yards.stock_handed_over",
                provenance: ScenarioKnowledgeProvenance::Witnessed,
                turn: 8,
            }],
            required_character_inventory: &[],
            required_character_resources: &[
                ScenarioResourceExpectation {
                    resource: "coin",
                    amount: 12,
                },
                ScenarioResourceExpectation {
                    resource: "stamina",
                    amount: 3,
                },
            ],
            required_npc_inventory: &[],
            forbidden_npc_inventory: PILOT_EMPTY_NESSA,
            forbidden_character_inventory: PILOT_SPENT_ITEMS,
            recipe_events: PILOT_SCREEN_EVENTS,
            storage_balances: UNTOUCHED_COLLATERAL,
            storage_transfers: &[],
            entropy_draws: &[],
            deferred_events: &[],
            pending_deferred_events: &[],
            required_legal_definitions: &[],
            forbidden_legal_definitions: &[
                "fume_yards.take_stock",
                "fume_yards.pack_catch_screen",
                "fume_yards.fit_catch_screen",
                "fume_yards.load_freight",
                "fume_yards.load_screened_freight",
                "return.patch_stand",
                "return.sort_dry_goods",
                "fume_yards.press_repair_plugs",
            ],
        },
    },
    ScenarioSpec {
        id: "m2-fume-unscreened-freight",
        claim_id: "fume-yards.pilot.unscreened-freight",
        start: ScenarioStartSpec::Preset {
            character_preset_id: "ilyan",
        },
        seed: 71,
        steps: &PILOT_UNSCREENED_STEPS,
        expectations: ScenarioExpectations {
            staffing_history: None,
            final_location: "fume_yards.workshop",
            final_action_definition: "return.visit_workshop",
            final_observation_contains: "Lowsail",
            forbidden_observation_contains: &[],
            final_world_time: Some(12),
            exclusive_after_action: "",
            required_world_flags: &[
                "sluice_outcome_chosen",
                "flow_locked_market",
                "ending_council",
            ],
            forbidden_world_flags: &[
                "flow_split",
                "old_channel_open",
                "sluice_failure",
                "surge_missed",
                "flow_relief",
                "ending_relief",
            ],
            forbidden_location_flags: &[],
            required_location_flags: &[
                ("fume_yards.workshop", "fume_yards.stock_given"),
                ("fume_yards.workshop", "fume_yards.freight_loaded"),
            ],
            required_deeds: &[],
            required_visited_locations: &[
                "lowsail.levee",
                "red_sluice.top",
                "lowsail.return",
                "fume_yards.workshop",
            ],
            required_npc_locations: AFTERMATH_NPC_LOCATIONS,
            required_npc_knowledge: &[],
            forbidden_npc_knowledge: &[ScenarioNpcKnowledgeAbsence {
                npc: "oren_pell",
                knowledge_id: "fume_yards.stand_patched",
            }],
            required_npc_memories: &[ScenarioNpcMemoryExpectation {
                npc: "fume_yards.nessa_tern",
                memory_id: "fume_yards.stock_handed_over",
                provenance: ScenarioKnowledgeProvenance::Witnessed,
                turn: 8,
            }],
            required_character_inventory: &[
                ScenarioInventoryExpectation {
                    item: "fume_yards.clay",
                    count: 2,
                },
                ScenarioInventoryExpectation {
                    item: "fume_yards.mesh",
                    count: 1,
                },
            ],
            required_character_resources: &[
                ScenarioResourceExpectation {
                    resource: "coin",
                    amount: 12,
                },
                ScenarioResourceExpectation {
                    resource: "stamina",
                    amount: 1,
                },
            ],
            required_npc_inventory: &[],
            forbidden_npc_inventory: PILOT_EMPTY_NESSA,
            forbidden_character_inventory: &["fume_yards.repair_lot", "fume_yards.catch_screen"],
            recipe_events: &[],
            storage_balances: UNTOUCHED_COLLATERAL,
            storage_transfers: &[],
            entropy_draws: &[],
            deferred_events: &[],
            pending_deferred_events: &[],
            required_legal_definitions: &["fume_yards.press_repair_plugs"],
            forbidden_legal_definitions: &[
                "fume_yards.take_stock",
                "fume_yards.pack_catch_screen",
                "fume_yards.fit_catch_screen",
                "fume_yards.load_freight",
                "fume_yards.load_screened_freight",
                "return.patch_stand",
                "return.sort_dry_goods",
            ],
        },
    },
    ScenarioSpec {
        id: "m2-fume-late-repair",
        claim_id: "fume-yards.pilot.late-repair",
        start: ScenarioStartSpec::Preset {
            character_preset_id: "rook",
        },
        seed: 71,
        steps: &PILOT_LATE_STEPS,
        expectations: ScenarioExpectations {
            staffing_history: None,
            final_location: "fume_yards.workshop",
            final_action_definition: "return.visit_workshop",
            final_observation_contains: "spent",
            forbidden_observation_contains: &[],
            final_world_time: Some(136),
            exclusive_after_action: "",
            required_world_flags: &["sluice_outcome_chosen", "flow_relief", "ending_relief"],
            forbidden_world_flags: &[
                "flow_split",
                "old_channel_open",
                "sluice_failure",
                "surge_missed",
                "flow_locked_market",
                "ending_council",
            ],
            forbidden_location_flags: &[],
            required_location_flags: &[
                ("fume_yards.workshop", "fume_yards.stock_given"),
                ("fume_yards.workshop", "fume_yards.freight_loaded"),
                ("lowsail.return", "fume_yards.stand_patched"),
                ("lowsail.return", "fume_yards.dry_goods_sorted"),
            ],
            required_deeds: &[],
            required_visited_locations: &[
                "lowsail.levee",
                "red_sluice.top",
                "lowsail.return",
                "fume_yards.workshop",
            ],
            required_npc_locations: AFTERMATH_NPC_LOCATIONS,
            required_npc_knowledge: &[ScenarioNpcKnowledgeExpectation {
                npc: "oren_pell",
                knowledge_id: "fume_yards.stand_patched",
                provenance: ScenarioKnowledgeProvenance::Witnessed,
                turn: 133,
            }],
            forbidden_npc_knowledge: &[],
            required_npc_memories: &[
                ScenarioNpcMemoryExpectation {
                    npc: "fume_yards.nessa_tern",
                    memory_id: "fume_yards.stock_handed_over",
                    provenance: ScenarioKnowledgeProvenance::Witnessed,
                    turn: 129,
                },
                ScenarioNpcMemoryExpectation {
                    npc: "oren_pell",
                    memory_id: "fume_yards.dry_goods_paid",
                    provenance: ScenarioKnowledgeProvenance::Witnessed,
                    turn: 134,
                },
            ],
            required_character_inventory: &[],
            required_character_resources: &[
                ScenarioResourceExpectation {
                    resource: "coin",
                    amount: 7,
                },
                ScenarioResourceExpectation {
                    resource: "stamina",
                    amount: 2,
                },
            ],
            required_npc_inventory: &[],
            forbidden_npc_inventory: PILOT_EMPTY_NESSA,
            forbidden_character_inventory: PILOT_SPENT_ITEMS,
            recipe_events: PILOT_LATE_EVENTS,
            storage_balances: UNTOUCHED_COLLATERAL,
            storage_transfers: &[],
            entropy_draws: &[],
            deferred_events: &[],
            pending_deferred_events: &[],
            required_legal_definitions: &[],
            forbidden_legal_definitions: &[
                "fume_yards.take_stock",
                "fume_yards.pack_catch_screen",
                "fume_yards.fit_catch_screen",
                "fume_yards.load_freight",
                "fume_yards.load_screened_freight",
                "return.patch_stand",
                "return.sort_dry_goods",
                "fume_yards.press_repair_plugs",
            ],
        },
    },
    BATCH_READY_SPEC,
    BATCH_LOCAL_SPEC,
    BATCH_SALE_SPEC,
    BATCH_BARE_SPEC,
    BATCH_REMOTE_SPEC,
    BATCH_BANK_SPEC,
    BATCH_MISSED_SPEC,
    BATCH_LATE_SPEC,
    BATCH_RECLAIM_SPEC,
    SALVAGE_SAFE_SALE_SPEC,
    SALVAGE_SAFE_LOCAL_SPEC,
    SALVAGE_SKILLED_SPEC,
    SALVAGE_INTACT_SPEC,
    SALVAGE_BROKEN_SPEC,
    SALVAGE_PRIOR_FILTER_BROKEN_SPEC,
    SALVAGE_PROTECTED_PRODUCTION_SPEC,
    SALVAGE_REPORTED_SPEC,
    SALVAGE_UNREPORTED_SPEC,
    MARKET_PURCHASE_SPEC,
    MARKET_FUEL_SPEC,
    MARKET_LOCAL_SPEC,
    MARKET_SALE_SPEC,
    MARKET_WATER_SPEC,
    MARKET_COMPOSED_SPEC,
    MARKET_CASK_DELIVERED_SPEC,
    MARKET_CASK_UNDELIVERED_SPEC,
    MARKET_AFTER_REPORT_SPEC,
    STAFF_STAFFED_SPEC,
    STAFF_ORDINARY_SPEC,
    STAFF_OTHER_HISTORY_SPEC,
    STAFF_NO_ACCOUNT_SPEC,
    STAFF_NO_HELP_SPEC,
    STAFF_WALKED_AWAY_SPEC,
    STAFF_CANCELLED_SPEC,
    STAFF_WATER_COMPOSED_SPEC,
];

#[derive(Serialize)]
struct BoundScenario<'a> {
    format: &'static str,
    spec: &'a ScenarioSpec,
}

#[derive(Serialize)]
struct BoundRecipe<'a> {
    format: &'static str,
    start: &'a ScenarioStartSpec,
    seed: u64,
    steps: &'a [BoundRecipeStep<'a>],
}

#[derive(Serialize)]
struct BoundRecipeStep<'a> {
    definition_id: &'a str,
    parameters: BTreeMap<String, String>,
}

pub(super) fn all() -> &'static [ScenarioSpec] {
    SCENARIOS
}

pub(super) fn get(id: &str) -> Result<&'static ScenarioSpec, VerifyError> {
    validate_registry()?;
    SCENARIOS
        .iter()
        .find(|scenario| scenario.id == id)
        .ok_or_else(|| VerifyError::new(format!("unknown evidence scenario {id}")))
}

pub(super) fn binding(spec: &ScenarioSpec) -> Result<String, VerifyError> {
    sha256_json(&BoundScenario {
        format: SCENARIO_BINDING_FORMAT,
        spec,
    })
    .map_err(|_| VerifyError::new("could not hash scenario binding"))
}

pub(super) fn run<'content>(
    spec: &ScenarioSpec,
    content: &'content CompiledContent,
) -> Result<Session<'content>, VerifyError> {
    let mut session = match spec.start {
        ScenarioStartSpec::Preset {
            character_preset_id,
        } => Session::new_game(character_preset_id, spec.seed, content).map_err(replay_error)?,
        ScenarioStartSpec::Custom { name, choices } => {
            let selection = CharacterSelection {
                name: name.to_owned(),
                choices: choices
                    .iter()
                    .map(|(slot_id, choice_id)| CharacterChoiceSelection {
                        slot_id: (*slot_id).to_owned(),
                        choice_id: (*choice_id).to_owned(),
                    })
                    .collect(),
            };
            Session::new_custom_game(&selection, spec.seed, content).map_err(replay_error)?
        }
    };
    for step in spec.steps {
        record_step(&mut session, content, step)?;
        if step.definition_id == spec.expectations.exclusive_after_action {
            validate_forbidden_actions(
                session.state(),
                content,
                spec.expectations.forbidden_legal_definitions,
            )?;
        }
    }
    validate_session(spec, &session, content)?;
    Ok(session)
}

pub(super) fn record_step(
    session: &mut Session<'_>,
    content: &CompiledContent,
    step: &ScenarioStep,
) -> Result<(), VerifyError> {
    let expected_parameters = parameter_map(step)?;
    let action = enumerate_legal_actions(session.state(), content)
        .map_err(|_| VerifyError::new("could not enumerate scenario actions"))?
        .into_iter()
        .find(|action| {
            action.definition_id == step.definition_id && action.parameters == expected_parameters
        })
        .ok_or_else(|| {
            VerifyError::new(format!(
                "declared scenario action {} with exact parameters is not legal",
                step.definition_id
            ))
        })?;
    session.record(&action).map_err(replay_error)?;
    Ok(())
}

pub(super) fn validate_session(
    spec: &ScenarioSpec,
    session: &Session<'_>,
    content: &CompiledContent,
) -> Result<(), VerifyError> {
    validate_recipe(spec, session, content)?;
    let state = session.state();
    let expected = &spec.expectations;
    if let Some(history) = expected.staffing_history {
        validate_staffing_history(history, session.trace(), state)?;
    }
    if state.world.current_location != expected.final_location {
        return Err(VerifyError::new(
            "scenario did not reach its claimed final location",
        ));
    }
    if expected
        .final_world_time
        .is_some_and(|time| state.world.time != time)
    {
        return Err(VerifyError::new(
            "scenario did not reach its claimed final world time",
        ));
    }
    for flag in expected.required_world_flags {
        if !state.world.flags.contains(*flag) {
            return Err(VerifyError::new(
                "scenario did not establish a required world consequence",
            ));
        }
    }
    for flag in expected.forbidden_world_flags {
        if state.world.flags.contains(*flag) {
            return Err(VerifyError::new(
                "scenario established a contradictory world consequence",
            ));
        }
    }
    for (location, flag) in expected.forbidden_location_flags {
        if state
            .world
            .locations
            .get(*location)
            .is_none_or(|runtime| runtime.flags.contains(*flag))
        {
            return Err(VerifyError::new(
                "scenario established a forbidden location consequence",
            ));
        }
    }
    for (location, flag) in expected.required_location_flags {
        if !state
            .world
            .locations
            .get(*location)
            .is_some_and(|runtime| runtime.flags.contains(*flag))
        {
            return Err(VerifyError::new(
                "scenario did not establish a required location consequence",
            ));
        }
    }
    for deed in expected.required_deeds {
        if !state.character.deeds.contains(*deed) {
            return Err(VerifyError::new(
                "scenario did not establish a required character consequence",
            ));
        }
    }
    validate_inventory(
        state,
        expected.required_character_inventory,
        expected.forbidden_npc_inventory,
    )?;
    validate_character_resources(state, expected.required_character_resources)?;
    validate_forbidden_inventory(state, expected.forbidden_character_inventory)?;
    for expected_stock in expected.required_npc_inventory {
        if state
            .world
            .npcs
            .get(expected_stock.npc)
            .and_then(|npc| npc.inventory.get(expected_stock.item))
            != Some(&expected_stock.count)
        {
            return Err(VerifyError::new(
                "scenario NPC owned stock differs from its claim",
            ));
        }
    }
    validate_recipe_events(state, expected.recipe_events)?;
    validate_entropy_history(state, expected.entropy_draws)?;
    validate_storage_history(state, expected.storage_balances, expected.storage_transfers)?;
    validate_deferred_history(
        state,
        expected.deferred_events,
        expected.pending_deferred_events,
    )?;
    validate_npc_locations(state, expected.required_npc_locations)?;

    let mut visited = BTreeSet::new();
    visited.insert(session.trace().initial_observation.location_id.as_str());
    visited.extend(
        session
            .trace()
            .steps
            .iter()
            .map(|step| step.observation.location_id.as_str()),
    );
    for location in expected.required_visited_locations {
        if !visited.contains(location) {
            return Err(VerifyError::new(format!(
                "scenario did not visit required location {location}"
            )));
        }
    }

    let final_step = session
        .trace()
        .steps
        .last()
        .ok_or_else(|| VerifyError::new("scenario has no final action"))?;
    if final_step.action.definition_id != expected.final_action_definition {
        return Err(VerifyError::new(
            "scenario final action does not match its semantic claim",
        ));
    }
    if !expected.final_observation_contains.is_empty()
        && !final_step
            .observation
            .text
            .contains(expected.final_observation_contains)
    {
        return Err(VerifyError::new(
            "scenario final observation does not express its semantic claim",
        ));
    }

    for phrase in expected.forbidden_observation_contains {
        if final_step.observation.text.contains(*phrase) {
            return Err(VerifyError::new(
                "scenario final observation contains a forbidden phase detail",
            ));
        }
    }

    validate_npc_knowledge(
        state,
        expected.required_npc_knowledge,
        expected.forbidden_npc_knowledge,
    )?;
    validate_npc_memories(state, expected.required_npc_memories)?;
    validate_legal_set(
        state,
        content,
        expected.required_legal_definitions,
        expected.forbidden_legal_definitions,
    )
}

fn validate_recipe_events(
    state: &forge_kernel::GameState,
    expected: &[ScenarioRecipeExpectation],
) -> Result<(), VerifyError> {
    let actual: Vec<_> = state
        .event_log
        .iter()
        .filter_map(|event| match &event.kind {
            forge_kernel::EventKind::RecipeApplied {
                recipe,
                inputs,
                outputs,
            } => Some((event.turn, recipe.as_str(), inputs.clone(), outputs.clone())),
            _ => None,
        })
        .collect();
    let expected: Vec<_> = expected
        .iter()
        .map(|event| {
            (
                event.turn,
                event.recipe,
                event
                    .inputs
                    .iter()
                    .map(|(item, count)| ((*item).to_owned(), *count))
                    .collect::<BTreeMap<_, _>>(),
                event
                    .outputs
                    .iter()
                    .map(|(item, count)| ((*item).to_owned(), *count))
                    .collect::<BTreeMap<_, _>>(),
            )
        })
        .collect();
    if actual != expected {
        return Err(VerifyError::new(
            "scenario recipe events differ from required consumption and production",
        ));
    }
    Ok(())
}

fn validate_forbidden_actions(
    state: &forge_kernel::GameState,
    content: &CompiledContent,
    forbidden: &[&str],
) -> Result<(), VerifyError> {
    validate_legal_set(state, content, &[], forbidden)
}

fn validate_inventory(
    state: &forge_kernel::GameState,
    required_character: &[ScenarioInventoryExpectation],
    forbidden_npc: &[ScenarioNpcInventoryAbsence],
) -> Result<(), VerifyError> {
    for expected in required_character {
        let actual = state.character.inventory.get(expected.item).copied();
        if actual != Some(expected.count) {
            return Err(VerifyError::new(format!(
                "scenario character inventory has the wrong count for {}",
                expected.item
            )));
        }
    }
    for expected in forbidden_npc {
        let npc = state.world.npcs.get(expected.npc).ok_or_else(|| {
            VerifyError::new(format!(
                "scenario references unknown NPC {} for inventory assertion",
                expected.npc
            ))
        })?;
        if npc.inventory.contains_key(expected.item) {
            return Err(VerifyError::new(format!(
                "scenario NPC {} still owns forbidden item {}",
                expected.npc, expected.item
            )));
        }
    }
    Ok(())
}

fn validate_forbidden_inventory(
    state: &forge_kernel::GameState,
    forbidden: &[&str],
) -> Result<(), VerifyError> {
    for item in forbidden {
        if state.character.inventory.contains_key(*item) {
            return Err(VerifyError::new(format!(
                "scenario still owns consumed or forbidden item {item}"
            )));
        }
    }
    Ok(())
}

fn validate_character_resources(
    state: &forge_kernel::GameState,
    required: &[ScenarioResourceExpectation],
) -> Result<(), VerifyError> {
    for expected in required {
        let actual = state.character.resources.get(expected.resource).copied();
        if actual != Some(expected.amount) {
            return Err(VerifyError::new(format!(
                "scenario character resources have the wrong amount for {}",
                expected.resource
            )));
        }
    }
    Ok(())
}

fn validate_npc_locations(
    state: &forge_kernel::GameState,
    expected: &[ScenarioNpcLocationExpectation],
) -> Result<(), VerifyError> {
    for expected in expected {
        let destination = state
            .world
            .locations
            .get(expected.location)
            .ok_or_else(|| {
                VerifyError::new(format!(
                    "scenario references unknown location {} for NPC {}",
                    expected.location, expected.npc
                ))
            })?;
        let npc = state.world.npcs.get(expected.npc).ok_or_else(|| {
            VerifyError::new(format!(
                "scenario references unknown NPC {} for location assertion",
                expected.npc
            ))
        })?;
        if npc.location != expected.location {
            return Err(VerifyError::new(format!(
                "scenario NPC {} has the wrong location",
                expected.npc
            )));
        }
        if !destination.entities.contains(expected.npc) {
            return Err(VerifyError::new(format!(
                "scenario NPC {} is missing from location index {}",
                expected.npc, expected.location
            )));
        }
        for (location, runtime) in &state.world.locations {
            let indexed = runtime.entities.contains(expected.npc);
            if location != expected.location && indexed {
                return Err(VerifyError::new(format!(
                    "scenario NPC {} remains in location index {}",
                    expected.npc, location
                )));
            }
        }
    }
    Ok(())
}

fn validate_legal_set(
    state: &forge_kernel::GameState,
    content: &CompiledContent,
    required: &[&str],
    forbidden: &[&str],
) -> Result<(), VerifyError> {
    let final_definitions: BTreeSet<_> = enumerate_legal_actions(state, content)
        .map_err(|_| VerifyError::new("could not enumerate final scenario actions"))?
        .into_iter()
        .map(|action| action.definition_id)
        .collect();
    for definition in required {
        if !final_definitions.contains(*definition) {
            return Err(VerifyError::new(
                "scenario did not leave a required legal action available",
            ));
        }
    }
    for definition in forbidden {
        if final_definitions.contains(*definition) {
            return Err(VerifyError::new(
                "scenario left a contradictory outcome action legal",
            ));
        }
    }
    Ok(())
}

fn validate_npc_knowledge(
    state: &forge_kernel::GameState,
    required: &[ScenarioNpcKnowledgeExpectation],
    forbidden: &[ScenarioNpcKnowledgeAbsence],
) -> Result<(), VerifyError> {
    for expected in required {
        let npc = state.world.npcs.get(expected.npc).ok_or_else(|| {
            VerifyError::new(format!("scenario references unknown NPC {}", expected.npc))
        })?;
        let knowledge = npc.knowledge.get(expected.knowledge_id).ok_or_else(|| {
            VerifyError::new(format!(
                "scenario did not establish {} knowledge for {}",
                expected.knowledge_id, expected.npc
            ))
        })?;
        if knowledge.turn != expected.turn
            || !knowledge_provenance_matches(&expected.provenance, &knowledge.provenance)
        {
            return Err(VerifyError::new(format!(
                "scenario established {} knowledge for {} with the wrong turn or provenance",
                expected.knowledge_id, expected.npc
            )));
        }
    }
    for expected in forbidden {
        let npc = state.world.npcs.get(expected.npc).ok_or_else(|| {
            VerifyError::new(format!("scenario references unknown NPC {}", expected.npc))
        })?;
        if npc.knowledge.contains_key(expected.knowledge_id) {
            return Err(VerifyError::new(format!(
                "scenario established forbidden {} knowledge for {}",
                expected.knowledge_id, expected.npc
            )));
        }
    }
    Ok(())
}

fn validate_npc_memories(
    state: &forge_kernel::GameState,
    required: &[ScenarioNpcMemoryExpectation],
) -> Result<(), VerifyError> {
    for expected in required {
        let npc = state.world.npcs.get(expected.npc).ok_or_else(|| {
            VerifyError::new(format!("scenario references unknown NPC {}", expected.npc))
        })?;
        let memory = npc.memories.get(expected.memory_id).ok_or_else(|| {
            VerifyError::new(format!(
                "scenario did not establish {} memory for {}",
                expected.memory_id, expected.npc
            ))
        })?;
        if memory.turn != expected.turn
            || !knowledge_provenance_matches(&expected.provenance, &memory.provenance)
        {
            return Err(VerifyError::new(format!(
                "scenario established {} memory for {} with the wrong turn or provenance",
                expected.memory_id, expected.npc
            )));
        }
    }
    Ok(())
}

fn knowledge_provenance_matches(
    expected: &ScenarioKnowledgeProvenance,
    actual: &forge_kernel::KnowledgeProvenance,
) -> bool {
    match (expected, actual) {
        (ScenarioKnowledgeProvenance::Witnessed, forge_kernel::KnowledgeProvenance::Witnessed) => {
            true
        }
        (
            ScenarioKnowledgeProvenance::Read { source },
            forge_kernel::KnowledgeProvenance::Read {
                source: actual_source,
            },
        ) => actual_source == source,
        (
            ScenarioKnowledgeProvenance::Told { by },
            forge_kernel::KnowledgeProvenance::Told { by: actual_by },
        ) => actual_by == by,
        (
            ScenarioKnowledgeProvenance::Rumor {
                from: expected_from,
            },
            forge_kernel::KnowledgeProvenance::Rumor { from: actual_from },
        ) => actual_from.as_deref() == Some(*expected_from),
        _ => false,
    }
}

fn validate_recipe(
    spec: &ScenarioSpec,
    session: &Session<'_>,
    content: &CompiledContent,
) -> Result<(), VerifyError> {
    let start_matches = match (spec.start, &session.trace().start) {
        (
            ScenarioStartSpec::Preset {
                character_preset_id: expected_id,
            },
            TraceStart::CharacterPreset {
                character_preset_id,
                seed,
            },
        ) => character_preset_id == expected_id && *seed == spec.seed,
        (
            ScenarioStartSpec::Custom { name, choices },
            TraceStart::CharacterCreation { selection, seed },
        ) => {
            let authored = CharacterSelection {
                name: name.to_owned(),
                choices: choices
                    .iter()
                    .map(|(slot_id, choice_id)| CharacterChoiceSelection {
                        slot_id: (*slot_id).to_owned(),
                        choice_id: (*choice_id).to_owned(),
                    })
                    .collect(),
            };
            content
                .canonical_character_selection(&authored)
                .is_ok_and(|canonical| canonical == *selection)
                && *seed == spec.seed
        }
        _ => false,
    };
    if !start_matches {
        return Err(VerifyError::new(
            "scenario start does not match its bound recipe",
        ));
    }
    if session.trace().steps.len() != spec.steps.len() {
        return Err(VerifyError::new(
            "scenario step count does not match its bound recipe",
        ));
    }
    for (actual, expected) in session.trace().steps.iter().zip(spec.steps) {
        if actual.action.definition_id != expected.definition_id
            || actual.action.parameters != parameter_map(expected)?
        {
            return Err(VerifyError::new(
                "scenario action does not match its exact bound recipe",
            ));
        }
    }
    Ok(())
}

fn parameter_map(step: &ScenarioStep) -> Result<BTreeMap<String, String>, VerifyError> {
    let mut parameters = BTreeMap::new();
    for (name, value) in step.parameters {
        if parameters
            .insert((*name).to_owned(), (*value).to_owned())
            .is_some()
        {
            return Err(VerifyError::new(
                "scenario recipe contains a duplicate parameter",
            ));
        }
    }
    Ok(parameters)
}

fn validate_registry() -> Result<(), VerifyError> {
    validate_specs(SCENARIOS)?;
    validate_required_scenario_ids(SCENARIOS)
}

fn validate_required_scenario_ids(specs: &[ScenarioSpec]) -> Result<(), VerifyError> {
    let actual: BTreeSet<_> = specs.iter().map(|spec| spec.id).collect();
    let required: BTreeSet<_> = REQUIRED_SCENARIO_IDS.iter().copied().collect();
    if actual != required {
        return Err(VerifyError::new(
            "scenario registry does not contain the exact reviewed scenario set",
        ));
    }
    Ok(())
}

fn validate_specs(specs: &[ScenarioSpec]) -> Result<(), VerifyError> {
    let mut ids = BTreeSet::new();
    let mut claims = BTreeSet::new();
    let mut recipes = BTreeSet::new();
    for spec in specs {
        if !ids.insert(spec.id) {
            return Err(VerifyError::new(format!(
                "duplicate scenario id {}",
                spec.id
            )));
        }
        if spec.claim_id.is_empty() || !claims.insert(spec.claim_id) {
            return Err(VerifyError::new(
                "scenario claim identifiers must be nonempty and unique",
            ));
        }
        if let ScenarioStartSpec::Custom { name, choices } = spec.start {
            if name.trim().is_empty() || choices.is_empty() || choices.len() > 16 {
                return Err(VerifyError::new(
                    "custom scenario start has invalid public recipe bounds",
                ));
            }
            let mut slots = BTreeSet::new();
            for (slot_id, choice_id) in choices {
                if slot_id.is_empty() || choice_id.is_empty() || !slots.insert(slot_id) {
                    return Err(VerifyError::new(
                        "custom scenario recipe has an empty or duplicate selection",
                    ));
                }
            }
        }
        if spec.steps.is_empty() || spec.steps.len() > crate::MAX_WITNESS_STEPS {
            return Err(VerifyError::new(
                "scenario recipe must have between 1 and 4096 steps",
            ));
        }
        if spec
            .steps
            .last()
            .is_none_or(|step| step.definition_id != spec.expectations.final_action_definition)
        {
            return Err(VerifyError::new(
                "scenario recipe does not end with its claimed final action",
            ));
        }
        let exclusivity_checks = spec
            .steps
            .iter()
            .filter(|step| step.definition_id == spec.expectations.exclusive_after_action)
            .count();
        if !spec.expectations.exclusive_after_action.is_empty() && exclusivity_checks != 1 {
            return Err(VerifyError::new(
                "scenario exclusivity assertion is not bound to exactly one recipe action",
            ));
        }
        for expected in spec.expectations.required_character_inventory {
            if expected.item.trim().is_empty() || expected.count == 0 {
                return Err(VerifyError::new(
                    "scenario character inventory assertions must name a positive item count",
                ));
            }
        }
        for expected in spec.expectations.required_character_resources {
            if expected.resource.trim().is_empty() {
                return Err(VerifyError::new(
                    "scenario character resource assertions must name a resource",
                ));
            }
        }
        for item in spec.expectations.forbidden_character_inventory {
            if item.trim().is_empty() {
                return Err(VerifyError::new(
                    "scenario forbidden inventory assertion must name an item",
                ));
            }
        }
        let mut previous_turn = None;
        for expected in spec.expectations.recipe_events {
            if expected.recipe.trim().is_empty()
                || expected.inputs.is_empty()
                || previous_turn.is_some_and(|turn| turn > expected.turn)
            {
                return Err(VerifyError::new(
                    "scenario recipe assertions require ordered turns and named consumed inputs",
                ));
            }
            previous_turn = Some(expected.turn);
            for quantities in [expected.inputs, expected.outputs] {
                let mut names = BTreeSet::new();
                for (item, count) in quantities {
                    if item.trim().is_empty() || *count == 0 || !names.insert(item) {
                        return Err(VerifyError::new(
                            "scenario recipe quantities must be positive and unique",
                        ));
                    }
                }
            }
        }
        for expected in spec.expectations.required_npc_memories {
            if expected.npc.trim().is_empty() || expected.memory_id.trim().is_empty() {
                return Err(VerifyError::new(
                    "scenario NPC memory assertions must name an NPC and memory",
                ));
            }
        }
        for expected in spec.expectations.required_npc_inventory {
            if expected.npc.trim().is_empty()
                || expected.item.trim().is_empty()
                || expected.count == 0
            {
                return Err(VerifyError::new(
                    "scenario NPC stock assertions must name an NPC and positive item count",
                ));
            }
        }
        for (location, flag) in spec.expectations.forbidden_location_flags {
            if location.trim().is_empty()
                || flag.trim().is_empty()
                || spec
                    .expectations
                    .required_location_flags
                    .contains(&(*location, *flag))
            {
                return Err(VerifyError::new(
                    "scenario forbidden local flags must be named and cannot also be required",
                ));
            }
        }
        if let Some(history) = spec.expectations.staffing_history {
            validate_staffing_spec(history, spec)?;
        }
        validate_entropy_expectations(
            spec.expectations.entropy_draws,
            spec.expectations.final_world_time,
        )?;
        validate_storage_expectations(
            spec.expectations.storage_balances,
            spec.expectations.storage_transfers,
            spec.expectations.final_world_time,
        )?;
        validate_deferred_expectations(
            spec.expectations.deferred_events,
            spec.expectations.pending_deferred_events,
            spec.expectations.final_world_time,
        )?;
        for expected in spec.expectations.forbidden_npc_inventory {
            if expected.npc.trim().is_empty() || expected.item.trim().is_empty() {
                return Err(VerifyError::new(
                    "scenario NPC inventory assertions must name an NPC and item",
                ));
            }
        }
        let mut normalized_steps = Vec::with_capacity(spec.steps.len());
        for step in spec.steps {
            if step.definition_id.is_empty() {
                return Err(VerifyError::new(
                    "scenario recipe action identifiers cannot be empty",
                ));
            }
            normalized_steps.push(BoundRecipeStep {
                definition_id: step.definition_id,
                parameters: parameter_map(step)?,
            });
        }
        let recipe = sha256_json(&BoundRecipe {
            format: RECIPE_BINDING_FORMAT,
            start: &spec.start,
            seed: spec.seed,
            steps: &normalized_steps,
        })
        .map_err(|_| VerifyError::new("could not hash scenario recipe"))?;
        if !recipes.insert(recipe) {
            return Err(VerifyError::new(
                "multiple scenarios use the same bound recipe",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PARAMETERS_AB: &[(&str, &str)] = &[("alpha", "1"), ("beta", "2")];
    const PARAMETERS_BA: &[(&str, &str)] = &[("beta", "2"), ("alpha", "1")];
    const RECIPE_AB: &[ScenarioStep] = &[ScenarioStep {
        definition_id: "fixture",
        parameters: PARAMETERS_AB,
    }];
    const RECIPE_BA: &[ScenarioStep] = &[ScenarioStep {
        definition_id: "fixture",
        parameters: PARAMETERS_BA,
    }];

    #[test]
    fn registry_rejects_duplicate_ids_and_recipes() {
        let first = SCENARIOS[0];
        let duplicate_id = validate_specs(&[first, first]).unwrap_err();
        assert!(duplicate_id.to_string().contains("duplicate scenario id"));

        let mut duplicate_recipe = first;
        duplicate_recipe.id = "different-id";
        duplicate_recipe.claim_id = "different-claim";
        let duplicate_recipe = validate_specs(&[first, duplicate_recipe]).unwrap_err();
        assert!(duplicate_recipe.to_string().contains("same bound recipe"));

        let mut duplicate_claim = first;
        duplicate_claim.id = "different-id";
        duplicate_claim.seed += 1;
        let duplicate_claim = validate_specs(&[first, duplicate_claim]).unwrap_err();
        assert!(duplicate_claim.to_string().contains("claim identifiers"));
        assert!(validate_registry().is_ok());
    }

    #[test]
    fn registry_requires_the_exact_reviewed_scenario_set() {
        assert!(validate_required_scenario_ids(SCENARIOS).is_ok());
        assert!(validate_required_scenario_ids(&SCENARIOS[..SCENARIOS.len() - 1]).is_err());

        let mut replaced = SCENARIOS.to_vec();
        replaced[SCENARIOS.len() - 1].id = "m1-area-unreviewed";
        assert!(validate_required_scenario_ids(&replaced).is_err());
    }

    #[test]
    fn exact_parameter_maps_reject_extras_and_duplicates() {
        let step = ScenarioStep {
            definition_id: "fixture",
            parameters: &[("destination", "a")],
        };
        let expected = parameter_map(&step).unwrap();
        let mut extra = expected.clone();
        extra.insert("unexpected".to_owned(), "value".to_owned());
        assert_ne!(expected, extra);

        let duplicate = ScenarioStep {
            definition_id: "fixture",
            parameters: &[("destination", "a"), ("destination", "b")],
        };
        assert!(parameter_map(&duplicate).is_err());

        let mut ordered = SCENARIOS[0];
        ordered.steps = RECIPE_AB;
        let mut reordered = ordered;
        reordered.steps = RECIPE_BA;
        let ordered_hash = sha256_json(&BoundRecipe {
            format: RECIPE_BINDING_FORMAT,
            start: &ordered.start,
            seed: ordered.seed,
            steps: &[BoundRecipeStep {
                definition_id: RECIPE_AB[0].definition_id,
                parameters: parameter_map(&RECIPE_AB[0]).unwrap(),
            }],
        })
        .unwrap();
        let reordered_hash = sha256_json(&BoundRecipe {
            format: RECIPE_BINDING_FORMAT,
            start: &reordered.start,
            seed: reordered.seed,
            steps: &[BoundRecipeStep {
                definition_id: RECIPE_BA[0].definition_id,
                parameters: parameter_map(&RECIPE_BA[0]).unwrap(),
            }],
        })
        .unwrap();
        assert_eq!(ordered_hash, reordered_hash);
    }

    #[test]
    fn warning_scenarios_bind_real_npc_provenance_and_legal_sets() {
        let content = crate::load_content().unwrap();
        let unrelayed_spec = get("m1-warning-unrelayed").unwrap();
        let unrelayed = run(unrelayed_spec, &content).unwrap();
        assert_eq!(unrelayed.state().world.time, 6);
        assert!(
            validate_npc_knowledge(
                unrelayed.state(),
                unrelayed_spec.expectations.required_npc_knowledge,
                unrelayed_spec.expectations.forbidden_npc_knowledge,
            )
            .is_ok()
        );
        assert!(
            validate_legal_set(
                unrelayed.state(),
                &content,
                unrelayed_spec.expectations.required_legal_definitions,
                unrelayed_spec.expectations.forbidden_legal_definitions,
            )
            .is_ok()
        );

        let relayed_spec = get("m1-warning-relayed").unwrap();
        let relayed = run(relayed_spec, &content).unwrap();
        assert_eq!(relayed.state().world.time, 6);
        assert!(
            validate_npc_knowledge(
                relayed.state(),
                relayed_spec.expectations.required_npc_knowledge,
                relayed_spec.expectations.forbidden_npc_knowledge,
            )
            .is_ok()
        );
        assert!(
            validate_legal_set(
                relayed.state(),
                &content,
                relayed_spec.expectations.required_legal_definitions,
                relayed_spec.expectations.forbidden_legal_definitions,
            )
            .is_ok()
        );

        assert!(
            validate_legal_set(unrelayed.state(), &content, &["floor.open_relief"], &[],).is_err()
        );
        assert!(
            validate_legal_set(relayed.state(), &content, &[], &["floor.open_relief"],).is_err()
        );
    }

    #[test]
    fn warning_knowledge_checks_reject_wrong_provenance_and_forbidden_presence() {
        let content = crate::load_content().unwrap();
        let unrelayed_spec = get("m1-warning-unrelayed").unwrap();
        let unrelayed = run(unrelayed_spec, &content).unwrap();
        let mut forbidden_presence = unrelayed.state().clone();
        let oren_knowledge =
            forbidden_presence.world.npcs["oren_pell"].knowledge["market_warned"].clone();
        forbidden_presence
            .world
            .npcs
            .get_mut("edrik_voss")
            .unwrap()
            .knowledge
            .insert("market_warned".to_owned(), oren_knowledge);
        assert!(
            validate_npc_knowledge(
                &forbidden_presence,
                unrelayed_spec.expectations.required_npc_knowledge,
                unrelayed_spec.expectations.forbidden_npc_knowledge,
            )
            .is_err()
        );

        let relayed_spec = get("m1-warning-relayed").unwrap();
        let relayed = run(relayed_spec, &content).unwrap();
        let mut wrong_source = relayed.state().clone();
        wrong_source
            .world
            .npcs
            .get_mut("edrik_voss")
            .unwrap()
            .knowledge
            .get_mut("market_warned")
            .unwrap()
            .provenance = forge_kernel::KnowledgeProvenance::Rumor {
            from: Some("mira_kett".to_owned()),
        };
        assert!(
            validate_npc_knowledge(
                &wrong_source,
                relayed_spec.expectations.required_npc_knowledge,
                relayed_spec.expectations.forbidden_npc_knowledge,
            )
            .is_err()
        );

        let mut wrong_kind = relayed.state().clone();
        wrong_kind
            .world
            .npcs
            .get_mut("edrik_voss")
            .unwrap()
            .knowledge
            .get_mut("market_warned")
            .unwrap()
            .provenance = forge_kernel::KnowledgeProvenance::Witnessed;
        assert!(
            validate_npc_knowledge(
                &wrong_kind,
                relayed_spec.expectations.required_npc_knowledge,
                relayed_spec.expectations.forbidden_npc_knowledge,
            )
            .is_err()
        );

        let wrong_timestamp = [ScenarioNpcKnowledgeExpectation {
            npc: "edrik_voss",
            knowledge_id: "market_warned",
            provenance: ScenarioKnowledgeProvenance::Rumor { from: "oren_pell" },
            turn: 5,
        }];
        assert!(validate_npc_knowledge(relayed.state(), &wrong_timestamp, &[],).is_err());

        let original_binding = binding(relayed_spec).unwrap();
        let mut changed = *relayed_spec;
        let mut changed_expectations = changed.expectations;
        changed_expectations.final_world_time = Some(7);
        changed.expectations = changed_expectations;
        assert_ne!(original_binding, binding(&changed).unwrap());
    }

    #[test]
    fn aftermath_npc_locations_bind_position_and_entity_indexes() {
        let content = crate::load_content().unwrap();
        let spec = get("m1-outcome-split-flow").unwrap();
        let session = run(spec, &content).unwrap();
        validate_npc_locations(session.state(), spec.expectations.required_npc_locations).unwrap();

        let mut wrong_position = session.state().clone();
        wrong_position
            .world
            .npcs
            .get_mut("mira_kett")
            .unwrap()
            .location = "red_sluice.top".to_owned();
        assert!(
            validate_npc_locations(&wrong_position, spec.expectations.required_npc_locations,)
                .is_err()
        );

        let mut wrong_index = session.state().clone();
        assert!(
            wrong_index
                .world
                .locations
                .get_mut("lowsail.return")
                .unwrap()
                .entities
                .remove("mira_kett")
        );
        assert!(
            validate_npc_locations(&wrong_index, spec.expectations.required_npc_locations).is_err()
        );

        let mut duplicate_index = session.state().clone();
        assert!(
            duplicate_index
                .world
                .locations
                .get_mut("red_sluice.top")
                .unwrap()
                .entities
                .insert("mira_kett".to_owned())
        );
        assert!(
            validate_npc_locations(&duplicate_index, spec.expectations.required_npc_locations)
                .is_err()
        );

        let mut missing_destination = session.state().clone();
        assert!(
            missing_destination
                .world
                .locations
                .remove("lowsail.return")
                .is_some()
        );
        assert!(
            validate_npc_locations(
                &missing_destination,
                spec.expectations.required_npc_locations,
            )
            .is_err()
        );

        let original_binding = binding(spec).unwrap();
        let mut changed = *spec;
        let mut changed_expectations = changed.expectations;
        changed_expectations.required_npc_locations = &[];
        changed.expectations = changed_expectations;
        assert_ne!(original_binding, binding(&changed).unwrap());
    }

    #[test]
    fn tide_key_inventory_checks_reject_wrong_count_and_owner() {
        let content = crate::load_content().unwrap();
        let spec = get("m1-tide-key-split-flow").unwrap();
        let session = run(spec, &content).unwrap();
        validate_inventory(
            session.state(),
            spec.expectations.required_character_inventory,
            spec.expectations.forbidden_npc_inventory,
        )
        .unwrap();

        let mut wrong_count = session.state().clone();
        wrong_count
            .character
            .inventory
            .insert("split_tide.tide_key".to_owned(), 2);
        assert!(
            validate_inventory(
                &wrong_count,
                spec.expectations.required_character_inventory,
                spec.expectations.forbidden_npc_inventory,
            )
            .is_err()
        );

        let mut wrong_owner = session.state().clone();
        wrong_owner
            .world
            .npcs
            .get_mut("yara_dene")
            .unwrap()
            .inventory
            .insert("split_tide.tide_key".to_owned(), 1);
        assert!(
            validate_inventory(
                &wrong_owner,
                spec.expectations.required_character_inventory,
                spec.expectations.forbidden_npc_inventory,
            )
            .is_err()
        );
    }

    #[test]
    fn tide_key_inventory_postconditions_are_binding_material() {
        let spec = get("m1-tide-key-split-flow").unwrap();
        let original_binding = binding(spec).unwrap();

        let mut wrong_count = *spec;
        let mut wrong_count_expectations = wrong_count.expectations;
        wrong_count_expectations.required_character_inventory = WRONG_TIDE_KEY_CHARACTER_INVENTORY;
        wrong_count.expectations = wrong_count_expectations;
        assert_ne!(original_binding, binding(&wrong_count).unwrap());

        let mut missing_absence = *spec;
        let mut missing_absence_expectations = missing_absence.expectations;
        missing_absence_expectations.forbidden_npc_inventory = EMPTY_NPC_INVENTORY_ABSENCE;
        missing_absence.expectations = missing_absence_expectations;
        assert_ne!(original_binding, binding(&missing_absence).unwrap());
    }

    #[test]
    fn paid_towline_postconditions_bind_resources_and_memory() {
        let content = crate::load_content().unwrap();
        let spec = get("m1-paid-towline-relief").unwrap();
        let session = run(spec, &content).unwrap();
        validate_inventory(
            session.state(),
            spec.expectations.required_character_inventory,
            spec.expectations.forbidden_npc_inventory,
        )
        .unwrap();
        validate_character_resources(
            session.state(),
            spec.expectations.required_character_resources,
        )
        .unwrap();
        validate_npc_memories(session.state(), spec.expectations.required_npc_memories).unwrap();

        let mut wrong_coin = session.state().clone();
        wrong_coin.character.resources.insert("coin".to_owned(), 3);
        assert!(
            validate_character_resources(
                &wrong_coin,
                spec.expectations.required_character_resources,
            )
            .is_err()
        );

        let mut missing_coin = session.state().clone();
        missing_coin.character.resources.remove("coin");
        assert!(
            validate_character_resources(
                &missing_coin,
                spec.expectations.required_character_resources,
            )
            .is_err()
        );

        let mut wrong_tool_count = session.state().clone();
        wrong_tool_count
            .character
            .inventory
            .insert("wire".to_owned(), 0);
        assert!(
            validate_inventory(
                &wrong_tool_count,
                spec.expectations.required_character_inventory,
                spec.expectations.forbidden_npc_inventory,
            )
            .is_err()
        );

        let mut wrong_memory = session.state().clone();
        wrong_memory
            .world
            .npcs
            .get_mut("oren_pell")
            .unwrap()
            .memories
            .get_mut("oren_saw_towline")
            .unwrap()
            .provenance = forge_kernel::KnowledgeProvenance::Read {
            source: "a rumor".to_owned(),
        };
        assert!(
            validate_npc_memories(&wrong_memory, spec.expectations.required_npc_memories,).is_err()
        );

        let mut wrong_memory_time = session.state().clone();
        wrong_memory_time
            .world
            .npcs
            .get_mut("oren_pell")
            .unwrap()
            .memories
            .get_mut("oren_saw_towline")
            .unwrap()
            .turn = 3;
        assert!(
            validate_npc_memories(&wrong_memory_time, spec.expectations.required_npc_memories,)
                .is_err()
        );

        let original_binding = binding(spec).unwrap();
        let mut changed_resources = *spec;
        let mut changed_expectations = changed_resources.expectations;
        changed_expectations.required_character_resources = &[];
        changed_resources.expectations = changed_expectations;
        assert_ne!(original_binding, binding(&changed_resources).unwrap());

        let mut changed_memory = *spec;
        let mut changed_expectations = changed_memory.expectations;
        changed_expectations.required_npc_memories = &[];
        changed_memory.expectations = changed_expectations;
        assert_ne!(original_binding, binding(&changed_memory).unwrap());
    }
    #[test]
    fn pilot_witnesses_bind_consumption_production_and_forbidden_inventory() {
        let content = crate::load_content().unwrap();
        let spec = *get("m2-fume-cold-repair").unwrap();
        let session = run(&spec, &content).unwrap();
        let mut altered = spec;
        const WRONG_RECIPE_EVENTS: &[ScenarioRecipeExpectation] = &[
            ScenarioRecipeExpectation {
                turn: 9,
                recipe: "fume_yards.press_repair_plugs",
                inputs: &[("fume_yards.clay", 1), ("fume_yards.mesh", 1)],
                outputs: &[("fume_yards.repair_lot", 1)],
            },
            PILOT_REPAIR_EVENTS[1],
        ];
        altered.expectations.recipe_events = WRONG_RECIPE_EVENTS;
        assert_ne!(binding(&spec).unwrap(), binding(&altered).unwrap());
        assert!(validate_session(&altered, &session, &content).is_err());
        altered = spec;
        altered.expectations.forbidden_character_inventory = &["rope"];
        assert_ne!(binding(&spec).unwrap(), binding(&altered).unwrap());
        assert!(validate_session(&altered, &session, &content).is_err());
        let recipe_positions: Vec<_> = session
            .state()
            .event_log
            .iter()
            .enumerate()
            .filter_map(|(i, event)| {
                matches!(event.kind, forge_kernel::EventKind::RecipeApplied { .. }).then_some(i)
            })
            .collect();
        assert_eq!(recipe_positions.len(), 2);
        for mutation in 0..5 {
            let mut bad = session.state().clone();
            match mutation {
                0 => {
                    bad.event_log.remove(recipe_positions[0]);
                }
                1 => {
                    bad.event_log[recipe_positions[0]].turn += 1;
                }
                2 => {
                    if let forge_kernel::EventKind::RecipeApplied { inputs, .. } =
                        &mut bad.event_log[recipe_positions[0]].kind
                    {
                        inputs.insert("fume_yards.clay".to_owned(), 1);
                    }
                }
                3 => {
                    if let forge_kernel::EventKind::RecipeApplied { outputs, .. } =
                        &mut bad.event_log[recipe_positions[0]].kind
                    {
                        outputs.insert("fume_yards.repair_lot".to_owned(), 2);
                    }
                }
                _ => bad
                    .event_log
                    .push(bad.event_log[recipe_positions[0]].clone()),
            }
            assert!(
                validate_recipe_events(&bad, PILOT_REPAIR_EVENTS).is_err(),
                "recipe corruption {mutation} passed"
            );
        }
    }

    #[test]
    fn pilot_choices_and_late_entry_have_checked_semantic_witnesses() {
        let content = crate::load_content().unwrap();
        let screen = run(get("m2-fume-cold-screen").unwrap(), &content).unwrap();
        let bare = run(get("m2-fume-unscreened-freight").unwrap(), &content).unwrap();
        assert_eq!(
            screen.state().character.resources["coin"],
            bare.state().character.resources["coin"]
        );
        assert_eq!(
            screen.state().character.resources["stamina"],
            bare.state().character.resources["stamina"] + 2
        );
        let late = run(get("m2-fume-late-repair").unwrap(), &content).unwrap();
        assert!(
            late.trace().steps[..128]
                .iter()
                .all(|step| step.observation.location_id != "fume_yards.workshop")
        );
        assert_eq!(
            late.trace().steps[128].observation.location_id,
            "fume_yards.workshop"
        );
        assert!(late.state().world.flags.contains("ending_relief"));
    }
}
