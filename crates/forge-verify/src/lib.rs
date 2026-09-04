//! Independent-process evidence generation and checking for Adventure Forge.
//!
//! The verifier reconstructs player-safe traces through the production kernel
//! and compares public observations plus opaque hashes and receipts. Checked
//! witnesses prove deterministic execution without publishing hidden state or
//! events.

use forge_content::parse_and_compile_production;
use forge_kernel::{CompiledContent, Observation, sha256_json, validate_unique_json_keys};
use forge_replay::{PlayerTrace, ReplayError, Session, resume_player_trace, verify};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

mod crawler;
mod scale;
mod scenarios;

pub use crawler::{CrawlBudget, CrawlReport, crawl_production};
pub use scale::{
    SCALE_MAX_REPORT_BYTES, ScaleBudget, ScaleReport, check_scale_report, generate_scale_report,
};

const SPLIT_TIDE: &str = include_str!("../../../content/split-tide.json");
pub const WITNESS_FORMAT_VERSION: &str = "forge-evidence-witness-v3";
pub const MAX_WITNESS_STEPS: usize = 4_096;
pub const MAX_PLAYER_TRACE_BYTES: u64 = 16 * 1024 * 1024;

include!(concat!(env!("OUT_DIR"), "/verifier_id.rs"));

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifyError(String);

impl VerifyError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl Display for VerifyError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for VerifyError {}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StepFingerprint {
    pub step_index: u64,
    pub action_id: String,
    pub action_definition_id: String,
    pub action_parameters: std::collections::BTreeMap<String, String>,
    pub legal_action_set_digest: String,
    pub pre_state_id: String,
    pub events_hash: String,
    pub entropy_before_hash: String,
    pub entropy_draws_hash: String,
    pub entropy_after_hash: String,
    pub post_state_id: String,
    pub observation: Observation,
    pub observation_hash: String,
    pub receipt: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EvidenceWitness {
    pub format_version: String,
    pub verifier_id: String,
    pub scenario_id: String,
    pub scenario_binding: String,
    pub player_trace: PlayerTrace,
    pub initial_state_id: String,
    pub initial_observation: Observation,
    pub initial_observation_hash: String,
    pub initial_receipt: String,
    pub steps: Vec<StepFingerprint>,
    pub final_state_id: String,
    pub final_receipt: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerTraceVerification {
    pub verifier_id: String,
    pub build_id: String,
    pub action_count: usize,
    pub final_state_id: String,
    pub final_receipt: String,
}

pub fn scenario_ids() -> impl ExactSizeIterator<Item = &'static str> {
    scenarios::all().iter().map(|scenario| scenario.id)
}

impl EvidenceWitness {
    pub fn to_pretty_json(&self) -> Result<String, VerifyError> {
        let mut json = serde_json::to_string_pretty(self)
            .map_err(|_| VerifyError::new("could not serialize evidence witness"))?;
        json.try_reserve(1)
            .map_err(|_| VerifyError::new("evidence witness exceeded the memory budget"))?;
        json.push('\n');
        Ok(json)
    }

    pub fn from_json(input: &str) -> Result<Self, VerifyError> {
        validate_unique_json_keys(input)
            .map_err(|error| VerifyError::new(format!("invalid evidence witness JSON: {error}")))?;
        serde_json::from_str(input)
            .map_err(|error| VerifyError::new(format!("invalid evidence witness JSON: {error}")))
    }
}

pub fn generate_witness(scenario_id: &str) -> Result<EvidenceWitness, VerifyError> {
    let spec = scenarios::get(scenario_id)?;
    let content = load_content()?;
    let session = scenarios::run(spec, &content)?;
    verify(session.trace(), &content).map_err(replay_error)?;
    witness_from_session(spec, &session)
}

pub fn generate_crawl_report() -> Result<CrawlReport, VerifyError> {
    let content = load_content()?;
    crawl_production(&content, CrawlBudget::default())
}

pub fn check_player_trace(
    player_trace: &PlayerTrace,
) -> Result<PlayerTraceVerification, VerifyError> {
    if player_trace.action_count() > MAX_WITNESS_STEPS {
        return Err(VerifyError::new(
            "player trace exceeds the 4096-step verification budget",
        ));
    }
    let content = load_content()?;
    let session = resume_player_trace(player_trace, &content).map_err(replay_error)?;
    verify(session.trace(), &content).map_err(replay_error)?;
    let trace = session.trace();
    Ok(PlayerTraceVerification {
        verifier_id: VERIFIER_ID.to_owned(),
        build_id: trace.build_id.clone(),
        action_count: trace.steps.len(),
        final_state_id: trace.final_state_id.clone(),
        final_receipt: trace.final_receipt.clone(),
    })
}

pub fn check_witness(witness: &EvidenceWitness) -> Result<(), VerifyError> {
    if witness.format_version != WITNESS_FORMAT_VERSION {
        return Err(VerifyError::new(format!(
            "unsupported witness format {}",
            witness.format_version
        )));
    }
    if witness.verifier_id != VERIFIER_ID {
        return Err(VerifyError::new("witness verifier identity does not match"));
    }
    let spec = scenarios::get(&witness.scenario_id)?;
    if witness.scenario_binding != scenarios::binding(spec)? {
        return Err(VerifyError::new(
            "witness scenario binding does not match its reviewed specification",
        ));
    }
    if witness.steps.len() > MAX_WITNESS_STEPS
        || witness.player_trace.action_count() > MAX_WITNESS_STEPS
    {
        return Err(VerifyError::new(
            "evidence witness exceeds the 4096-step verification budget",
        ));
    }
    if witness.steps.len() != witness.player_trace.action_count() {
        return Err(VerifyError::new(
            "evidence step count does not match the player trace",
        ));
    }

    let content = load_content()?;
    let session = resume_player_trace(&witness.player_trace, &content).map_err(replay_error)?;
    verify(session.trace(), &content).map_err(replay_error)?;
    scenarios::validate_session(spec, &session, &content)?;
    let actual = witness_from_session(spec, &session)?;
    compare_witness(witness, &actual)?;

    let expected_session = scenarios::run(spec, &content)?;
    verify(expected_session.trace(), &content).map_err(replay_error)?;
    let expected = witness_from_session(spec, &expected_session)?;
    compare_witness(&expected, &actual)
}

fn load_content() -> Result<CompiledContent, VerifyError> {
    parse_and_compile_production(SPLIT_TIDE)
        .map_err(|_| VerifyError::new("embedded production content failed validation"))
}

fn witness_from_session(
    spec: &scenarios::ScenarioSpec,
    session: &Session<'_>,
) -> Result<EvidenceWitness, VerifyError> {
    let trace = session.trace();
    let mut steps = Vec::new();
    steps
        .try_reserve(trace.steps.len())
        .map_err(|_| VerifyError::new("could not reserve evidence fingerprints"))?;
    for step in &trace.steps {
        steps.push(StepFingerprint {
            step_index: step.step_index,
            action_id: step.action.action_id.clone(),
            action_definition_id: step.action.definition_id.clone(),
            action_parameters: step.action.parameters.clone(),
            legal_action_set_digest: step.legal_action_set_digest.clone(),
            pre_state_id: step.pre_state_id.clone(),
            events_hash: step.events_hash.clone(),
            entropy_before_hash: opaque_hash(&step.entropy_before)?,
            entropy_draws_hash: opaque_hash(&step.entropy_draws)?,
            entropy_after_hash: opaque_hash(&step.entropy_after)?,
            post_state_id: step.post_state_id.clone(),
            observation: step.observation.clone(),
            observation_hash: step.observation_hash.clone(),
            receipt: step.receipt.clone(),
        });
    }
    Ok(EvidenceWitness {
        format_version: WITNESS_FORMAT_VERSION.to_owned(),
        verifier_id: VERIFIER_ID.to_owned(),
        scenario_id: spec.id.to_owned(),
        scenario_binding: scenarios::binding(spec)?,
        player_trace: session.player_trace().map_err(replay_error)?,
        initial_state_id: trace.initial_state_id.clone(),
        initial_observation: trace.initial_observation.clone(),
        initial_observation_hash: trace.initial_observation_hash.clone(),
        initial_receipt: trace.initial_receipt.clone(),
        steps,
        final_state_id: trace.final_state_id.clone(),
        final_receipt: trace.final_receipt.clone(),
    })
}

fn opaque_hash<T: Serialize>(value: &T) -> Result<String, VerifyError> {
    sha256_json(value).map_err(|_| VerifyError::new("could not hash an evidence field"))
}

fn compare_witness(
    expected: &EvidenceWitness,
    actual: &EvidenceWitness,
) -> Result<(), VerifyError> {
    if expected == actual {
        return Ok(());
    }
    if expected.format_version != actual.format_version {
        return Err(VerifyError::new("witness mismatch at format_version"));
    }
    if expected.verifier_id != actual.verifier_id {
        return Err(VerifyError::new("witness mismatch at verifier_id"));
    }
    if expected.scenario_id != actual.scenario_id {
        return Err(VerifyError::new("witness mismatch at scenario_id"));
    }
    if expected.scenario_binding != actual.scenario_binding {
        return Err(VerifyError::new("witness mismatch at scenario_binding"));
    }
    if expected.player_trace != actual.player_trace {
        return Err(VerifyError::new("witness mismatch at player_trace"));
    }
    if expected.initial_state_id != actual.initial_state_id {
        return Err(VerifyError::new("witness mismatch at initial_state_id"));
    }
    if expected.initial_observation != actual.initial_observation {
        return Err(VerifyError::new("witness mismatch at initial_observation"));
    }
    if expected.initial_observation_hash != actual.initial_observation_hash {
        return Err(VerifyError::new(
            "witness mismatch at initial_observation_hash",
        ));
    }
    if expected.initial_receipt != actual.initial_receipt {
        return Err(VerifyError::new("witness mismatch at initial_receipt"));
    }
    if expected.steps != actual.steps {
        return Err(VerifyError::new("witness mismatch in step fingerprints"));
    }
    if expected.final_state_id != actual.final_state_id {
        return Err(VerifyError::new("witness mismatch at final_state_id"));
    }
    if expected.final_receipt != actual.final_receipt {
        return Err(VerifyError::new("witness mismatch at final_receipt"));
    }
    Err(VerifyError::new("witness metadata mismatch"))
}

fn replay_error(error: ReplayError) -> VerifyError {
    match error {
        ReplayError::Mismatch { path, .. } => {
            VerifyError::new(format!("replay mismatch at {path}"))
        }
        ReplayError::Json(_) => VerifyError::new("replay JSON is invalid"),
        ReplayError::InvalidTrace(message) => {
            VerifyError::new(format!("replay trace rejected: {message}"))
        }
        ReplayError::Kernel(_) => VerifyError::new("kernel rejected replay evidence"),
        ReplayError::Hash(_) | ReplayError::ResourceExhausted(_) => {
            VerifyError::new("replay verification could not complete safely")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn character_scenarios_are_deterministic_and_materially_distinct() {
        let ids: Vec<_> = scenario_ids().collect();
        assert_eq!(ids.len(), 11);
        for scenario in ids {
            let first = generate_witness(scenario).unwrap();
            let second = generate_witness(scenario).unwrap();
            assert_eq!(first, second);
            check_witness(&first).unwrap();
        }
        let ilyan = generate_witness("m0-ilyan").unwrap();
        let rook = generate_witness("m0-rook").unwrap();
        assert_ne!(ilyan.player_trace, rook.player_trace);
        assert_ne!(ilyan.initial_state_id, rook.initial_state_id);
        assert_eq!(
            ilyan.initial_observation.title,
            rook.initial_observation.title
        );
        assert_ne!(
            ilyan.initial_observation.text,
            rook.initial_observation.text
        );
        assert_ne!(
            ilyan.initial_observation_hash,
            rook.initial_observation_hash
        );
        assert_ne!(ilyan.steps[0].action_id, rook.steps[0].action_id);
        assert_eq!(
            ilyan.steps[0].action_definition_id,
            "checkpoint.audit_order"
        );
        let cross_current = generate_witness("m1-custom-cross-current").unwrap();
        let unlikely_ally = generate_witness("m1-custom-unlikely-ally").unwrap();
        assert_ne!(cross_current.player_trace, unlikely_ally.player_trace);
        assert_ne!(
            cross_current.initial_state_id,
            unlikely_ally.initial_state_id
        );
        assert_ne!(cross_current.initial_state_id, ilyan.initial_state_id);
        assert_ne!(unlikely_ally.initial_state_id, rook.initial_state_id);
        assert_eq!(
            rook.steps[0].action_definition_id,
            "checkpoint.blend_workers"
        );
        assert_ne!(
            ilyan.steps[0].legal_action_set_digest,
            rook.steps[0].legal_action_set_digest
        );
        assert_ne!(ilyan.steps[0].post_state_id, rook.steps[0].post_state_id);
    }

    #[test]
    fn independent_player_trace_check_binds_verifier_and_final_commitments() {
        let witness = generate_witness("m0-ilyan").unwrap();
        let checked = check_player_trace(&witness.player_trace).unwrap();
        assert_eq!(checked.verifier_id, VERIFIER_ID);
        assert!(
            witness
                .player_trace
                .to_json()
                .unwrap()
                .contains(&checked.build_id)
        );
        assert_eq!(checked.action_count, witness.steps.len());
        assert_eq!(checked.final_state_id, witness.final_state_id);
        assert_eq!(checked.final_receipt, witness.final_receipt);

        let mut tampered_json = witness.player_trace.to_json().unwrap();
        tampered_json = tampered_json.replacen(&witness.final_receipt, "00", 1);
        let tampered = PlayerTrace::from_json(&tampered_json).unwrap();
        assert!(check_player_trace(&tampered).is_err());
    }

    #[test]
    fn witness_tampering_and_unknown_scenarios_fail() {
        let original = generate_witness("m0-ilyan").unwrap();

        let mut changed = original.clone();
        changed.steps[0].receipt.push('x');
        assert!(check_witness(&changed).is_err());

        let mut changed = original.clone();
        changed.initial_observation.text.push_str(" altered");
        assert!(check_witness(&changed).is_err());

        let mut changed = original.clone();
        changed.steps.pop();
        assert!(check_witness(&changed).is_err());

        let mut changed = original.clone();
        changed.verifier_id.push('x');
        assert!(check_witness(&changed).is_err());

        let mut changed = original.clone();
        changed.scenario_binding.push('x');
        assert!(check_witness(&changed).is_err());

        let mut changed = original.clone();
        changed.scenario_id = "m0-rook".to_owned();
        assert!(check_witness(&changed).is_err());

        let target = generate_witness("m1-outcome-hold-market").unwrap();
        let mut relabeled = generate_witness("m1-outcome-split-flow").unwrap();
        relabeled.scenario_id = target.scenario_id;
        relabeled.scenario_binding = target.scenario_binding;
        assert!(check_witness(&relabeled).is_err());

        let mut changed = original;
        changed.scenario_id = "unknown".to_owned();
        assert!(check_witness(&changed).is_err());
        assert!(generate_witness("unknown").is_err());
    }

    #[test]
    fn witness_json_contains_public_observations_but_not_hidden_values() {
        for scenario in scenario_ids() {
            let json = generate_witness(scenario)
                .unwrap()
                .to_pretty_json()
                .unwrap();
            assert!(json.contains("scenario_binding"));
            for hidden in [
                "initial_state\"",
                "scheduled_events",
                "entropy_before\"",
                "entropy_after\"",
                "entropy_draws\"",
                "knowledge\"",
            ] {
                assert!(!json.contains(hidden), "witness {scenario} leaked {hidden}");
            }
        }
    }

    #[test]
    fn alternate_valid_path_cannot_substitute_for_bound_scenario_recipe() {
        let content = load_content().unwrap();
        let spec = scenarios::get("m1-outcome-split-flow").unwrap();
        let mut session = Session::new_game("ilyan", 71, &content).unwrap();
        let wait = forge_kernel::enumerate_legal_actions(session.state(), &content)
            .unwrap()
            .into_iter()
            .find(|action| action.definition_id == "wait_tide" && action.parameters.is_empty())
            .unwrap();
        session.record(&wait).unwrap();
        for step in spec.steps {
            scenarios::record_step(&mut session, &content, step).unwrap();
        }
        assert!(scenarios::validate_session(spec, &session, &content).is_err());

        let substituted = witness_from_session(spec, &session).unwrap();
        assert!(check_witness(&substituted).is_err());
    }
}
