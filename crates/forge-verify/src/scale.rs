//! Synthetic capacity evidence for the authoritative content compiler and
//! kernel. This fixture is deliberately not authored game content and makes
//! no breadth, depth, or area-quality claim.

use forge_content::{
    ActionDefinition, CharacterPreset, ContentContract, ContentDraft, Effect, LocationDefinition,
    ParameterDomain, ParameterSpec, StringRef,
};
use forge_kernel::{
    Character, CompiledContent, Condition, GameState, enumerate_legal_actions, legal_action_digest,
    sha256_json, step,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const SCALE_FORMAT_VERSION: &str = "forge-scale-report-v1";
pub const SCALE_LOCATION_COUNT: usize = 500;
pub const SCALE_HOP_COUNT: usize = 500;
pub const SCALE_DIRECTED_EXIT_COUNT: usize = SCALE_LOCATION_COUNT * 2;
pub const SCALE_ACTION_DEFINITION_COUNT: usize = 1;
pub const SCALE_ACTIONS_PER_STATE: usize = 2;
pub const SCALE_CATALOG_ACTION_COUNT: usize = SCALE_HOP_COUNT * SCALE_ACTIONS_PER_STATE;
pub const SCALE_CATALOG_PAGE_COUNT: usize = SCALE_HOP_COUNT;
pub const SCALE_MAX_DRAFT_BYTES: usize = 4 * 1024 * 1024;
pub const SCALE_MAX_COMPILED_BYTES: usize = 4 * 1024 * 1024;
pub const SCALE_MAX_INITIAL_STATE_BYTES: usize = 512 * 1024;
pub const SCALE_MAX_FINAL_STATE_BYTES: usize = 4 * 1024 * 1024;
pub const SCALE_MAX_REPORT_BYTES: usize = 256 * 1024;

const SCALE_FIXTURE_ID: &str = "synthetic-ring-500-v1";
const SCALE_WORLD_ID: &str = "synthetic-scale-ring";
const SCALE_START_LOCATION: &str = "loc-0000";
const SCALE_PRESET_ID: &str = "scale-runner";
const SCALE_TRAVEL_DEFINITION: &str = "travel_adjacent";
const SCALE_CLAIM_SCOPE: &str = "capacity_fixture";
const SCALE_DISCLAIMER: &str = "Generated substrate capacity evidence only; not authored breadth, NPC depth, mechanic depth, area quality, or Skyrim parity.";

/// Structural work and serialization limits are part of the checked claim.
/// Host timing is deliberately excluded so scheduling noise cannot change
/// whether identical evidence passes on another machine.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ScaleBudget {
    pub exact_locations: usize,
    pub exact_directed_exits: usize,
    pub exact_hops: usize,
    pub exact_actions_per_state: usize,
    pub max_graph_frontier_entries: usize,
    pub max_catalog_states_checked: usize,
    pub max_catalog_actions_checked: usize,
    pub max_catalog_pages_checked: usize,
    pub initial_page_size: usize,
    pub exact_initial_pages: usize,
    pub traversal_page_size: usize,
    pub max_draft_bytes: usize,
    pub max_compiled_bytes: usize,
    pub max_initial_state_bytes: usize,
    pub max_final_state_bytes: usize,
    pub max_report_bytes: usize,
}

impl Default for ScaleBudget {
    fn default() -> Self {
        Self {
            exact_locations: SCALE_LOCATION_COUNT,
            exact_directed_exits: SCALE_DIRECTED_EXIT_COUNT,
            exact_hops: SCALE_HOP_COUNT,
            exact_actions_per_state: SCALE_ACTIONS_PER_STATE,
            max_graph_frontier_entries: SCALE_LOCATION_COUNT,
            max_catalog_states_checked: SCALE_HOP_COUNT,
            max_catalog_actions_checked: SCALE_CATALOG_ACTION_COUNT,
            max_catalog_pages_checked: SCALE_CATALOG_PAGE_COUNT,
            initial_page_size: 1,
            exact_initial_pages: SCALE_ACTIONS_PER_STATE,
            traversal_page_size: SCALE_ACTIONS_PER_STATE,
            max_draft_bytes: SCALE_MAX_DRAFT_BYTES,
            max_compiled_bytes: SCALE_MAX_COMPILED_BYTES,
            max_initial_state_bytes: SCALE_MAX_INITIAL_STATE_BYTES,
            max_final_state_bytes: SCALE_MAX_FINAL_STATE_BYTES,
            max_report_bytes: SCALE_MAX_REPORT_BYTES,
        }
    }
}

/// A deterministic, machine-checkable capacity report. The disclaimer is
/// repeated in the serialized artifact to prevent synthetic nodes being
/// mistaken for authored locations.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ScaleReport {
    pub format_version: String,
    pub verifier_id: String,
    pub fixture_id: String,
    pub claim_scope: String,
    pub disclaimer: String,
    pub generated_substrate: bool,
    pub budget: ScaleBudget,
    pub start_locations: Vec<String>,
    pub location_count: usize,
    pub directed_exit_count: usize,
    pub action_definition_count: usize,
    pub character_preset_count: usize,
    pub npc_count: usize,
    pub draft_bytes: usize,
    pub compiled_content_bytes: usize,
    pub initial_state_bytes: usize,
    pub final_state_bytes: usize,
    pub draft_hash: String,
    pub content_build_id: String,
    pub source_id_digest: String,
    pub compiled_id_digest: String,
    pub runtime_id_digest: String,
    pub graph_reached_location_count: usize,
    pub max_graph_frontier_entries: usize,
    pub graph_reachability_digest: String,
    pub graph_predecessor_digest: String,
    pub initial_action_count: usize,
    pub initial_action_set_digest: String,
    pub initial_page_count: usize,
    pub initial_paged_action_ids_digest: String,
    pub hop_count: usize,
    pub catalog_states_checked: usize,
    pub catalog_actions_checked: usize,
    pub catalog_pages_checked: usize,
    pub max_legal_actions: usize,
    pub catalog_fingerprint_digest: String,
    pub canonical_action_reached_location_count: usize,
    pub canonical_action_reachability_digest: String,
    pub transition_fingerprint_digest: String,
    pub action_sequence_digest: String,
    pub final_event_count: usize,
    pub final_observation_digest: String,
    pub final_location: String,
    pub final_runtime_id_digest: String,
}

impl ScaleReport {
    pub fn to_pretty_json(&self) -> Result<String, crate::VerifyError> {
        let mut json = serde_json::to_string_pretty(self).map_err(|error| {
            crate::VerifyError::new(format!("could not serialize scale report: {error}"))
        })?;
        json.try_reserve(1)
            .map_err(|_| crate::VerifyError::new("scale report exceeded the memory budget"))?;
        json.push('\n');
        if json.len() > self.budget.max_report_bytes {
            return Err(crate::VerifyError::new(format!(
                "scale report exceeds its {}-byte ceiling",
                self.budget.max_report_bytes
            )));
        }
        Ok(json)
    }

    pub fn from_json(input: &str) -> Result<Self, crate::VerifyError> {
        if input.len() > SCALE_MAX_REPORT_BYTES {
            return Err(crate::VerifyError::new(format!(
                "scale report exceeds the {SCALE_MAX_REPORT_BYTES}-byte input ceiling"
            )));
        }
        forge_kernel::validate_unique_json_keys(input).map_err(|error| {
            crate::VerifyError::new(format!("invalid scale report JSON: {error}"))
        })?;
        serde_json::from_str(input)
            .map_err(|error| crate::VerifyError::new(format!("invalid scale report JSON: {error}")))
    }
}

#[derive(Clone, Debug, Serialize)]
struct HopFingerprint {
    hop: usize,
    from: String,
    to: String,
    action_id: String,
    pre_state_id: String,
    post_state_id: String,
}

#[derive(Clone, Debug, Serialize)]
struct PredecessorGraph {
    entries: Vec<(String, String)>,
}

struct GraphTraversal {
    reached_ids: Vec<String>,
    predecessor_entries: Vec<(String, String)>,
    max_frontier_entries: usize,
}

struct RingTraversal {
    hop_count: usize,
    catalog_states_checked: usize,
    catalog_actions_checked: usize,
    catalog_pages_checked: usize,
    max_legal_actions: usize,
    catalog_fingerprint_digest: String,
    reached_ids: Vec<String>,
    transition_fingerprint_digest: String,
    action_sequence_digest: String,
    final_observation_digest: String,
    final_state: GameState,
}

#[derive(Clone, Debug, Serialize)]
struct CatalogFingerprint {
    location: String,
    state_id: String,
    action_set_digest: String,
    paged_action_ids_digest: String,
    page_count: usize,
}

struct PageEvidence {
    page_count: usize,
    action_ids_digest: String,
}

/// Generate the fixed 500-location synthetic capacity report.
pub fn generate_scale_report() -> Result<ScaleReport, crate::VerifyError> {
    generate_scale_report_with_budget(ScaleBudget::default())
}

/// Verify a serialized or caller-provided report against fresh deterministic
/// fixture generation. No measured duration is stored or compared.
pub fn check_scale_report(report: &ScaleReport) -> Result<(), crate::VerifyError> {
    validate_report_shape(report)?;
    let expected = generate_scale_report_with_budget(report.budget.clone())?;
    if report != &expected {
        return Err(crate::VerifyError::new(
            "scale report does not match regenerated fixture evidence",
        ));
    }
    Ok(())
}

fn generate_scale_report_with_budget(
    budget: ScaleBudget,
) -> Result<ScaleReport, crate::VerifyError> {
    validate_budget(&budget)?;
    let draft = scale_draft()?;
    let expected_ids = expected_location_ids()?;
    let mut source_ids = Vec::new();
    source_ids
        .try_reserve(draft.locations.len())
        .map_err(|_| crate::VerifyError::new("scale source ID allocation failed"))?;
    source_ids.extend(draft.locations.iter().map(|location| location.id.clone()));
    source_ids.sort();
    if source_ids != expected_ids {
        return Err(crate::VerifyError::new(
            "scale source does not contain the exact canonical location ID set",
        ));
    }
    let source_id_digest = sha256_json(&source_ids).map_err(hash_error)?;
    let draft_bytes = serde_json::to_vec(&draft).map_err(|error| {
        crate::VerifyError::new(format!("could not serialize scale draft: {error}"))
    })?;
    if draft_bytes.len() > budget.max_draft_bytes {
        return Err(crate::VerifyError::new(format!(
            "scale draft exceeds its {}-byte ceiling",
            budget.max_draft_bytes
        )));
    }
    let draft_hash = sha256_json(&draft)
        .map_err(|error| crate::VerifyError::new(format!("could not hash scale draft: {error}")))?;
    let content = forge_content::compile(draft.clone()).map_err(|error| {
        crate::VerifyError::new(format!("scale fixture failed validation: {error}"))
    })?;
    if !content.has_valid_build_id() {
        return Err(crate::VerifyError::new(
            "scale fixture produced an invalid content build identity",
        ));
    }
    let compiled_bytes = serde_json::to_vec(&content).map_err(|error| {
        crate::VerifyError::new(format!(
            "could not serialize compiled scale content: {error}"
        ))
    })?;
    if compiled_bytes.len() > budget.max_compiled_bytes {
        return Err(crate::VerifyError::new(format!(
            "compiled scale content exceeds its {}-byte ceiling",
            budget.max_compiled_bytes
        )));
    }
    let (compiled_ids, directed_exit_count) =
        validate_compiled_fixture(&content, &expected_ids, &budget)?;
    let compiled_id_digest = sha256_json(&compiled_ids).map_err(hash_error)?;

    let initial = content
        .new_game(SCALE_PRESET_ID, 71)
        .map_err(|error| crate::VerifyError::new(format!("scale fixture start failed: {error}")))?;
    let initial_state_bytes = serde_json::to_vec(&initial)
        .map_err(|error| {
            crate::VerifyError::new(format!("could not serialize initial scale state: {error}"))
        })?
        .len();
    if initial_state_bytes > budget.max_initial_state_bytes {
        return Err(crate::VerifyError::new(format!(
            "initial scale state exceeds its {}-byte ceiling",
            budget.max_initial_state_bytes
        )));
    }
    let initial_runtime_ids = runtime_ids(&initial)?;
    if initial_runtime_ids != expected_ids {
        return Err(crate::VerifyError::new(
            "scale runtime does not contain the exact canonical location ID set",
        ));
    }
    let runtime_id_digest = sha256_json(&initial_runtime_ids).map_err(hash_error)?;
    let graph = graph_reachability(&content, &budget)?;
    if graph.reached_ids.len() != budget.exact_locations {
        return Err(crate::VerifyError::new(format!(
            "scale graph reached {} locations; expected {}",
            graph.reached_ids.len(),
            budget.exact_locations
        )));
    }
    let graph_reachability_digest = sha256_json(&graph.reached_ids).map_err(hash_error)?;
    let graph_predecessor_digest = sha256_json(&PredecessorGraph {
        entries: graph.predecessor_entries,
    })
    .map_err(hash_error)?;

    let (initial_actions, initial_action_set_digest, initial_page_count, initial_page_digest) =
        initial_catalog(&initial, &content, &budget)?;

    let traversal = traverse_ring(&initial, &content, &budget)?;
    let final_runtime_ids = runtime_ids(&traversal.final_state)?;
    if final_runtime_ids != initial_runtime_ids {
        return Err(crate::VerifyError::new(
            "runtime location IDs changed during scale traversal",
        ));
    }
    let final_runtime_id_digest = sha256_json(&final_runtime_ids).map_err(hash_error)?;
    let canonical_action_reachability_digest =
        sha256_json(&traversal.reached_ids).map_err(hash_error)?;
    if source_id_digest != compiled_id_digest
        || source_id_digest != runtime_id_digest
        || source_id_digest != graph_reachability_digest
        || source_id_digest != canonical_action_reachability_digest
        || source_id_digest != final_runtime_id_digest
    {
        return Err(crate::VerifyError::new(
            "scale source, compiled, runtime, graph, and canonical-action location sets diverged",
        ));
    }
    if traversal.final_state.world.current_location != SCALE_START_LOCATION {
        return Err(crate::VerifyError::new(
            "500-hop scale traversal did not return to its start location",
        ));
    }
    if traversal.final_state.event_log.len() != budget.exact_hops {
        return Err(crate::VerifyError::new(
            "scale traversal event count does not match its hop count",
        ));
    }
    let final_state_bytes = serde_json::to_vec(&traversal.final_state)
        .map_err(|error| {
            crate::VerifyError::new(format!("could not serialize final scale state: {error}"))
        })?
        .len();
    if final_state_bytes > budget.max_final_state_bytes {
        return Err(crate::VerifyError::new(format!(
            "final scale state exceeds its {}-byte ceiling",
            budget.max_final_state_bytes
        )));
    }

    let report = ScaleReport {
        format_version: SCALE_FORMAT_VERSION.to_owned(),
        verifier_id: crate::VERIFIER_ID.to_owned(),
        fixture_id: SCALE_FIXTURE_ID.to_owned(),
        claim_scope: SCALE_CLAIM_SCOPE.to_owned(),
        disclaimer: SCALE_DISCLAIMER.to_owned(),
        generated_substrate: true,
        budget,
        start_locations: vec![SCALE_START_LOCATION.to_owned()],
        location_count: content.locations().count(),
        directed_exit_count,
        action_definition_count: content.actions().count(),
        character_preset_count: content.character_presets().count(),
        npc_count: content.npcs().count(),
        draft_bytes: draft_bytes.len(),
        compiled_content_bytes: compiled_bytes.len(),
        initial_state_bytes,
        final_state_bytes,
        draft_hash,
        content_build_id: content.build_id().to_owned(),
        source_id_digest,
        compiled_id_digest,
        runtime_id_digest,
        graph_reached_location_count: graph.reached_ids.len(),
        max_graph_frontier_entries: graph.max_frontier_entries,
        graph_reachability_digest,
        graph_predecessor_digest,
        initial_action_count: initial_actions,
        initial_action_set_digest,
        initial_page_count,
        initial_paged_action_ids_digest: initial_page_digest,
        hop_count: traversal.hop_count,
        catalog_states_checked: traversal.catalog_states_checked,
        catalog_actions_checked: traversal.catalog_actions_checked,
        catalog_pages_checked: traversal.catalog_pages_checked,
        max_legal_actions: traversal.max_legal_actions,
        catalog_fingerprint_digest: traversal.catalog_fingerprint_digest,
        canonical_action_reached_location_count: traversal.reached_ids.len(),
        canonical_action_reachability_digest,
        transition_fingerprint_digest: traversal.transition_fingerprint_digest,
        action_sequence_digest: traversal.action_sequence_digest,
        final_event_count: traversal.final_state.event_log.len(),
        final_observation_digest: traversal.final_observation_digest,
        final_location: traversal.final_state.world.current_location,
        final_runtime_id_digest,
    };
    validate_report_shape(&report)?;
    let serialized_len = serde_json::to_vec(&report)
        .map_err(|error| {
            crate::VerifyError::new(format!("could not serialize scale report: {error}"))
        })?
        .len();
    if serialized_len > report.budget.max_report_bytes {
        return Err(crate::VerifyError::new(format!(
            "scale report exceeds its {}-byte ceiling",
            report.budget.max_report_bytes
        )));
    }
    Ok(report)
}

fn validate_budget(budget: &ScaleBudget) -> Result<(), crate::VerifyError> {
    if budget != &ScaleBudget::default() {
        return Err(crate::VerifyError::new(
            "scale budget does not match the reviewed capacity and performance ceilings",
        ));
    }
    Ok(())
}

fn validate_report_shape(report: &ScaleReport) -> Result<(), crate::VerifyError> {
    if report.format_version != SCALE_FORMAT_VERSION {
        return Err(crate::VerifyError::new(format!(
            "unsupported scale report format {}",
            report.format_version
        )));
    }
    if report.verifier_id != crate::VERIFIER_ID {
        return Err(crate::VerifyError::new(
            "scale report verifier identity does not match",
        ));
    }
    if report.fixture_id != SCALE_FIXTURE_ID {
        return Err(crate::VerifyError::new(
            "scale report fixture identity does not match",
        ));
    }
    if report.claim_scope != SCALE_CLAIM_SCOPE {
        return Err(crate::VerifyError::new(
            "scale report claim scope is not the synthetic capacity fixture",
        ));
    }
    if report.disclaimer != SCALE_DISCLAIMER || !report.generated_substrate {
        return Err(crate::VerifyError::new(
            "scale report disclaimer does not preserve the non-breadth boundary",
        ));
    }
    validate_budget(&report.budget)?;
    if report.start_locations != [SCALE_START_LOCATION] {
        return Err(crate::VerifyError::new(
            "scale report start set does not match the reviewed fixture",
        ));
    }
    if report.location_count != report.budget.exact_locations
        || report.directed_exit_count != report.budget.exact_directed_exits
        || report.action_definition_count != SCALE_ACTION_DEFINITION_COUNT
        || report.character_preset_count != 1
        || report.npc_count != 0
        || report.hop_count != report.budget.exact_hops
        || report.graph_reached_location_count != report.budget.exact_locations
        || report.canonical_action_reached_location_count != report.budget.exact_locations
        || report.catalog_states_checked != report.budget.max_catalog_states_checked
        || report.catalog_actions_checked != report.budget.max_catalog_actions_checked
        || report.catalog_pages_checked != report.budget.max_catalog_pages_checked
        || report.max_legal_actions != report.budget.exact_actions_per_state
        || report.final_event_count != report.budget.exact_hops
    {
        return Err(crate::VerifyError::new(
            "scale report counts do not match its checked structural budget",
        ));
    }
    if report.initial_action_count != report.budget.exact_actions_per_state
        || report.initial_page_count != report.budget.exact_initial_pages
        || report.max_graph_frontier_entries > report.budget.max_graph_frontier_entries
        || report.draft_bytes > report.budget.max_draft_bytes
        || report.compiled_content_bytes > report.budget.max_compiled_bytes
        || report.initial_state_bytes > report.budget.max_initial_state_bytes
        || report.final_state_bytes > report.budget.max_final_state_bytes
    {
        return Err(crate::VerifyError::new(
            "scale report exceeds a checked resource or catalog budget",
        ));
    }
    if report.source_id_digest != report.compiled_id_digest
        || report.source_id_digest != report.runtime_id_digest
        || report.source_id_digest != report.graph_reachability_digest
        || report.source_id_digest != report.canonical_action_reachability_digest
        || report.source_id_digest != report.final_runtime_id_digest
    {
        return Err(crate::VerifyError::new(
            "scale report location-set digests disagree",
        ));
    }
    if report.final_location != SCALE_START_LOCATION {
        return Err(crate::VerifyError::new(
            "scale report final location is not the ring start",
        ));
    }
    Ok(())
}

fn initial_catalog(
    state: &GameState,
    content: &CompiledContent,
    budget: &ScaleBudget,
) -> Result<(usize, String, usize, String), crate::VerifyError> {
    let legal = enumerate_legal_actions(state, content).map_err(kernel_error)?;
    validate_ring_catalog(state, content, &legal, budget)?;
    if legal.len() != budget.exact_actions_per_state {
        return Err(crate::VerifyError::new(format!(
            "scale start enumerated {} actions; expected exactly {}",
            legal.len(),
            budget.exact_actions_per_state
        )));
    }
    let action_set_digest = legal_action_digest(&legal).map_err(hash_error)?;
    let pages = verify_paged_catalog(
        state,
        content,
        &legal,
        budget.initial_page_size,
        budget.exact_initial_pages,
    )?;
    Ok((
        legal.len(),
        action_set_digest,
        pages.page_count,
        pages.action_ids_digest,
    ))
}

fn verify_paged_catalog(
    state: &GameState,
    content: &CompiledContent,
    legal: &[forge_kernel::CanonicalAction],
    page_size: usize,
    max_pages: usize,
) -> Result<PageEvidence, crate::VerifyError> {
    if legal.windows(2).any(|pair| {
        (
            pair[0].definition_id.as_str(),
            &pair[0].parameters,
            pair[0].action_id.as_str(),
        ) >= (
            pair[1].definition_id.as_str(),
            &pair[1].parameters,
            pair[1].action_id.as_str(),
        )
    }) {
        return Err(crate::VerifyError::new(
            "scale kernel enumeration is not in canonical semantic order",
        ));
    }
    if page_size == 0 || max_pages == 0 {
        return Err(crate::VerifyError::new(
            "scale paging budgets must be positive",
        ));
    }
    let expected_page_count = legal.len().div_ceil(page_size);
    if expected_page_count == 0 || expected_page_count > max_pages {
        return Err(crate::VerifyError::new(
            "scale catalog cannot fit its checked page budget",
        ));
    }
    let action_set_digest = legal_action_digest(legal).map_err(hash_error)?;
    let mut page_ids = Vec::new();
    page_ids
        .try_reserve(legal.len())
        .map_err(|_| crate::VerifyError::new("scale page ID allocation failed"))?;
    let mut offset = 0usize;
    let mut page_count = 0usize;
    loop {
        page_count = page_count
            .checked_add(1)
            .ok_or_else(|| crate::VerifyError::new("scale page count overflowed"))?;
        if page_count > max_pages {
            return Err(crate::VerifyError::new(
                "scale page count exceeded its structural budget",
            ));
        }
        let page = content
            .action_page(state, offset, page_size)
            .map_err(|error| {
                crate::VerifyError::new(format!("scale action page failed: {error}"))
            })?;
        if page.total != legal.len()
            || page.digest != action_set_digest
            || page.build_id != content.build_id()
            || page.state_id != state.state_id()
            || page.offset != offset
            || page.actions.len() > page_size
        {
            return Err(crate::VerifyError::new(
                "scale paged catalog disagrees with complete kernel enumeration",
            ));
        }
        page_ids.extend(page.actions.into_iter().map(|action| action.action_id));
        match page.next_offset {
            Some(next) => {
                if next <= offset {
                    return Err(crate::VerifyError::new(
                        "scale action page cursor did not advance",
                    ));
                }
                offset = next;
            }
            None => break,
        }
    }
    let legal_ids: Vec<_> = legal
        .iter()
        .map(|action| action.action_id.clone())
        .collect();
    if page_ids != legal_ids {
        return Err(crate::VerifyError::new(
            "scale paged catalog union differs from kernel enumeration",
        ));
    }
    if page_count != expected_page_count {
        return Err(crate::VerifyError::new(
            "scale paging did not consume the exact complete catalog",
        ));
    }
    let page_digest = sha256_json(&page_ids).map_err(hash_error)?;
    Ok(PageEvidence {
        page_count,
        action_ids_digest: page_digest,
    })
}

fn expected_location_ids() -> Result<Vec<String>, crate::VerifyError> {
    let mut ids = Vec::new();
    ids.try_reserve(SCALE_LOCATION_COUNT)
        .map_err(|_| crate::VerifyError::new("scale expected-ID allocation failed"))?;
    ids.extend((0..SCALE_LOCATION_COUNT).map(location_id));
    Ok(ids)
}

fn validate_compiled_fixture(
    content: &CompiledContent,
    expected_ids: &[String],
    budget: &ScaleBudget,
) -> Result<(Vec<String>, usize), crate::VerifyError> {
    if content.contract() != ContentContract::Fixture
        || content.world_id() != SCALE_WORLD_ID
        || content.start_location() != SCALE_START_LOCATION
        || content.locations().count() != budget.exact_locations
        || content.actions().count() != SCALE_ACTION_DEFINITION_COUNT
        || content.character_presets().count() != 1
        || content.npcs().count() != 0
    {
        return Err(crate::VerifyError::new(
            "compiled scale fixture registries do not match the reviewed synthetic contract",
        ));
    }
    if content.character_preset(SCALE_PRESET_ID).is_none() {
        return Err(crate::VerifyError::new(
            "compiled scale fixture is missing its declared preset",
        ));
    }

    let mut compiled_ids = Vec::new();
    compiled_ids
        .try_reserve(content.locations().count())
        .map_err(|_| crate::VerifyError::new("scale compiled-ID allocation failed"))?;
    compiled_ids.extend(content.locations().map(|(id, _)| id.clone()));
    if compiled_ids != expected_ids {
        return Err(crate::VerifyError::new(
            "compiled scale fixture does not contain the exact canonical location ID set",
        ));
    }

    let mut directed_exit_count = 0usize;
    for index in 0..SCALE_LOCATION_COUNT {
        let id = location_id(index);
        let location = content
            .location(&id)
            .ok_or_else(|| crate::VerifyError::new("scale topology location is missing"))?;
        let mut expected_exits = vec![
            location_id((index + SCALE_LOCATION_COUNT - 1) % SCALE_LOCATION_COUNT),
            location_id((index + 1) % SCALE_LOCATION_COUNT),
        ];
        expected_exits.sort();
        if !location.terminal || location.exits != expected_exits {
            return Err(crate::VerifyError::new(
                "scale topology is not the exact terminal bidirectional ring",
            ));
        }
        directed_exit_count = directed_exit_count
            .checked_add(location.exits.len())
            .ok_or_else(|| crate::VerifyError::new("scale directed-exit count overflowed"))?;
        for exit in &location.exits {
            if content
                .location(exit)
                .is_none_or(|neighbor| !neighbor.exits.contains(&id))
            {
                return Err(crate::VerifyError::new(
                    "scale ring contains a non-reciprocal exit",
                ));
            }
        }
    }
    if directed_exit_count != budget.exact_directed_exits {
        return Err(crate::VerifyError::new(
            "scale ring directed-exit count does not match its structural budget",
        ));
    }

    let travel = content
        .action(SCALE_TRAVEL_DEFINITION)
        .ok_or_else(|| crate::VerifyError::new("scale travel definition is missing"))?;
    let parameter_is_adjacent = travel.parameters.as_slice()
        == [ParameterSpec {
            name: "destination".to_owned(),
            domain: ParameterDomain::LocationsAdjacent,
        }];
    let effect_moves_parameter = travel.effects.as_slice()
        == [Effect::MoveCharacter {
            location: StringRef::Parameter("destination".to_owned()),
        }];
    if !travel.meaningful
        || !travel.movement
        || !travel.locations.is_empty()
        || travel.condition != Condition::Always
        || !parameter_is_adjacent
        || !effect_moves_parameter
    {
        return Err(crate::VerifyError::new(
            "scale travel definition does not match the reviewed adjacent-movement contract",
        ));
    }
    Ok((compiled_ids, directed_exit_count))
}

fn validate_ring_catalog(
    state: &GameState,
    content: &CompiledContent,
    legal: &[forge_kernel::CanonicalAction],
    budget: &ScaleBudget,
) -> Result<(), crate::VerifyError> {
    if legal.len() != budget.exact_actions_per_state {
        return Err(crate::VerifyError::new(format!(
            "scale location enumerated {} actions; expected exactly {}",
            legal.len(),
            budget.exact_actions_per_state
        )));
    }
    let location = content
        .location(&state.world.current_location)
        .ok_or_else(|| crate::VerifyError::new("scale catalog location is missing"))?;
    let expected_destinations: BTreeSet<_> = location.exits.iter().cloned().collect();
    let expected_state_id = state.state_id();
    let mut actual_destinations = BTreeSet::new();
    for action in legal {
        let destination = action.parameters.get("destination").ok_or_else(|| {
            crate::VerifyError::new("scale travel action omitted its destination")
        })?;
        if action.definition_id != SCALE_TRAVEL_DEFINITION
            || action.build_id != content.build_id()
            || action.pre_state_id != expected_state_id
            || action.action_id != action.recomputed_id()
            || action.parameters.len() != 1
            || !actual_destinations.insert(destination.clone())
        {
            return Err(crate::VerifyError::new(
                "scale catalog contains a malformed, duplicated, or misbound action",
            ));
        }
    }
    if actual_destinations != expected_destinations {
        return Err(crate::VerifyError::new(
            "scale catalog does not exactly enumerate both adjacent destinations",
        ));
    }
    Ok(())
}

fn traverse_ring(
    initial: &GameState,
    content: &CompiledContent,
    budget: &ScaleBudget,
) -> Result<RingTraversal, crate::VerifyError> {
    let mut state = initial.clone();
    let mut action_ids = Vec::new();
    let mut fingerprints = Vec::new();
    let mut catalog_fingerprints = Vec::new();
    let mut reached = BTreeSet::new();
    reached.insert(state.world.current_location.clone());
    action_ids
        .try_reserve(budget.exact_hops)
        .map_err(|_| crate::VerifyError::new("scale action sequence allocation failed"))?;
    fingerprints
        .try_reserve(budget.exact_hops)
        .map_err(|_| crate::VerifyError::new("scale transition fingerprint allocation failed"))?;
    catalog_fingerprints
        .try_reserve(budget.max_catalog_states_checked)
        .map_err(|_| crate::VerifyError::new("scale catalog fingerprint allocation failed"))?;
    let mut hop_count = 0usize;
    let mut catalog_states_checked = 0usize;
    let mut catalog_actions_checked = 0usize;
    let mut catalog_pages_checked = 0usize;
    let mut max_legal_actions = 0usize;
    let mut final_observation_digest = None;
    for hop in 0..budget.exact_hops {
        let target = location_id((hop + 1) % budget.exact_locations);
        let from = state.world.current_location.clone();
        if from != location_id(hop % budget.exact_locations) {
            return Err(crate::VerifyError::new(
                "scale traversal departed from the canonical ring sequence",
            ));
        }
        let pre_state_id = state.state_id();
        let legal = enumerate_legal_actions(&state, content).map_err(kernel_error)?;
        validate_ring_catalog(&state, content, &legal, budget)?;
        let action_set_digest = legal_action_digest(&legal).map_err(hash_error)?;
        let expected_pages = legal.len().div_ceil(budget.traversal_page_size);
        let pages = verify_paged_catalog(
            &state,
            content,
            &legal,
            budget.traversal_page_size,
            expected_pages,
        )?;
        catalog_states_checked = catalog_states_checked
            .checked_add(1)
            .ok_or_else(|| crate::VerifyError::new("scale catalog state count overflowed"))?;
        catalog_actions_checked = catalog_actions_checked
            .checked_add(legal.len())
            .ok_or_else(|| crate::VerifyError::new("scale catalog action count overflowed"))?;
        catalog_pages_checked = catalog_pages_checked
            .checked_add(pages.page_count)
            .ok_or_else(|| crate::VerifyError::new("scale catalog page count overflowed"))?;
        max_legal_actions = max_legal_actions.max(legal.len());
        if catalog_states_checked > budget.max_catalog_states_checked
            || catalog_actions_checked > budget.max_catalog_actions_checked
            || catalog_pages_checked > budget.max_catalog_pages_checked
        {
            return Err(crate::VerifyError::new(
                "scale traversal exceeded its checked catalog work budget",
            ));
        }
        catalog_fingerprints.push(CatalogFingerprint {
            location: from.clone(),
            state_id: pre_state_id.clone(),
            action_set_digest,
            paged_action_ids_digest: pages.action_ids_digest,
            page_count: pages.page_count,
        });
        let action = legal
            .into_iter()
            .find(|candidate| {
                candidate.definition_id == SCALE_TRAVEL_DEFINITION
                    && candidate.parameters.get("destination") == Some(&target)
            })
            .ok_or_else(|| {
                crate::VerifyError::new(format!(
                    "scale traversal could not find legal hop from {from} to {target}"
                ))
            })?;
        let entropy = state.entropy.clone();
        let transition = step(&state, &action, content, &entropy).map_err(kernel_error)?;
        if transition.pre_state_id() != pre_state_id
            || transition.action() != &action
            || transition.state().world.current_location != target
        {
            return Err(crate::VerifyError::new(
                "scale traversal transition failed its canonical predecessor check",
            ));
        }
        content
            .validate_state(transition.state())
            .map_err(|error| {
                crate::VerifyError::new(format!("scale state validation failed: {error}"))
            })?;
        if hop + 1 == budget.exact_hops {
            let observation = content
                .observe_after_transition(&transition)
                .map_err(|error| {
                    crate::VerifyError::new(format!("scale final observation failed: {error}"))
                })?;
            final_observation_digest = Some(sha256_json(&observation).map_err(hash_error)?);
        }
        reached.insert(target.clone());
        action_ids.push(action.action_id.clone());
        fingerprints.push(HopFingerprint {
            hop,
            from,
            to: target,
            action_id: action.action_id,
            pre_state_id,
            post_state_id: transition.post_state_id().to_owned(),
        });
        state = transition.into_state();
        hop_count = hop_count
            .checked_add(1)
            .ok_or_else(|| crate::VerifyError::new("scale hop counter overflowed"))?;
    }
    if hop_count != budget.exact_hops {
        return Err(crate::VerifyError::new(
            "scale traversal completed an unexpected hop count",
        ));
    }
    if reached.len() != budget.exact_locations
        || catalog_states_checked != budget.max_catalog_states_checked
        || catalog_actions_checked != budget.max_catalog_actions_checked
        || catalog_pages_checked != budget.max_catalog_pages_checked
        || max_legal_actions != budget.exact_actions_per_state
    {
        return Err(crate::VerifyError::new(
            "scale traversal did not satisfy its exact reachability and catalog work contract",
        ));
    }
    let transition_fingerprint_digest = sha256_json(&fingerprints).map_err(hash_error)?;
    let action_sequence_digest = sha256_json(&action_ids).map_err(hash_error)?;
    let catalog_fingerprint_digest = sha256_json(&catalog_fingerprints).map_err(hash_error)?;
    Ok(RingTraversal {
        hop_count,
        catalog_states_checked,
        catalog_actions_checked,
        catalog_pages_checked,
        max_legal_actions,
        catalog_fingerprint_digest,
        reached_ids: reached.into_iter().collect(),
        transition_fingerprint_digest,
        action_sequence_digest,
        final_observation_digest: final_observation_digest.ok_or_else(|| {
            crate::VerifyError::new("scale traversal did not produce a final observation")
        })?,
        final_state: state,
    })
}

fn graph_reachability(
    content: &CompiledContent,
    budget: &ScaleBudget,
) -> Result<GraphTraversal, crate::VerifyError> {
    if !content.has_location(SCALE_START_LOCATION) {
        return Err(crate::VerifyError::new(
            "scale graph is missing its declared start location",
        ));
    }
    let mut queue = VecDeque::new();
    let mut seen = BTreeSet::new();
    let mut discovered = BTreeSet::new();
    let mut predecessors = BTreeMap::new();
    queue
        .try_reserve(1)
        .map_err(|_| crate::VerifyError::new("scale graph queue allocation failed"))?;
    queue.push_back(SCALE_START_LOCATION.to_owned());
    discovered.insert(SCALE_START_LOCATION.to_owned());
    let mut max_frontier_entries = queue.len();
    while let Some(location_id) = queue.pop_front() {
        if !seen.insert(location_id.clone()) {
            return Err(crate::VerifyError::new(
                "scale graph frontier contained a duplicate location",
            ));
        }
        let location = content
            .location(&location_id)
            .ok_or_else(|| crate::VerifyError::new("scale graph references an unknown location"))?;
        for exit in &location.exits {
            if !content.has_location(exit) {
                return Err(crate::VerifyError::new(
                    "scale graph contains an unknown exit",
                ));
            }
            if discovered.insert(exit.clone()) {
                predecessors.insert(exit.clone(), location_id.clone());
                queue
                    .try_reserve(1)
                    .map_err(|_| crate::VerifyError::new("scale graph queue allocation failed"))?;
                queue.push_back(exit.clone());
                max_frontier_entries = max_frontier_entries.max(queue.len());
                if discovered.len() > budget.exact_locations
                    || max_frontier_entries > budget.max_graph_frontier_entries
                {
                    return Err(crate::VerifyError::new(
                        "scale graph traversal exceeded its checked frontier budget",
                    ));
                }
            }
        }
    }
    if seen.len() != content.locations().count() || seen != discovered {
        return Err(crate::VerifyError::new(format!(
            "scale graph reached {} of {} locations",
            seen.len(),
            content.locations().count()
        )));
    }
    let predecessor_entries = predecessors.into_iter().collect();
    Ok(GraphTraversal {
        reached_ids: seen.into_iter().collect(),
        predecessor_entries,
        max_frontier_entries,
    })
}

fn runtime_ids(state: &GameState) -> Result<Vec<String>, crate::VerifyError> {
    let mut ids = Vec::new();
    ids.try_reserve(state.world.locations.len())
        .map_err(|_| crate::VerifyError::new("scale runtime ID allocation failed"))?;
    ids.extend(state.world.locations.keys().cloned());
    Ok(ids)
}

fn scale_draft() -> Result<ContentDraft, crate::VerifyError> {
    let mut locations = Vec::new();
    locations
        .try_reserve(SCALE_LOCATION_COUNT)
        .map_err(|_| crate::VerifyError::new("scale location allocation failed"))?;
    for index in 0..SCALE_LOCATION_COUNT {
        locations.push(LocationDefinition {
            id: location_id(index),
            name: format!("Synthetic Location {index}"),
            description: "A synthetic ring road continues.".to_owned(),
            description_variants: Vec::new(),
            exits: vec![
                location_id((index + SCALE_LOCATION_COUNT - 1) % SCALE_LOCATION_COUNT),
                location_id((index + 1) % SCALE_LOCATION_COUNT),
            ],
            terminal: true,
        });
    }
    Ok(ContentDraft {
        schema_version: "forge-schema-v5".to_owned(),
        rules_version: "forge-rules-v3".to_owned(),
        world_id: SCALE_WORLD_ID.to_owned(),
        contract: ContentContract::Fixture,
        start_location: SCALE_START_LOCATION.to_owned(),
        character_presets: vec![CharacterPreset {
            id: SCALE_PRESET_ID.to_owned(),
            display_name: "Scale Runner".to_owned(),
            summary: "A synthetic capacity traveler.".to_owned(),
            character: scale_character(),
        }],
        character_creation: None,
        locations,
        npcs: Vec::new(),
        timed_events: Vec::new(),
        actions: vec![ActionDefinition {
            id: SCALE_TRAVEL_DEFINITION.to_owned(),
            label: "Travel".to_owned(),
            category: "Route".to_owned(),
            result: "You travel onward.".to_owned(),
            result_variants: Vec::new(),
            locations: Vec::new(),
            condition: Condition::Always,
            effects: vec![Effect::MoveCharacter {
                location: StringRef::Parameter("destination".to_owned()),
            }],
            parameters: vec![ParameterSpec {
                name: "destination".to_owned(),
                domain: ParameterDomain::LocationsAdjacent,
            }],
            meaningful: true,
            movement: true,
        }],
    })
}

fn scale_character() -> Character {
    Character {
        id: SCALE_PRESET_ID.to_owned(),
        lineage: "synthetic".to_owned(),
        origin: "synthetic".to_owned(),
        background: "runner".to_owned(),
        aptitudes: BTreeMap::new(),
        skills: BTreeSet::new(),
        values: BTreeSet::new(),
        traits: BTreeSet::new(),
        flaws: BTreeSet::new(),
        appearance: BTreeMap::new(),
        affiliations: BTreeMap::new(),
        reputation: BTreeMap::new(),
        knowledge: BTreeSet::new(),
        inventory: BTreeMap::new(),
        resources: BTreeMap::new(),
        injuries: BTreeSet::new(),
        deeds: BTreeSet::new(),
        promises: BTreeSet::new(),
        discoveries: BTreeSet::new(),
        facets: BTreeMap::new(),
    }
}

fn location_id(index: usize) -> String {
    format!("loc-{index:04}")
}

fn hash_error(error: forge_kernel::HashError) -> crate::VerifyError {
    crate::VerifyError::new(format!("scale hash failed: {error}"))
}

fn kernel_error(error: forge_kernel::KernelError) -> crate::VerifyError {
    crate::VerifyError::new(format!("scale kernel operation failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::OnceLock;

    fn checked_report() -> ScaleReport {
        static REPORT: OnceLock<ScaleReport> = OnceLock::new();
        REPORT
            .get_or_init(|| generate_scale_report().expect("scale report generates"))
            .clone()
    }

    #[test]
    fn scale_report_is_complete_and_explicitly_synthetic() {
        let report = checked_report();
        assert_eq!(report.location_count, SCALE_LOCATION_COUNT);
        assert_eq!(report.directed_exit_count, SCALE_DIRECTED_EXIT_COUNT);
        assert_eq!(report.hop_count, SCALE_HOP_COUNT);
        assert_eq!(report.catalog_states_checked, SCALE_HOP_COUNT);
        assert_eq!(report.catalog_actions_checked, SCALE_CATALOG_ACTION_COUNT);
        assert_eq!(report.catalog_pages_checked, SCALE_CATALOG_PAGE_COUNT);
        assert_eq!(report.final_location, SCALE_START_LOCATION);
        assert_eq!(report.claim_scope, SCALE_CLAIM_SCOPE);
        assert!(report.generated_substrate);
        assert!(report.disclaimer.contains("not authored breadth"));
        assert_eq!(report.source_id_digest, report.compiled_id_digest);
        assert_eq!(report.source_id_digest, report.runtime_id_digest);
        assert_eq!(
            report.source_id_digest,
            report.canonical_action_reachability_digest
        );
        let json = report.to_pretty_json().expect("scale JSON");
        assert!(!json.contains("event_log"));
        assert!(!json.contains("entropy"));
    }

    #[test]
    fn too_small_budget_is_rejected_before_work() {
        let budget = ScaleBudget {
            exact_hops: SCALE_HOP_COUNT - 1,
            ..ScaleBudget::default()
        };
        assert!(validate_budget(&budget).is_err());

        let budget = ScaleBudget {
            exact_initial_pages: 1,
            ..ScaleBudget::default()
        };
        assert!(validate_budget(&budget).is_err());

        let budget = ScaleBudget {
            max_draft_bytes: usize::MAX,
            ..ScaleBudget::default()
        };
        assert!(validate_budget(&budget).is_err());
    }

    #[test]
    fn malformed_disconnected_fixture_is_rejected() {
        let mut draft = scale_draft().expect("scale draft");
        let first = draft
            .locations
            .iter_mut()
            .find(|location| location.id == SCALE_START_LOCATION)
            .expect("start location");
        first.exits.clear();
        for location in &mut draft.locations {
            if location.id == "loc-0001" || location.id == "loc-0499" {
                location.exits.retain(|exit| exit != SCALE_START_LOCATION);
            }
        }
        assert!(forge_content::compile(draft).is_err());
    }

    #[test]
    fn report_tampering_is_rejected() {
        let mut report = checked_report();
        report.graph_reachability_digest.push('x');
        assert!(check_scale_report(&report).is_err());
    }

    #[test]
    fn report_parser_rejects_unknown_fields() {
        let report = checked_report();
        let mut json = report.to_pretty_json().expect("scale JSON");
        json.insert(json.len() - 2, ',');
        json.insert_str(json.len() - 2, "\"unexpected\":true");
        assert!(ScaleReport::from_json(&json).is_err());
    }

    #[test]
    fn generated_draft_compiles_to_one_deterministic_identity() {
        let left = forge_content::compile(scale_draft().unwrap()).unwrap();
        let right = forge_content::compile(scale_draft().unwrap()).unwrap();
        assert_eq!(left.build_id(), right.build_id());
    }
}
