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
    (usize, Reverse<usize>, usize, usize, Reverse<usize>),
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
    let mut relevant_flags = BTreeSet::new();
    let mut relevant_items = BTreeSet::new();
    let mut recipe_items = BTreeSet::new();
    // Preserve established multi-target search ordering. Once only one goal
    // remains, its item prerequisite can justify a temporary walk away before
    // comparing raw clause counts. A closed source must not outrank available
    // acquisition merely because it satisfies more unrelated clauses.
    let final_target = report
        .required_definitions
        .difference(&report.covered_definitions)
        .count()
        == 1;
    for (id, action) in content.actions() {
        if report.required_definitions.contains(id) && !report.covered_definitions.contains(id) {
            collect_condition_flags(&action.condition, &mut relevant_flags);
            collect_condition_items(&action.condition, &mut recipe_items);
            if final_target {
                collect_condition_items(&action.condition, &mut relevant_items);
            }
        }
    }
    let producer_flags =
        potential_flag_producer_projections(&frontier.state, content, &relevant_flags)?;
    let acquisition_hints =
        potential_item_acquisition_projections(&frontier.state, content, &relevant_items)?;
    let recipe_hints = potential_recipe_work_projections(&frontier.state, content, &recipe_items)?;
    let mut acquisition_ready = 0usize;
    let mut nearest_acquisition = usize::MAX;
    let mut uncovered_here = 0usize;
    let mut ready_one_move = 0usize;
    let mut ready_uncovered = 0usize;
    let mut nearest_ready_target = usize::MAX;
    let mut deferred_ready = 0usize;
    let mut nearest_deferred_ready = u64::MAX;
    let mut producer_ready = 0usize;
    let mut nearest_producer = usize::MAX;
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
            if let Some(distance) =
                item_acquisition_distance(definition, &acquisition_hints, content)?
            {
                acquisition_ready += 1;
                nearest_acquisition = nearest_acquisition.min(distance);
            }
            let flag_distance = producer_flags
                .iter()
                .filter_map(|(distance, projection)| {
                    ((definition.locations.is_empty()
                        || definition
                            .locations
                            .contains(&projection.world.current_location))
                        && definition.condition.evaluate(projection))
                    .then_some(*distance)
                })
                .min();
            let recipe_distance = item_acquisition_distance(definition, &recipe_hints, content)?;
            let producer_distance = flag_distance.into_iter().chain(recipe_distance).min();
            if let Some(distance) = producer_distance {
                producer_ready += 1;
                nearest_producer = nearest_producer.min(distance);
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
        (
            acquisition_ready,
            Reverse(nearest_acquisition),
            total_progress,
            producer_ready,
            Reverse(nearest_producer),
        ),
        best_partial.0,
        Reverse(frontier.depth.saturating_add((best_partial.1).0)),
        state_progress(&frontier.state),
        !report.reached_locations.contains(location),
        Reverse(frontier.ordinal),
    ))
}

/// Identify a possible local flag-producing action after an available walk.
/// The current-location case requires a kernel-enumerated action. Distant
/// cases use existing condition-gated graph hints and remain only potential:
/// movement effects, inventory consumption, time, and NPC facts are not
/// simulated. Complete direct flag writes preserve negative target clauses.
/// No projection is admitted, reduced, hashed, enqueued, or counted as coverage.
fn potential_flag_producer_projections(
    state: &GameState,
    content: &CompiledContent,
    relevant_flags: &BTreeSet<&str>,
) -> Result<Vec<(usize, GameState)>, VerifyError> {
    let mut projections = Vec::new();
    let mut local_catalog: Option<BTreeSet<String>> = None;
    for (_, action) in content.actions() {
        if action.movement || !action.parameters.is_empty() {
            continue;
        }
        let mut has_flag = false;
        let supported = action.effects.iter().all(|effect| match effect {
            Effect::SetFlag { flag, .. }
            | Effect::SetWorldFlag { flag, .. }
            | Effect::SetLocationFlag {
                location: StringRef::Literal(_),
                flag,
                ..
            } => {
                has_flag |= relevant_flags.contains(flag.as_str());
                true
            }
            Effect::RandomChance { .. }
            | Effect::ScheduleEvent { .. }
            | Effect::MoveCharacter { .. }
            | Effect::MoveNpc { .. }
            | Effect::SetLocationFlag {
                location: StringRef::Parameter(_),
                ..
            } => false,
            Effect::Noop
            | Effect::AdjustResource { .. }
            | Effect::AdjustNpcRelationship { .. }
            | Effect::AddNpcMemory { .. }
            | Effect::TeachNpc { .. }
            | Effect::TransferNpcItemToCharacter { .. }
            | Effect::TransferStorageItemToCharacter { .. }
            | Effect::TransferCharacterItemToStorage { .. }
            | Effect::ApplyRecipe { .. }
            | Effect::AddCharacterDeed { .. }
            | Effect::AdvanceTime { .. } => true,
        });
        if !supported || !has_flag {
            continue;
        }
        let locations = if action.locations.is_empty() {
            std::slice::from_ref(&state.world.current_location)
        } else {
            action.locations.as_slice()
        };
        for location in locations {
            let mut projection = state.clone();
            projection.world.current_location.clone_from(location);
            if !action.condition.evaluate(&projection) {
                continue;
            }
            let distance = location_distance(
                content,
                &state.world.current_location,
                location,
                Some(state),
            )?;
            if distance == usize::MAX {
                continue;
            }
            if distance == 0 {
                if local_catalog.is_none() {
                    local_catalog = Some(
                        enumerate_legal_actions(state, content)
                            .map_err(|_| {
                                VerifyError::new(
                                    "crawler could not enumerate flag producer candidates",
                                )
                            })?
                            .into_iter()
                            .map(|action| action.definition_id)
                            .collect(),
                    );
                }
                if !local_catalog.as_ref().unwrap().contains(&action.id) {
                    continue;
                }
            }
            for effect in &action.effects {
                let (flags, flag, value) = match effect {
                    Effect::SetFlag { flag, value } | Effect::SetWorldFlag { flag, value } => {
                        (&mut projection.world.flags, flag, value)
                    }
                    Effect::SetLocationFlag {
                        location: StringRef::Literal(location),
                        flag,
                        value,
                    } => (
                        &mut projection
                            .world
                            .locations
                            .get_mut(location)
                            .ok_or_else(|| {
                                VerifyError::new("flag producer references an unknown location")
                            })?
                            .flags,
                        flag,
                        value,
                    ),
                    _ => continue,
                };
                if *value {
                    flags.insert(flag.clone());
                } else {
                    flags.remove(flag);
                }
            }
            projections.try_reserve(1).map_err(|_| {
                VerifyError::new("crawler flag producer planning allocation failed")
            })?;
            projections.push((distance.saturating_add(1), projection));
        }
    }
    Ok(projections)
}

/// Potential inventory and flags after one fixed recipe program and an
/// available walk. All recipes and direct flag writes are staged in order;
/// other inventory mutations, movement, randomness and scheduling are excluded.
/// Local work must be in the actual canonical catalog. Distant work and the
/// onward walk remain hints: travel effects and crossed timers are not modeled.
/// Ignored record writes can also leave a negative knowledge/deed guard looking
/// ready when real execution would invalidate it; this is only potential work.
/// No projected time, knowledge, history or entropy enters real state or coverage.
fn potential_recipe_work_projections(
    state: &GameState,
    content: &CompiledContent,
    relevant_items: &BTreeSet<&str>,
) -> Result<Vec<(usize, GameState)>, VerifyError> {
    let mut hints = Vec::new();
    let mut local_catalog: Option<BTreeSet<String>> = None;
    for (_, action) in content.actions() {
        if action.movement || !action.parameters.is_empty() {
            continue;
        }
        let mut relevant = false;
        let supported = action.effects.iter().all(|effect| match effect {
            Effect::ApplyRecipe { recipe } => {
                let Some(recipe) = content.recipe(recipe) else {
                    return false;
                };
                relevant |= recipe
                    .outputs
                    .keys()
                    .any(|item| relevant_items.contains(item.as_str()));
                true
            }
            Effect::SetFlag { .. }
            | Effect::SetWorldFlag { .. }
            | Effect::SetLocationFlag {
                location: StringRef::Literal(_),
                ..
            }
            | Effect::Noop
            | Effect::AddNpcMemory { .. }
            | Effect::TeachNpc { .. }
            | Effect::AddCharacterDeed { .. }
            | Effect::AdvanceTime { .. } => true,
            Effect::TransferNpcItemToCharacter { .. }
            | Effect::TransferStorageItemToCharacter { .. }
            | Effect::TransferCharacterItemToStorage { .. }
            | Effect::AdjustResource { .. }
            | Effect::AdjustNpcRelationship { .. }
            | Effect::SetLocationFlag {
                location: StringRef::Parameter(_),
                ..
            }
            | Effect::MoveCharacter { .. }
            | Effect::MoveNpc { .. }
            | Effect::RandomChance { .. }
            | Effect::ScheduleEvent { .. } => false,
        });
        if !supported || !relevant {
            continue;
        }
        let locations = if action.locations.is_empty() {
            std::slice::from_ref(&state.world.current_location)
        } else {
            action.locations.as_slice()
        };
        for location in locations {
            let mut projected = state.clone();
            projected.world.current_location.clone_from(location);
            if !action.condition.evaluate(&projected) {
                continue;
            }
            let distance = location_distance(
                content,
                &state.world.current_location,
                location,
                Some(state),
            )?;
            if distance == usize::MAX {
                continue;
            }
            if distance == 0 {
                if local_catalog.is_none() {
                    local_catalog = Some(
                        enumerate_legal_actions(state, content)
                            .map_err(|_| {
                                VerifyError::new("crawler could not enumerate recipe work")
                            })?
                            .into_iter()
                            .map(|action| action.definition_id)
                            .collect(),
                    );
                }
                if !local_catalog.as_ref().unwrap().contains(&action.id) {
                    continue;
                }
            }
            if !project_recipe_work(&mut projected, action, content) {
                continue;
            }
            hints
                .try_reserve(1)
                .map_err(|_| VerifyError::new("crawler recipe work hint allocation failed"))?;
            hints.push((distance.saturating_add(1), projected));
        }
    }
    Ok(hints)
}

fn project_recipe_work(
    projected: &mut GameState,
    action: &ActionDefinition,
    content: &CompiledContent,
) -> bool {
    let mut latest_time = projected.world.time;
    for effect in &action.effects {
        match effect {
            Effect::ApplyRecipe { recipe } => {
                let Some(recipe) = content.recipe(recipe) else {
                    return false;
                };
                for (item, count) in &recipe.inputs {
                    let held = projected
                        .character
                        .inventory
                        .get(item)
                        .copied()
                        .unwrap_or(0);
                    let Some(remaining) = held.checked_sub(*count) else {
                        return false;
                    };
                    if remaining == 0 {
                        projected.character.inventory.remove(item);
                    } else {
                        projected
                            .character
                            .inventory
                            .insert(item.clone(), remaining);
                    }
                }
                for (item, count) in &recipe.outputs {
                    let held = projected
                        .character
                        .inventory
                        .get(item)
                        .copied()
                        .unwrap_or(0);
                    let Some(total) = held.checked_add(*count) else {
                        return false;
                    };
                    projected.character.inventory.insert(item.clone(), total);
                }
            }
            Effect::SetFlag { flag, value } | Effect::SetWorldFlag { flag, value } => {
                if *value {
                    projected.world.flags.insert(flag.clone());
                } else {
                    projected.world.flags.remove(flag);
                }
            }
            Effect::SetLocationFlag {
                location: StringRef::Literal(location),
                flag,
                value,
            } => {
                let Some(runtime) = projected.world.locations.get_mut(location) else {
                    return false;
                };
                if *value {
                    runtime.flags.insert(flag.clone());
                } else {
                    runtime.flags.remove(flag);
                }
            }
            Effect::AdvanceTime { ticks } => {
                let Some(next) = latest_time.checked_add(*ticks) else {
                    return false;
                };
                latest_time = next;
            }
            // These records do not establish projected target knowledge or
            // history. Their real effects still require canonical execution.
            Effect::Noop
            | Effect::AddNpcMemory { .. }
            | Effect::TeachNpc { .. }
            | Effect::AddCharacterDeed { .. } => {}
            _ => return false,
        }
    }
    true
}

/// Potential item ownership after one fixed withdrawal program. Sources remain
/// separate from the projected state and are checked in effect order. These
/// values only rank real frontiers; they never enter admission or coverage.
/// Travel side effects and crossed timers remain unmodeled, so even a legal
/// withdrawal may yield a false-positive hint if a due recipe consumes it.
fn potential_item_acquisition_projections(
    state: &GameState,
    content: &CompiledContent,
    relevant_items: &BTreeSet<&str>,
) -> Result<Vec<(usize, GameState)>, VerifyError> {
    let mut hints = Vec::new();
    let mut local_catalog: Option<BTreeSet<String>> = None;
    for (_, action) in content.actions() {
        if action.movement || !action.parameters.is_empty() {
            continue;
        }
        let mut relevant = false;
        let supported = action.effects.iter().all(|effect| match effect {
            Effect::TransferNpcItemToCharacter {
                npc: StringRef::Literal(_),
                item,
                count,
            }
            | Effect::TransferStorageItemToCharacter { item, count, .. } => {
                relevant |= relevant_items.contains(item.as_str());
                *count > 0
            }
            Effect::SetFlag { .. }
            | Effect::SetWorldFlag { .. }
            | Effect::SetLocationFlag {
                location: StringRef::Literal(_),
                ..
            }
            | Effect::Noop
            | Effect::AddNpcMemory { .. }
            | Effect::TeachNpc { .. }
            | Effect::AddCharacterDeed { .. }
            | Effect::AdvanceTime { .. } => true,
            Effect::TransferNpcItemToCharacter {
                npc: StringRef::Parameter(_),
                ..
            }
            | Effect::TransferCharacterItemToStorage { .. }
            | Effect::ApplyRecipe { .. }
            | Effect::AdjustResource { .. }
            | Effect::AdjustNpcRelationship { .. }
            | Effect::SetLocationFlag {
                location: StringRef::Parameter(_),
                ..
            }
            | Effect::MoveCharacter { .. }
            | Effect::MoveNpc { .. }
            | Effect::RandomChance { .. }
            | Effect::ScheduleEvent { .. } => false,
        });
        if !supported || !relevant {
            continue;
        }
        let locations = if action.locations.is_empty() {
            std::slice::from_ref(&state.world.current_location)
        } else {
            action.locations.as_slice()
        };
        for location in locations {
            let mut projected = state.clone();
            projected.world.current_location.clone_from(location);
            if !action.condition.evaluate(&projected) {
                continue;
            }
            let distance = location_distance(
                content,
                &state.world.current_location,
                location,
                Some(state),
            )?;
            if distance == usize::MAX {
                continue;
            }
            if distance == 0 {
                if local_catalog.is_none() {
                    local_catalog = Some(
                        enumerate_legal_actions(state, content)
                            .map_err(|_| {
                                VerifyError::new(
                                    "crawler could not enumerate item acquisition candidates",
                                )
                            })?
                            .into_iter()
                            .map(|action| action.definition_id)
                            .collect(),
                    );
                }
                if !local_catalog.as_ref().unwrap().contains(&action.id) {
                    continue;
                }
            }
            if !project_item_acquisition(&mut projected, action, content) {
                continue;
            }
            hints
                .try_reserve(1)
                .map_err(|_| VerifyError::new("crawler item acquisition hint allocation failed"))?;
            hints.push((distance.saturating_add(1), projected));
        }
    }
    Ok(hints)
}

fn project_item_acquisition(
    projected: &mut GameState,
    action: &ActionDefinition,
    content: &CompiledContent,
) -> bool {
    let mut remaining_sources = BTreeMap::new();
    let mut latest_time = projected.world.time;
    for effect in &action.effects {
        let source = match effect {
            Effect::TransferNpcItemToCharacter {
                npc: StringRef::Literal(npc),
                item,
                count,
            } => {
                let Some(source) = projected.world.npcs.get(npc) else {
                    return false;
                };
                if source.location != projected.world.current_location {
                    return false;
                }
                Some((
                    false,
                    npc,
                    item,
                    count,
                    source.inventory.get(item).copied().unwrap_or_default(),
                ))
            }
            Effect::TransferStorageItemToCharacter {
                storage,
                item,
                count,
            } => {
                let Some(definition) = content.storage(storage) else {
                    return false;
                };
                let Some(source) = projected.world.storages.get(storage) else {
                    return false;
                };
                if definition.location != projected.world.current_location {
                    return false;
                }
                Some((
                    true,
                    storage,
                    item,
                    count,
                    source.inventory.get(item).copied().unwrap_or_default(),
                ))
            }
            Effect::SetFlag { flag, value } | Effect::SetWorldFlag { flag, value } => {
                if *value {
                    projected.world.flags.insert(flag.clone());
                } else {
                    projected.world.flags.remove(flag);
                }
                None
            }
            Effect::SetLocationFlag {
                location: StringRef::Literal(location),
                flag,
                value,
            } => {
                let Some(runtime) = projected.world.locations.get_mut(location) else {
                    return false;
                };
                if *value {
                    runtime.flags.insert(flag.clone());
                } else {
                    runtime.flags.remove(flag);
                }
                None
            }
            Effect::AdvanceTime { ticks } => {
                let Some(next) = latest_time.checked_add(*ticks) else {
                    return false;
                };
                latest_time = next;
                None
            }
            // No NPC facts, deeds, entropy, history, time, or source balances
            // are invented. A later target must already have those premises.
            _ => None,
        };
        if let Some((storage, owner, item, count, initial)) = source {
            let remaining = remaining_sources
                .entry((storage, owner, item))
                .or_insert(initial);
            let Some(next_source) = remaining.checked_sub(*count) else {
                return false;
            };
            let held = projected
                .character
                .inventory
                .get(item)
                .copied()
                .unwrap_or_default();
            let Some(next_held) = held.checked_add(*count) else {
                return false;
            };
            *remaining = next_source;
            projected
                .character
                .inventory
                .insert(item.clone(), next_held);
        }
    }
    true
}

fn item_acquisition_distance(
    target: &ActionDefinition,
    hints: &[(usize, GameState)],
    content: &CompiledContent,
) -> Result<Option<usize>, VerifyError> {
    let mut nearest = None;
    for (approach, hint) in hints {
        let locations = if target.locations.is_empty() {
            std::slice::from_ref(&hint.world.current_location)
        } else {
            target.locations.as_slice()
        };
        for location in locations {
            let mut at_target = hint.clone();
            at_target.world.current_location.clone_from(location);
            if !target.condition.evaluate(&at_target) {
                continue;
            }
            let onward =
                location_distance(content, &hint.world.current_location, location, Some(hint))?;
            if onward != usize::MAX {
                let distance = approach.saturating_add(onward);
                nearest = Some(nearest.map_or(distance, |prior: usize| prior.min(distance)));
            }
        }
    }
    Ok(nearest)
}

fn collect_condition_items<'a>(condition: &'a Condition, items: &mut BTreeSet<&'a str>) {
    match condition {
        Condition::HasItem { item, .. } => {
            items.insert(item);
        }
        Condition::All { conditions } | Condition::Any { conditions } => {
            for condition in conditions {
                collect_condition_items(condition, items);
            }
        }
        Condition::Not { condition } => collect_condition_items(condition, items),
        _ => {}
    }
}

fn collect_condition_flags<'a>(condition: &'a Condition, flags: &mut BTreeSet<&'a str>) {
    match condition {
        Condition::WorldFlag { flag } | Condition::LocationFlag { flag, .. } => {
            flags.insert(flag.as_str());
        }
        Condition::All { conditions } | Condition::Any { conditions } => {
            for condition in conditions {
                collect_condition_flags(condition, flags);
            }
        }
        Condition::Not { condition } => collect_condition_flags(condition, flags),
        _ => {}
    }
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
        | Effect::TransferStorageItemToCharacter { .. }
        | Effect::TransferCharacterItemToStorage { .. }
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

    fn fixture_producer_hints(
        state: &GameState,
        content: &CompiledContent,
    ) -> Result<Vec<(usize, GameState)>, VerifyError> {
        let mut flags = BTreeSet::new();
        collect_condition_flags(&content.action("test.use").unwrap().condition, &mut flags);
        super::potential_flag_producer_projections(state, content, &flags)
    }

    fn flag_producer_fixture() -> forge_kernel::ContentDraft {
        let mut draft = parse(SPLIT_TIDE).unwrap();
        draft.contract = forge_kernel::ContentContract::Fixture;
        draft.character_presets.truncate(1);
        draft.character_presets[0].character.inventory =
            BTreeMap::from([("test.part".to_owned(), 1)]);
        draft.character_creation = None;
        draft.timed_events.clear();
        draft.deferred_events.clear();
        draft.npcs.retain(|npc| npc.id == "sava_rusk");
        draft.storages.clear();
        draft
            .locations
            .retain(|location| ["lowsail_market", "lowsail.docks"].contains(&location.id.as_str()));
        for location in &mut draft.locations {
            location.terminal = true;
            location.description = "A workbench stands here.".to_owned();
            location.description_variants.clear();
            location.exits = vec![
                if location.id == "lowsail_market" {
                    "lowsail.docks"
                } else {
                    "lowsail_market"
                }
                .to_owned(),
            ];
        }
        draft.recipes = vec![forge_kernel::RecipeDefinition {
            id: "test.install".to_owned(),
            inputs: BTreeMap::from([("test.part".to_owned(), 1)]),
            outputs: BTreeMap::new(),
        }];
        let flag = |name: &str| Condition::WorldFlag {
            flag: name.to_owned(),
        };
        let absent = |name: &str| Condition::Not {
            condition: Box::new(flag(name)),
        };
        let action = |id: &str, condition, effects| ActionDefinition {
            id: id.to_owned(),
            label: "Act".to_owned(),
            category: "Work".to_owned(),
            result: "Done.".to_owned(),
            result_variants: Vec::new(),
            locations: vec!["lowsail.docks".to_owned()],
            condition,
            effects,
            parameters: Vec::new(),
            meaningful: true,
            movement: false,
        };
        let mut walk = action(
            "test.walk",
            Condition::Always,
            vec![
                Effect::MoveCharacter {
                    location: StringRef::Parameter("destination".to_owned()),
                },
                Effect::AdvanceTime { ticks: 1 },
            ],
        );
        walk.locations.clear();
        walk.movement = true;
        walk.parameters = vec![forge_kernel::ParameterSpec {
            name: "destination".to_owned(),
            domain: ParameterDomain::LocationsAdjacent,
        }];
        draft.actions = vec![
            walk,
            action(
                "test.fit",
                Condition::All {
                    conditions: vec![
                        Condition::HasItem {
                            item: "test.part".to_owned(),
                            count: 1,
                        },
                        absent("test.ready"),
                    ],
                },
                vec![
                    Effect::ApplyRecipe {
                        recipe: "test.install".to_owned(),
                    },
                    Effect::SetWorldFlag {
                        flag: "test.ready".to_owned(),
                        value: true,
                    },
                    Effect::SetWorldFlag {
                        flag: "test.closed".to_owned(),
                        value: false,
                    },
                    Effect::AdvanceTime { ticks: 1 },
                ],
            ),
            action(
                "test.use",
                Condition::All {
                    conditions: vec![
                        flag("test.ready"),
                        absent("test.closed"),
                        absent("test.done"),
                    ],
                },
                vec![
                    Effect::SetWorldFlag {
                        flag: "test.done".to_owned(),
                        value: true,
                    },
                    Effect::AdvanceTime { ticks: 1 },
                ],
            ),
        ];
        draft
    }

    fn acquisition_fixture(stored: bool) -> forge_kernel::ContentDraft {
        let mut draft = flag_producer_fixture();
        draft.character_presets[0].character.inventory.clear();
        draft.npcs[0].location = "lowsail.docks".to_owned();
        draft.npcs[0].inventory = if stored {
            BTreeMap::new()
        } else {
            BTreeMap::from([("test.part".to_owned(), 1)])
        };
        if stored {
            draft.storages.push(forge_kernel::StorageDefinition {
                id: "test.reserve".to_owned(),
                name: "Reserve".to_owned(),
                location: "lowsail.docks".to_owned(),
                inventory: BTreeMap::from([("test.part".to_owned(), 1)]),
            });
        }
        draft.actions[1].id = "test.take".to_owned();
        draft.actions[1].condition = Condition::NpcAtLocation {
            npc: "sava_rusk".to_owned(),
            location: "lowsail.docks".to_owned(),
        };
        let transfer = if stored {
            Effect::TransferStorageItemToCharacter {
                storage: "test.reserve".to_owned(),
                item: "test.part".to_owned(),
                count: 1,
            }
        } else {
            Effect::TransferNpcItemToCharacter {
                npc: StringRef::Literal("sava_rusk".to_owned()),
                item: "test.part".to_owned(),
                count: 1,
            }
        };
        draft.actions[1].effects = vec![
            transfer,
            Effect::SetWorldFlag {
                flag: "test.taken".to_owned(),
                value: true,
            },
            Effect::AddCharacterDeed {
                deed_id: "test.received".to_owned(),
            },
            Effect::TeachNpc {
                npc: StringRef::Literal("sava_rusk".to_owned()),
                knowledge_id: "test.receipt".to_owned(),
                subject: "The item changed hands.".to_owned(),
                provenance: forge_kernel::KnowledgeProvenance::Witnessed,
            },
            Effect::AdvanceTime { ticks: 1 },
        ];
        draft.actions[2].locations = vec!["lowsail_market".to_owned()];
        draft.actions[2].condition = Condition::All {
            conditions: vec![
                Condition::HasItem {
                    item: "test.part".to_owned(),
                    count: 1,
                },
                Condition::Not {
                    condition: Box::new(Condition::WorldFlag {
                        flag: "test.done".to_owned(),
                    }),
                },
            ],
        };
        draft.actions[2].effects = vec![
            Effect::ApplyRecipe {
                recipe: "test.install".to_owned(),
            },
            Effect::SetWorldFlag {
                flag: "test.done".to_owned(),
                value: true,
            },
            Effect::AdvanceTime { ticks: 1 },
        ];
        draft
    }

    fn recipe_work_fixture() -> forge_kernel::ContentDraft {
        let mut draft = flag_producer_fixture();
        draft.character_presets[0].character.inventory.clear();
        draft.npcs[0].inventory = BTreeMap::from([("test.part".to_owned(), 1)]);
        draft.recipes = vec![
            forge_kernel::RecipeDefinition {
                id: "test.make".to_owned(),
                inputs: BTreeMap::from([("test.part".to_owned(), 1)]),
                outputs: BTreeMap::from([("test.product".to_owned(), 1)]),
            },
            forge_kernel::RecipeDefinition {
                id: "test.consume".to_owned(),
                inputs: BTreeMap::from([("test.product".to_owned(), 1)]),
                outputs: BTreeMap::new(),
            },
        ];
        draft.actions[1].effects[0] = Effect::ApplyRecipe {
            recipe: "test.make".to_owned(),
        };
        draft.actions[1].effects.insert(
            3,
            Effect::AddCharacterDeed {
                deed_id: "test.made".to_owned(),
            },
        );
        draft.actions[1].effects.push(Effect::SetLocationFlag {
            location: StringRef::Literal("lowsail.docks".to_owned()),
            flag: "test.made_here".to_owned(),
            value: true,
        });
        draft.actions[2].locations = vec!["lowsail_market".to_owned()];
        if let Condition::All { conditions } = &mut draft.actions[2].condition {
            conditions.push(Condition::HasItem {
                item: "test.product".to_owned(),
                count: 1,
            });
            conditions.push(Condition::LocationFlag {
                location: "lowsail.docks".to_owned(),
                flag: "test.made_here".to_owned(),
            });
        }
        draft.actions[2].effects.insert(
            0,
            Effect::ApplyRecipe {
                recipe: "test.consume".to_owned(),
            },
        );
        let mut take = draft.actions[2].clone();
        take.id = "test.take".to_owned();
        take.condition = Condition::Not {
            condition: Box::new(Condition::WorldFlag {
                flag: "test.stock_given".to_owned(),
            }),
        };
        take.effects = vec![
            Effect::TransferNpcItemToCharacter {
                npc: StringRef::Literal("sava_rusk".to_owned()),
                item: "test.part".to_owned(),
                count: 1,
            },
            Effect::SetWorldFlag {
                flag: "test.stock_given".to_owned(),
                value: true,
            },
            Effect::AdvanceTime { ticks: 1 },
        ];
        draft.actions.push(take);
        draft
    }

    fn recipe_hints(state: &GameState, content: &CompiledContent) -> Vec<(usize, GameState)> {
        let mut items = BTreeSet::new();
        collect_condition_items(&content.action("test.use").unwrap().condition, &mut items);
        potential_recipe_work_projections(state, content, &items).unwrap()
    }

    fn record_fixture_action(session: &mut Session<'_>, content: &CompiledContent, id: &str) {
        let action = enumerate_legal_actions(session.state(), content)
            .unwrap()
            .into_iter()
            .find(|action| action.definition_id == id)
            .unwrap();
        session.record(&action).unwrap();
    }

    #[test]
    fn recipe_work_hints_require_actual_stock_walk_recipe_return_and_complete_catalogs() {
        let content = forge_content::compile(recipe_work_fixture()).unwrap();
        let mut session = Session::new_game("ilyan", 71, &content).unwrap();
        assert!(recipe_hints(session.state(), &content).is_empty());
        record_fixture_action(&mut session, &content, "test.take");
        let before = session.state().clone();
        let hints = recipe_hints(&before, &content);
        assert_eq!(hints.len(), 1);
        assert_eq!(
            item_acquisition_distance(content.action("test.use").unwrap(), &hints, &content)
                .unwrap(),
            Some(3),
            "one approach, one recipe action, and one return are required"
        );
        let projected = &hints[0].1;
        assert_eq!(projected.character.inventory["test.product"], 1);
        assert!(!projected.character.inventory.contains_key("test.part"));
        assert!(projected.world.flags.contains("test.ready"));
        assert!(!projected.world.flags.contains("test.closed"));
        assert_eq!(projected.character.deeds, before.character.deeds);
        assert_eq!(projected.character.knowledge, before.character.knowledge);
        assert_eq!(projected.world.npcs, before.world.npcs);
        assert_eq!(projected.world.storages, before.world.storages);
        assert_eq!(projected.world.time, before.world.time);
        assert_eq!(projected.event_log, before.event_log);
        assert_eq!(projected.entropy, before.entropy);
        assert_eq!(*session.state(), before);
        assert!(
            enumerate_legal_actions(&before, &content)
                .unwrap()
                .iter()
                .all(|action| action.definition_id != "test.use")
        );
        for id in ["test.walk", "test.fit", "test.walk", "test.use"] {
            record_fixture_action(&mut session, &content, id);
        }
        assert!(session.state().character.inventory.is_empty());
        assert!(session.state().world.flags.contains("test.done"));
        assert_eq!(session.state().world.time, 5);
        assert_eq!(session.state().entropy.cursor, 0);
        assert_eq!(
            forge_replay::verify(session.trace(), &content).unwrap(),
            *session.state()
        );
        let report = crawl_targets(
            &content,
            CrawlBudget {
                max_depth: 5,
                max_expanded_states: 5,
                max_discovered_frontiers: 16,
                max_action_executions: 8,
                catalog_page_size: 1,
            },
            BTreeSet::from(["test.use".to_owned()]),
        )
        .unwrap();
        assert_eq!(report.expanded_states, 5);
        assert_eq!(report.successful_actions, 8);
        assert_eq!(
            report.covered_definitions,
            ["test.walk", "test.take", "test.fit", "test.use"]
                .map(str::to_owned)
                .into_iter()
                .collect()
        );
    }

    #[test]
    fn recipe_work_hints_help_an_uncovered_item_goal_before_the_final_goal_phase() {
        let mut draft = recipe_work_fixture();
        let mut other = draft.actions[2].clone();
        other.id = "test.other".to_owned();
        other.condition = Condition::WorldFlag {
            flag: "test.other_ready".to_owned(),
        };
        draft.actions.push(other);
        let content = forge_content::compile(draft).unwrap();
        let mut session = Session::new_game("ilyan", 71, &content).unwrap();
        let initial = Frontier {
            state: session.state().clone(),
            depth: 0,
            ordinal: 0,
            used_actions: BTreeSet::new(),
        };
        record_fixture_action(&mut session, &content, "test.take");
        let stocked = Frontier {
            state: session.state().clone(),
            depth: 1,
            ordinal: 1,
            used_actions: BTreeSet::new(),
        };
        let report = CrawlReport {
            verifier_id: crate::VERIFIER_ID.to_owned(),
            build_id: content.build_id().to_owned(),
            budget: CrawlBudget::default(),
            expanded_states: 0,
            discovered_frontiers: 0,
            successful_actions: 0,
            max_legal_actions: 0,
            reached_locations: BTreeSet::new(),
            covered_definitions: BTreeSet::new(),
            advertised_definitions: content.actions().map(|(id, _)| id.clone()).collect(),
            required_definitions: BTreeSet::from(["test.use".to_owned(), "test.other".to_owned()]),
            starting_sessions: Vec::new(),
            execution_receipt: "score fixture only".to_owned(),
        };
        let initial_score = frontier_score(&initial, &report, &content).unwrap();
        let stocked_score = frontier_score(&stocked, &report, &content).unwrap();
        // Both states satisfy two of use's five clauses and zero of other's
        // one clause. Existing withdrawal guidance is inactive with two goals.
        assert_eq!(
            initial_score.6,
            (0, Reverse(usize::MAX), 409, 0, Reverse(usize::MAX))
        );
        assert_eq!(
            stocked_score.6,
            (0, Reverse(usize::MAX), 409, 1, Reverse(3))
        );
        assert!(stocked_score > initial_score);
        assert_eq!(report.covered_definitions, BTreeSet::new());
    }

    #[test]
    fn recipe_work_hints_reject_missing_stock_overflow_closed_guards_and_consumed_outputs() {
        for case in [
            "missing",
            "overflow",
            "closed",
            "consumed",
            "negative",
            "blocked_walk",
        ] {
            let mut draft = recipe_work_fixture();
            draft.character_presets[0].character.inventory =
                BTreeMap::from([("test.part".to_owned(), 1)]);
            match case {
                "missing" => draft.character_presets[0].character.inventory.clear(),
                "overflow" => {
                    draft.character_presets[0]
                        .character
                        .inventory
                        .insert("test.product".to_owned(), u32::MAX);
                }
                "closed" => {
                    draft.actions[1].condition = Condition::WorldFlag {
                        flag: "test.unopened".to_owned(),
                    }
                }
                "blocked_walk" => {
                    draft.actions[0].condition = Condition::WorldFlag {
                        flag: "test.unopened".to_owned(),
                    }
                }
                "consumed" => draft.actions[1].effects.push(Effect::ApplyRecipe {
                    recipe: "test.consume".to_owned(),
                }),
                "negative" => draft.actions[1].effects.push(Effect::SetWorldFlag {
                    flag: "test.closed".to_owned(),
                    value: true,
                }),
                _ => unreachable!(),
            }
            let content = forge_content::compile(draft).unwrap();
            let state = content.new_game("ilyan", 71).unwrap();
            let hints = recipe_hints(&state, &content);
            assert_eq!(
                item_acquisition_distance(content.action("test.use").unwrap(), &hints, &content)
                    .unwrap(),
                None,
                "accepted {case} recipe work"
            );
        }
        // A local producer whose explicit guard is true still needs kernel
        // preflight; direct recipe underflow must not become local work.
        let mut draft = recipe_work_fixture();
        draft.actions[1].locations = vec!["lowsail_market".to_owned()];
        draft.actions[1].condition = Condition::Always;
        let content = forge_content::compile(draft).unwrap();
        let state = content.new_game("ilyan", 71).unwrap();
        assert!(recipe_hints(&state, &content).is_empty());
        assert!(
            enumerate_legal_actions(&state, &content)
                .unwrap()
                .iter()
                .all(|action| action.definition_id != "test.fit")
        );
    }

    #[test]
    fn recipe_work_hints_exclude_unsupported_programs_and_never_supply_target_knowledge() {
        for effect in [
            Effect::RandomChance {
                success_percent: 50,
                on_success: Box::new(Effect::Noop),
                on_failure: Box::new(Effect::Noop),
            },
            Effect::MoveCharacter {
                location: StringRef::Literal("lowsail_market".to_owned()),
            },
            Effect::AdjustResource {
                resource: "coin".to_owned(),
                amount: 1,
            },
            Effect::TransferNpcItemToCharacter {
                npc: StringRef::Literal("sava_rusk".to_owned()),
                item: "test.part".to_owned(),
                count: 1,
            },
            Effect::TransferStorageItemToCharacter {
                storage: "test.reserve".to_owned(),
                item: "test.part".to_owned(),
                count: 1,
            },
            Effect::TransferCharacterItemToStorage {
                storage: "test.reserve".to_owned(),
                item: "test.part".to_owned(),
                count: 1,
            },
            Effect::ScheduleEvent {
                event: "test.later".to_owned(),
            },
        ] {
            let mut draft = recipe_work_fixture();
            draft.storages.push(forge_kernel::StorageDefinition {
                id: "test.reserve".to_owned(),
                name: "Reserve".to_owned(),
                location: "lowsail.docks".to_owned(),
                inventory: BTreeMap::from([("test.part".to_owned(), 1)]),
            });
            draft
                .deferred_events
                .push(forge_kernel::DeferredEventDefinition {
                    id: "test.later".to_owned(),
                    delay: 1,
                    event_kind: "test.later".to_owned(),
                    label: "Later".to_owned(),
                    result: "Done.".to_owned(),
                    condition: Condition::Always,
                    effects: vec![Effect::SetWorldFlag {
                        flag: "test.later".to_owned(),
                        value: true,
                    }],
                });
            draft.character_presets[0].character.inventory =
                BTreeMap::from([("test.part".to_owned(), 1)]);
            draft.actions[1].effects.push(effect);
            let content = forge_content::compile(draft).unwrap();
            let state = content.new_game("ilyan", 71).unwrap();
            assert!(recipe_hints(&state, &content).is_empty());
        }
        for condition in [
            Condition::CharacterHasDeed {
                deed_id: "test.made".to_owned(),
            },
            Condition::NpcKnows {
                npc: "sava_rusk".to_owned(),
                knowledge_id: "test.receipt".to_owned(),
            },
            Condition::Not {
                condition: Box::new(Condition::HasItem {
                    item: "test.product".to_owned(),
                    count: 1,
                }),
            },
        ] {
            let mut draft = recipe_work_fixture();
            draft.character_presets[0].character.inventory =
                BTreeMap::from([("test.part".to_owned(), 1)]);
            if let Condition::All { conditions } = &mut draft.actions[2].condition {
                conditions.push(condition);
            }
            let content = forge_content::compile(draft).unwrap();
            let state = content.new_game("ilyan", 71).unwrap();
            assert_eq!(
                item_acquisition_distance(
                    content.action("test.use").unwrap(),
                    &recipe_hints(&state, &content),
                    &content
                )
                .unwrap(),
                None
            );
        }
    }

    fn acquisition_hints(state: &GameState, content: &CompiledContent) -> Vec<(usize, GameState)> {
        let mut items = BTreeSet::new();
        collect_condition_items(&content.action("test.use").unwrap().condition, &mut items);
        potential_item_acquisition_projections(state, content, &items).unwrap()
    }

    #[test]
    fn item_acquisition_hints_require_canonical_walk_withdraw_return_and_complete_use_catalog() {
        for stored in [false, true] {
            let content = forge_content::compile(acquisition_fixture(stored)).unwrap();
            let initial = content.new_game("ilyan", 71).unwrap();
            let before = initial.clone();
            let hints = acquisition_hints(&initial, &content);
            assert_eq!(hints.len(), 1);
            assert_eq!(
                item_acquisition_distance(content.action("test.use").unwrap(), &hints, &content)
                    .unwrap(),
                Some(3)
            );
            let projected = &hints[0].1;
            assert_eq!(projected.character.inventory["test.part"], 1);
            assert_eq!(projected.character.deeds, before.character.deeds);
            assert_eq!(projected.character.knowledge, before.character.knowledge);
            assert_eq!(projected.world.npcs, before.world.npcs);
            assert_eq!(projected.world.storages, before.world.storages);
            assert_eq!(projected.entropy, before.entropy);
            assert_eq!(projected.event_log, before.event_log);
            assert_eq!(projected.world.time, before.world.time);
            assert_eq!(initial, before);
            assert!(
                enumerate_legal_actions(&initial, &content)
                    .unwrap()
                    .iter()
                    .all(|action| action.definition_id != "test.use")
            );
            let report = crawl_targets(
                &content,
                CrawlBudget {
                    max_depth: 4,
                    max_expanded_states: 4,
                    max_discovered_frontiers: 16,
                    max_action_executions: 6,
                    catalog_page_size: 1,
                },
                BTreeSet::from(["test.use".to_owned()]),
            )
            .unwrap();
            assert_eq!(report.expanded_states, 4);
            assert_eq!(
                report.successful_actions, 6,
                "all four complete catalogs must execute"
            );
            assert_eq!(
                report.covered_definitions,
                BTreeSet::from([
                    "test.walk".to_owned(),
                    "test.take".to_owned(),
                    "test.use".to_owned()
                ])
            );
            let mut session = Session::new_game("ilyan", 71, &content).unwrap();
            for id in ["test.walk", "test.take", "test.walk", "test.use"] {
                let action = enumerate_legal_actions(session.state(), &content)
                    .unwrap()
                    .into_iter()
                    .find(|action| action.definition_id == id)
                    .unwrap();
                session.record(&action).unwrap();
            }
            assert!(session.state().character.inventory.is_empty());
            assert_eq!(session.state().entropy.cursor, 0);
            assert!(session.state().world.flags.contains("test.done"));
            assert_eq!(
                forge_replay::verify(session.trace(), &content).unwrap(),
                *session.state()
            );
        }
    }

    #[test]
    fn item_acquisition_hints_preserve_literal_multitarget_priorities_until_the_last_goal() {
        let mut source = acquisition_fixture(false);
        source.character_presets[0].character.deeds.clear();
        source.character_presets[0].character.knowledge.clear();
        source.character_presets[0].character.resources.clear();
        let mut other = source.actions[2].clone();
        other.id = "test.other".to_owned();
        other.condition = Condition::WorldFlag {
            flag: "test.other_ready".to_owned(),
        };
        source.actions.push(other);
        let partial_flags = ["test.one", "test.two", "test.three", "test.four"];
        source.actions[2].condition = Condition::Any {
            conditions: vec![
                source.actions[2].condition.clone(),
                Condition::All {
                    conditions: partial_flags
                        .iter()
                        .map(|flag| Condition::WorldFlag {
                            flag: (*flag).to_owned(),
                        })
                        .collect(),
                },
            ],
        };
        let mut close_source = source.actions[2].clone();
        close_source.id = "test.close_source".to_owned();
        close_source.condition = Condition::NpcAtLocation {
            npc: "sava_rusk".to_owned(),
            location: "lowsail.docks".to_owned(),
        };
        close_source.effects = vec![Effect::MoveNpc {
            npc: StringRef::Literal("sava_rusk".to_owned()),
            location: StringRef::Literal("lowsail_market".to_owned()),
        }];
        close_source
            .effects
            .extend(partial_flags[..3].iter().map(|flag| Effect::SetWorldFlag {
                flag: (*flag).to_owned(),
                value: true,
            }));
        close_source.effects.push(Effect::AdvanceTime { ticks: 1 });
        source.actions.push(close_source);
        let content = forge_content::compile(source).unwrap();
        let initial = content.new_game("ilyan", 71).unwrap();
        let walk = enumerate_legal_actions(&initial, &content)
            .unwrap()
            .into_iter()
            .find(|action| action.definition_id == "test.walk")
            .unwrap();
        let moved = step(&initial, &walk, &content, &initial.entropy)
            .unwrap()
            .into_state();
        let start_frontier = Frontier {
            state: initial,
            depth: 0,
            ordinal: 0,
            used_actions: BTreeSet::new(),
        };
        let stock_frontier = Frontier {
            state: moved,
            depth: 1,
            ordinal: 1,
            used_actions: BTreeSet::new(),
        };
        let mut report = CrawlReport {
            verifier_id: crate::VERIFIER_ID.to_owned(),
            build_id: content.build_id().to_owned(),
            budget: CrawlBudget::default(),
            expanded_states: 0,
            discovered_frontiers: 0,
            successful_actions: 0,
            max_legal_actions: 0,
            reached_locations: BTreeSet::from([
                "lowsail_market".to_owned(),
                "lowsail.docks".to_owned(),
            ]),
            covered_definitions: BTreeSet::new(),
            advertised_definitions: content.actions().map(|(id, _)| id.clone()).collect(),
            required_definitions: BTreeSet::from(["test.use".to_owned(), "test.other".to_owned()]),
            starting_sessions: Vec::new(),
            execution_receipt: "score fixture only".to_owned(),
        };
        // Hand-computed pre-acquisition priorities: use has one of two
        // clauses, other has zero of one; neither goal nor flag work is ready.
        let initial_expected = (
            0,
            0,
            0,
            Reverse(usize::MAX),
            0,
            Reverse(u64::MAX),
            (0, Reverse(usize::MAX), 512, 0, Reverse(usize::MAX)),
            512,
            Reverse(0),
            0,
            false,
            Reverse(0),
        );
        let moved_expected = (
            0,
            0,
            0,
            Reverse(usize::MAX),
            0,
            Reverse(u64::MAX),
            (0, Reverse(usize::MAX), 512, 0, Reverse(usize::MAX)),
            512,
            Reverse(2),
            0,
            false,
            Reverse(1),
        );
        assert_eq!(
            frontier_score(&start_frontier, &report, &content).unwrap(),
            initial_expected
        );
        assert_eq!(
            frontier_score(&stock_frontier, &report, &content).unwrap(),
            moved_expected
        );
        assert!(
            initial_expected > moved_expected,
            "multi-goal search retains the earlier shallow ordering"
        );
        report.covered_definitions.insert("test.other".to_owned());
        let initial_last = frontier_score(&start_frontier, &report, &content).unwrap();
        let moved_last = frontier_score(&stock_frontier, &report, &content).unwrap();
        assert_eq!(initial_last.6, (1, Reverse(3), 512, 0, Reverse(usize::MAX)));
        assert_eq!(moved_last.6, (1, Reverse(2), 512, 0, Reverse(usize::MAX)));
        assert!(
            moved_last > initial_last,
            "only the last goal justifies approaching the actual supplier"
        );
        let close = enumerate_legal_actions(&start_frontier.state, &content)
            .unwrap()
            .into_iter()
            .find(|action| action.definition_id == "test.close_source")
            .unwrap();
        let closed = step(
            &start_frontier.state,
            &close,
            &content,
            &start_frontier.state.entropy,
        )
        .unwrap()
        .into_state();
        let closed_frontier = Frontier {
            state: closed,
            depth: 1,
            ordinal: 2,
            used_actions: BTreeSet::new(),
        };
        let closed_score = frontier_score(&closed_frontier, &report, &content).unwrap();
        assert_eq!(closed_score.6.0, 0, "the actual custodian left the source");
        assert_eq!(closed_score.6.2, 768, "three of four clauses are satisfied");
        assert!(initial_last.6.2 < closed_score.6.2);
        assert!(
            initial_last > closed_score,
            "last-goal acquisition precedes infeasible raw progress"
        );
        // The separate canonical acquisition fixture still requires every
        // walk, withdrawal, return, and use catalog to claim any coverage.
    }

    #[test]
    fn item_acquisition_hints_reject_absent_empty_remote_and_overdrawn_sources_and_custodians() {
        for stored in [false, true] {
            let source = acquisition_fixture(stored);
            let content = forge_content::compile(source.clone()).unwrap();
            let initial = content.new_game("ilyan", 71).unwrap();
            for defect in ["absent", "empty", "wrong location", "capacity"] {
                let mut state = initial.clone();
                match defect {
                    "absent" if stored => {
                        state.world.storages.clear();
                    }
                    "absent" => {
                        state.world.npcs.clear();
                    }
                    "empty" if stored => {
                        state
                            .world
                            .storages
                            .get_mut("test.reserve")
                            .unwrap()
                            .inventory
                            .clear();
                    }
                    "empty" => {
                        state
                            .world
                            .npcs
                            .get_mut("sava_rusk")
                            .unwrap()
                            .inventory
                            .clear();
                    }
                    "wrong location" => {
                        state.world.npcs.get_mut("sava_rusk").unwrap().location =
                            "lowsail_market".to_owned()
                    }
                    "capacity" => {
                        state
                            .character
                            .inventory
                            .insert("test.part".to_owned(), u32::MAX);
                    }
                    _ => unreachable!(),
                }
                assert!(
                    acquisition_hints(&state, &content).is_empty(),
                    "accepted {stored}/{defect}"
                );
            }
            let mut overdrawn = source.clone();
            let transfer = overdrawn.actions[1].effects[0].clone();
            overdrawn.actions[1].effects.insert(1, transfer);
            let content = forge_content::compile(overdrawn).unwrap();
            let initial = content.new_game("ilyan", 71).unwrap();
            assert!(
                acquisition_hints(&initial, &content).is_empty(),
                "whole program cannot withdraw the same sole item twice"
            );
            let walk = enumerate_legal_actions(&initial, &content)
                .unwrap()
                .remove(0);
            let local = step(&initial, &walk, &content, &initial.entropy)
                .unwrap()
                .into_state();
            assert!(
                content
                    .action("test.take")
                    .unwrap()
                    .condition
                    .evaluate(&local)
            );
            assert!(
                enumerate_legal_actions(&local, &content)
                    .unwrap()
                    .iter()
                    .all(|action| action.definition_id != "test.take")
            );
            assert!(
                acquisition_hints(&local, &content).is_empty(),
                "local producer must be canonical catalog legal"
            );
            if stored {
                let mut wrong = source;
                wrong.storages[0].location = "lowsail_market".to_owned();
                let content = forge_content::compile(wrong).unwrap();
                assert!(
                    acquisition_hints(&content.new_game("ilyan", 71).unwrap(), &content).is_empty()
                );
            }
        }
    }

    #[test]
    fn item_acquisition_hints_exclude_rich_programs_and_never_invent_deeds_or_npc_knowledge() {
        for extra in [
            Effect::RandomChance {
                success_percent: 100,
                on_success: Box::new(Effect::Noop),
                on_failure: Box::new(Effect::Noop),
            },
            Effect::ApplyRecipe {
                recipe: "test.install".to_owned(),
            },
            Effect::TransferCharacterItemToStorage {
                storage: "test.reserve".to_owned(),
                item: "test.part".to_owned(),
                count: 1,
            },
            Effect::MoveCharacter {
                location: StringRef::Literal("lowsail_market".to_owned()),
            },
            Effect::AdjustResource {
                resource: "coin".to_owned(),
                amount: -1,
            },
        ] {
            let mut source = acquisition_fixture(true);
            source.actions[1].effects.push(extra);
            let content = forge_content::compile(source).unwrap();
            assert!(
                acquisition_hints(&content.new_game("ilyan", 71).unwrap(), &content).is_empty()
            );
        }
        for prerequisite in [
            Condition::CharacterHasDeed {
                deed_id: "test.received".to_owned(),
            },
            Condition::NpcKnows {
                npc: "sava_rusk".to_owned(),
                knowledge_id: "test.receipt".to_owned(),
            },
            Condition::Not {
                condition: Box::new(Condition::WorldFlag {
                    flag: "test.taken".to_owned(),
                }),
            },
        ] {
            let mut source = acquisition_fixture(false);
            if let Condition::All { conditions } = &mut source.actions[2].condition {
                conditions.push(prerequisite);
            }
            let content = forge_content::compile(source).unwrap();
            let initial = content.new_game("ilyan", 71).unwrap();
            assert_eq!(
                item_acquisition_distance(
                    content.action("test.use").unwrap(),
                    &acquisition_hints(&initial, &content),
                    &content
                )
                .unwrap(),
                None
            );
        }
        let mut blocked_walk = acquisition_fixture(false);
        blocked_walk.actions[0].condition = Condition::WorldFlag {
            flag: "test.unopened_route".to_owned(),
        };
        let content = forge_content::compile(blocked_walk).unwrap();
        assert!(acquisition_hints(&content.new_game("ilyan", 71).unwrap(), &content).is_empty());
    }

    #[test]
    fn flag_producer_hints_guide_a_real_walk_and_require_complete_canonical_execution() {
        let content = forge_content::compile(flag_producer_fixture()).unwrap();
        let initial = content.new_game("ilyan", 71).unwrap();
        let before = initial.clone();
        let hints = fixture_producer_hints(&initial, &content).unwrap();
        let goal = content.action("test.use").unwrap();
        assert_eq!(hints.len(), 1);
        assert_eq!(
            hints[0].0, 2,
            "one actual walk precedes the possible installation"
        );
        assert!(goal.condition.evaluate(&hints[0].1));
        assert_eq!(initial, before);
        assert_eq!(hints[0].1.event_log, before.event_log);
        assert_eq!(hints[0].1.entropy, before.entropy);
        assert_eq!(hints[0].1.character.inventory, before.character.inventory);
        assert_eq!(hints[0].1.world.npcs, before.world.npcs);
        assert_eq!(hints[0].1.world.time, before.world.time);
        assert!(
            !enumerate_legal_actions(&initial, &content)
                .unwrap()
                .iter()
                .any(|action| action.definition_id == "test.use")
        );

        let report = crawl_targets(
            &content,
            CrawlBudget {
                max_depth: 3,
                max_expanded_states: 3,
                max_discovered_frontiers: 16,
                max_action_executions: 5,
                catalog_page_size: 1,
            },
            BTreeSet::from(["test.use".to_owned()]),
        )
        .unwrap();
        assert_eq!(report.expanded_states, 3);
        assert_eq!(
            report.successful_actions, 5,
            "both complete two-action workbench catalogs must execute"
        );
        assert_eq!(
            report.covered_definitions,
            BTreeSet::from([
                "test.walk".to_owned(),
                "test.fit".to_owned(),
                "test.use".to_owned()
            ])
        );

        let mut session = Session::new_game("ilyan", 71, &content).unwrap();
        for id in ["test.walk", "test.fit", "test.use"] {
            let action = enumerate_legal_actions(session.state(), &content)
                .unwrap()
                .into_iter()
                .find(|action| action.definition_id == id)
                .unwrap();
            session.record(&action).unwrap();
        }
        assert!(
            !session
                .state()
                .character
                .inventory
                .contains_key("test.part")
        );
        assert!(session.state().world.flags.contains("test.done"));
        assert_eq!(
            forge_replay::verify(session.trace(), &content).unwrap(),
            *session.state()
        );
    }

    #[test]
    fn flag_producer_hints_reject_false_guards_negative_targets_randomness_and_unavailable_stock() {
        let original = flag_producer_fixture();
        let content = forge_content::compile(original.clone()).unwrap();
        let mut no_part = content.new_game("ilyan", 71).unwrap();
        no_part.character.inventory.clear();
        assert!(
            fixture_producer_hints(&no_part, &content)
                .unwrap()
                .is_empty()
        );

        let mut closes = original.clone();
        closes.actions[1].effects.push(Effect::SetWorldFlag {
            flag: "test.closed".to_owned(),
            value: true,
        });
        let content = forge_content::compile(closes).unwrap();
        let initial = content.new_game("ilyan", 71).unwrap();
        assert!(
            !fixture_producer_hints(&initial, &content)
                .unwrap()
                .iter()
                .any(|(_, hint)| content.action("test.use").unwrap().condition.evaluate(hint)),
            "all ordered flag writes, including negative prerequisites, matter"
        );

        let mut random = original.clone();
        random.actions[1].effects.push(Effect::RandomChance {
            success_percent: 100,
            on_success: Box::new(Effect::Noop),
            on_failure: Box::new(Effect::Noop),
        });
        let content = forge_content::compile(random).unwrap();
        assert!(
            fixture_producer_hints(&content.new_game("ilyan", 71).unwrap(), &content)
                .unwrap()
                .is_empty()
        );

        let mut unavailable = original;
        unavailable.recipes[0]
            .inputs
            .insert("test.part".to_owned(), 2);
        let content = forge_content::compile(unavailable).unwrap();
        let initial = content.new_game("ilyan", 71).unwrap();
        let walk = enumerate_legal_actions(&initial, &content)
            .unwrap()
            .remove(0);
        let local = step(&initial, &walk, &content, &initial.entropy)
            .unwrap()
            .into_state();
        assert!(
            content
                .action("test.fit")
                .unwrap()
                .condition
                .evaluate(&local)
        );
        assert!(
            fixture_producer_hints(&local, &content).unwrap().is_empty(),
            "actual local catalog must reject a recipe whose input bound is unavailable"
        );
    }

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
        assert_eq!(report.advertised_definitions.len(), 100);
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
