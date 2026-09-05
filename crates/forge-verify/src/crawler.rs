use forge_kernel::{
    ActionDefinition, CanonicalAction, CompiledContent, Condition, Effect, EventKind, GameState,
    LocationDefinition, ParameterDomain, StringRef, enumerate_legal_actions, legal_action_digest,
    sha256_json, step,
};
use forge_replay::{Session, Trace, TraceStart};
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
            max_depth: 13,
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
    /// Explicit coverage targets. Every complete catalog is still executed;
    /// an optional-area crawl targets fewer definitions, never fewer actions
    /// from an expanded state's legal set.
    pub required_definitions: BTreeSet<String>,
    /// Authored sessions from which frontiers begin. Prefix actions establish
    /// lineage and consume depth; only expanded catalogs count as coverage.
    pub starting_sessions: Vec<CrawlStartingSession>,
    /// Opaque hash chain over ordered starts, expansions, legal catalogs, and
    /// transitions. This makes the checked report sensitive to traversal or
    /// catalog-order drift even when its aggregate coverage totals match.
    pub execution_receipt: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct CrawlStartingSession {
    pub label: String,
    pub start: TraceStart,
    pub depth: usize,
    pub final_receipt: String,
    pub state_id: String,
}

struct CrawlSeed {
    provenance: CrawlStartingSession,
    state: GameState,
    used_actions: BTreeSet<ActionShape>,
}

impl CrawlSeed {
    fn from_trace(
        label: String,
        trace: &Trace,
        content: &CompiledContent,
    ) -> Result<Self, VerifyError> {
        let state = forge_replay::verify(trace, content).map_err(crate::replay_error)?;
        content
            .validate_state(&state)
            .map_err(|error| VerifyError::new(format!("invalid crawl seed: {error}")))?;
        let mut used_actions = BTreeSet::new();
        let mut location = trace.initial_state.world.current_location.clone();
        for step in &trace.steps {
            used_actions.insert(ActionShape {
                location: location.clone(),
                definition_id: step.action.definition_id.clone(),
                parameters: step.action.parameters.clone(),
            });
            // Replay has checked these exact ordered events. This only
            // reconstructs the crawler's prior-action bookkeeping.
            for event in &step.events {
                if let EventKind::Moved { to, .. } = &event.kind {
                    location.clone_from(to);
                }
            }
        }
        if location != state.world.current_location {
            return Err(VerifyError::new(
                "crawl seed movement history differs from replay",
            ));
        }
        Ok(Self {
            provenance: CrawlStartingSession {
                label,
                start: trace.start.clone(),
                depth: trace.steps.len(),
                final_receipt: trace.final_receipt.clone(),
                state_id: state.state_id(),
            },
            state,
            used_actions,
        })
    }
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
        self.required_definitions
            .difference(&self.covered_definitions)
            .cloned()
            .collect()
    }

    pub fn is_complete(&self) -> bool {
        self.required_definitions
            .is_subset(&self.covered_definitions)
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
    usize,
    usize,
    Reverse<usize>,
    usize,
    Reverse<u64>,
    usize,
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
    let required = content.actions().map(|(id, _)| id.clone()).collect();
    crawl_targets(content, budget, required)
}

pub(super) fn crawl_targets(
    content: &CompiledContent,
    budget: CrawlBudget,
    required_definitions: BTreeSet<String>,
) -> Result<CrawlReport, VerifyError> {
    crawl_targets_with_scenarios(content, budget, required_definitions, &[])
}

pub(super) fn crawl_targets_with_scenarios(
    content: &CompiledContent,
    budget: CrawlBudget,
    required_definitions: BTreeSet<String>,
    scenario_seeds: &[&str],
) -> Result<CrawlReport, VerifyError> {
    validate_budget(budget)?;
    let advertised_definitions: BTreeSet<_> = content.actions().map(|(id, _)| id.clone()).collect();
    if required_definitions.is_empty() || !required_definitions.is_subset(&advertised_definitions) {
        return Err(VerifyError::new(
            "crawl targets must name existing action definitions",
        ));
    }
    let execution_receipt = sha256_json(&(
        CRAWL_EXECUTION_RECEIPT_FORMAT,
        "genesis",
        content.build_id(),
        budget,
        &required_definitions,
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
        required_definitions,
        starting_sessions: Vec::new(),
        execution_receipt,
    };
    let mut pending = Vec::new();
    let mut dominance: BTreeMap<String, Vec<BTreeSet<ActionShape>>> = BTreeMap::new();

    let mut seeds = Vec::new();
    let seed_count = content
        .character_presets()
        .size_hint()
        .0
        .checked_add(scenario_seeds.len())
        .ok_or_else(|| VerifyError::new("crawler starting-session count overflowed"))?;
    seeds
        .try_reserve(seed_count)
        .map_err(|_| VerifyError::new("crawler starting-session allocation failed"))?;
    report
        .starting_sessions
        .try_reserve(seed_count)
        .map_err(|_| VerifyError::new("crawler starting-session report allocation failed"))?;
    for (preset_id, _) in content.character_presets() {
        let session = Session::new_game(preset_id, 71, content).map_err(crate::replay_error)?;
        seeds.push(CrawlSeed::from_trace(
            format!("preset:{preset_id}"),
            session.trace(),
            content,
        )?);
    }
    let mut scenario_ids = BTreeSet::new();
    for scenario_id in scenario_seeds {
        if !scenario_ids.insert(*scenario_id) {
            return Err(VerifyError::new("crawler cannot repeat a scenario seed"));
        }
        let spec = crate::scenarios::get(scenario_id)?;
        let session = crate::scenarios::run(spec, content)?;
        seeds.push(CrawlSeed::from_trace(
            format!("scenario:{scenario_id}"),
            session.trace(),
            content,
        )?);
    }
    for seed in seeds {
        let CrawlSeed {
            provenance,
            state,
            used_actions,
        } = seed;
        if provenance.depth > budget.max_depth {
            return Err(VerifyError::new(
                "crawl seed prefix exceeds the depth budget",
            ));
        }
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
                &("start", &provenance, ordinal),
            )?;
            pending.push(Frontier {
                state,
                depth: provenance.depth,
                ordinal,
                used_actions,
            });
            report.starting_sessions.push(provenance);
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

        let initial_observation = content
            .observe(&frontier.state)
            .map_err(|_| VerifyError::new("crawler could not observe a valid frontier state"))?;
        verify_supply_projection(&frontier.state, content, &initial_observation)?;

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
            let observation = content
                .observe_after_transition(&transition)
                .map_err(|_| VerifyError::new("crawler could not observe a valid transition"))?;
            verify_supply_projection(transition.state(), content, &observation)?;
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
            let definition = content.action(&action.definition_id).ok_or_else(|| {
                VerifyError::new("crawler transition referenced an unknown action definition")
            })?;
            if !definition.movement
                && definition.category != "Time"
                && definition.parameters.is_empty()
                && definition.condition.evaluate(&next_state)
                && enumerate_legal_actions(&next_state, content)
                    .map_err(|_| VerifyError::new("crawler could not check action retirement"))?
                    .iter()
                    .any(|candidate| candidate.definition_id == definition.id)
            {
                return Err(VerifyError::new(format!(
                    "resolved non-time action {} remains legal after use",
                    definition.id
                )));
            }
            report.successful_actions = report
                .successful_actions
                .checked_add(1)
                .ok_or_else(|| VerifyError::new("crawler successful-action count overflowed"))?;
            report
                .covered_definitions
                .insert(action.definition_id.clone());

            let advances_deferred_timer = definition.category == "Time"
                && next_state.world.time > frontier.state.world.time
                && frontier
                    .state
                    .world
                    .scheduled_events
                    .iter()
                    .any(|event| content.deferred_event(&event.id).is_some());
            if frontier.depth >= budget.max_depth
                || (frontier.used_actions.contains(&shape) && !advances_deferred_timer)
            {
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
    // Prefer a ready uncovered definition when a movement that is legal in
    // this state can reach its location. Static graph distance still guides
    // longer routes, but cannot let a currently gated shortcut monopolize the
    // frontier queue.
    let immediate_targets = immediate_movement_targets(&frontier.state, content)?;
    let npc_arrivals = immediate_npc_arrival_projections(&frontier.state, content)?;
    let deferred_flags = pending_deferred_flag_projections(&frontier.state, content)?;
    let mut uncovered_here = 0usize;
    let mut ready_one_move = 0usize;
    let mut ready_uncovered = 0usize;
    let mut nearest_ready_target = usize::MAX;
    let mut deferred_ready = 0usize;
    let mut nearest_deferred_ready = u64::MAX;
    let mut total_progress = 0usize;
    let mut best_partial = (0usize, Reverse(usize::MAX));
    for (definition_id, definition) in content.actions() {
        if !report.required_definitions.contains(definition_id)
            || report.covered_definitions.contains(definition_id)
        {
            continue;
        }
        let ready = definition.condition.evaluate(&frontier.state);
        if !ready {
            for (remaining, projection) in &deferred_flags {
                if definition.condition.evaluate(projection) {
                    deferred_ready += 1;
                    nearest_deferred_ready = nearest_deferred_ready.min(*remaining);
                    break;
                }
            }
        }
        let ready_on_arrival = npc_arrivals.iter().any(|arrival| {
            (definition.locations.is_empty()
                || definition
                    .locations
                    .contains(&arrival.world.current_location))
                && definition.condition.evaluate(arrival)
        });
        let target_distance = if definition.locations.is_empty() {
            0
        } else {
            let mut distance = usize::MAX;
            for target in &definition.locations {
                distance = distance.min(location_distance(content, location, target, None)?);
            }
            distance
        };
        let (satisfied, total) = condition_progress(&definition.condition, &frontier.state);
        let scaled_progress = satisfied.saturating_mul(1_024) / total.max(1);
        total_progress = total_progress.saturating_add(scaled_progress);
        best_partial = best_partial.max((scaled_progress, Reverse(target_distance)));

        if ready || ready_on_arrival {
            let mut available_distance = if ready && definition.locations.is_empty() {
                0
            } else {
                usize::MAX
            };
            if ready {
                for target in &definition.locations {
                    available_distance = available_distance.min(location_distance(
                        content,
                        location,
                        target,
                        Some(&frontier.state),
                    )?);
                }
            }
            if ready_on_arrival {
                available_distance = available_distance.min(1);
            }
            if available_distance == usize::MAX {
                continue;
            }
            ready_uncovered = ready_uncovered.saturating_add(1);
            nearest_ready_target = nearest_ready_target.min(available_distance);
            if available_distance == 0 {
                uncovered_here += 1;
            } else if ready_on_arrival
                || definition
                    .locations
                    .iter()
                    .any(|target| immediate_targets.contains(target))
            {
                ready_one_move += 1;
            }
        }
    }

    Ok((
        uncovered_here,
        ready_one_move,
        ready_uncovered,
        Reverse(nearest_ready_target),
        deferred_ready,
        Reverse(nearest_deferred_ready),
        total_progress,
        best_partial.0,
        Reverse(frontier.depth.saturating_add((best_partial.1).0)),
        state_progress(&frontier.state),
        !report.reached_locations.contains(location),
        Reverse(frontier.ordinal),
    ))
}

/// Prioritize the flag consequences of currently guarded deferred events.
/// These are search hints only: inventory, elapsed time, and other effects
/// remain unmodeled. A projected state is never admitted, executed, hashed,
/// or counted as coverage; actual waiting still uses complete kernel catalogs.
fn pending_deferred_flag_projections(
    state: &GameState,
    content: &CompiledContent,
) -> Result<Vec<(u64, GameState)>, VerifyError> {
    let mut projections = Vec::new();
    projections
        .try_reserve(state.world.scheduled_events.len())
        .map_err(|_| VerifyError::new("crawler deferred planning allocation failed"))?;
    for pending in &state.world.scheduled_events {
        let project = || {
            let event = content.deferred_event(&pending.id)?;
            if !event.condition.evaluate(state) {
                return None;
            }
            let mut projected = state.clone();
            let mut changes_flag = false;
            for effect in &event.effects {
                let (flags, flag, value) = match effect {
                    Effect::SetFlag { flag, value } | Effect::SetWorldFlag { flag, value } => {
                        (&mut projected.world.flags, flag, value)
                    }
                    Effect::SetLocationFlag {
                        location: StringRef::Literal(location),
                        flag,
                        value,
                    } => (
                        &mut projected.world.locations.get_mut(location)?.flags,
                        flag,
                        value,
                    ),
                    Effect::RandomChance { .. } => return None,
                    _ => continue,
                };
                changes_flag = true;
                if *value {
                    flags.insert(flag.clone());
                } else {
                    flags.remove(flag);
                }
            }
            changes_flag.then_some((pending.due_time.saturating_sub(state.world.time), projected))
        };
        if let Some(projection) = project() {
            projections.push(projection);
        }
    }
    Ok(projections)
}

/// Rank conversations opened by NPC relocation during an available travel
/// program. These are condition-only projections, never admitted states or
/// counted executions: indexes, history, and deadlines are not simulated.
/// Only movement/time programs with one direct player move and literal NPC
/// moves are projected; other programs are left to real play.
fn immediate_npc_arrival_projections(
    state: &GameState,
    content: &CompiledContent,
) -> Result<Vec<GameState>, VerifyError> {
    let location = content
        .location(&state.world.current_location)
        .ok_or_else(|| VerifyError::new("crawler encountered an unknown location"))?;
    let mut arrivals = Vec::new();
    for (_, action) in content.actions() {
        if !action.movement
            || (!action.locations.is_empty() && !action.locations.contains(&location.id))
            || !action.condition.evaluate(state)
            || action
                .effects
                .iter()
                .filter(|effect| matches!(effect, Effect::MoveCharacter { .. }))
                .count()
                != 1
        {
            continue;
        }
        let mut npc_destinations = BTreeMap::new();
        let mut supported = true;
        for effect in &action.effects {
            match effect {
                Effect::MoveNpc {
                    npc: StringRef::Literal(npc),
                    location: StringRef::Literal(destination),
                } => {
                    npc_destinations.insert(npc, destination);
                }
                Effect::Noop | Effect::MoveCharacter { .. } | Effect::AdvanceTime { .. } => {}
                _ => {
                    supported = false;
                    break;
                }
            }
        }
        if !supported || npc_destinations.is_empty() {
            continue;
        }
        let mut targets = BTreeSet::new();
        visit_movement_targets(action, location, &mut |destination| {
            targets.insert(destination.to_owned());
        });
        arrivals
            .try_reserve(targets.len())
            .map_err(|_| VerifyError::new("crawler arrival planning allocation failed"))?;
        for destination in targets {
            let mut arrival = state.clone();
            arrival.world.current_location = destination;
            for (npc, destination) in &npc_destinations {
                let npc_state = arrival.world.npcs.get_mut(*npc).ok_or_else(|| {
                    VerifyError::new("crawler arrival planning encountered an unknown NPC")
                })?;
                npc_state.location.clone_from(destination);
            }
            arrivals.push(arrival);
        }
    }
    Ok(arrivals)
}

fn immediate_movement_targets(
    state: &GameState,
    content: &CompiledContent,
) -> Result<BTreeSet<String>, VerifyError> {
    let location = content
        .location(&state.world.current_location)
        .ok_or_else(|| VerifyError::new("crawler encountered an unknown location"))?;
    let mut targets = BTreeSet::new();
    for (_, action) in content.actions() {
        if !action.movement
            || (!action.locations.is_empty()
                && !action
                    .locations
                    .iter()
                    .any(|candidate| candidate == &location.id))
            || !action.condition.evaluate(state)
        {
            continue;
        }
        visit_movement_targets(action, location, &mut |destination| {
            if content.has_location(destination) {
                targets.insert(destination.to_owned());
            }
        });
    }
    Ok(targets)
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
    available_in: Option<&GameState>,
) -> Result<usize, VerifyError> {
    if start == target {
        return Ok(0);
    }
    let mut pending = VecDeque::from([(start.to_owned(), 0usize)]);
    let mut visited = BTreeSet::from([start.to_owned()]);
    let mut projected = available_in.cloned();
    while let Some((location_id, distance)) = pending.pop_front() {
        let location = content
            .location(&location_id)
            .ok_or_else(|| VerifyError::new("crawler encountered an unknown location"))?;
        if let Some(state) = &mut projected {
            state.world.current_location.clone_from(&location_id);
        }
        for exit in location.exits.iter().filter(|_| available_in.is_none()) {
            if exit == target {
                return Ok(distance + 1);
            }
            if visited.insert(exit.clone()) {
                pending.push_back((exit.clone(), distance + 1));
            }
        }
        for (_, action) in content.actions() {
            if !action.movement {
                continue;
            }
            if !action.locations.is_empty()
                && !action
                    .locations
                    .iter()
                    .any(|candidate| candidate == &location_id)
            {
                continue;
            }
            // This is only a search heuristic, never an admitted game state.
            // Keep current world prerequisites while testing each graph node's
            // location-dependent movement gates. Actual paths still execute
            // exclusively through kernel-enumerated canonical actions.
            if projected
                .as_ref()
                .is_some_and(|state| !action.condition.evaluate(state))
            {
                continue;
            }
            let mut found_target = false;
            visit_movement_targets(action, location, &mut |destination| {
                if destination == target {
                    found_target = true;
                } else if content.has_location(destination)
                    && visited.insert(destination.to_owned())
                {
                    pending.push_back((destination.to_owned(), distance + 1));
                }
            });
            if found_target {
                return Ok(distance + 1);
            }
        }
    }
    Ok(usize::MAX)
}

fn visit_movement_targets(
    action: &ActionDefinition,
    location: &LocationDefinition,
    visitor: &mut impl FnMut(&str),
) {
    fn visit_effect(
        effect: &Effect,
        action: &ActionDefinition,
        location: &LocationDefinition,
        visitor: &mut impl FnMut(&str),
    ) {
        match effect {
            Effect::MoveCharacter {
                location: StringRef::Literal(destination),
            } => visitor(destination),
            Effect::MoveCharacter {
                location: StringRef::Parameter(name),
            } => {
                let Some(parameter) = action
                    .parameters
                    .iter()
                    .find(|parameter| parameter.name == *name)
                else {
                    return;
                };
                match &parameter.domain {
                    ParameterDomain::Values(values) => {
                        for destination in values {
                            visitor(destination);
                        }
                    }
                    ParameterDomain::LocationsAdjacent => {
                        for destination in &location.exits {
                            visitor(destination);
                        }
                    }
                    ParameterDomain::InventoryItems | ParameterDomain::NpcsAtCurrentLocation => {}
                }
            }
            // NPC relocation changes world presence, not the player's graph.
            Effect::MoveNpc { .. } => {}
            Effect::RandomChance {
                on_success,
                on_failure,
                ..
            } => {
                visit_effect(on_success, action, location, visitor);
                visit_effect(on_failure, action, location, visitor);
            }
            _ => {}
        }
    }

    for effect in &action.effects {
        visit_effect(effect, action, location, visitor);
    }
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
        return Err(VerifyError::new(
            "crawler kernel enumeration is not in canonical semantic order",
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
            let (minimum_ticks, maximum_ticks) = independent_action_time_cost(definition)?;
            let consequence_preview = matches!(definition.category.as_str(), "Outcome" | "Ending")
                .then_some(definition.result.as_str());
            if view.action_id != expected.action_id
                || view.definition_id != expected.definition_id
                || view.parameters != expected.parameters
                || view.label != definition.label
                || view.category != definition.category
                || view.time_cost.minimum_ticks != minimum_ticks
                || view.time_cost.maximum_ticks != maximum_ticks
                || view.consequence_preview.as_deref() != consequence_preview
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

/// Independently reconstruct the public supply projection from the
/// authoritative player's maps. This deliberately does not call
/// `CompiledContent::supply_view`, so a shared projection bug is detectable.
fn verify_supply_projection(
    state: &GameState,
    content: &CompiledContent,
    observation: &forge_kernel::Observation,
) -> Result<(), VerifyError> {
    let expected_resources: Vec<_> = state
        .character
        .resources
        .iter()
        .map(|(id, amount)| {
            (
                id.clone(),
                content
                    .supply_labels()
                    .resources
                    .get(id)
                    .map(String::as_str)
                    .unwrap_or(id)
                    .to_owned(),
                *amount,
            )
        })
        .collect();
    let actual_resources: Vec<_> = observation
        .supplies
        .resources
        .iter()
        .map(|resource| (resource.id.clone(), resource.name.clone(), resource.amount))
        .collect();
    if actual_resources != expected_resources {
        return Err(VerifyError::new(
            "crawler supply view does not exactly match player resources",
        ));
    }

    let expected_items: Vec<_> = state
        .character
        .inventory
        .iter()
        .map(|(id, count)| {
            (
                id.clone(),
                content
                    .supply_labels()
                    .items
                    .get(id)
                    .map(String::as_str)
                    .unwrap_or(id)
                    .to_owned(),
                *count,
            )
        })
        .collect();
    let actual_items: Vec<_> = observation
        .supplies
        .items
        .iter()
        .map(|item| (item.id.clone(), item.name.clone(), item.count))
        .collect();
    if actual_items != expected_items {
        return Err(VerifyError::new(
            "crawler supply view does not exactly match player inventory",
        ));
    }
    Ok(())
}

fn independent_action_time_cost(definition: &ActionDefinition) -> Result<(u64, u64), VerifyError> {
    definition
        .effects
        .iter()
        .try_fold((0_u64, 0_u64), |(total_minimum, total_maximum), effect| {
            let (minimum, maximum) = independent_effect_time_cost(effect)?;
            Ok((
                total_minimum.checked_add(minimum).ok_or_else(|| {
                    VerifyError::new("crawler action minimum time cost overflowed")
                })?,
                total_maximum.checked_add(maximum).ok_or_else(|| {
                    VerifyError::new("crawler action maximum time cost overflowed")
                })?,
            ))
        })
}

fn independent_effect_time_cost(effect: &Effect) -> Result<(u64, u64), VerifyError> {
    match effect {
        Effect::AdvanceTime { ticks } => Ok((*ticks, *ticks)),
        Effect::RandomChance {
            success_percent,
            on_success,
            on_failure,
        } => {
            let success = independent_effect_time_cost(on_success)?;
            let failure = independent_effect_time_cost(on_failure)?;
            match success_percent {
                0 => Ok(failure),
                100 => Ok(success),
                _ => Ok((success.0.min(failure.0), success.1.max(failure.1))),
            }
        }
        Effect::Noop
        | Effect::SetFlag { .. }
        | Effect::SetWorldFlag { .. }
        | Effect::SetLocationFlag { .. }
        | Effect::AdjustResource { .. }
        | Effect::MoveCharacter { .. }
        | Effect::MoveNpc { .. }
        | Effect::AdjustNpcRelationship { .. }
        | Effect::TransferNpcItemToCharacter { .. }
        | Effect::ApplyRecipe { .. }
        | Effect::ScheduleEvent { .. }
        | Effect::AddNpcMemory { .. }
        | Effect::TeachNpc { .. }
        | Effect::AddCharacterDeed { .. } => Ok((0, 0)),
    }
}

fn normalized_state_id(state: &GameState) -> Result<String, VerifyError> {
    // A resolved template remains unavailable forever. Its schedule ledger
    // affects future legality even after its queue entry and visible effects
    // disappear; preserve those IDs while discarding redundant event detail.
    let scheduled_templates: BTreeSet<_> = state
        .event_log
        .iter()
        .filter_map(|event| {
            if let EventKind::EventScheduled { event_id, .. } = &event.kind {
                Some(event_id.as_str())
            } else {
                None
            }
        })
        .collect();
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
    sha256_json(&NormalizedState(&normalized, scheduled_templates))
        .map_err(|_| VerifyError::new("crawler could not hash a normalized state"))
}

#[derive(Serialize)]
struct NormalizedState<'a>(&'a GameState, BTreeSet<&'a str>);

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
    use forge_content::{compile_production, parse, parse_and_compile_production};

    const SPLIT_TIDE: &str = include_str!("../../../content/split-tide.json");

    #[test]
    fn deferred_hints_preserve_real_state_and_repeated_waits_use_canonical_actions() {
        let mut draft = parse(SPLIT_TIDE).unwrap();
        draft.contract = forge_kernel::ContentContract::Fixture;
        draft.character_presets.truncate(1);
        draft.timed_events.clear();
        draft.recipes.clear();
        for location in &mut draft.locations {
            location.terminal = true;
        }
        draft.deferred_events = vec![forge_kernel::DeferredEventDefinition {
            id: "test.delayed".to_owned(),
            delay: 4,
            event_kind: "test".to_owned(),
            label: "Ready".to_owned(),
            result: "The signal arrives.".to_owned(),
            condition: Condition::Always,
            effects: vec![
                Effect::SetWorldFlag {
                    flag: "test.ready".to_owned(),
                    value: true,
                },
                Effect::TeachNpc {
                    npc: StringRef::Literal("sava_rusk".to_owned()),
                    knowledge_id: "test.signal".to_owned(),
                    subject: "Sava saw the signal.".to_owned(),
                    provenance: forge_kernel::KnowledgeProvenance::Witnessed,
                },
            ],
        }];
        let action = |id: &str, category: &str, condition, effects| ActionDefinition {
            id: id.to_owned(),
            label: "Act".to_owned(),
            category: category.to_owned(),
            result: "Done.".to_owned(),
            result_variants: Vec::new(),
            locations: Vec::new(),
            condition,
            effects,
            parameters: Vec::new(),
            meaningful: true,
            movement: false,
        };
        draft.actions = vec![
            action(
                "test.schedule",
                "Act",
                Condition::Always,
                vec![
                    Effect::ScheduleEvent {
                        event: "test.delayed".to_owned(),
                    },
                    Effect::AdvanceTime { ticks: 1 },
                ],
            ),
            action(
                "test.wait",
                "Time",
                Condition::Always,
                vec![Effect::AdvanceTime { ticks: 1 }],
            ),
            action(
                "test.inspect",
                "Act",
                Condition::All {
                    conditions: vec![
                        Condition::WorldFlag {
                            flag: "test.ready".to_owned(),
                        },
                        Condition::Not {
                            condition: Box::new(Condition::WorldFlag {
                                flag: "test.inspected".to_owned(),
                            }),
                        },
                    ],
                },
                vec![Effect::SetWorldFlag {
                    flag: "test.inspected".to_owned(),
                    value: true,
                }],
            ),
        ];
        let content = forge_content::compile(draft).unwrap();
        let mut session = Session::new_game("ilyan", 71, &content).unwrap();
        let record = |session: &mut Session<'_>, id: &str| {
            let action = enumerate_legal_actions(session.state(), &content)
                .unwrap()
                .into_iter()
                .find(|action| action.definition_id == id)
                .unwrap();
            session.record(&action).unwrap();
        };
        record(&mut session, "test.schedule");
        let before = session.state().clone();
        let hints = pending_deferred_flag_projections(session.state(), &content).unwrap();
        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].0, 3);
        assert!(hints[0].1.world.flags.contains("test.ready"));
        assert_eq!(
            hints[0].1.world.npcs, before.world.npcs,
            "hints cannot invent knowledge"
        );
        assert_eq!(hints[0].1.event_log, before.event_log);
        assert_eq!(
            *session.state(),
            before,
            "hints cannot alter the authoritative state"
        );
        for _ in 0..3 {
            assert!(!session.state().world.flags.contains("test.ready"));
            assert!(
                !session.state().world.npcs["sava_rusk"]
                    .knowledge
                    .contains_key("test.signal")
            );
            assert!(
                !enumerate_legal_actions(session.state(), &content)
                    .unwrap()
                    .iter()
                    .any(|action| action.definition_id == "test.inspect")
            );
            record(&mut session, "test.wait");
        }
        assert!(session.state().world.flags.contains("test.ready"));
        assert!(
            session.state().world.npcs["sava_rusk"]
                .knowledge
                .contains_key("test.signal")
        );
        record(&mut session, "test.inspect");
        assert_eq!(
            forge_replay::verify(session.trace(), &content).unwrap(),
            *session.state()
        );
        let report = crawl_production(
            &content,
            CrawlBudget {
                max_depth: 6,
                max_expanded_states: 8,
                max_discovered_frontiers: 16,
                max_action_executions: 32,
                catalog_page_size: 1,
            },
        )
        .unwrap();
        assert_eq!(report.covered_definitions, report.advertised_definitions);
        assert_eq!(report.covered_definitions.len(), 3);
        assert_eq!(
            report.expanded_states, 5,
            "the crawler must expand each real waiting state"
        );
        assert_eq!(
            report.successful_actions, 7,
            "every legal action in all five catalogs executes"
        );
    }

    #[test]
    fn normalization_preserves_resolved_one_shot_history_when_visible_state_matches() {
        let mut draft = parse(SPLIT_TIDE).unwrap();
        draft.contract = forge_kernel::ContentContract::Fixture;
        draft.timed_events.clear();
        draft.recipes.clear();
        for location in &mut draft.locations {
            location.terminal = true;
        }
        let flag = Effect::SetWorldFlag {
            flag: "test.ready".to_owned(),
            value: true,
        };
        draft.deferred_events = vec![forge_kernel::DeferredEventDefinition {
            id: "test.once".to_owned(),
            delay: 1,
            event_kind: "test".to_owned(),
            label: "Ready".to_owned(),
            result: "Still ready.".to_owned(),
            condition: Condition::Always,
            effects: vec![flag.clone()],
        }];
        let action = |id: &str, effects| ActionDefinition {
            id: id.to_owned(),
            label: "Act".to_owned(),
            category: "Time".to_owned(),
            result: "Done.".to_owned(),
            result_variants: Vec::new(),
            locations: Vec::new(),
            condition: Condition::Always,
            effects,
            parameters: Vec::new(),
            meaningful: true,
            movement: false,
        };
        draft.actions = vec![
            action("test.prepare", vec![flag]),
            action("test.wait", vec![Effect::AdvanceTime { ticks: 1 }]),
            action(
                "test.schedule",
                vec![
                    Effect::ScheduleEvent {
                        event: "test.once".to_owned(),
                    },
                    Effect::AdvanceTime { ticks: 1 },
                ],
            ),
        ];
        let content = forge_content::compile(draft).unwrap();
        let apply = |state: &GameState, id: &str| {
            let action = enumerate_legal_actions(state, &content)
                .unwrap()
                .into_iter()
                .find(|action| action.definition_id == id)
                .unwrap();
            step(state, &action, &content, &state.entropy)
                .unwrap()
                .into_state()
        };
        let initial = content.new_game("ilyan", 71).unwrap();
        let prepared = apply(&initial, "test.prepare");
        let unused = apply(&prepared, "test.wait");
        let resolved = apply(&prepared, "test.schedule");
        content.validate_state(&unused).unwrap();
        content.validate_state(&resolved).unwrap();
        assert!(resolved.world.scheduled_events.is_empty());
        assert_eq!(unused.world, resolved.world);
        assert_eq!(unused.character, resolved.character);
        assert_eq!(unused.entropy, resolved.entropy);
        let can_schedule = |state: &GameState| {
            enumerate_legal_actions(state, &content)
                .unwrap()
                .iter()
                .any(|action| action.definition_id == "test.schedule")
        };
        assert!(can_schedule(&unused));
        assert!(!can_schedule(&resolved));
        assert_ne!(
            normalized_state_id(&unused).unwrap(),
            normalized_state_id(&resolved).unwrap()
        );
    }

    #[test]
    fn scenario_seed_replays_exact_prefix_and_preserves_depth_and_action_history() {
        let content = parse_and_compile_production(SPLIT_TIDE).unwrap();
        let spec = crate::scenarios::get("m1-outcome-hold-market").unwrap();
        let session = crate::scenarios::run(spec, &content).unwrap();
        let seed = CrawlSeed::from_trace(
            "scenario:m1-outcome-hold-market".to_owned(),
            session.trace(),
            &content,
        )
        .unwrap();
        assert_eq!(seed.provenance.depth, 7);
        assert_eq!(seed.provenance.final_receipt, session.trace().final_receipt);
        assert_eq!(seed.provenance.state_id, session.state().state_id());
        assert_eq!(seed.state, *session.state());
        assert_eq!(seed.used_actions.len(), 7);
        assert!(seed.used_actions.contains(&ActionShape {
            location: "red_sluice.top".to_owned(),
            definition_id: "top.hold_market".to_owned(),
            parameters: BTreeMap::new(),
        }));
        assert!(seed.used_actions.contains(&ActionShape {
            location: "lowsail.return".to_owned(),
            definition_id: "return.count_dry_stalls".to_owned(),
            parameters: BTreeMap::new(),
        }));

        let mut forged_receipt = session.trace().clone();
        forged_receipt.final_receipt.push('0');
        assert!(CrawlSeed::from_trace("forged".to_owned(), &forged_receipt, &content).is_err());
        let mut reordered = session.trace().clone();
        reordered.steps.swap(0, 1);
        assert!(CrawlSeed::from_trace("reordered".to_owned(), &reordered, &content).is_err());
        let mut fabricated_genesis = session.trace().clone();
        fabricated_genesis
            .initial_state
            .world
            .flags
            .insert("sluice_outcome_chosen".to_owned());
        assert!(
            CrawlSeed::from_trace("fabricated".to_owned(), &fabricated_genesis, &content).is_err()
        );

        let error = crawl_targets_with_scenarios(
            &content,
            CrawlBudget {
                max_depth: 6,
                ..CrawlBudget::default()
            },
            BTreeSet::from(["return.patch_stand".to_owned()]),
            &["m1-outcome-hold-market"],
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("seed prefix exceeds the depth budget")
        );
    }

    #[test]
    fn bounded_regression_crawl_preserves_all_sixty_previous_definitions() {
        let content = parse_and_compile_production(SPLIT_TIDE).unwrap();
        let budget = CrawlBudget {
            max_expanded_states: 96,
            max_discovered_frontiers: 512,
            ..CrawlBudget::default()
        };
        let report = crate::expansion::crawl_regression(&content, budget)
            .unwrap()
            .crawl;
        assert!(report.is_complete());
        assert_eq!(report.required_definitions.len(), 60);
        assert_eq!(report.advertised_definitions.len(), 73);
        assert!(report.reached_locations.len() >= 7);
        assert!(report.successful_actions >= report.required_definitions.len());
        assert_eq!(report.starting_sessions.len(), 2);
        assert!(
            report
                .starting_sessions
                .iter()
                .all(|start| start.depth == 0 && start.label.starts_with("preset:"))
        );
        assert!(report.expanded_states <= 96);
        assert!(report.discovered_frontiers <= 512);
        assert_eq!(report.execution_receipt.len(), 64);

        let repeated = crate::expansion::crawl_regression(&content, budget)
            .unwrap()
            .crawl;
        assert_eq!(report.execution_receipt, repeated.execution_receipt);
    }

    #[test]
    fn supply_oracle_projects_exact_player_stock_and_excludes_npc_stock() {
        let content = parse_and_compile_production(SPLIT_TIDE).unwrap();
        let state = content.new_game("rook", 71).unwrap();
        assert!(
            state.world.npcs["yara_dene"]
                .inventory
                .contains_key("split_tide.tide_key")
        );
        let observation = content.observe(&state).unwrap();
        verify_supply_projection(&state, &content, &observation).unwrap();
        assert_eq!(
            observation
                .supplies
                .resources
                .iter()
                .map(|resource| resource.id.as_str())
                .collect::<Vec<_>>(),
            vec!["coin", "stamina"]
        );
        assert_eq!(
            observation
                .supplies
                .items
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec!["rope", "wire"]
        );
        assert!(
            observation
                .supplies
                .items
                .iter()
                .all(|item| item.id != "split_tide.tide_key")
        );
    }

    #[test]
    fn supply_oracle_rejects_missing_duplicate_reordered_and_tampered_entries() {
        let content = parse_and_compile_production(SPLIT_TIDE).unwrap();
        let state = content.new_game("rook", 71).unwrap();
        let observation = content.observe(&state).unwrap();
        let rejects = |tampered: forge_kernel::Observation| {
            assert!(
                verify_supply_projection(&state, &content, &tampered).is_err(),
                "tampered supply projection was accepted"
            );
        };

        let mut missing_resource = observation.clone();
        missing_resource.supplies.resources.pop();
        rejects(missing_resource);

        let mut duplicate_resource = observation.clone();
        duplicate_resource
            .supplies
            .resources
            .insert(0, observation.supplies.resources[0].clone());
        rejects(duplicate_resource);

        let mut reordered_resource = observation.clone();
        reordered_resource.supplies.resources.reverse();
        rejects(reordered_resource);

        let mut missing_item = observation.clone();
        missing_item.supplies.items.pop();
        rejects(missing_item);

        let mut duplicate_item = observation.clone();
        duplicate_item
            .supplies
            .items
            .insert(0, observation.supplies.items[0].clone());
        rejects(duplicate_item);

        let mut reordered_item = observation.clone();
        reordered_item.supplies.items.reverse();
        rejects(reordered_item);

        let mut wrong_amount = observation.clone();
        wrong_amount.supplies.resources[0].amount += 1;
        rejects(wrong_amount);

        let mut wrong_count = observation.clone();
        wrong_count.supplies.items[0].count += 1;
        rejects(wrong_count);

        let mut wrong_name = observation.clone();
        wrong_name.supplies.resources[0].name = "False Coin".to_owned();
        rejects(wrong_name);

        let mut wrong_id = observation.clone();
        wrong_id.supplies.items[0].id = "other_item".to_owned();
        rejects(wrong_id);

        let mut npc_stock = observation;
        npc_stock.supplies.items.push(forge_kernel::ItemView {
            id: "split_tide.tide_key".to_owned(),
            name: "Tide Key".to_owned(),
            count: 1,
        });
        rejects(npc_stock);
    }

    #[test]
    fn distance_uses_authored_literal_movement_across_gated_edges() {
        let content = parse_and_compile_production(SPLIT_TIDE).unwrap();
        assert_eq!(
            location_distance(&content, "lowsail.levee", "red_sluice.floor", None).unwrap(),
            1
        );
        assert_eq!(
            location_distance(&content, "red_sluice.top", "lowsail.return", None).unwrap(),
            1
        );
    }

    #[test]
    fn ready_target_distance_requires_an_earned_movement_route() {
        let content = parse_and_compile_production(SPLIT_TIDE).unwrap();
        let apply = |state: GameState, id: &str, destination: Option<&str>| {
            let action = enumerate_legal_actions(&state, &content)
                .unwrap()
                .into_iter()
                .find(|action| {
                    action.definition_id == id
                        && destination.is_none_or(|destination| {
                            action.parameters.get("destination").map(String::as_str)
                                == Some(destination)
                        })
                })
                .expect("route regression must use a canonical legal action");
            step(&state, &action, &content, &state.entropy)
                .unwrap()
                .into_state()
        };
        let start = content.new_game("rook", 71).unwrap();
        let docks = apply(start, "travel_adjacent", Some("lowsail.docks"));
        let carrying = apply(docks, "docks.press_yara", None);
        assert!(
            content
                .action("floor.key_calibration")
                .unwrap()
                .condition
                .evaluate(&carrying)
        );
        assert_eq!(
            location_distance(&content, "lowsail.docks", "red_sluice.floor", None).unwrap(),
            2
        );
        assert_eq!(
            location_distance(
                &content,
                "lowsail.docks",
                "red_sluice.floor",
                Some(&carrying)
            )
            .unwrap(),
            usize::MAX
        );
        let informed = apply(carrying, "docks.ask_oren", None);
        assert_eq!(
            location_distance(
                &content,
                "lowsail.docks",
                "red_sluice.floor",
                Some(&informed)
            )
            .unwrap(),
            2
        );
    }

    #[test]
    fn crawl_rejects_a_resolved_non_time_action_that_remains_legal() {
        let mut draft = parse(SPLIT_TIDE).unwrap();
        draft
            .actions
            .iter_mut()
            .find(|action| action.id == "checkpoint.read_flag")
            .unwrap()
            .condition = Condition::Always;
        let content = compile_production(draft).unwrap();
        let error = crawl_production(&content, CrawlBudget::default()).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("resolved non-time action checkpoint.read_flag remains legal")
        );
    }

    fn resolved_rook_for_arrival_planning(content: &CompiledContent) -> GameState {
        let mut state = content.new_game("rook", 71).unwrap();
        for (definition, destination) in [
            ("checkpoint.use_stolen_permit", None),
            ("travel_adjacent", Some("lowsail.levee")),
            ("levee.stolen_path", None),
            ("floor.climb_hot_face", None),
            ("top.overload", None),
        ] {
            let action = enumerate_legal_actions(&state, content)
                .unwrap()
                .into_iter()
                .find(|action| {
                    action.definition_id == definition
                        && destination.is_none_or(|destination| {
                            action.parameters.get("destination").map(String::as_str)
                                == Some(destination)
                        })
                })
                .unwrap();
            state = step(&state, &action, content, &state.entropy)
                .unwrap()
                .into_state();
        }
        state
    }

    #[test]
    fn npc_arrival_planning_requires_open_player_travel_and_never_mutates_state() {
        let content = parse_and_compile_production(SPLIT_TIDE).unwrap();
        let initial = content.new_game("rook", 71).unwrap();
        assert!(
            immediate_npc_arrival_projections(&initial, &content)
                .unwrap()
                .is_empty()
        );
        let resolved = resolved_rook_for_arrival_planning(&content);
        let before = resolved.clone();
        let ending = content.action("return.face_flood").unwrap();
        assert!(!ending.condition.evaluate(&resolved));
        let arrivals = immediate_npc_arrival_projections(&resolved, &content).unwrap();
        assert_eq!(resolved, before);
        assert_eq!(arrivals.len(), 1);
        assert_eq!(arrivals[0].world.current_location, "lowsail.return");
        assert!(ending.condition.evaluate(&arrivals[0]));

        let action = enumerate_legal_actions(&resolved, &content)
            .unwrap()
            .into_iter()
            .find(|action| action.definition_id == "world.enter_aftermath")
            .unwrap();
        let actual = step(&resolved, &action, &content, &resolved.entropy).unwrap();
        assert_eq!(arrivals[0].world.npcs, actual.state().world.npcs);
        assert_eq!(
            arrivals[0].event_log, resolved.event_log,
            "planning cannot fabricate history"
        );
        assert_eq!(
            arrivals[0].world.time, resolved.world.time,
            "planning is not execution"
        );
        assert!(
            enumerate_legal_actions(actual.state(), &content)
                .unwrap()
                .iter()
                .any(|action| action.definition_id == "return.face_flood")
        );

        let mut npc_only = content.action("world.enter_aftermath").unwrap().clone();
        npc_only
            .effects
            .retain(|effect| matches!(effect, Effect::MoveNpc { .. }));
        let mut targets = BTreeSet::new();
        visit_movement_targets(
            &npc_only,
            content.location("red_sluice.top").unwrap(),
            &mut |target| {
                targets.insert(target.to_owned());
            },
        );
        assert!(
            targets.is_empty(),
            "moving an NPC must not add a player graph edge"
        );
    }

    #[test]
    fn npc_arrival_planning_declines_unmodeled_programs() {
        for variant in [
            "random",
            "resource",
            "parameterized",
            "multiple_player_moves",
        ] {
            let mut draft = parse(SPLIT_TIDE).unwrap();
            let action = draft
                .actions
                .iter_mut()
                .find(|action| action.id == "world.enter_aftermath")
                .unwrap();
            match variant {
                "random" => {
                    action.effects[0] = Effect::RandomChance {
                        success_percent: 50,
                        on_success: Box::new(action.effects[0].clone()),
                        on_failure: Box::new(Effect::Noop),
                    };
                }
                "resource" => action.effects.push(Effect::AdjustResource {
                    resource: "coin".to_owned(),
                    amount: 1,
                }),
                "parameterized" => {
                    let Effect::MoveNpc { npc, .. } = &mut action.effects[0] else {
                        panic!()
                    };
                    *npc = StringRef::Parameter("inhabitant".to_owned());
                    action.parameters.push(forge_kernel::ParameterSpec {
                        name: "inhabitant".to_owned(),
                        domain: ParameterDomain::Values(vec!["oren_pell".to_owned()]),
                    });
                }
                "multiple_player_moves" => action.effects.push(Effect::MoveCharacter {
                    location: StringRef::Literal("red_sluice.top".to_owned()),
                }),
                _ => unreachable!(),
            }
            let content = compile_production(draft).unwrap();
            let resolved = resolved_rook_for_arrival_planning(&content);
            assert!(
                enumerate_legal_actions(&resolved, &content)
                    .unwrap()
                    .iter()
                    .any(|action| action.definition_id == "world.enter_aftermath")
            );
            assert!(
                immediate_npc_arrival_projections(&resolved, &content)
                    .unwrap()
                    .is_empty(),
                "{variant}"
            );
        }
    }

    #[test]
    fn exhausted_transfer_retires_without_a_redundant_flag_guard() {
        let mut draft = parse(SPLIT_TIDE).unwrap();
        let take_key = draft
            .actions
            .iter_mut()
            .find(|action| action.id == "docks.press_yara")
            .unwrap();
        let Condition::All { conditions } = &mut take_key.condition else {
            panic!("Tide Key action uses a conjunction");
        };
        conditions.retain(|condition| {
            !matches!(condition,
                Condition::Not { condition }
                    if matches!(condition.as_ref(), Condition::WorldFlag { flag }
                        if flag == "tide_key_offered")
            )
        });
        let content = compile_production(draft).unwrap();
        let report = crate::expansion::crawl_regression(
            &content,
            CrawlBudget {
                max_expanded_states: 96,
                max_discovered_frontiers: 512,
                ..CrawlBudget::default()
            },
        )
        .expect("source depletion is authoritative retirement even if the guard remains true");
        assert!(report.crawl.is_complete());
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
