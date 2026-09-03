//! Independent-process evidence generation and checking for Adventure Forge.
//!
//! The verifier reconstructs player-safe traces through the production kernel
//! and compares public observations plus opaque hashes and receipts. Checked
//! witnesses prove deterministic execution without publishing hidden state or
//! events.

use forge_content::parse_and_compile_production;
use forge_kernel::{CanonicalAction, CompiledContent, Observation, sha256_json};
use forge_replay::{PlayerTrace, ReplayError, Session, resume_player_trace, verify};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

mod crawler;

pub use crawler::{CrawlBudget, CrawlReport, crawl_production};

const SPLIT_TIDE: &str = include_str!("../../../content/split-tide.json");
pub const WITNESS_FORMAT_VERSION: &str = "forge-evidence-witness-v1";
pub const SCENARIO_IDS: [&str; 2] = ["m0-ilyan", "m0-rook"];
pub const MAX_WITNESS_STEPS: usize = 4_096;

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
    pub player_trace: PlayerTrace,
    pub initial_state_id: String,
    pub initial_observation: Observation,
    pub initial_observation_hash: String,
    pub initial_receipt: String,
    pub steps: Vec<StepFingerprint>,
    pub final_state_id: String,
    pub final_receipt: String,
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
        serde_json::from_str(input)
            .map_err(|error| VerifyError::new(format!("invalid evidence witness JSON: {error}")))
    }
}

pub fn generate_witness(scenario_id: &str) -> Result<EvidenceWitness, VerifyError> {
    let content = load_content()?;
    let session = run_scenario(scenario_id, &content)?;
    verify(session.trace(), &content).map_err(replay_error)?;
    witness_from_session(scenario_id, &session)
}

pub fn generate_crawl_report() -> Result<CrawlReport, VerifyError> {
    let content = load_content()?;
    crawl_production(&content, CrawlBudget::default())
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
    if !SCENARIO_IDS.contains(&witness.scenario_id.as_str()) {
        return Err(VerifyError::new(format!(
            "unknown evidence scenario {}",
            witness.scenario_id
        )));
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
    let actual = witness_from_session(&witness.scenario_id, &session)?;
    compare_witness(witness, &actual)?;

    let expected_session = run_scenario(&witness.scenario_id, &content)?;
    verify(expected_session.trace(), &content).map_err(replay_error)?;
    let expected = witness_from_session(&witness.scenario_id, &expected_session)?;
    compare_witness(&expected, &actual)
}

fn load_content() -> Result<CompiledContent, VerifyError> {
    parse_and_compile_production(SPLIT_TIDE)
        .map_err(|_| VerifyError::new("embedded production content failed validation"))
}

fn run_scenario<'content>(
    scenario_id: &str,
    content: &'content CompiledContent,
) -> Result<Session<'content>, VerifyError> {
    let (character, first_action) = match scenario_id {
        "m0-ilyan" => ("ilyan", "checkpoint.audit_order"),
        "m0-rook" => ("rook", "checkpoint.blend_workers"),
        other => {
            return Err(VerifyError::new(format!(
                "unknown evidence scenario {other}"
            )));
        }
    };
    let mut session = Session::new_game(character, 71, content).map_err(replay_error)?;
    record_matching(&mut session, content, first_action, None)?;
    record_matching(
        &mut session,
        content,
        "travel_adjacent",
        Some(("destination", "lowsail.levee")),
    )?;
    Ok(session)
}

fn record_matching(
    session: &mut Session<'_>,
    content: &CompiledContent,
    definition_id: &str,
    parameter: Option<(&str, &str)>,
) -> Result<(), VerifyError> {
    let action = forge_kernel::enumerate_legal_actions(session.state(), content)
        .map_err(|_| VerifyError::new("could not enumerate scenario actions"))?
        .into_iter()
        .find(|action| action_matches(action, definition_id, parameter))
        .ok_or_else(|| VerifyError::new("declared evidence action is not currently legal"))?;
    session.record(&action).map_err(replay_error)?;
    Ok(())
}

fn action_matches(
    action: &CanonicalAction,
    definition_id: &str,
    parameter: Option<(&str, &str)>,
) -> bool {
    action.definition_id == definition_id
        && parameter.is_none_or(|(name, value)| {
            action
                .parameters
                .get(name)
                .is_some_and(|found| found == value)
        })
}

fn witness_from_session(
    scenario_id: &str,
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
        scenario_id: scenario_id.to_owned(),
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
        for scenario in SCENARIO_IDS {
            assert_eq!(
                generate_witness(scenario).unwrap(),
                generate_witness(scenario).unwrap()
            );
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
        changed.scenario_id = "m0-rook".to_owned();
        assert!(check_witness(&changed).is_err());

        let mut changed = original;
        changed.scenario_id = "unknown".to_owned();
        assert!(check_witness(&changed).is_err());
        assert!(generate_witness("unknown").is_err());
    }

    #[test]
    fn witness_json_contains_public_observations_but_not_hidden_values() {
        let json = generate_witness("m0-rook")
            .unwrap()
            .to_pretty_json()
            .unwrap();
        assert!(json.contains("Sava watches the wanted runner"));
        for hidden in [
            "initial_state\"",
            "scheduled_events",
            "entropy_before\"",
            "entropy_after\"",
            "entropy_draws\"",
            "knowledge\"",
        ] {
            assert!(!json.contains(hidden), "witness leaked {hidden}");
        }
    }
}
