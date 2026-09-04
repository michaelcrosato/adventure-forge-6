use forge_kernel::{
    CanonicalAction, CompiledContent, Condition, GameState, enumerate_legal_actions,
    legal_action_digest, sha256_json, step,
};
use serde::Serialize;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::VerifyError;

const CRAWL_EXECUTION_RECEIPT_FORMAT: &str = "forge-crawl-execution-v1";

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub struct CrawlBudget {
    pub max_depth: usize,
    pub max_expanded_states: usize,
    pub max_discovered_frontiers: usize,
    pub max_action_executions: usize,
    pub catalog_page_size: usize,
}

impl Default for CrawlBudget {
    fn default() -> Self {
        Self {
            max_depth: 12,
            max_expanded_states: 4_096,
            max_discovered_frontiers: 65_536,
            max_action_executions: 65_536,
            catalog_page_size: 7,
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct CrawlReport {
    pub verifier_id: String,
    pub build_id: String,
    pub budget: CrawlBudget,
    pub expanded_states: usize,
    pub discovered_frontiers: usize,
    pub successful_actions: usize,
    pub max_legal_actions: usize,
    pub reached_locations: BTreeSet<String>,
    pub covered_definitions: BTreeSet<String>,
    pub advertised_definitions: BTreeSet<String>,
    /// Opaque hash chain over ordered starts, expansions, legal catalogs, and
    /// transitions. This makes the checked report sensitive to traversal or
    /// catalog-order drift even when its aggregate coverage totals match.
    pub execution_receipt: String,
}

impl CrawlReport {
    pub fn to_pretty_json(&self) -> Result<String, VerifyError> {
        let mut json = serde_json::to_string_pretty(self)
            .map_err(|_| VerifyError::new("could not serialize crawl report"))?;
        json.try_reserve(1)
            .map_err(|_| VerifyError::new("crawl report exceeded the memory budget"))?;
        json.push('\n');
        Ok(json)
    }

    pub fn uncovered_definitions(&self) -> BTreeSet<String> {
        self.advertised_definitions
            .difference(&self.covered_definitions)
            .cloned()
            .collect()
    }

    pub fn is_complete(&self) -> bool {
        self.covered_definitions == self.advertised_definitions
    }
}

#[derive(Clone)]
struct Frontier {
    state: GameState,
    depth: usize,
    ordinal: usize,
    used_actions: BTreeSet<ActionShape>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ActionShape {
    location: String,
    definition_id: String,
    parameters: BTreeMap<String, String>,
}

type FrontierScore = (
    usize,
    Reverse<usize>,
    usize,
    Reverse<usize>,
    usize,
    bool,
    Reverse<usize>,
);

impl ActionShape {
    fn at_state(action: &CanonicalAction, state: &GameState) -> Self {
        Self {
            location: state.world.current_location.clone(),
            definition_id: action.definition_id.clone(),
            parameters: action.parameters.clone(),
        }
    }
}

/// Explore the production content from every authored character preset.
///
/// This is a bounded coverage proof, not a claim of exhaustive reachability.
/// Every action advertised by every expanded state is checked against the
/// complete paged catalog, executed through the reducer, observed, and state
/// validated. Success additionally requires at least one execution of every
/// authored action definition.
pub fn crawl_production(
    content: &CompiledContent,
    budget: CrawlBudget,
) -> Result<CrawlReport, VerifyError> {
    validate_budget(budget)?;
    let advertised_definitions: BTreeSet<_> = content.actions().map(|(id, _)| id.clone()).collect();
    let execution_receipt = sha256_json(&(
        CRAWL_EXECUTION_RECEIPT_FORMAT,
        "genesis",
        content.build_id(),
        budget,
    ))
    .map_err(|_| VerifyError::new("crawler could not create its execution receipt"))?;
    let mut report = CrawlReport {
        verifier_id: crate::VERIFIER_ID.to_owned(),
        build_id: content.build_id().to_owned(),
        budget,
        expanded_states: 0,
        discovered_frontiers: 0,
        successful_actions: 0,
        max_legal_actions: 0,
        reached_locations: BTreeSet::new(),
        covered_definitions: BTreeSet::new(),
        advertised_definitions,
        execution_receipt,
    };
    let mut pending = Vec::new();
    let mut dominance: BTreeMap<String, Vec<BTreeSet<ActionShape>>> = BTreeMap::new();

    for (preset_id, _) in content.character_presets() {
        let state = content
            .new_game(preset_id, 71)
            .map_err(|_| VerifyError::new("could not create a crawler start state"))?;
        let used_actions = BTreeSet::new();
        let key = normalized_state_id(&state)?;
        if accept_frontier(&mut dominance, key, &used_actions)? {
            if report.discovered_frontiers >= budget.max_discovered_frontiers {
                return Err(VerifyError::new(format!(
                    "crawler exhausted its {}-frontier budget while adding start states",
                    budget.max_discovered_frontiers
                )));
            }
            pending
                .try_reserve(1)
                .map_err(|_| VerifyError::new("crawler frontier allocation failed"))?;
            let ordinal = report.discovered_frontiers;
            report.execution_receipt = advance_execution_receipt(
                &report.execution_receipt,
                &("start", preset_id.as_str(), state.state_id(), ordinal),
            )?;
            pending.push(Frontier {
                state,
                depth: 0,
                ordinal,
                used_actions,
            });
            report.discovered_frontiers += 1;
        }
    }

    while !pending.is_empty() {
        let frontier = pop_best_frontier(&mut pending, &report, content)?;
        if report.expanded_states >= budget.max_expanded_states {
            return Err(VerifyError::new(format!(
                "crawler exhausted its {}-state budget with uncovered definitions: {}",
                budget.max_expanded_states,
                join_ids(&report.uncovered_definitions())
            )));
        }
        report.expanded_states += 1;
        report
            .reached_locations
            .insert(frontier.state.world.current_location.clone());

        let legal = enumerate_legal_actions(&frontier.state, content)
            .map_err(|_| VerifyError::new("crawler could not enumerate a valid state"))?;
        verify_catalog(&frontier.state, content, &legal, budget.catalog_page_size)?;
        report.max_legal_actions = report.max_legal_actions.max(legal.len());
        let expanded_index = report.expanded_states - 1;
        let state_id = frontier.state.state_id();
        let mut ordered_action_ids = Vec::new();
        ordered_action_ids
            .try_reserve(legal.len())
            .map_err(|_| VerifyError::new("crawler receipt catalog allocation failed"))?;
        ordered_action_ids.extend(legal.iter().map(|action| action.action_id.as_str()));
        report.execution_receipt = advance_execution_receipt(
            &report.execution_receipt,
            &(
                "expansion",
                expanded_index,
                frontier.ordinal,
                frontier.depth,
                state_id.as_str(),
                &ordered_action_ids,
            ),
        )?;

        for (action_index, action) in legal.into_iter().enumerate() {
            if report.successful_actions >= budget.max_action_executions {
                return Err(VerifyError::new(format!(
                    "crawler exhausted its {}-action budget with uncovered definitions: {}",
                    budget.max_action_executions,
                    join_ids(&report.uncovered_definitions())
                )));
            }
            let shape = ActionShape::at_state(&action, &frontier.state);
            let transition = step(&frontier.state, &action, content, &frontier.state.entropy)
                .map_err(|error| {
                    VerifyError::new(format!(
                        "advertised action {} failed reduction: {error}",
                        action.definition_id
                    ))
                })?;
            content
                .observe_after_transition(&transition)
                .map_err(|_| VerifyError::new("crawler could not observe a valid transition"))?;
            report.execution_receipt = advance_execution_receipt(
                &report.execution_receipt,
                &(
                    "transition",
                    expanded_index,
                    action_index,
                    &action,
                    transition.events(),
                    transition.entropy_before(),
                    transition.entropy_draws(),
                    transition.entropy_after(),
                    transition.post_state_id(),
                ),
            )?;
            let next_state = transition.into_state();
            content
                .validate_state(&next_state)
                .map_err(|_| VerifyError::new("crawler transition produced an invalid state"))?;
            report.successful_actions = report
                .successful_actions
                .checked_add(1)
                .ok_or_else(|| VerifyError::new("crawler successful-action count overflowed"))?;
            report
                .covered_definitions
                .insert(action.definition_id.clone());

            if frontier.depth >= budget.max_depth || frontier.used_actions.contains(&shape) {
                continue;
            }
            let mut used_actions = frontier.used_actions.clone();
            used_actions.insert(shape);
            let key = normalized_state_id(&next_state)?;
            if !accept_frontier(&mut dominance, key, &used_actions)? {
                continue;
            }
            if report.discovered_frontiers >= budget.max_discovered_frontiers {
                return Err(VerifyError::new(format!(
                    "crawler exhausted its {}-frontier budget with uncovered definitions: {}",
                    budget.max_discovered_frontiers,
                    join_ids(&report.uncovered_definitions())
                )));
            }
            pending
                .try_reserve(1)
                .map_err(|_| VerifyError::new("crawler frontier allocation failed"))?;
            pending.push(Frontier {
                state: next_state,
                depth: frontier.depth + 1,
                ordinal: report.discovered_frontiers,
                used_actions,
            });
            report.discovered_frontiers = report
                .discovered_frontiers
                .checked_add(1)
                .ok_or_else(|| VerifyError::new("crawler frontier count overflowed"))?;
        }

        if report.is_complete() {
            return Ok(report);
        }
    }

    Err(VerifyError::new(format!(
        "crawler exhausted its frontier with uncovered definitions: {}",
        join_ids(&report.uncovered_definitions())
    )))
}

fn advance_execution_receipt<T: Serialize>(prior: &str, entry: &T) -> Result<String, VerifyError> {
    sha256_json(&(CRAWL_EXECUTION_RECEIPT_FORMAT, prior, entry))
        .map_err(|_| VerifyError::new("crawler could not extend its execution receipt"))
}

fn validate_budget(budget: CrawlBudget) -> Result<(), VerifyError> {
    if budget.max_depth == 0
        || budget.max_expanded_states == 0
        || budget.max_discovered_frontiers == 0
        || budget.max_action_executions == 0
        || budget.catalog_page_size == 0
    {
        Err(VerifyError::new("crawler budgets must be positive"))
    } else {
        Ok(())
    }
}

fn pop_best_frontier(
    pending: &mut Vec<Frontier>,
    report: &CrawlReport,
    content: &CompiledContent,
) -> Result<Frontier, VerifyError> {
    let mut best_index = 0usize;
    let mut best_score = frontier_score(&pending[0], report, content)?;
    for (index, frontier) in pending.iter().enumerate().skip(1) {
        let score = frontier_score(frontier, report, content)?;
        if score > best_score {
            best_index = index;
            best_score = score;
        }
    }
    Ok(pending.swap_remove(best_index))
}

fn frontier_score(
    frontier: &Frontier,
    report: &CrawlReport,
    content: &CompiledContent,
) -> Result<FrontierScore, VerifyError> {
    let location = &frontier.state.world.current_location;
    let mut uncovered_here = 0usize;
    let mut nearest_ready_target = usize::MAX;
    let mut best_partial = (0usize, Reverse(usize::MAX));
    for (definition_id, definition) in content.actions() {
        if report.covered_definitions.contains(definition_id) {
            continue;
        }
        let target_distance = if definition.locations.is_empty() {
            0
        } else {
            let mut distance = usize::MAX;
            for target in &definition.locations {
                distance = distance.min(location_distance(content, location, target)?);
            }
            distance
        };
        let (satisfied, total) = condition_progress(&definition.condition, &frontier.state);
        let scaled_progress = satisfied.saturating_mul(1_024) / total.max(1);
        best_partial = best_partial.max((scaled_progress, Reverse(target_distance)));

        if definition.condition.evaluate(&frontier.state) {
            nearest_ready_target = nearest_ready_target.min(target_distance);
            if target_distance == 0 {
                uncovered_here += 1;
            }
        }
    }

    Ok((
        uncovered_here,
        Reverse(nearest_ready_target),
        best_partial.0,
        Reverse(frontier.depth.saturating_add((best_partial.1).0)),
        state_progress(&frontier.state),
        !report.reached_locations.contains(location),
        Reverse(frontier.ordinal),
    ))
}

fn condition_progress(condition: &Condition, state: &GameState) -> (usize, usize) {
    match condition {
        Condition::Always => (1, 1),
        Condition::Never => (0, 1),
        Condition::All { conditions } => conditions
            .iter()
            .map(|condition| condition_progress(condition, state))
            .fold((0usize, 0usize), |(left_met, left_total), (met, total)| {
                (left_met + met, left_total + total)
            }),
        Condition::Any { conditions } => conditions
            .iter()
            .map(|condition| condition_progress(condition, state))
            .max_by_key(|(met, total)| met.saturating_mul(1_024) / (*total).max(1))
            .unwrap_or((0, 1)),
        Condition::Not { .. } => (usize::from(condition.evaluate(state)), 1),
        _ => (usize::from(condition.evaluate(state)), 1),
    }
}

fn location_distance(
    content: &CompiledContent,
    start: &str,
    target: &str,
) -> Result<usize, VerifyError> {
    if start == target {
        return Ok(0);
    }
    let mut pending = VecDeque::from([(start.to_owned(), 0usize)]);
    let mut visited = BTreeSet::from([start.to_owned()]);
    while let Some((location_id, distance)) = pending.pop_front() {
        let location = content
            .location(&location_id)
            .ok_or_else(|| VerifyError::new("crawler encountered an unknown location"))?;
        for exit in &location.exits {
            if exit == target {
                return Ok(distance + 1);
            }
            if visited.insert(exit.clone()) {
                pending.push_back((exit.clone(), distance + 1));
            }
        }
    }
    Ok(usize::MAX)
}

fn state_progress(state: &GameState) -> usize {
    let location_flags = state
        .world
        .locations
        .values()
        .map(|location| location.flags.len())
        .sum::<usize>();
    let npc_facts = state
        .world
        .npcs
        .values()
        .map(|npc| npc.memories.len() + npc.knowledge.len())
        .sum::<usize>();
    state.world.flags.len()
        + location_flags
        + npc_facts
        + state.character.deeds.len()
        + state.character.knowledge.len()
        + state.character.inventory.len()
        + state.character.resources.len()
}

fn verify_catalog(
    state: &GameState,
    content: &CompiledContent,
    legal: &[CanonicalAction],
    page_size: usize,
) -> Result<(), VerifyError> {
    if legal
        .windows(2)
        .any(|pair| pair[0].action_id.as_str() >= pair[1].action_id.as_str())
    {
        return Err(VerifyError::new(
            "crawler kernel enumeration is not in canonical action-ID order",
        ));
    }
    let expected_digest = legal_action_digest(legal)
        .map_err(|_| VerifyError::new("crawler could not hash the legal catalog"))?;
    let expected_ids: Vec<_> = legal
        .iter()
        .map(|action| action.action_id.clone())
        .collect();
    let expected_state_id = state.state_id();
    let mut actual_ids = Vec::new();
    actual_ids
        .try_reserve(legal.len())
        .map_err(|_| VerifyError::new("crawler catalog allocation failed"))?;
    let mut offset = 0usize;
    loop {
        let page = content
            .action_page(state, offset, page_size)
            .map_err(|_| VerifyError::new("crawler could not page the legal catalog"))?;
        if page.build_id != content.build_id()
            || page.state_id != expected_state_id
            || page.digest != expected_digest
            || page.total != legal.len()
            || page.offset != offset
        {
            return Err(VerifyError::new(
                "crawler found inconsistent catalog metadata",
            ));
        }
        for (index, view) in page.actions.iter().enumerate() {
            let expected = legal.get(offset + index).ok_or_else(|| {
                VerifyError::new("crawler catalog page exceeded kernel enumeration")
            })?;
            let definition = content.action(&expected.definition_id).ok_or_else(|| {
                VerifyError::new("crawler catalog referenced an unknown definition")
            })?;
            if view.action_id != expected.action_id
                || view.definition_id != expected.definition_id
                || view.parameters != expected.parameters
                || view.label != definition.label
                || view.category != definition.category
            {
                return Err(VerifyError::new(
                    "crawler catalog view does not match kernel enumeration",
                ));
            }
        }
        actual_ids.extend(page.actions.into_iter().map(|action| action.action_id));
        match page.next_offset {
            Some(next) if next > offset => offset = next,
            Some(_) => return Err(VerifyError::new("crawler catalog cursor did not advance")),
            None => break,
        }
    }
    if actual_ids == expected_ids {
        Ok(())
    } else {
        Err(VerifyError::new(
            "crawler paged catalog does not equal kernel enumeration",
        ))
    }
}

fn normalized_state_id(state: &GameState) -> Result<String, VerifyError> {
    let mut normalized = state.clone();
    let current_time = normalized.world.time;
    normalized.world.time = 0;
    normalized.event_log.clear();
    for event in &mut normalized.world.scheduled_events {
        event.due_time = event.due_time.saturating_sub(current_time);
    }
    for npc in normalized.world.npcs.values_mut() {
        for memory in npc.memories.values_mut() {
            memory.turn = 0;
        }
        for knowledge in npc.knowledge.values_mut() {
            knowledge.turn = 0;
        }
    }
    sha256_json(&NormalizedState(&normalized))
        .map_err(|_| VerifyError::new("crawler could not hash a normalized state"))
}

#[derive(Serialize)]
struct NormalizedState<'a>(&'a GameState);

fn accept_frontier(
    dominance: &mut BTreeMap<String, Vec<BTreeSet<ActionShape>>>,
    key: String,
    used_actions: &BTreeSet<ActionShape>,
) -> Result<bool, VerifyError> {
    let known = dominance.entry(key).or_default();
    if known
        .iter()
        .any(|existing| existing.is_subset(used_actions))
    {
        return Ok(false);
    }
    known.retain(|existing| !used_actions.is_subset(existing));
    known
        .try_reserve(1)
        .map_err(|_| VerifyError::new("crawler dominance allocation failed"))?;
    known.push(used_actions.clone());
    Ok(true)
}

fn join_ids(ids: &BTreeSet<String>) -> String {
    ids.iter().cloned().collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_content::parse_and_compile_production;

    const SPLIT_TIDE: &str = include_str!("../../../content/split-tide.json");

    #[test]
    fn bounded_crawl_executes_every_split_tide_definition() {
        let content = parse_and_compile_production(SPLIT_TIDE).unwrap();
        let report = crawl_production(&content, CrawlBudget::default()).unwrap();
        assert!(report.is_complete());
        assert_eq!(report.advertised_definitions.len(), 47);
        assert_eq!(report.reached_locations.len(), 6);
        assert!(report.successful_actions >= report.advertised_definitions.len());
        assert!(report.expanded_states <= 64);
        assert!(report.discovered_frontiers <= 512);
        assert_eq!(report.execution_receipt.len(), 64);

        let repeated = crawl_production(&content, CrawlBudget::default()).unwrap();
        assert_eq!(report.execution_receipt, repeated.execution_receipt);
    }

    #[test]
    fn insufficient_budget_fails_without_overclaiming_coverage() {
        let content = parse_and_compile_production(SPLIT_TIDE).unwrap();
        let error = crawl_production(
            &content,
            CrawlBudget {
                max_depth: 1,
                max_expanded_states: 2,
                max_discovered_frontiers: 32,
                max_action_executions: 32,
                catalog_page_size: 3,
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("uncovered definitions"));
    }

    #[test]
    fn frontier_and_action_budgets_are_enforced_independently() {
        let content = parse_and_compile_production(SPLIT_TIDE).unwrap();
        let frontier_error = crawl_production(
            &content,
            CrawlBudget {
                max_discovered_frontiers: 1,
                ..CrawlBudget::default()
            },
        )
        .unwrap_err();
        assert!(frontier_error.to_string().contains("1-frontier budget"));

        let action_error = crawl_production(
            &content,
            CrawlBudget {
                max_action_executions: 1,
                ..CrawlBudget::default()
            },
        )
        .unwrap_err();
        assert!(action_error.to_string().contains("1-action budget"));
        assert!(action_error.to_string().contains("uncovered definitions"));
    }
}
