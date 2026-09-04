use forge_kernel::{
    CharacterChoiceSelection, CharacterSelection, CompiledContent, enumerate_legal_actions,
    sha256_json,
};
use forge_replay::{Session, TraceStart};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

use crate::{VerifyError, replay_error};

const SCENARIO_BINDING_FORMAT: &str = "forge-scenario-spec-v1";
const RECIPE_BINDING_FORMAT: &str = "forge-scenario-recipe-v1";

const REQUIRED_SCENARIO_IDS: &[&str] = &[
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
    "m1-warning-unrelayed",
    "m1-warning-relayed",
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
pub(super) struct ScenarioExpectations {
    final_location: &'static str,
    final_action_definition: &'static str,
    final_observation_contains: &'static str,
    forbidden_observation_contains: &'static [&'static str],
    final_world_time: Option<u64>,
    exclusive_after_action: &'static str,
    required_world_flags: &'static [&'static str],
    forbidden_world_flags: &'static [&'static str],
    required_location_flags: &'static [(&'static str, &'static str)],
    required_deeds: &'static [&'static str],
    required_visited_locations: &'static [&'static str],
    required_npc_knowledge: &'static [ScenarioNpcKnowledgeExpectation],
    forbidden_npc_knowledge: &'static [ScenarioNpcKnowledgeAbsence],
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
            final_location: "lowsail.levee",
            final_action_definition: "travel_adjacent",
            final_observation_contains: "",
            forbidden_observation_contains: &[],
            final_world_time: None,
            exclusive_after_action: "",
            required_world_flags: &["forged_order_found"],
            forbidden_world_flags: &[],
            required_location_flags: &[("lowsail_market", "order_audited")],
            required_deeds: &["read_forged_order"],
            required_visited_locations: &["lowsail_market", "lowsail.levee"],
            required_npc_knowledge: &[],
            forbidden_npc_knowledge: &[],
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
            final_location: "lowsail.levee",
            final_action_definition: "travel_adjacent",
            final_observation_contains: "",
            forbidden_observation_contains: &[],
            final_world_time: None,
            exclusive_after_action: "",
            required_world_flags: &["culvert_revealed"],
            forbidden_world_flags: &[],
            required_location_flags: &[("lowsail_market", "worker_cover")],
            required_deeds: &["found_worker_cover"],
            required_visited_locations: &["lowsail_market", "lowsail.levee"],
            required_npc_knowledge: &[],
            forbidden_npc_knowledge: &[],
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
            final_location: "lowsail.levee",
            final_action_definition: "travel_adjacent",
            final_observation_contains: "",
            forbidden_observation_contains: &[],
            final_world_time: None,
            exclusive_after_action: "",
            required_world_flags: &["forged_order_found", "stolen_route"],
            forbidden_world_flags: &[],
            required_location_flags: &[("lowsail_market", "order_audited")],
            required_deeds: &["stole_permit", "read_forged_order"],
            required_visited_locations: &["lowsail_market", "lowsail.levee"],
            required_npc_knowledge: &[],
            forbidden_npc_knowledge: &[],
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
            final_location: "lowsail.levee",
            final_action_definition: "travel_adjacent",
            final_observation_contains: "",
            forbidden_observation_contains: &[],
            final_world_time: None,
            exclusive_after_action: "",
            required_world_flags: &["worker_credit"],
            forbidden_world_flags: &[],
            required_location_flags: &[],
            required_deeds: &["saved_worker"],
            required_visited_locations: &["lowsail_market", "lowsail.levee"],
            required_npc_knowledge: &[],
            forbidden_npc_knowledge: &[],
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
            required_location_flags: &[("lowsail.return", "market_flooded")],
            required_deeds: &["faced_flood"],
            required_visited_locations: &["lowsail_market", "lowsail.return"],
            required_npc_knowledge: &[],
            forbidden_npc_knowledge: &[],
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
            required_npc_knowledge: &[],
            forbidden_npc_knowledge: &[],
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
            required_npc_knowledge: &[],
            forbidden_npc_knowledge: &[],
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
            final_location: "red_sluice.floor",
            final_action_definition: "levee.authority_path",
            final_observation_contains: "",
            forbidden_observation_contains: &["Edrik knows Lowsail has been warned"],
            final_world_time: Some(6),
            exclusive_after_action: "",
            required_world_flags: &["market_warned"],
            forbidden_world_flags: &[],
            required_location_flags: &[("red_sluice.floor", "authorized_entry")],
            required_deeds: &[],
            required_visited_locations: &[
                "lowsail_market",
                "lowsail.docks",
                "lowsail.levee",
                "red_sluice.floor",
            ],
            required_npc_knowledge: OREN_MARKET_WARNING_KNOWLEDGE,
            forbidden_npc_knowledge: UNRELAYED_FORBIDDEN_NPC_KNOWLEDGE,
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
            final_location: "red_sluice.floor",
            final_action_definition: "levee.authority_path",
            final_observation_contains: "Edrik knows Lowsail has been warned",
            forbidden_observation_contains: &[],
            final_world_time: Some(6),
            exclusive_after_action: "",
            required_world_flags: &["market_warned"],
            forbidden_world_flags: &[],
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
            required_npc_knowledge: RELAYED_REQUIRED_NPC_KNOWLEDGE,
            forbidden_npc_knowledge: &[],
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
            required_npc_knowledge: &[],
            forbidden_npc_knowledge: &[],
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
            required_npc_knowledge: &[],
            forbidden_npc_knowledge: &[],
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
            required_npc_knowledge: &[],
            forbidden_npc_knowledge: &[],
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
            required_npc_knowledge: &[],
            forbidden_npc_knowledge: &[],
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
            required_npc_knowledge: &[],
            forbidden_npc_knowledge: &[],
            required_legal_definitions: &[],
            forbidden_legal_definitions: OUTCOME_DEFINITIONS,
        },
    },
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
    validate_legal_set(
        state,
        content,
        expected.required_legal_definitions,
        expected.forbidden_legal_definitions,
    )
}

fn validate_forbidden_actions(
    state: &forge_kernel::GameState,
    content: &CompiledContent,
    forbidden: &[&str],
) -> Result<(), VerifyError> {
    validate_legal_set(state, content, &[], forbidden)
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

fn knowledge_provenance_matches(
    expected: &ScenarioKnowledgeProvenance,
    actual: &forge_kernel::KnowledgeProvenance,
) -> bool {
    match (expected, actual) {
        (ScenarioKnowledgeProvenance::Witnessed, forge_kernel::KnowledgeProvenance::Witnessed) => {
            true
        }
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
}
