use forge_kernel::{CompiledContent, enumerate_legal_actions, sha256_json};
use forge_replay::{Session, TraceStart};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

use crate::{VerifyError, replay_error};

const SCENARIO_BINDING_FORMAT: &str = "forge-scenario-spec-v1";
const RECIPE_BINDING_FORMAT: &str = "forge-scenario-recipe-v1";

const REQUIRED_SCENARIO_IDS: &[&str] = &[
    "m0-ilyan",
    "m0-rook",
    "m1-outcome-split-flow",
    "m1-outcome-hold-market",
    "m1-outcome-relief-channel",
    "m1-outcome-break-toll",
    "m1-outcome-overload-disaster",
    "m1-area-lowsail-market",
    "m1-area-red-sluice",
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
pub(super) struct ScenarioExpectations {
    final_location: &'static str,
    final_action_definition: &'static str,
    final_observation_contains: &'static str,
    exclusive_after_action: &'static str,
    required_world_flags: &'static [&'static str],
    forbidden_world_flags: &'static [&'static str],
    required_location_flags: &'static [(&'static str, &'static str)],
    required_deeds: &'static [&'static str],
    required_visited_locations: &'static [&'static str],
    forbidden_legal_definitions: &'static [&'static str],
}

#[derive(Clone, Copy, Debug, Serialize)]
pub(super) struct ScenarioSpec {
    pub id: &'static str,
    claim_id: &'static str,
    character_preset_id: &'static str,
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

const SPLIT_FLOW_STEPS: &[ScenarioStep] = &[
    action!("travel_adjacent", "destination" => "lowsail.levee"),
    action!("travel_adjacent", "destination" => "red_sluice.floor"),
    action!("floor.read_harmonics"),
    action!("travel_adjacent", "destination" => "red_sluice.top"),
    action!("top.check_wheels"),
    action!("top.split_flow"),
    action!("travel_adjacent", "destination" => "lowsail.return"),
    action!("return.share_water"),
];

const HOLD_MARKET_STEPS: &[ScenarioStep] = &[
    action!("checkpoint.show_charter"),
    action!("travel_adjacent", "destination" => "lowsail.levee"),
    action!("travel_adjacent", "destination" => "red_sluice.floor"),
    action!("travel_adjacent", "destination" => "red_sluice.top"),
    action!("top.hold_market"),
    action!("travel_adjacent", "destination" => "lowsail.return"),
    action!("return.count_dry_stalls"),
];

const RELIEF_CHANNEL_STEPS: &[ScenarioStep] = &[
    action!("travel_adjacent", "destination" => "lowsail.docks"),
    action!("docks.ring_warning"),
    action!("travel_adjacent", "destination" => "lowsail.levee"),
    action!("travel_adjacent", "destination" => "red_sluice.floor"),
    action!("floor.open_relief"),
    action!("travel_adjacent", "destination" => "red_sluice.top"),
    action!("top.divert_relief"),
    action!("travel_adjacent", "destination" => "lowsail.return"),
    action!("return.move_inland"),
];

const BREAK_TOLL_STEPS: &[ScenarioStep] = &[
    action!("travel_adjacent", "destination" => "lowsail.levee"),
    action!("travel_adjacent", "destination" => "red_sluice.floor"),
    action!("travel_adjacent", "destination" => "red_sluice.top"),
    action!("top.break_toll"),
    action!("travel_adjacent", "destination" => "lowsail.return"),
    action!("return.open_ferry"),
];

const OVERLOAD_STEPS: &[ScenarioStep] = &[
    action!("travel_adjacent", "destination" => "lowsail.levee"),
    action!("travel_adjacent", "destination" => "red_sluice.floor"),
    action!("floor.force_wheel"),
    action!("travel_adjacent", "destination" => "red_sluice.top"),
    action!("top.overload"),
    action!("travel_adjacent", "destination" => "lowsail.return"),
    action!("return.face_flood"),
];

const LOWSAIL_AREA_STEPS: &[ScenarioStep] = &[
    action!("checkpoint.blend_workers"),
    action!("travel_adjacent", "destination" => "lowsail.docks"),
    action!("docks.press_yara"),
    action!("docks.ring_warning"),
    action!("travel_adjacent", "destination" => "lowsail.levee"),
    action!("levee.culvert_path"),
    action!("floor.open_relief"),
    action!("travel_adjacent", "destination" => "red_sluice.top"),
    action!("top.divert_relief"),
    action!("travel_adjacent", "destination" => "lowsail.return"),
    action!("return.move_inland"),
];

const RED_SLUICE_AREA_STEPS: &[ScenarioStep] = &[
    action!("checkpoint.audit_order"),
    action!("travel_adjacent", "destination" => "lowsail.levee"),
    action!("levee.inspect_damage"),
    action!("levee.send_report"),
    action!("travel_adjacent", "destination" => "red_sluice.floor"),
    action!("floor.test_pressure"),
    action!("floor.stabilize_gauge"),
    action!("floor.read_harmonics"),
    action!("travel_adjacent", "destination" => "red_sluice.top"),
    action!("top.rescue_worker"),
    action!("top.check_wheels"),
    action!("top.split_flow"),
    action!("travel_adjacent", "destination" => "lowsail.return"),
    action!("return.share_water"),
];

const SCENARIOS: &[ScenarioSpec] = &[
    ScenarioSpec {
        id: "m0-ilyan",
        claim_id: "milestone-0.character-path.ilyan",
        character_preset_id: "ilyan",
        seed: 71,
        steps: M0_ILYAN_STEPS,
        expectations: ScenarioExpectations {
            final_location: "lowsail.levee",
            final_action_definition: "travel_adjacent",
            final_observation_contains: "",
            exclusive_after_action: "",
            required_world_flags: &["forged_order_found"],
            forbidden_world_flags: &[],
            required_location_flags: &[("lowsail_market", "order_audited")],
            required_deeds: &["read_forged_order"],
            required_visited_locations: &["lowsail_market", "lowsail.levee"],
            forbidden_legal_definitions: &[],
        },
    },
    ScenarioSpec {
        id: "m0-rook",
        claim_id: "milestone-0.character-path.rook",
        character_preset_id: "rook",
        seed: 71,
        steps: M0_ROOK_STEPS,
        expectations: ScenarioExpectations {
            final_location: "lowsail.levee",
            final_action_definition: "travel_adjacent",
            final_observation_contains: "",
            exclusive_after_action: "",
            required_world_flags: &["culvert_revealed"],
            forbidden_world_flags: &[],
            required_location_flags: &[("lowsail_market", "worker_cover")],
            required_deeds: &["found_worker_cover"],
            required_visited_locations: &["lowsail_market", "lowsail.levee"],
            forbidden_legal_definitions: &[],
        },
    },
    ScenarioSpec {
        id: "m1-outcome-split-flow",
        claim_id: "split-tide.outcome.split-flow",
        character_preset_id: "ilyan",
        seed: 71,
        steps: SPLIT_FLOW_STEPS,
        expectations: ScenarioExpectations {
            final_location: "lowsail.return",
            final_action_definition: "return.share_water",
            final_observation_contains: "Mira records the shared flow as a new market charter.",
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
            forbidden_legal_definitions: OUTCOME_DEFINITIONS,
        },
    },
    ScenarioSpec {
        id: "m1-outcome-hold-market",
        claim_id: "split-tide.outcome.hold-market",
        character_preset_id: "ilyan",
        seed: 71,
        steps: HOLD_MARKET_STEPS,
        expectations: ScenarioExpectations {
            final_location: "lowsail.return",
            final_action_definition: "return.count_dry_stalls",
            final_observation_contains: "The market stays open, but the dry works lose ground.",
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
            forbidden_legal_definitions: OUTCOME_DEFINITIONS,
        },
    },
    ScenarioSpec {
        id: "m1-outcome-relief-channel",
        claim_id: "split-tide.outcome.relief-channel",
        character_preset_id: "ilyan",
        seed: 71,
        steps: RELIEF_CHANNEL_STEPS,
        expectations: ScenarioExpectations {
            final_location: "lowsail.return",
            final_action_definition: "return.move_inland",
            final_observation_contains: "The market moves inland before the next surge.",
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
            forbidden_legal_definitions: OUTCOME_DEFINITIONS,
        },
    },
    ScenarioSpec {
        id: "m1-outcome-break-toll",
        claim_id: "split-tide.outcome.break-toll",
        character_preset_id: "rook",
        seed: 71,
        steps: BREAK_TOLL_STEPS,
        expectations: ScenarioExpectations {
            final_location: "lowsail.return",
            final_action_definition: "return.open_ferry",
            final_observation_contains: "The free ferry route links both shores.",
            exclusive_after_action: "top.break_toll",
            required_world_flags: &[
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
            required_location_flags: &[("lowsail.return", "ferry_free")],
            required_deeds: &["freed_ferry", "opened_free_ferry"],
            required_visited_locations: &[
                "lowsail_market",
                "lowsail.levee",
                "red_sluice.floor",
                "red_sluice.top",
                "lowsail.return",
            ],
            forbidden_legal_definitions: OUTCOME_DEFINITIONS,
        },
    },
    ScenarioSpec {
        id: "m1-outcome-overload-disaster",
        claim_id: "split-tide.outcome.overload-disaster",
        character_preset_id: "rook",
        seed: 71,
        steps: OVERLOAD_STEPS,
        expectations: ScenarioExpectations {
            final_location: "lowsail.return",
            final_action_definition: "return.face_flood",
            final_observation_contains: "The lower market floods and the broken gates remain.",
            exclusive_after_action: "top.overload",
            required_world_flags: &[
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
            forbidden_legal_definitions: OUTCOME_DEFINITIONS,
        },
    },
    ScenarioSpec {
        id: "m1-area-lowsail-market",
        claim_id: "split-tide.area.lowsail-market",
        character_preset_id: "rook",
        seed: 71,
        steps: LOWSAIL_AREA_STEPS,
        expectations: ScenarioExpectations {
            final_location: "lowsail.return",
            final_action_definition: "return.move_inland",
            final_observation_contains: "The market moves inland before the next surge.",
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
            forbidden_legal_definitions: OUTCOME_DEFINITIONS,
        },
    },
    ScenarioSpec {
        id: "m1-area-red-sluice",
        claim_id: "split-tide.area.red-sluice",
        character_preset_id: "ilyan",
        seed: 71,
        steps: RED_SLUICE_AREA_STEPS,
        expectations: ScenarioExpectations {
            final_location: "lowsail.return",
            final_action_definition: "return.share_water",
            final_observation_contains: "Mira records the shared flow as a new market charter.",
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
    character_preset_id: &'a str,
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
    let mut session =
        Session::new_game(spec.character_preset_id, spec.seed, content).map_err(replay_error)?;
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
    validate_recipe(spec, session)?;
    let state = session.state();
    let expected = &spec.expectations;
    if state.world.current_location != expected.final_location {
        return Err(VerifyError::new(
            "scenario did not reach its claimed final location",
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

    validate_forbidden_actions(state, content, expected.forbidden_legal_definitions)
}

fn validate_forbidden_actions(
    state: &forge_kernel::GameState,
    content: &CompiledContent,
    forbidden: &[&str],
) -> Result<(), VerifyError> {
    let final_definitions: BTreeSet<_> = enumerate_legal_actions(state, content)
        .map_err(|_| VerifyError::new("could not enumerate final scenario actions"))?
        .into_iter()
        .map(|action| action.definition_id)
        .collect();
    for definition in forbidden {
        if final_definitions.contains(*definition) {
            return Err(VerifyError::new(
                "scenario left a contradictory outcome action legal",
            ));
        }
    }
    Ok(())
}

fn validate_recipe(spec: &ScenarioSpec, session: &Session<'_>) -> Result<(), VerifyError> {
    match &session.trace().start {
        TraceStart::CharacterPreset {
            character_preset_id,
            seed,
        } if character_preset_id == spec.character_preset_id && *seed == spec.seed => {}
        _ => {
            return Err(VerifyError::new(
                "scenario start does not match its bound recipe",
            ));
        }
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
        if (!spec.expectations.forbidden_legal_definitions.is_empty()
            && spec.expectations.exclusive_after_action.is_empty())
            || (!spec.expectations.exclusive_after_action.is_empty() && exclusivity_checks != 1)
        {
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
            character_preset_id: spec.character_preset_id,
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
            character_preset_id: ordered.character_preset_id,
            seed: ordered.seed,
            steps: &[BoundRecipeStep {
                definition_id: RECIPE_AB[0].definition_id,
                parameters: parameter_map(&RECIPE_AB[0]).unwrap(),
            }],
        })
        .unwrap();
        let reordered_hash = sha256_json(&BoundRecipe {
            format: RECIPE_BINDING_FORMAT,
            character_preset_id: reordered.character_preset_id,
            seed: reordered.seed,
            steps: &[BoundRecipeStep {
                definition_id: RECIPE_BA[0].definition_id,
                parameters: parameter_map(&RECIPE_BA[0]).unwrap(),
            }],
        })
        .unwrap();
        assert_eq!(ordered_hash, reordered_hash);
    }
}
