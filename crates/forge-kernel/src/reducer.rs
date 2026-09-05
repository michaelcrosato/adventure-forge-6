use crate::content::{CompiledContent, Effect, ParameterDomain, StringRef};
use crate::hash::{HashError, sha256_json};
use crate::model::{ActionId, Event, EventKind, GameState, Knowledge, KnowledgeProvenance, Memory};
use crate::{EntropyDraw, EntropyError, EntropyState};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelError {
    WrongBuild { expected: String, actual: String },
    StaleAction { expected: String, actual: String },
    UnknownAction(String),
    IllegalAction(String),
    InvalidAction(String),
    InvalidContent(String),
    InvalidState(String),
    ResourceExhausted(String),
    EntropyMismatch,
    Entropy(EntropyError),
    Hash(HashError),
}

impl Display for KernelError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongBuild { expected, actual } => {
                write!(f, "wrong build: expected {expected}, got {actual}")
            }
            Self::StaleAction { expected, actual } => {
                write!(f, "stale action: expected state {expected}, got {actual}")
            }
            Self::UnknownAction(action) => write!(f, "unknown action {action}"),
            Self::IllegalAction(action) => write!(f, "illegal action {action}"),
            Self::InvalidAction(message)
            | Self::InvalidContent(message)
            | Self::InvalidState(message)
            | Self::ResourceExhausted(message) => f.write_str(message),
            Self::EntropyMismatch => f.write_str("explicit entropy does not match state"),
            Self::Entropy(error) => Display::fmt(error, f),
            Self::Hash(error) => Display::fmt(error, f),
        }
    }
}

impl std::error::Error for KernelError {}

impl From<HashError> for KernelError {
    fn from(value: HashError) -> Self {
        Self::Hash(value)
    }
}

impl From<EntropyError> for KernelError {
    fn from(value: EntropyError) -> Self {
        Self::Entropy(value)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CanonicalAction {
    pub action_id: ActionId,
    pub build_id: String,
    pub pre_state_id: String,
    pub definition_id: String,
    #[serde(default)]
    pub parameters: BTreeMap<String, String>,
}

#[derive(Serialize)]
struct ActionIdentity<'a> {
    build_id: &'a str,
    pre_state_id: &'a str,
    definition_id: &'a str,
    parameters: &'a BTreeMap<String, String>,
}

impl CanonicalAction {
    pub fn new(
        build_id: impl Into<String>,
        pre_state_id: impl Into<String>,
        definition_id: impl Into<String>,
        parameters: BTreeMap<String, String>,
    ) -> Self {
        let build_id = build_id.into();
        let pre_state_id = pre_state_id.into();
        let definition_id = definition_id.into();
        let identity = ActionIdentity {
            build_id: &build_id,
            pre_state_id: &pre_state_id,
            definition_id: &definition_id,
            parameters: &parameters,
        };
        let action_id = sha256_json(&identity).expect("canonical action must be serializable");
        Self {
            action_id,
            build_id,
            pre_state_id,
            definition_id,
            parameters,
        }
    }

    pub fn recomputed_id(&self) -> ActionId {
        let identity = ActionIdentity {
            build_id: &self.build_id,
            pre_state_id: &self.pre_state_id,
            definition_id: &self.definition_id,
            parameters: &self.parameters,
        };
        sha256_json(&identity).expect("canonical action must be serializable")
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Transition {
    pre_state_id: String,
    action: CanonicalAction,
    events: Vec<Event>,
    entropy_before: EntropyState,
    entropy_draws: Vec<EntropyDraw>,
    entropy_after: EntropyState,
    post_state_id: String,
    state: GameState,
}

impl Transition {
    pub fn pre_state_id(&self) -> &str {
        &self.pre_state_id
    }

    pub fn action(&self) -> &CanonicalAction {
        &self.action
    }

    pub fn events(&self) -> &[Event] {
        &self.events
    }

    pub fn entropy_before(&self) -> &EntropyState {
        &self.entropy_before
    }

    pub fn entropy_draws(&self) -> &[EntropyDraw] {
        &self.entropy_draws
    }

    pub fn entropy_after(&self) -> &EntropyState {
        &self.entropy_after
    }

    pub fn post_state_id(&self) -> &str {
        &self.post_state_id
    }

    pub fn state(&self) -> &GameState {
        &self.state
    }

    pub fn into_state(self) -> GameState {
        self.state
    }
}

struct ActionCandidates<'a> {
    definition: &'a crate::ActionDefinition,
    domains: Vec<Vec<String>>,
    combinations: usize,
}

/// Enumerate every legal action.  There is intentionally no action count
/// limit: dynamic parameter domains are expanded into a complete Cartesian
/// product and then stably sorted by semantic identity. Action hashes bind the
/// current state, so using them as presentation order would reshuffle otherwise
/// unchanged choices after every turn.
pub fn enumerate_legal_actions(
    state: &GameState,
    content: &CompiledContent,
) -> Result<Vec<CanonicalAction>, KernelError> {
    ensure_content_and_state(state, content)?;
    let pre_state_id = state.state_id();
    let mut candidates = Vec::new();
    try_reserve(
        &mut candidates,
        content.actions().size_hint().0,
        "action candidate list",
    )?;
    let mut raw_combinations = 0usize;

    for (_, definition) in content.actions() {
        if !definition.locations.is_empty()
            && !definition
                .locations
                .iter()
                .any(|location| location == &state.world.current_location)
        {
            continue;
        }
        if !definition.condition.evaluate(state) {
            continue;
        }

        let mut names = BTreeSet::new();
        let mut domains = Vec::new();
        try_reserve(
            &mut domains,
            definition.parameters.len(),
            "parameter domain list",
        )?;
        for parameter in &definition.parameters {
            if parameter.name.is_empty() || !names.insert(parameter.name.clone()) {
                return Err(KernelError::InvalidContent(format!(
                    "action {} has duplicate or empty parameter name",
                    definition.id
                )));
            }
            domains.push(domain_values(&parameter.domain, state, content)?);
        }

        let combinations =
            checked_combination_count(domains.iter().map(|domain| domain.len()), &definition.id)?;
        raw_combinations = raw_combinations.checked_add(combinations).ok_or_else(|| {
            KernelError::ResourceExhausted(format!(
                "legal action count overflow after action {}",
                definition.id
            ))
        })?;
        candidates.push(ActionCandidates {
            definition,
            domains,
            combinations,
        });
    }

    let mut actions = Vec::new();
    try_reserve(&mut actions, raw_combinations, "legal action list")?;
    let mut visited_combinations = 0usize;
    for candidate in candidates {
        let visited = append_parameter_combinations(
            candidate.definition,
            &candidate.domains,
            candidate.combinations,
            content,
            &pre_state_id,
            state,
            &mut actions,
        )?;
        visited_combinations = visited_combinations.checked_add(visited).ok_or_else(|| {
            KernelError::ResourceExhausted("visited action combination count overflow".to_owned())
        })?;
    }
    if visited_combinations != raw_combinations {
        return Err(KernelError::InvalidContent(format!(
            "raw action combination count mismatch: expected {raw_combinations}, visited {visited_combinations}"
        )));
    }
    actions.sort_by(canonical_action_order);
    let mut action_ids = Vec::new();
    try_reserve(&mut action_ids, actions.len(), "action identity list")?;
    action_ids.extend(actions.iter().map(|action| action.action_id.as_str()));
    action_ids.sort_unstable();
    if action_ids.windows(2).any(|window| window[0] == window[1]) {
        return Err(KernelError::InvalidContent(
            "legal action identity collision".to_owned(),
        ));
    }
    Ok(actions)
}

fn canonical_action_order(left: &CanonicalAction, right: &CanonicalAction) -> Ordering {
    left.definition_id
        .cmp(&right.definition_id)
        .then_with(|| left.parameters.cmp(&right.parameters))
        .then_with(|| left.action_id.cmp(&right.action_id))
}

fn ensure_content_and_state(
    state: &GameState,
    content: &CompiledContent,
) -> Result<(), KernelError> {
    if !content.has_valid_build_id() {
        return Err(KernelError::InvalidContent(
            "compiled content build identity is invalid".to_owned(),
        ));
    }
    if state.build_id != content.build_id() {
        return Err(KernelError::WrongBuild {
            expected: content.build_id().to_owned(),
            actual: state.build_id.clone(),
        });
    }
    state.entropy.validate()?;
    if let Err(error) = content.validate_state(state) {
        return Err(KernelError::InvalidState(error.to_string()));
    }
    Ok(())
}

fn domain_values(
    domain: &ParameterDomain,
    state: &GameState,
    content: &CompiledContent,
) -> Result<Vec<String>, KernelError> {
    let mut values = Vec::new();
    match domain {
        ParameterDomain::Values(domain_values) => {
            try_reserve(&mut values, domain_values.len(), "literal parameter values")?;
            values.extend(domain_values.iter().cloned());
        }
        ParameterDomain::InventoryItems => {
            try_reserve(
                &mut values,
                state.character.inventory.len(),
                "inventory parameter values",
            )?;
            values.extend(
                state
                    .character
                    .inventory
                    .iter()
                    .filter(|(_, count)| **count > 0)
                    .map(|(item, _)| item.clone()),
            );
        }
        ParameterDomain::NpcsAtCurrentLocation => {
            try_reserve(&mut values, state.world.npcs.len(), "NPC parameter values")?;
            values.extend(
                state
                    .world
                    .npcs
                    .values()
                    .filter(|npc| npc.location == state.world.current_location)
                    .map(|npc| npc.id.clone()),
            );
        }
        ParameterDomain::LocationsAdjacent => {
            let location = content
                .location(&state.world.current_location)
                .ok_or_else(|| {
                    KernelError::InvalidState("current location is not compiled".to_owned())
                })?;
            try_reserve(
                &mut values,
                location.exits.len(),
                "adjacent location values",
            )?;
            values.extend(
                location
                    .exits
                    .iter()
                    .filter(|location| content.has_location(location))
                    .cloned(),
            );
        }
    }
    values.sort();
    values.dedup();
    Ok(values)
}

fn checked_combination_count<I>(
    domain_lengths: I,
    definition_id: &str,
) -> Result<usize, KernelError>
where
    I: IntoIterator<Item = usize>,
{
    let mut count = 1usize;
    for length in domain_lengths {
        count = count.checked_mul(length).ok_or_else(|| {
            KernelError::ResourceExhausted(format!(
                "parameter combination count overflow for action {definition_id}"
            ))
        })?;
        if count == 0 {
            break;
        }
    }
    Ok(count)
}

fn append_parameter_combinations(
    definition: &crate::ActionDefinition,
    domains: &[Vec<String>],
    expected: usize,
    content: &CompiledContent,
    pre_state_id: &str,
    state: &GameState,
    output: &mut Vec<CanonicalAction>,
) -> Result<usize, KernelError> {
    if expected == 0 {
        return Ok(0);
    }

    let mut indexes = Vec::new();
    try_reserve(&mut indexes, domains.len(), "parameter enumeration indexes")?;
    indexes.extend(std::iter::repeat_n(0usize, domains.len()));

    let mut visited = 0usize;
    loop {
        let mut parameters = BTreeMap::new();
        for (spec, (domain, index)) in definition
            .parameters
            .iter()
            .zip(domains.iter().zip(indexes.iter()))
        {
            let value = domain.get(*index).ok_or_else(|| {
                KernelError::InvalidContent(format!(
                    "parameter index out of range for action {}",
                    definition.id
                ))
            })?;
            parameters.insert(spec.name.clone(), value.clone());
        }
        visited = visited.checked_add(1).ok_or_else(|| {
            KernelError::ResourceExhausted(format!(
                "enumerated action count overflow for action {}",
                definition.id
            ))
        })?;
        if inventory_candidate_available(definition, &parameters, state, content)? {
            output.push(CanonicalAction::new(
                content.build_id().to_owned(),
                pre_state_id.to_owned(),
                definition.id.clone(),
                parameters,
            ));
        }

        let mut advanced = false;
        for index in (0..indexes.len()).rev() {
            let next = indexes[index].checked_add(1).ok_or_else(|| {
                KernelError::ResourceExhausted(format!(
                    "parameter index overflow for action {}",
                    definition.id
                ))
            })?;
            if next < domains[index].len() {
                indexes[index] = next;
                advanced = true;
                break;
            }
            indexes[index] = 0;
        }
        if !advanced {
            break;
        }
    }

    if visited != expected {
        return Err(KernelError::InvalidContent(format!(
            "parameter enumeration count mismatch for action {}: expected {expected}, visited {visited}",
            definition.id
        )));
    }
    Ok(visited)
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum InventoryOwner {
    Character,
    Npc(String),
}

#[derive(Clone, Copy)]
struct InventoryRange {
    minimum: u32,
    maximum: u32,
}

/// Every inventory operation changes one item by a fixed integer amount.
/// Pointwise extrema therefore suffice to prove stock and capacity for every
/// reachable random path, including operations after a branch rejoins. This
/// does not inspect entropy or expand the number of possible random paths.
#[derive(Clone)]
struct InventoryBounds<'a> {
    initial: &'a GameState,
    counts: BTreeMap<(InventoryOwner, String), InventoryRange>,
    latest_time: Option<u64>,
    scheduled: BTreeSet<String>,
}

impl<'a> InventoryBounds<'a> {
    fn new(initial: &'a GameState) -> Self {
        Self {
            initial,
            counts: BTreeMap::new(),
            latest_time: Some(initial.world.time),
            scheduled: BTreeSet::new(),
        }
    }

    fn count(&self, owner: &InventoryOwner, item: &str) -> InventoryRange {
        let key = (owner.clone(), item.to_owned());
        if let Some(range) = self.counts.get(&key) {
            return *range;
        }
        let inventory = match owner {
            InventoryOwner::Character => Some(&self.initial.character.inventory),
            InventoryOwner::Npc(npc) => self.initial.world.npcs.get(npc).map(|npc| &npc.inventory),
        };
        let count = inventory
            .and_then(|inventory| inventory.get(item))
            .copied()
            .unwrap_or_default();
        InventoryRange {
            minimum: count,
            maximum: count,
        }
    }

    fn consume(&mut self, owner: InventoryOwner, item: &str, count: u32) -> bool {
        let current = self.count(&owner, item);
        let Some(minimum) = current.minimum.checked_sub(count) else {
            return false;
        };
        let Some(maximum) = current.maximum.checked_sub(count) else {
            return false;
        };
        self.counts.insert(
            (owner, item.to_owned()),
            InventoryRange { minimum, maximum },
        );
        true
    }

    fn produce(&mut self, owner: InventoryOwner, item: &str, count: u32) -> bool {
        let current = self.count(&owner, item);
        let Some(minimum) = current.minimum.checked_add(count) else {
            return false;
        };
        let Some(maximum) = current.maximum.checked_add(count) else {
            return false;
        };
        self.counts.insert(
            (owner, item.to_owned()),
            InventoryRange { minimum, maximum },
        );
        true
    }

    fn merge(&mut self, other: Self) {
        self.latest_time = match (self.latest_time, other.latest_time) {
            (Some(left), Some(right)) => Some(left.max(right)),
            _ => None,
        };
        self.scheduled.extend(other.scheduled.iter().cloned());
        let keys: BTreeSet<_> = self
            .counts
            .keys()
            .chain(other.counts.keys())
            .cloned()
            .collect();
        for (owner, item) in keys {
            let left = self.count(&owner, &item);
            let right = other.count(&owner, &item);
            self.counts.insert(
                (owner, item),
                InventoryRange {
                    minimum: left.minimum.min(right.minimum),
                    maximum: left.maximum.max(right.maximum),
                },
            );
        }
    }
}

fn inventory_candidate_available(
    definition: &crate::ActionDefinition,
    parameters: &BTreeMap<String, String>,
    state: &GameState,
    content: &CompiledContent,
) -> Result<bool, KernelError> {
    let mut inventories = InventoryBounds::new(state);
    for effect in &definition.effects {
        if !inventory_effect_available(effect, parameters, content, &mut inventories)? {
            return Ok(false);
        }
    }
    if !inventories.scheduled.is_empty() && inventories.latest_time.is_none() {
        return Ok(false);
    }
    let latest_time = inventories.latest_time.unwrap_or(u64::MAX);
    let pending_recipe = state.world.scheduled_events.iter().any(|scheduled| {
        scheduled.due_time <= latest_time
            && content
                .deferred_event(&scheduled.id)
                .is_some_and(|event| effects_contain_recipe(&event.effects))
    });
    let new_recipe = inventories.scheduled.iter().any(|id| {
        content.deferred_event(id).is_some_and(|event| {
            effects_contain_recipe(&event.effects)
                && state
                    .world
                    .time
                    .checked_add(event.delay)
                    .is_some_and(|due| due <= latest_time)
        })
    });
    if pending_recipe || new_recipe {
        return preflight_candidate_available(definition, parameters, state, content);
    }
    Ok(true)
}

fn inventory_effect_available(
    effect: &Effect,
    parameters: &BTreeMap<String, String>,
    content: &CompiledContent,
    inventories: &mut InventoryBounds<'_>,
) -> Result<bool, KernelError> {
    match effect {
        Effect::TransferNpcItemToCharacter { npc, item, count } => {
            let source = resolve_ref(npc, parameters)?;
            Ok(
                inventories.consume(InventoryOwner::Npc(source), item, *count)
                    && inventories.produce(InventoryOwner::Character, item, *count),
            )
        }
        Effect::ApplyRecipe { recipe } => {
            let definition = content
                .recipe(recipe)
                .ok_or_else(|| KernelError::InvalidContent(format!("unknown recipe {recipe}")))?;
            for (item, count) in &definition.inputs {
                if !inventories.consume(InventoryOwner::Character, item, *count) {
                    return Ok(false);
                }
            }
            for (item, count) in &definition.outputs {
                if !inventories.produce(InventoryOwner::Character, item, *count) {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Effect::ScheduleEvent { event } => {
            let template = content.deferred_event(event).ok_or_else(|| {
                KernelError::InvalidContent(format!("unknown deferred event {event}"))
            })?;
            if event_already_used(inventories.initial, &[], event)
                || !inventories.scheduled.insert(event.clone())
            {
                return Ok(false);
            }
            Ok(inventories
                .latest_time
                .and_then(|time| time.checked_add(template.delay))
                .is_some())
        }
        Effect::AdvanceTime { ticks } => {
            inventories.latest_time = inventories
                .latest_time
                .and_then(|time| time.checked_add(*ticks));
            Ok(true)
        }
        Effect::RandomChance {
            success_percent,
            on_success,
            on_failure,
        } => match success_percent {
            0 => inventory_effect_available(on_failure, parameters, content, inventories),
            100 => inventory_effect_available(on_success, parameters, content, inventories),
            _ => {
                let mut success = inventories.clone();
                let mut failure = inventories.clone();
                if !inventory_effect_available(on_success, parameters, content, &mut success)?
                    || !inventory_effect_available(on_failure, parameters, content, &mut failure)?
                {
                    return Ok(false);
                }
                success.merge(failure);
                *inventories = success;
                Ok(true)
            }
        },
        Effect::Noop
        | Effect::SetFlag { .. }
        | Effect::SetWorldFlag { .. }
        | Effect::SetLocationFlag { .. }
        | Effect::AdjustResource { .. }
        | Effect::MoveCharacter { .. }
        | Effect::MoveNpc { .. }
        | Effect::AdjustNpcRelationship { .. }
        | Effect::AddNpcMemory { .. }
        | Effect::TeachNpc { .. }
        | Effect::AddCharacterDeed { .. } => Ok(true),
    }
}

fn effects_contain_recipe(effects: &[Effect]) -> bool {
    effects.iter().any(|effect| match effect {
        Effect::ApplyRecipe { .. } => true,
        Effect::RandomChance {
            success_percent,
            on_success,
            on_failure,
        } => {
            (*success_percent > 0 && effects_contain_recipe(std::slice::from_ref(on_success)))
                || (*success_percent < 100
                    && effects_contain_recipe(std::slice::from_ref(on_failure)))
        }
        _ => false,
    })
}

fn event_already_used(state: &GameState, staged_events: &[Event], id: &str) -> bool {
    state
        .world
        .scheduled_events
        .iter()
        .any(|event| event.id == id)
        || state
            .event_log
            .iter()
            .chain(staged_events)
            .any(|event| match &event.kind {
                EventKind::EventScheduled { event_id, .. }
                | EventKind::ScheduledEventResolved { event_id, .. } => event_id == id,
                _ => false,
            })
}

/// Timed recipe guards can correlate inventory with flags changed by random
/// actions. Explore these paths exactly, without drawing or reading entropy.
/// Equivalent states merge; work vectors grow fallibly and have no path cap.
fn preflight_candidate_available(
    definition: &crate::ActionDefinition,
    parameters: &BTreeMap<String, String>,
    state: &GameState,
    content: &CompiledContent,
) -> Result<bool, KernelError> {
    let mut paths = Vec::new();
    push_preflight_state(&mut paths, state.clone())?;
    if !preflight_effects(&mut paths, &definition.effects, parameters, content)? {
        return Ok(false);
    }
    for mut path in paths {
        let due_count = path
            .world
            .scheduled_events
            .partition_point(|event| event.due_time <= path.world.time);
        let mut due = std::mem::take(&mut path.world.scheduled_events);
        path.world.scheduled_events = due.split_off(due_count);
        let mut event_paths = Vec::new();
        push_preflight_state(&mut event_paths, path)?;
        for scheduled in due {
            let (condition, effects) = scheduled_program(&scheduled, content)?;
            let mut next = Vec::new();
            for event_path in event_paths {
                if condition.evaluate(&event_path) {
                    let mut applied = Vec::new();
                    push_preflight_state(&mut applied, event_path)?;
                    if !preflight_effects(&mut applied, effects, &BTreeMap::new(), content)? {
                        return Ok(false);
                    }
                    for result in applied {
                        push_preflight_state(&mut next, result)?;
                    }
                } else {
                    push_preflight_state(&mut next, event_path)?;
                }
            }
            event_paths = next;
        }
    }
    Ok(true)
}

fn preflight_effects(
    paths: &mut Vec<GameState>,
    effects: &[Effect],
    parameters: &BTreeMap<String, String>,
    content: &CompiledContent,
) -> Result<bool, KernelError> {
    for effect in effects {
        let mut next = Vec::new();
        for path in paths.drain(..) {
            if !preflight_effect(path, effect, parameters, content, &mut next)? {
                return Ok(false);
            }
        }
        *paths = next;
    }
    Ok(true)
}

fn preflight_effect(
    mut state: GameState,
    effect: &Effect,
    parameters: &BTreeMap<String, String>,
    content: &CompiledContent,
    output: &mut Vec<GameState>,
) -> Result<bool, KernelError> {
    if let Effect::RandomChance {
        success_percent,
        on_success,
        on_failure,
    } = effect
    {
        // Count required draws without computing their values. Different cursor
        // counts remain distinct because later exhaustion can affect legality.
        if state.entropy.cursor >= crate::MAX_ENTROPY_CURSOR {
            return Ok(false);
        }
        state.entropy.cursor += 1;
        return match success_percent {
            0 => preflight_effect(state, on_failure, parameters, content, output),
            100 => preflight_effect(state, on_success, parameters, content, output),
            _ => {
                if !preflight_effect(state.clone(), on_success, parameters, content, output)? {
                    return Ok(false);
                }
                preflight_effect(state, on_failure, parameters, content, output)
            }
        };
    }
    let mut entropy = state.entropy.clone();
    match apply_effect(
        &mut state,
        effect,
        parameters,
        content,
        &mut entropy,
        &mut Vec::new(),
        &mut Vec::new(),
    ) {
        Ok(()) => {
            push_preflight_state(output, state)?;
            Ok(true)
        }
        Err(error @ KernelError::ResourceExhausted(_)) => Err(error),
        Err(_) => Ok(false),
    }
}

fn push_preflight_state(paths: &mut Vec<GameState>, state: GameState) -> Result<(), KernelError> {
    // Emitted events are deliberately omitted: current conditions inspect only
    // state, and new one-shot identities remain in the staged pending queue.
    if !paths.contains(&state) {
        try_reserve(paths, 1, "timed recipe preflight paths")?;
        paths.push(state);
    }
    Ok(())
}

fn try_reserve<T>(
    values: &mut Vec<T>,
    additional: usize,
    context: &str,
) -> Result<(), KernelError> {
    values
        .try_reserve(additional)
        .map_err(|error| KernelError::ResourceExhausted(format!("{context}: {error:?}")))
}

pub fn legal_action_digest(actions: &[CanonicalAction]) -> Result<String, HashError> {
    let mut ids: Vec<&str> = actions
        .iter()
        .map(|action| action.action_id.as_str())
        .collect();
    ids.sort_unstable();
    sha256_json(&ids)
}

pub fn validate_action(
    state: &GameState,
    content: &CompiledContent,
    action: &CanonicalAction,
) -> Result<(), KernelError> {
    ensure_content_and_state(state, content)?;
    if action.build_id != content.build_id() {
        return Err(KernelError::WrongBuild {
            expected: content.build_id().to_owned(),
            actual: action.build_id.clone(),
        });
    }
    let expected_state_id = state.state_id();
    if action.pre_state_id != expected_state_id {
        return Err(KernelError::StaleAction {
            expected: expected_state_id,
            actual: action.pre_state_id.clone(),
        });
    }
    if content.action(&action.definition_id).is_none() {
        return Err(KernelError::UnknownAction(action.definition_id.clone()));
    }
    if action.action_id != action.recomputed_id() {
        return Err(KernelError::InvalidAction(
            "action identity does not match its canonical fields".to_owned(),
        ));
    }
    let legal = enumerate_legal_actions(state, content)?;
    if legal.iter().any(|candidate| candidate == action) {
        Ok(())
    } else {
        Err(KernelError::IllegalAction(action.action_id.clone()))
    }
}

/// Apply one validated action to an immutable input state.  The original
/// state is never mutated, including on errors.
pub fn step(
    state: &GameState,
    action: &CanonicalAction,
    content: &CompiledContent,
    entropy: &EntropyState,
) -> Result<Transition, KernelError> {
    if entropy != &state.entropy {
        return Err(KernelError::EntropyMismatch);
    }
    validate_action(state, content, action)?;
    reduce_validated_action(state, action, content, entropy)
}

fn reduce_validated_action(
    state: &GameState,
    action: &CanonicalAction,
    content: &CompiledContent,
    entropy: &EntropyState,
) -> Result<Transition, KernelError> {
    let definition = content
        .action(&action.definition_id)
        .ok_or_else(|| KernelError::UnknownAction(action.definition_id.clone()))?;

    let pre_state_id = state.state_id();
    let entropy_before = entropy.clone();
    let mut next = state.clone();
    let mut entropy_cursor = entropy.clone();
    let mut events = Vec::new();
    let mut entropy_draws = Vec::new();
    for effect in &definition.effects {
        apply_effect(
            &mut next,
            effect,
            &action.parameters,
            content,
            &mut entropy_cursor,
            &mut entropy_draws,
            &mut events,
        )?;
    }
    resolve_due_timed_events(
        &mut next,
        content,
        &mut entropy_cursor,
        &mut entropy_draws,
        &mut events,
    )?;
    next.entropy = entropy_cursor.clone();
    next.event_log.extend(events.iter().cloned());
    content
        .validate_state(&next)
        .map_err(|error| KernelError::InvalidState(error.to_string()))?;
    let post_state_id = next.state_id();
    Ok(Transition {
        pre_state_id,
        action: action.clone(),
        events,
        entropy_before,
        entropy_draws,
        entropy_after: entropy_cursor,
        post_state_id,
        state: next,
    })
}

fn resolve_due_timed_events(
    state: &mut GameState,
    content: &CompiledContent,
    entropy: &mut EntropyState,
    entropy_draws: &mut Vec<EntropyDraw>,
    events: &mut Vec<Event>,
) -> Result<(), KernelError> {
    let due_count = state
        .world
        .scheduled_events
        .partition_point(|event| event.due_time <= state.world.time);
    if due_count == 0 {
        return Ok(());
    }

    let mut due = std::mem::take(&mut state.world.scheduled_events);
    state.world.scheduled_events = due.split_off(due_count);
    let parameters = BTreeMap::new();
    for scheduled in due {
        let (condition, effects) = scheduled_program(&scheduled, content)?;
        let applied = condition.evaluate(state);
        events.push(Event {
            turn: state.world.time,
            kind: EventKind::ScheduledEventResolved {
                event_id: scheduled.id,
                event_kind: scheduled.event_kind,
                applied,
            },
        });
        if applied {
            for effect in effects {
                apply_effect(
                    state,
                    effect,
                    &parameters,
                    content,
                    entropy,
                    entropy_draws,
                    events,
                )?;
            }
        }
    }
    Ok(())
}

fn scheduled_program<'a>(
    scheduled: &crate::ScheduledEvent,
    content: &'a CompiledContent,
) -> Result<(&'a crate::Condition, &'a [Effect]), KernelError> {
    if let Some(definition) = content.timed_event(&scheduled.id) {
        if definition.due_time == scheduled.due_time
            && definition.event_kind == scheduled.event_kind
        {
            return Ok((&definition.condition, &definition.effects));
        }
    } else if let Some(definition) = content.deferred_event(&scheduled.id)
        && definition.event_kind == scheduled.event_kind
    {
        return Ok((&definition.condition, &definition.effects));
    }
    Err(KernelError::InvalidState(format!(
        "timed event {} differs from compiled content",
        scheduled.id
    )))
}

fn resolve_ref(
    reference: &StringRef,
    parameters: &BTreeMap<String, String>,
) -> Result<String, KernelError> {
    match reference {
        StringRef::Literal(value) => Ok(value.clone()),
        StringRef::Parameter(name) => parameters.get(name).cloned().ok_or_else(|| {
            KernelError::InvalidAction(format!("missing value for parameter {name}"))
        }),
    }
}

fn apply_effect(
    state: &mut GameState,
    effect: &Effect,
    parameters: &BTreeMap<String, String>,
    content: &CompiledContent,
    entropy: &mut EntropyState,
    entropy_draws: &mut Vec<EntropyDraw>,
    events: &mut Vec<Event>,
) -> Result<(), KernelError> {
    let turn = state.world.time;
    match effect {
        Effect::Noop => {}
        Effect::SetFlag { flag, value } | Effect::SetWorldFlag { flag, value } => {
            if flag.is_empty() {
                return Err(KernelError::InvalidAction(
                    "flag cannot be empty".to_owned(),
                ));
            }
            if *value {
                state.world.flags.insert(flag.clone());
            } else {
                state.world.flags.remove(flag);
            }
            events.push(Event {
                turn,
                kind: EventKind::WorldFlagSet {
                    flag: flag.clone(),
                    value: *value,
                },
            });
        }
        Effect::SetLocationFlag {
            location,
            flag,
            value,
        } => {
            let location = resolve_ref(location, parameters)?;
            let runtime = state
                .world
                .locations
                .get_mut(&location)
                .ok_or_else(|| KernelError::InvalidAction("unknown location".to_owned()))?;
            if *value {
                runtime.flags.insert(flag.clone());
            } else {
                runtime.flags.remove(flag);
            }
            events.push(Event {
                turn,
                kind: EventKind::LocationFlagSet {
                    location,
                    flag: flag.clone(),
                    value: *value,
                },
            });
        }
        Effect::AdjustResource { resource, amount } => {
            let current = state
                .character
                .resources
                .get(resource)
                .copied()
                .unwrap_or(0);
            let next = current.checked_add(*amount).ok_or_else(|| {
                KernelError::InvalidState(format!("resource {resource} overflow"))
            })?;
            state.character.resources.insert(resource.clone(), next);
            events.push(Event {
                turn,
                kind: EventKind::ResourceAdjusted {
                    resource: resource.clone(),
                    amount: *amount,
                },
            });
        }
        Effect::MoveCharacter { location } => {
            let location = resolve_ref(location, parameters)?;
            if !content.has_location(&location) || !state.world.locations.contains_key(&location) {
                return Err(KernelError::InvalidAction(format!(
                    "cannot move to unknown location {location}"
                )));
            }
            let from = state.world.current_location.clone();
            state.world.current_location = location.clone();
            events.push(Event {
                turn,
                kind: EventKind::Moved { from, to: location },
            });
        }
        Effect::MoveNpc { npc, location } => {
            let npc = resolve_ref(npc, parameters)?;
            let location = resolve_ref(location, parameters)?;
            if !content.has_npc(&npc) {
                return Err(KernelError::InvalidAction(format!(
                    "cannot move unknown NPC {npc}"
                )));
            }
            if !content.has_location(&location) || !state.world.locations.contains_key(&location) {
                return Err(KernelError::InvalidAction(format!(
                    "cannot move NPC {npc} to unknown location {location}"
                )));
            }

            let from = state
                .world
                .npcs
                .get(&npc)
                .ok_or_else(|| KernelError::InvalidAction(format!("unknown NPC {npc}")))?
                .location
                .clone();
            let source_runtime = state.world.locations.get(&from).ok_or_else(|| {
                KernelError::InvalidState(format!("NPC {npc} has unknown source location {from}"))
            })?;
            if !source_runtime.entities.contains(&npc) {
                return Err(KernelError::InvalidState(format!(
                    "NPC {npc} is missing from source location index {from}"
                )));
            }
            if from == location {
                return Ok(());
            }
            if state
                .world
                .locations
                .get(&location)
                .is_some_and(|runtime| runtime.entities.contains(&npc))
            {
                return Err(KernelError::InvalidState(format!(
                    "NPC {npc} already appears at destination location {location}"
                )));
            }

            let removed = state
                .world
                .locations
                .get_mut(&from)
                .expect("source location was validated above")
                .entities
                .remove(&npc);
            if !removed {
                return Err(KernelError::InvalidState(format!(
                    "NPC {npc} is missing from source location index {from}"
                )));
            }
            state
                .world
                .locations
                .get_mut(&location)
                .expect("destination location was validated above")
                .entities
                .insert(npc.clone());
            state
                .world
                .npcs
                .get_mut(&npc)
                .expect("NPC was validated above")
                .location = location.clone();
            events.push(Event {
                turn,
                kind: EventKind::NpcMoved {
                    npc,
                    from,
                    to: location,
                },
            });
        }
        Effect::AdjustNpcRelationship { npc, amount } => {
            let npc = resolve_ref(npc, parameters)?;
            let npc_state = state
                .world
                .npcs
                .get_mut(&npc)
                .ok_or_else(|| KernelError::InvalidAction(format!("unknown NPC {npc}")))?;
            let relationship = npc_state.relationships.get("player").copied().unwrap_or(0);
            npc_state.relationships.insert(
                "player".to_owned(),
                relationship.checked_add(*amount).ok_or_else(|| {
                    KernelError::InvalidState(format!("relationship for {npc} overflow"))
                })?,
            );
            events.push(Event {
                turn,
                kind: EventKind::NpcRelationshipAdjusted {
                    npc,
                    amount: *amount,
                },
            });
        }
        Effect::AddNpcMemory {
            npc,
            memory_id,
            subject,
            provenance,
        } => {
            let npc = resolve_ref(npc, parameters)?;
            let npc_state = state
                .world
                .npcs
                .get_mut(&npc)
                .ok_or_else(|| KernelError::InvalidAction(format!("unknown NPC {npc}")))?;
            npc_state.memories.insert(
                memory_id.clone(),
                Memory {
                    id: memory_id.clone(),
                    subject: subject.clone(),
                    turn,
                    provenance: provenance.clone(),
                },
            );
            events.push(Event {
                turn,
                kind: EventKind::NpcMemoryAdded {
                    npc,
                    memory: memory_id.clone(),
                },
            });
        }
        Effect::TeachNpc {
            npc,
            knowledge_id,
            subject,
            provenance,
        } => {
            if let Some(source) = npc_knowledge_source(provenance)
                && !state
                    .world
                    .npcs
                    .get(source)
                    .is_some_and(|npc_state| npc_state.knows(knowledge_id))
            {
                return Err(KernelError::InvalidAction(format!(
                    "NPC {source} cannot transfer knowledge {knowledge_id} it does not possess"
                )));
            }
            let npc = resolve_ref(npc, parameters)?;
            let npc_state = state
                .world
                .npcs
                .get_mut(&npc)
                .ok_or_else(|| KernelError::InvalidAction(format!("unknown NPC {npc}")))?;
            npc_state.knowledge.insert(
                knowledge_id.clone(),
                Knowledge {
                    id: knowledge_id.clone(),
                    subject: subject.clone(),
                    turn,
                    provenance: provenance.clone(),
                },
            );
            events.push(Event {
                turn,
                kind: EventKind::NpcKnowledgeAdded {
                    npc,
                    knowledge: knowledge_id.clone(),
                },
            });
        }
        Effect::TransferNpcItemToCharacter { npc, item, count } => {
            if *count == 0 {
                return Err(KernelError::InvalidAction(
                    "NPC item transfer count must be positive".to_owned(),
                ));
            }
            let npc = resolve_ref(npc, parameters)?;
            let source_count = state
                .world
                .npcs
                .get(&npc)
                .and_then(|npc_state| npc_state.inventory.get(item))
                .copied()
                .unwrap_or_default();
            if source_count < *count {
                return Err(KernelError::InvalidAction(format!(
                    "NPC {npc} lacks {count} of item {item}"
                )));
            }
            let character_count = state
                .character
                .inventory
                .get(item)
                .copied()
                .unwrap_or_default();
            let next_character_count = character_count.checked_add(*count).ok_or_else(|| {
                KernelError::InvalidAction(format!(
                    "character inventory cannot hold {count} more of item {item}"
                ))
            })?;

            let npc_state = state
                .world
                .npcs
                .get_mut(&npc)
                .ok_or_else(|| KernelError::InvalidAction(format!("unknown NPC {npc}")))?;
            let next_source_count = source_count - *count;
            if next_source_count == 0 {
                npc_state.inventory.remove(item);
            } else {
                npc_state.inventory.insert(item.clone(), next_source_count);
            }
            state
                .character
                .inventory
                .insert(item.clone(), next_character_count);
            events.push(Event {
                turn,
                kind: EventKind::NpcItemTransferredToCharacter {
                    npc,
                    item: item.clone(),
                    count: *count,
                },
            });
        }
        Effect::ApplyRecipe { recipe } => {
            let definition = content
                .recipe(recipe)
                .ok_or_else(|| KernelError::InvalidAction(format!("unknown recipe {recipe}")))?;
            // Stage the entire inventory: no input, output, or event changes
            // until every subtraction and post-consumption addition succeeds.
            let mut inventory = state.character.inventory.clone();
            for (item, count) in &definition.inputs {
                let current = inventory.get(item).copied().unwrap_or_default();
                let next = current.checked_sub(*count).ok_or_else(|| {
                    KernelError::InvalidAction(format!(
                        "character lacks {count} of item {item} for recipe {recipe}"
                    ))
                })?;
                if next == 0 {
                    inventory.remove(item);
                } else {
                    inventory.insert(item.clone(), next);
                }
            }
            for (item, count) in &definition.outputs {
                let current = inventory.get(item).copied().unwrap_or_default();
                let next = current.checked_add(*count).ok_or_else(|| {
                    KernelError::InvalidAction(format!(
                        "character inventory cannot hold {count} more of item {item} for recipe {recipe}"
                    ))
                })?;
                inventory.insert(item.clone(), next);
            }
            state.character.inventory = inventory;
            events.push(Event {
                turn,
                kind: EventKind::RecipeApplied {
                    recipe: recipe.clone(),
                    inputs: definition.inputs.clone(),
                    outputs: definition.outputs.clone(),
                },
            });
        }
        Effect::ScheduleEvent { event } => {
            let template = content.deferred_event(event).ok_or_else(|| {
                KernelError::InvalidAction(format!("unknown deferred event {event}"))
            })?;
            if event_already_used(state, events, event) {
                return Err(KernelError::InvalidAction(format!(
                    "deferred event {event} was already scheduled or resolved"
                )));
            }
            let due_time = state
                .world
                .time
                .checked_add(template.delay)
                .ok_or_else(|| {
                    KernelError::InvalidAction(format!("deferred event {event} due time overflow"))
                })?;
            let scheduled = crate::ScheduledEvent {
                id: event.clone(),
                due_time,
                event_kind: template.event_kind.clone(),
            };
            let position = state
                .world
                .scheduled_events
                .binary_search_by(|existing| {
                    existing
                        .due_time
                        .cmp(&due_time)
                        .then_with(|| existing.id.cmp(event))
                })
                .unwrap_or_else(|position| position);
            try_reserve(
                &mut state.world.scheduled_events,
                1,
                "scheduled event queue",
            )?;
            state.world.scheduled_events.insert(position, scheduled);
            events.push(Event {
                turn,
                kind: EventKind::EventScheduled {
                    event_id: event.clone(),
                    event_kind: template.event_kind.clone(),
                    due_time,
                },
            });
        }
        Effect::AddCharacterDeed { deed_id } => {
            state.character.deeds.insert(deed_id.clone());
            events.push(Event {
                turn,
                kind: EventKind::FlagSet {
                    flag: format!("deed:{deed_id}"),
                    value: true,
                },
            });
        }
        Effect::AdvanceTime { ticks } => {
            state.world.time = state
                .world
                .time
                .checked_add(*ticks)
                .ok_or_else(|| KernelError::InvalidState("world time overflow".to_owned()))?;
            events.push(Event {
                turn,
                kind: EventKind::TimeAdvanced { ticks: *ticks },
            });
        }
        Effect::RandomChance {
            success_percent,
            on_success,
            on_failure,
        } => {
            if *success_percent > 100 {
                return Err(KernelError::InvalidAction(
                    "random chance must be between 0 and 100".to_owned(),
                ));
            }
            let draw = entropy.draw()?;
            let success = draw.value % 100 < u64::from(*success_percent);
            events.push(Event {
                turn,
                kind: EventKind::RandomDraw {
                    algorithm: draw.before.algorithm.clone(),
                    cursor: draw.before.cursor,
                    value: draw.value,
                },
            });
            *entropy = draw.after.clone();
            entropy_draws.push(draw);
            apply_effect(
                state,
                if success { on_success } else { on_failure },
                parameters,
                content,
                entropy,
                entropy_draws,
                events,
            )?;
        }
    }
    Ok(())
}

fn npc_knowledge_source(provenance: &KnowledgeProvenance) -> Option<&str> {
    match provenance {
        KnowledgeProvenance::Told { by } => Some(by),
        KnowledgeProvenance::Rumor { from: Some(from) } => Some(from),
        KnowledgeProvenance::Witnessed
        | KnowledgeProvenance::Read { .. }
        | KnowledgeProvenance::Inferred { .. }
        | KnowledgeProvenance::Rumor { from: None } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::{
        ActionDefinition, Condition, ContentDraft, Effect, LocationDefinition, NpcDefinition,
        TimedEventDefinition,
    };
    use crate::model::{Character, NpcState, WorldState};
    use crate::{EntropyState, MAX_ENTROPY_CURSOR};

    fn character() -> Character {
        Character {
            id: "hero".to_owned(),
            lineage: "fenborn".to_owned(),
            origin: "lowsail".to_owned(),
            background: "clerk".to_owned(),
            aptitudes: BTreeMap::from([("insight".to_owned(), 7)]),
            skills: BTreeSet::from(["audit".to_owned()]),
            values: BTreeSet::from(["order".to_owned()]),
            traits: BTreeSet::from(["tide-ear".to_owned()]),
            flaws: BTreeSet::from(["indebted".to_owned()]),
            appearance: BTreeMap::from([("marking".to_owned(), "council-ink".to_owned())]),
            affiliations: BTreeMap::from([("council".to_owned(), 2)]),
            reputation: BTreeMap::from([("lawful".to_owned(), 3)]),
            knowledge: BTreeSet::new(),
            inventory: BTreeMap::from([("rope".to_owned(), 1), ("wire".to_owned(), 1)]),
            resources: BTreeMap::from([("coin".to_owned(), 10)]),
            injuries: BTreeSet::new(),
            deeds: BTreeSet::new(),
            promises: BTreeSet::new(),
            discoveries: BTreeSet::new(),
            facets: BTreeMap::new(),
        }
    }

    fn draft(actions: Vec<ActionDefinition>) -> ContentDraft {
        ContentDraft {
            schema_version: "forge-schema-v9".to_owned(),
            rules_version: "forge-rules-v7".to_owned(),
            world_id: "world-1".to_owned(),
            contract: crate::ContentContract::Fixture,
            start_location: "gate".to_owned(),
            character_presets: Vec::new(),
            character_creation: None,
            supply_labels: Default::default(),
            recipes: Vec::new(),
            locations: vec![
                LocationDefinition {
                    id: "gate".to_owned(),
                    name: "Gate".to_owned(),
                    description: "A gate stands ahead.".to_owned(),
                    description_variants: Vec::new(),
                    exits: vec!["yard".to_owned()],
                    terminal: true,
                },
                LocationDefinition {
                    id: "yard".to_owned(),
                    name: "Yard".to_owned(),
                    description: "A quiet yard rests here.".to_owned(),
                    description_variants: Vec::new(),
                    exits: vec!["gate".to_owned()],
                    terminal: true,
                },
            ],
            npcs: vec![NpcDefinition {
                id: "sava".to_owned(),
                name: "Sava".to_owned(),
                location: "gate".to_owned(),
                goals: BTreeSet::new(),
                values: BTreeSet::new(),
                tags: BTreeSet::new(),
                inventory: BTreeMap::new(),
            }],
            timed_events: Vec::new(),
            deferred_events: Vec::new(),
            actions,
        }
    }

    fn content(actions: Vec<ActionDefinition>) -> CompiledContent {
        CompiledContent::try_compile(draft(actions)).unwrap()
    }

    fn content_with_mira(actions: Vec<ActionDefinition>) -> CompiledContent {
        let mut source = draft(actions);
        source.npcs.push(NpcDefinition {
            id: "mira".to_owned(),
            name: "Mira".to_owned(),
            location: "gate".to_owned(),
            goals: BTreeSet::new(),
            values: BTreeSet::new(),
            tags: BTreeSet::new(),
            inventory: BTreeMap::new(),
        });
        CompiledContent::try_compile(source).unwrap()
    }

    fn state(content: &CompiledContent) -> GameState {
        let mut locations = content.empty_location_runtime();
        let mut npcs = BTreeMap::new();
        for (id, definition) in content.npcs() {
            locations
                .get_mut(&definition.location)
                .expect("test NPC location is compiled")
                .entities
                .insert(id.clone());
            npcs.insert(
                id.clone(),
                NpcState {
                    id: id.clone(),
                    location: definition.location.clone(),
                    goals: definition.goals.clone(),
                    values: definition.values.clone(),
                    tags: definition.tags.clone(),
                    relationships: BTreeMap::new(),
                    memories: BTreeMap::new(),
                    knowledge: BTreeMap::new(),
                    inventory: definition.inventory.clone(),
                    suspicion: 0,
                },
            );
        }
        GameState::new(
            content.build_id().to_owned(),
            WorldState::new("world-1", "gate", locations, npcs),
            character(),
            EntropyState::new(42),
        )
    }

    fn simple_action(id: &str, condition: Condition, effects: Vec<Effect>) -> ActionDefinition {
        ActionDefinition {
            id: id.to_owned(),
            label: id.to_owned(),
            category: "Action".to_owned(),
            result: "The action is complete.".to_owned(),
            result_variants: Vec::new(),
            locations: vec!["gate".to_owned()],
            condition,
            effects,
            parameters: Vec::new(),
            meaningful: false,
            movement: false,
        }
    }

    fn transfer_effect(npc: StringRef, item: &str, count: u32) -> Effect {
        Effect::TransferNpcItemToCharacter {
            npc,
            item: item.to_owned(),
            count,
        }
    }

    fn recipe(
        id: &str,
        inputs: &[(&str, u32)],
        outputs: &[(&str, u32)],
    ) -> crate::RecipeDefinition {
        let items = |entries: &[(&str, u32)]| {
            entries
                .iter()
                .map(|(item, count)| ((*item).to_owned(), *count))
                .collect()
        };
        crate::RecipeDefinition {
            id: id.to_owned(),
            inputs: items(inputs),
            outputs: items(outputs),
        }
    }

    fn recipe_effect(id: &str) -> Effect {
        Effect::ApplyRecipe {
            recipe: id.to_owned(),
        }
    }

    fn recipe_content(
        actions: Vec<ActionDefinition>,
        recipes: Vec<crate::RecipeDefinition>,
    ) -> CompiledContent {
        let mut source = draft(actions);
        source.recipes = recipes;
        CompiledContent::try_compile(source).unwrap()
    }

    fn legal_ids(state: &GameState, content: &CompiledContent) -> Vec<String> {
        enumerate_legal_actions(state, content)
            .unwrap()
            .into_iter()
            .map(|action| action.definition_id)
            .collect()
    }

    fn schedule(id: &str) -> Effect {
        Effect::ScheduleEvent {
            event: id.to_owned(),
        }
    }

    fn deferred(
        id: &str,
        delay: u64,
        condition: Condition,
        effects: Vec<Effect>,
    ) -> crate::DeferredEventDefinition {
        crate::DeferredEventDefinition {
            id: id.to_owned(),
            delay,
            event_kind: "Work".to_owned(),
            label: "Work due".to_owned(),
            result: "The work changes.".to_owned(),
            condition,
            effects,
        }
    }

    fn record_test_action(state: &GameState, content: &CompiledContent, id: &str) -> Transition {
        let action = enumerate_legal_actions(state, content)
            .unwrap()
            .into_iter()
            .find(|action| action.definition_id == id)
            .unwrap_or_else(|| panic!("missing action {id}"));
        step(state, &action, content, &state.entropy).unwrap()
    }

    fn batch_content() -> CompiledContent {
        let active = Condition::WorldFlag {
            flag: "batch.lit".to_owned(),
        };
        let mut wait = simple_action(
            "wait",
            Condition::Always,
            vec![Effect::AdvanceTime { ticks: 1 }],
        );
        wait.locations.clear();
        let mut source = draft(vec![
            wait,
            simple_action(
                "late",
                Condition::Always,
                vec![Effect::AdvanceTime { ticks: 130 }],
            ),
            simple_action(
                "ignite",
                Condition::Always,
                vec![
                    recipe_effect("batch.ignite"),
                    Effect::SetWorldFlag {
                        flag: "batch.lit".to_owned(),
                        value: true,
                    },
                    schedule("batch.ready"),
                    schedule("batch.spoil"),
                    Effect::AdvanceTime { ticks: 1 },
                ],
            ),
            simple_action(
                "draw",
                Condition::All {
                    conditions: vec![
                        active.clone(),
                        Condition::WorldFlag {
                            flag: "batch.ready".to_owned(),
                        },
                    ],
                },
                vec![
                    recipe_effect("batch.draw"),
                    Effect::SetWorldFlag {
                        flag: "batch.lit".to_owned(),
                        value: false,
                    },
                    Effect::SetWorldFlag {
                        flag: "batch.drawn".to_owned(),
                        value: true,
                    },
                    Effect::AdvanceTime { ticks: 1 },
                ],
            ),
            simple_action(
                "leave",
                Condition::Always,
                vec![
                    Effect::MoveCharacter {
                        location: StringRef::Literal("yard".to_owned()),
                    },
                    Effect::AdvanceTime { ticks: 1 },
                ],
            ),
            simple_action(
                "reschedule",
                Condition::Always,
                vec![schedule("batch.ready")],
            ),
        ]);
        source.recipes = vec![
            recipe(
                "batch.ignite",
                &[("batch.charge", 1), ("batch.fuel", 1)],
                &[("batch.claim", 1)],
            ),
            recipe("batch.draw", &[("batch.claim", 1)], &[("batch.filter", 1)]),
            recipe(
                "batch.spoil",
                &[("batch.claim", 1)],
                &[("batch.spoiled", 1)],
            ),
        ];
        source.deferred_events = vec![
            deferred(
                "batch.ready",
                2,
                active.clone(),
                vec![Effect::SetWorldFlag {
                    flag: "batch.ready".to_owned(),
                    value: true,
                }],
            ),
            deferred(
                "batch.spoil",
                5,
                active,
                vec![
                    recipe_effect("batch.spoil"),
                    Effect::SetWorldFlag {
                        flag: "batch.lit".to_owned(),
                        value: false,
                    },
                    Effect::SetWorldFlag {
                        flag: "batch.spoiled".to_owned(),
                        value: true,
                    },
                ],
            ),
        ];
        CompiledContent::try_compile(source).unwrap()
    }

    fn batch_state(content: &CompiledContent) -> GameState {
        let mut initial = state(content);
        initial
            .character
            .inventory
            .extend([("batch.charge".to_owned(), 1), ("batch.fuel".to_owned(), 1)]);
        initial
    }

    fn move_npc_effect(npc: StringRef, location: StringRef) -> Effect {
        Effect::MoveNpc { npc, location }
    }

    #[test]
    fn deterministic_step_and_explicit_entropy() {
        let content = content(vec![simple_action(
            "roll",
            Condition::Always,
            vec![Effect::RandomChance {
                success_percent: 50,
                on_success: Box::new(Effect::SetFlag {
                    flag: "won".to_owned(),
                    value: true,
                }),
                on_failure: Box::new(Effect::SetFlag {
                    flag: "lost".to_owned(),
                    value: true,
                }),
            }],
        )]);
        let initial = state(&content);
        let action = enumerate_legal_actions(&initial, &content).unwrap()[0].clone();
        let left = step(&initial, &action, &content, &initial.entropy).unwrap();
        let right = step(&initial, &action, &content, &initial.entropy).unwrap();
        assert_eq!(left.post_state_id, right.post_state_id);
        assert_eq!(left.entropy_draws, right.entropy_draws);
        assert_ne!(left.state.entropy.cursor, initial.entropy.cursor);
        assert_eq!(initial.entropy.cursor, 0, "input state remains immutable");
    }

    #[test]
    fn stale_action_and_wrong_build_do_not_mutate() {
        let content = content(vec![simple_action(
            "wait",
            Condition::Always,
            vec![Effect::AdvanceTime { ticks: 1 }],
        )]);
        let initial = state(&content);
        let action = enumerate_legal_actions(&initial, &content).unwrap()[0].clone();
        let changed = step(&initial, &action, &content, &initial.entropy).unwrap();
        let changed_before_stale = changed.state.clone();
        let before = changed.state.state_id();
        let stale = step(&changed.state, &action, &content, &changed.state.entropy);
        assert!(matches!(stale, Err(KernelError::StaleAction { .. })));
        assert_eq!(changed.state.state_id(), before);
        assert_eq!(changed.state, changed_before_stale);

        let initial_before_wrong_build = initial.clone();
        let mut wrong = action;
        wrong.build_id = "other-build".to_owned();
        let wrong_result = step(&initial, &wrong, &content, &initial.entropy);
        assert!(matches!(wrong_result, Err(KernelError::WrongBuild { .. })));
        assert_eq!(initial, initial_before_wrong_build);
    }

    #[test]
    fn invalid_entropy_and_effect_errors_do_not_mutate_the_input() {
        let roll_content = content(vec![simple_action(
            "roll",
            Condition::Always,
            vec![Effect::RandomChance {
                success_percent: 50,
                on_success: Box::new(Effect::Noop),
                on_failure: Box::new(Effect::Noop),
            }],
        )]);
        let mut exhausted = state(&roll_content);
        exhausted.entropy.cursor = MAX_ENTROPY_CURSOR;
        let exhausted_before = exhausted.clone();
        let exhausted_action = enumerate_legal_actions(&exhausted, &roll_content)
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        let exhausted_result = step(
            &exhausted,
            &exhausted_action,
            &roll_content,
            &exhausted.entropy,
        );
        assert!(matches!(
            exhausted_result,
            Err(KernelError::Entropy(EntropyError::CursorExhausted))
        ));
        assert_eq!(exhausted, exhausted_before);

        let mut unsupported = state(&roll_content);
        unsupported.entropy.algorithm = "wrong-algorithm".to_owned();
        let unsupported_before = unsupported.clone();
        let unsupported_action = CanonicalAction::new(
            roll_content.build_id().to_owned(),
            unsupported.state_id(),
            "roll",
            BTreeMap::new(),
        );
        let unsupported_result = step(
            &unsupported,
            &unsupported_action,
            &roll_content,
            &unsupported.entropy,
        );
        assert!(matches!(
            unsupported_result,
            Err(KernelError::Entropy(
                EntropyError::UnsupportedAlgorithm { .. }
            ))
        ));
        assert_eq!(unsupported, unsupported_before);

        let overflow_content = content(vec![simple_action(
            "overflow",
            Condition::Always,
            vec![
                transfer_effect(StringRef::Literal("sava".to_owned()), "ore", 1),
                Effect::AdjustResource {
                    resource: "coin".to_owned(),
                    amount: 1,
                },
            ],
        )]);
        let mut overflowing = state(&overflow_content);
        overflowing
            .world
            .npcs
            .get_mut("sava")
            .unwrap()
            .inventory
            .insert("ore".to_owned(), 1);
        overflowing
            .character
            .resources
            .insert("coin".to_owned(), i64::MAX);
        let overflowing_before = overflowing.clone();
        let overflow_action = enumerate_legal_actions(&overflowing, &overflow_content)
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        let overflow_result = step(
            &overflowing,
            &overflow_action,
            &overflow_content,
            &overflowing.entropy,
        );
        assert!(matches!(overflow_result, Err(KernelError::InvalidState(_))));
        assert_eq!(overflowing, overflowing_before);
    }

    #[test]
    fn raw_unowned_told_transfer_is_rejected_without_mutation() {
        let content = content(Vec::new());
        for provenance in [
            KnowledgeProvenance::Told {
                by: "sava".to_owned(),
            },
            KnowledgeProvenance::Rumor {
                from: Some("sava".to_owned()),
            },
        ] {
            let mut state = state(&content);
            let before = state.clone();
            let mut entropy = state.entropy.clone();
            let mut entropy_draws = Vec::new();
            let mut events = Vec::new();
            let effect = Effect::TeachNpc {
                npc: StringRef::Literal("sava".to_owned()),
                knowledge_id: "tide-key".to_owned(),
                subject: "The Tide Key is safe.".to_owned(),
                provenance,
            };

            let result = apply_effect(
                &mut state,
                &effect,
                &BTreeMap::new(),
                &content,
                &mut entropy,
                &mut entropy_draws,
                &mut events,
            );
            assert!(matches!(
                result,
                Err(KernelError::InvalidAction(message))
                    if message.contains("NPC sava cannot transfer knowledge tide-key")
            ));
            assert_eq!(state, before);
            assert_eq!(entropy, before.entropy);
            assert!(entropy_draws.is_empty());
            assert!(events.is_empty());
        }
    }

    #[test]
    fn same_list_source_seed_is_legal_and_applied_in_order() {
        let content = content(vec![simple_action(
            "seed-and-relay",
            Condition::Always,
            vec![
                Effect::TeachNpc {
                    npc: StringRef::Literal("sava".to_owned()),
                    knowledge_id: "tide-key".to_owned(),
                    subject: "The Tide Key is safe.".to_owned(),
                    provenance: KnowledgeProvenance::Witnessed,
                },
                Effect::TeachNpc {
                    npc: StringRef::Literal("sava".to_owned()),
                    knowledge_id: "tide-key".to_owned(),
                    subject: "The Tide Key is safe.".to_owned(),
                    provenance: KnowledgeProvenance::Told {
                        by: "sava".to_owned(),
                    },
                },
            ],
        )]);
        let initial = state(&content);
        let action = enumerate_legal_actions(&initial, &content)
            .unwrap()
            .into_iter()
            .next()
            .expect("same-list source seed should make the transfer legal");
        let transition = step(&initial, &action, &content, &initial.entropy).unwrap();
        let next = transition.into_state();
        assert_eq!(
            next.world.npcs["sava"].knowledge["tide-key"].provenance,
            KnowledgeProvenance::Told {
                by: "sava".to_owned()
            }
        );
    }

    #[test]
    fn npc_item_transfer_is_atomic_and_emits_typed_event() {
        let content = content(vec![simple_action(
            "take-ore",
            Condition::Always,
            vec![
                Effect::SetWorldFlag {
                    flag: "deal".to_owned(),
                    value: true,
                },
                transfer_effect(StringRef::Literal("sava".to_owned()), "ore", 2),
                Effect::AdjustNpcRelationship {
                    npc: StringRef::Literal("sava".to_owned()),
                    amount: 1,
                },
            ],
        )]);
        let mut initial = state(&content);
        initial
            .world
            .npcs
            .get_mut("sava")
            .unwrap()
            .inventory
            .insert("ore".to_owned(), 3);
        initial.character.inventory.insert("ore".to_owned(), 1);
        let before = initial.clone();
        let action = enumerate_legal_actions(&initial, &content)
            .unwrap()
            .into_iter()
            .next()
            .expect("stocked transfer should be legal");
        let transition = step(&initial, &action, &content, &initial.entropy)
            .expect("stocked transfer should apply");
        assert_eq!(initial, before, "item transfer leaves input immutable");
        let next = transition.state();
        assert_eq!(next.world.npcs["sava"].inventory["ore"], 1);
        assert_eq!(next.character.inventory["ore"], 3);
        assert!(matches!(transition.events(), [
            Event { turn: 0, kind: EventKind::WorldFlagSet { flag, value: true } },
            Event { turn: 0, kind: EventKind::NpcItemTransferredToCharacter { npc, item, count: 2 } },
            Event { turn: 0, kind: EventKind::NpcRelationshipAdjusted { npc: related, amount: 1 } },
        ] if flag == "deal" && npc == "sava" && item == "ore" && related == "sava"));
    }

    #[test]
    fn npc_move_updates_indexes_preserves_state_and_emits_typed_event() {
        let content = content(vec![simple_action(
            "move-sava",
            Condition::Always,
            vec![move_npc_effect(
                StringRef::Literal("sava".to_owned()),
                StringRef::Literal("yard".to_owned()),
            )],
        )]);
        let mut initial = state(&content);
        let sava = initial.world.npcs.get_mut("sava").unwrap();
        sava.relationships.insert("player".to_owned(), 3);
        sava.memories.insert(
            "checkpoint".to_owned(),
            crate::Memory {
                id: "checkpoint".to_owned(),
                subject: "The gate was checked.".to_owned(),
                turn: 0,
                provenance: crate::KnowledgeProvenance::Witnessed,
            },
        );
        sava.knowledge.insert(
            "tide-key".to_owned(),
            crate::Knowledge {
                id: "tide-key".to_owned(),
                subject: "The key is safe.".to_owned(),
                turn: 0,
                provenance: crate::KnowledgeProvenance::Witnessed,
            },
        );
        sava.inventory.insert("ore".to_owned(), 2);
        sava.suspicion = 4;
        let before = initial.clone();
        let before_npc = initial.world.npcs["sava"].clone();
        let action = enumerate_legal_actions(&initial, &content)
            .unwrap()
            .into_iter()
            .next()
            .unwrap();

        let transition = step(&initial, &action, &content, &initial.entropy).unwrap();
        assert_eq!(initial, before, "moving an NPC leaves input immutable");
        let next = transition.state();
        let mut expected_npc = before_npc;
        expected_npc.location = "yard".to_owned();
        assert_eq!(next.world.npcs["sava"], expected_npc);
        assert!(!next.world.locations["gate"].entities.contains("sava"));
        assert!(next.world.locations["yard"].entities.contains("sava"));
        assert!(matches!(
            transition.events(),
            [Event {
                turn: 0,
                kind: EventKind::NpcMoved { npc, from, to }
            }] if npc == "sava" && from == "gate" && to == "yard"
        ));
        assert_eq!(next.event_log, transition.events());
    }

    #[test]
    fn npc_self_move_is_silent_and_preserves_state() {
        let content = content(vec![simple_action(
            "stay-sava",
            Condition::Always,
            vec![move_npc_effect(
                StringRef::Literal("sava".to_owned()),
                StringRef::Literal("gate".to_owned()),
            )],
        )]);
        let initial = state(&content);
        let action = enumerate_legal_actions(&initial, &content)
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        let transition = step(&initial, &action, &content, &initial.entropy).unwrap();
        assert!(transition.events().is_empty());
        assert_eq!(transition.state(), &initial);
        assert!(
            transition.state().world.locations["gate"]
                .entities
                .contains("sava")
        );
    }

    #[test]
    fn sequential_npc_moves_update_indexes_and_event_order() {
        let content = content_with_mira(vec![simple_action(
            "move-both",
            Condition::Always,
            vec![
                move_npc_effect(
                    StringRef::Literal("sava".to_owned()),
                    StringRef::Literal("yard".to_owned()),
                ),
                move_npc_effect(
                    StringRef::Literal("sava".to_owned()),
                    StringRef::Literal("gate".to_owned()),
                ),
                move_npc_effect(
                    StringRef::Literal("mira".to_owned()),
                    StringRef::Literal("yard".to_owned()),
                ),
            ],
        )]);
        let initial = state(&content);
        let action = enumerate_legal_actions(&initial, &content)
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        let transition = step(&initial, &action, &content, &initial.entropy).unwrap();
        assert!(matches!(
            transition.events(),
            [
                Event {
                    turn: 0,
                    kind: EventKind::NpcMoved { npc: first, from: first_from, to: first_to }
                },
                Event {
                    turn: 0,
                    kind: EventKind::NpcMoved { npc: middle, from: middle_from, to: middle_to }
                },
                Event {
                    turn: 0,
                    kind: EventKind::NpcMoved { npc: second, from: second_from, to: second_to }
                }
            ] if first == "sava"
                && first_from == "gate"
                && first_to == "yard"
                && middle == "sava"
                && middle_from == "yard"
                && middle_to == "gate"
                && second == "mira"
                && second_from == "gate"
                && second_to == "yard"
        ));
        assert_eq!(transition.state().world.npcs["sava"].location, "gate");
        assert_eq!(transition.state().world.npcs["mira"].location, "yard");
        assert_eq!(
            transition.state().world.locations["gate"].entities,
            BTreeSet::from(["sava".to_owned()])
        );
        assert_eq!(
            transition.state().world.locations["yard"].entities,
            BTreeSet::from(["mira".to_owned()])
        );
    }

    #[test]
    fn parameterized_npc_moves_follow_current_location_domain() {
        let move_action = ActionDefinition {
            id: "move-npc".to_owned(),
            label: "Move NPC".to_owned(),
            category: "World".to_owned(),
            result: "The NPC moves.".to_owned(),
            result_variants: Vec::new(),
            locations: vec!["gate".to_owned()],
            condition: Condition::Always,
            effects: vec![move_npc_effect(
                StringRef::Parameter("npc".to_owned()),
                StringRef::Parameter("destination".to_owned()),
            )],
            parameters: vec![
                crate::ParameterSpec {
                    name: "npc".to_owned(),
                    domain: ParameterDomain::NpcsAtCurrentLocation,
                },
                crate::ParameterSpec {
                    name: "destination".to_owned(),
                    domain: ParameterDomain::Values(vec!["yard".to_owned()]),
                },
            ],
            meaningful: true,
            movement: false,
        };
        let content = content_with_mira(vec![move_action]);
        let initial = state(&content);
        let actions = enumerate_legal_actions(&initial, &content).unwrap();
        assert_eq!(actions.len(), 2);
        assert_eq!(
            actions
                .iter()
                .map(|action| action.parameters["npc"].as_str())
                .collect::<Vec<_>>(),
            vec!["mira", "sava"]
        );
        let sava_action = actions
            .iter()
            .find(|action| action.parameters["npc"] == "sava")
            .unwrap();
        let after = step(&initial, sava_action, &content, &initial.entropy)
            .unwrap()
            .into_state();
        let remaining = enumerate_legal_actions(&after, &content).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].parameters["npc"], "mira");
        assert_eq!(after.world.npcs["sava"].location, "yard");
    }

    #[test]
    fn random_npc_move_is_fixed_by_explicit_seed() {
        let content = content(vec![simple_action(
            "random-move",
            Condition::Always,
            vec![Effect::RandomChance {
                success_percent: 50,
                on_success: Box::new(move_npc_effect(
                    StringRef::Literal("sava".to_owned()),
                    StringRef::Literal("yard".to_owned()),
                )),
                on_failure: Box::new(move_npc_effect(
                    StringRef::Literal("sava".to_owned()),
                    StringRef::Literal("gate".to_owned()),
                )),
            }],
        )]);
        for seed in [42, 43] {
            let mut initial = state(&content);
            initial.entropy = EntropyState::new(seed);
            let action = enumerate_legal_actions(&initial, &content)
                .unwrap()
                .into_iter()
                .next()
                .unwrap();
            let left = step(&initial, &action, &content, &initial.entropy).unwrap();
            let right = step(&initial, &action, &content, &initial.entropy).unwrap();
            assert_eq!(left.events(), right.events());
            assert_eq!(left.state(), right.state());
            let draw = left
                .events()
                .iter()
                .find_map(|event| match event.kind {
                    EventKind::RandomDraw { value, .. } => Some(value),
                    _ => None,
                })
                .unwrap();
            let succeeded = draw % 100 < 50;
            assert_eq!(
                left.state().world.npcs["sava"].location,
                if succeeded { "yard" } else { "gate" }
            );
            assert_eq!(
                left.events()
                    .iter()
                    .filter(|event| matches!(event.kind, EventKind::NpcMoved { .. }))
                    .count(),
                usize::from(succeeded)
            );
            if succeeded {
                assert!(matches!(
                    left.events(),
                    [
                        Event {
                            kind: EventKind::RandomDraw { .. },
                            ..
                        },
                        Event {
                            kind: EventKind::NpcMoved { .. },
                            ..
                        }
                    ]
                ));
            } else {
                assert!(matches!(
                    left.events(),
                    [Event {
                        kind: EventKind::RandomDraw { .. },
                        ..
                    }]
                ));
            }
        }
    }

    #[test]
    fn timed_npc_move_resolves_after_action_time_advance() {
        let mut source = draft(vec![simple_action(
            "wait",
            Condition::Always,
            vec![Effect::AdvanceTime { ticks: 1 }],
        )]);
        source.timed_events = vec![TimedEventDefinition {
            id: "move-sava-later".to_owned(),
            due_time: 1,
            event_kind: "npc_move".to_owned(),
            label: "Move Sava".to_owned(),
            result: "Sava moves to the yard.".to_owned(),
            condition: Condition::Always,
            effects: vec![move_npc_effect(
                StringRef::Literal("sava".to_owned()),
                StringRef::Literal("yard".to_owned()),
            )],
        }];
        let content = CompiledContent::try_compile(source).unwrap();
        let mut initial = state(&content);
        initial.world.scheduled_events = vec![crate::ScheduledEvent {
            id: "move-sava-later".to_owned(),
            due_time: 1,
            event_kind: "npc_move".to_owned(),
        }];
        let action = enumerate_legal_actions(&initial, &content)
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        let transition = step(&initial, &action, &content, &initial.entropy).unwrap();
        assert!(matches!(
            transition.events(),
            [
                Event {
                    turn: 0,
                    kind: EventKind::TimeAdvanced { ticks: 1 }
                },
                Event {
                    turn: 1,
                    kind: EventKind::ScheduledEventResolved {
                        event_id,
                        applied: true,
                        ..
                    }
                },
                Event {
                    turn: 1,
                    kind: EventKind::NpcMoved { npc, from, to }
                }
            ] if event_id == "move-sava-later"
                && npc == "sava"
                && from == "gate"
                && to == "yard"
        ));
        assert!(transition.state().world.scheduled_events.is_empty());
        assert_eq!(transition.state().world.npcs["sava"].location, "yard");
    }

    #[test]
    fn invalid_npc_move_destination_or_source_index_leaves_input_unchanged() {
        let content = content(vec![simple_action(
            "move-sava",
            Condition::Always,
            vec![move_npc_effect(
                StringRef::Literal("sava".to_owned()),
                StringRef::Literal("yard".to_owned()),
            )],
        )]);
        let mut invalid_destination = state(&content);
        let before_destination = invalid_destination.clone();
        let mut entropy = invalid_destination.entropy.clone();
        let mut entropy_draws = Vec::new();
        let mut events = Vec::new();
        let result = apply_effect(
            &mut invalid_destination,
            &move_npc_effect(
                StringRef::Literal("sava".to_owned()),
                StringRef::Literal("missing".to_owned()),
            ),
            &BTreeMap::new(),
            &content,
            &mut entropy,
            &mut entropy_draws,
            &mut events,
        );
        assert!(matches!(
            result,
            Err(KernelError::InvalidAction(message)) if message.contains("unknown location")
        ));
        assert_eq!(invalid_destination, before_destination);
        assert_eq!(entropy, before_destination.entropy);
        assert!(entropy_draws.is_empty());
        assert!(events.is_empty());

        let valid = state(&content);
        let action = enumerate_legal_actions(&valid, &content)
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        let mut invalid_index = valid.clone();
        invalid_index
            .world
            .locations
            .get_mut("gate")
            .unwrap()
            .entities
            .remove("sava");
        let before_index = invalid_index.clone();
        let result = step(&invalid_index, &action, &content, &invalid_index.entropy);
        assert!(matches!(
            result,
            Err(KernelError::InvalidState(message)) if message.contains("location entity index")
        ));
        assert_eq!(invalid_index, before_index);
    }

    #[test]
    fn later_resource_overflow_rolls_back_prior_npc_move() {
        let content = content(vec![simple_action(
            "move-and-overflow",
            Condition::Always,
            vec![
                move_npc_effect(
                    StringRef::Literal("sava".to_owned()),
                    StringRef::Literal("yard".to_owned()),
                ),
                Effect::AdjustResource {
                    resource: "coin".to_owned(),
                    amount: 1,
                },
            ],
        )]);
        let mut initial = state(&content);
        initial
            .character
            .resources
            .insert("coin".to_owned(), i64::MAX);
        let before = initial.clone();
        let action = enumerate_legal_actions(&initial, &content)
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        let result = step(&initial, &action, &content, &initial.entropy);
        assert!(matches!(
            result,
            Err(KernelError::InvalidState(message)) if message.contains("resource coin overflow")
        ));
        assert_eq!(initial, before);
    }

    #[test]
    fn item_transfer_filters_capacity_and_rechecks_without_mutation() {
        let content = content(vec![
            simple_action("always", Condition::Always, vec![Effect::Noop]),
            simple_action(
                "take-ore",
                Condition::Always,
                vec![transfer_effect(
                    StringRef::Literal("sava".to_owned()),
                    "ore",
                    2,
                )],
            ),
        ]);

        let mut missing = state(&content);
        missing
            .world
            .npcs
            .get_mut("sava")
            .unwrap()
            .inventory
            .insert("ore".to_owned(), 1);
        let missing_actions = enumerate_legal_actions(&missing, &content).unwrap();
        assert!(
            missing_actions
                .iter()
                .all(|action| action.definition_id != "take-ore")
        );
        assert!(
            missing_actions
                .iter()
                .any(|action| action.definition_id == "always")
        );
        let missing_before = missing.clone();
        let mut missing_entropy = missing.entropy.clone();
        let mut missing_draws = Vec::new();
        let mut missing_events = Vec::new();
        let missing_result = apply_effect(
            &mut missing,
            &transfer_effect(StringRef::Literal("sava".to_owned()), "ore", 2),
            &BTreeMap::new(),
            &content,
            &mut missing_entropy,
            &mut missing_draws,
            &mut missing_events,
        );
        assert!(matches!(
            missing_result,
            Err(KernelError::InvalidAction(message)) if message.contains("lacks 2 of item ore")
        ));
        assert_eq!(missing, missing_before);
        assert_eq!(missing_entropy, missing_before.entropy);
        assert!(missing_draws.is_empty());
        assert!(missing_events.is_empty());

        let mut overflow = state(&content);
        overflow
            .world
            .npcs
            .get_mut("sava")
            .unwrap()
            .inventory
            .insert("ore".to_owned(), 2);
        overflow
            .character
            .inventory
            .insert("ore".to_owned(), u32::MAX - 1);
        let overflow_actions = enumerate_legal_actions(&overflow, &content).unwrap();
        assert!(
            overflow_actions
                .iter()
                .all(|action| action.definition_id != "take-ore")
        );
        assert!(
            overflow_actions
                .iter()
                .any(|action| action.definition_id == "always")
        );
        let overflow_before = overflow.clone();
        let mut overflow_entropy = overflow.entropy.clone();
        let mut overflow_draws = Vec::new();
        let mut overflow_events = Vec::new();
        let overflow_result = apply_effect(
            &mut overflow,
            &transfer_effect(StringRef::Literal("sava".to_owned()), "ore", 2),
            &BTreeMap::new(),
            &content,
            &mut overflow_entropy,
            &mut overflow_draws,
            &mut overflow_events,
        );
        assert!(matches!(
            overflow_result,
            Err(KernelError::InvalidAction(message))
                if message.contains("character inventory cannot hold 2 more of item ore")
        ));
        assert_eq!(overflow, overflow_before);
        assert_eq!(overflow_entropy, overflow_before.entropy);
        assert!(overflow_draws.is_empty());
        assert!(overflow_events.is_empty());
    }

    #[test]
    fn sequential_item_transfers_aggregate_source_stock() {
        let content = content(vec![simple_action(
            "take-twice",
            Condition::Always,
            vec![
                transfer_effect(StringRef::Literal("sava".to_owned()), "ore", 2),
                transfer_effect(StringRef::Literal("sava".to_owned()), "ore", 2),
            ],
        )]);

        let mut short = state(&content);
        short
            .world
            .npcs
            .get_mut("sava")
            .unwrap()
            .inventory
            .insert("ore".to_owned(), 3);
        assert!(
            enumerate_legal_actions(&short, &content)
                .unwrap()
                .is_empty()
        );

        let mut enough = short;
        enough
            .world
            .npcs
            .get_mut("sava")
            .unwrap()
            .inventory
            .insert("ore".to_owned(), 4);
        let action = enumerate_legal_actions(&enough, &content)
            .unwrap()
            .into_iter()
            .next()
            .expect("aggregate stock should make the action legal");
        let transition = step(&enough, &action, &content, &enough.entropy)
            .expect("aggregate transfer should apply");
        assert!(
            !transition.state().world.npcs["sava"]
                .inventory
                .contains_key("ore")
        );
        assert_eq!(transition.state().character.inventory["ore"], 4);
        assert_eq!(
            transition
                .events()
                .iter()
                .filter(|event| matches!(
                    &event.kind,
                    EventKind::NpcItemTransferredToCharacter { .. }
                ))
                .count(),
            2
        );

        let mut destination_overflow = enough;
        destination_overflow
            .character
            .inventory
            .insert("ore".to_owned(), u32::MAX - 3);
        assert!(
            enumerate_legal_actions(&destination_overflow, &content)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn unreachable_random_item_transfer_branches_do_not_filter_actions() {
        let zero = simple_action(
            "random-zero",
            Condition::Always,
            vec![Effect::RandomChance {
                success_percent: 0,
                on_success: Box::new(transfer_effect(
                    StringRef::Literal("sava".to_owned()),
                    "ore",
                    2,
                )),
                on_failure: Box::new(Effect::Noop),
            }],
        );
        let hundred = simple_action(
            "random-hundred",
            Condition::Always,
            vec![Effect::RandomChance {
                success_percent: 100,
                on_success: Box::new(Effect::Noop),
                on_failure: Box::new(transfer_effect(
                    StringRef::Literal("sava".to_owned()),
                    "ore",
                    2,
                )),
            }],
        );
        let content = content(vec![zero, hundred]);
        let initial = state(&content);
        let actions = enumerate_legal_actions(&initial, &content).unwrap();
        assert_eq!(
            actions
                .iter()
                .map(|action| action.definition_id.as_str())
                .collect::<Vec<_>>(),
            vec!["random-hundred", "random-zero"]
        );
        assert_eq!(actions.len(), 2);
        for action in actions {
            step(&initial, &action, &content, &initial.entropy)
                .expect("only reachable random branch executes");
        }
    }

    #[test]
    fn uncertain_random_item_transfer_uses_pointwise_max_requirements() {
        let content = content(vec![simple_action(
            "random-ore",
            Condition::Always,
            vec![Effect::RandomChance {
                success_percent: 50,
                on_success: Box::new(transfer_effect(
                    StringRef::Literal("sava".to_owned()),
                    "ore",
                    1,
                )),
                on_failure: Box::new(transfer_effect(
                    StringRef::Literal("sava".to_owned()),
                    "ore",
                    2,
                )),
            }],
        )]);
        let mut short = state(&content);
        short
            .world
            .npcs
            .get_mut("sava")
            .unwrap()
            .inventory
            .insert("ore".to_owned(), 1);
        assert!(
            enumerate_legal_actions(&short, &content)
                .unwrap()
                .is_empty()
        );

        let mut enough = short;
        enough
            .world
            .npcs
            .get_mut("sava")
            .unwrap()
            .inventory
            .insert("ore".to_owned(), 2);
        let action = enumerate_legal_actions(&enough, &content)
            .unwrap()
            .into_iter()
            .next()
            .expect("both random branches fit the source stock");
        step(&enough, &action, &content, &enough.entropy)
            .expect("pointwise-max random transfer should apply");
    }

    #[test]
    fn parameterized_item_source_filters_candidates_and_keeps_catalog_complete() {
        let transfer = ActionDefinition {
            id: "take-from".to_owned(),
            label: "Take from".to_owned(),
            category: "Action".to_owned(),
            result: "The item is taken.".to_owned(),
            result_variants: Vec::new(),
            locations: vec!["gate".to_owned()],
            condition: Condition::Always,
            effects: vec![transfer_effect(
                StringRef::Parameter("source".to_owned()),
                "ore",
                1,
            )],
            parameters: vec![crate::ParameterSpec {
                name: "source".to_owned(),
                domain: ParameterDomain::NpcsAtCurrentLocation,
            }],
            meaningful: false,
            movement: false,
        };
        let content = content_with_mira(vec![
            simple_action("always", Condition::Always, vec![Effect::Noop]),
            transfer,
        ]);
        let mut initial = state(&content);
        initial
            .world
            .npcs
            .get_mut("sava")
            .unwrap()
            .inventory
            .insert("ore".to_owned(), 1);
        let actions = enumerate_legal_actions(&initial, &content).unwrap();
        assert_eq!(actions.len(), 2);
        assert_eq!(
            actions
                .iter()
                .map(|action| action.definition_id.as_str())
                .collect::<Vec<_>>(),
            vec!["always", "take-from"]
        );
        let transfer_actions: Vec<_> = actions
            .iter()
            .filter(|action| action.definition_id == "take-from")
            .collect();
        assert_eq!(transfer_actions.len(), 1);
        assert_eq!(transfer_actions[0].parameters["source"], "sava");
    }

    #[test]
    fn alternative_sources_share_receipt_capacity_but_sequential_sources_add() {
        let from_sava = transfer_effect(StringRef::Literal("sava".to_owned()), "ore", 2);
        let from_mira = transfer_effect(StringRef::Literal("mira".to_owned()), "ore", 2);
        let content = content_with_mira(vec![
            simple_action("always", Condition::Always, vec![Effect::Noop]),
            simple_action(
                "either",
                Condition::Always,
                vec![Effect::RandomChance {
                    success_percent: 50,
                    on_success: Box::new(from_sava.clone()),
                    on_failure: Box::new(from_mira.clone()),
                }],
            ),
            simple_action("both", Condition::Always, vec![from_sava, from_mira]),
        ]);
        let mut initial = state(&content);
        for npc in initial.world.npcs.values_mut() {
            npc.inventory.insert("ore".to_owned(), 2);
        }
        initial
            .character
            .inventory
            .insert("ore".to_owned(), u32::MAX - 3);
        let mut sources = BTreeSet::new();
        for seed in 0..8 {
            initial.entropy = EntropyState::new(seed);
            let actions = enumerate_legal_actions(&initial, &content).unwrap();
            assert_eq!(
                actions
                    .iter()
                    .map(|action| action.definition_id.as_str())
                    .collect::<Vec<_>>(),
                vec!["always", "either"]
            );
            let transition = step(&initial, &actions[1], &content, &initial.entropy).unwrap();
            assert_eq!(transition.state().character.inventory["ore"], u32::MAX - 1);
            assert!(matches!(
                transition.events(),
                [
                    Event {
                        kind: EventKind::RandomDraw { .. },
                        ..
                    },
                    Event {
                        kind: EventKind::NpcItemTransferredToCharacter { .. },
                        ..
                    },
                ]
            ));
            for event in transition.events() {
                if let EventKind::NpcItemTransferredToCharacter { npc, .. } = &event.kind {
                    sources.insert(npc.clone());
                }
            }
        }
        assert_eq!(
            sources,
            BTreeSet::from(["sava".to_owned(), "mira".to_owned()])
        );
    }

    #[test]
    fn recipe_transforms_exact_owned_stock_and_emits_ordered_event() {
        let press = recipe(
            "craft.press",
            &[("item.clay", 2), ("item.mesh", 1)],
            &[("item.offcut", 2), ("item.repair", 1)],
        );
        let content = recipe_content(
            vec![simple_action(
                "press",
                Condition::Always,
                vec![
                    Effect::SetWorldFlag {
                        flag: "started".to_owned(),
                        value: true,
                    },
                    recipe_effect("craft.press"),
                    Effect::AdvanceTime { ticks: 1 },
                ],
            )],
            vec![press.clone()],
        );
        let mut initial = state(&content);
        initial.character.inventory.extend([
            ("item.clay".to_owned(), 3),
            ("item.mesh".to_owned(), 1),
            ("item.repair".to_owned(), 2),
        ]);
        let before = initial.clone();
        let action = enumerate_legal_actions(&initial, &content)
            .unwrap()
            .remove(0);
        let transition = step(&initial, &action, &content, &initial.entropy).unwrap();
        let mut expected = before.character.inventory.clone();
        expected.insert("item.clay".to_owned(), 1);
        expected.remove("item.mesh");
        expected.insert("item.offcut".to_owned(), 2);
        expected.insert("item.repair".to_owned(), 3);
        assert_eq!(transition.state().character.inventory, expected);
        assert_eq!(transition.state().world.npcs, before.world.npcs);
        assert_eq!(
            transition.events(),
            &[
                Event {
                    turn: 0,
                    kind: EventKind::WorldFlagSet {
                        flag: "started".to_owned(),
                        value: true
                    }
                },
                Event {
                    turn: 0,
                    kind: EventKind::RecipeApplied {
                        recipe: press.id,
                        inputs: press.inputs,
                        outputs: press.outputs
                    }
                },
                Event {
                    turn: 0,
                    kind: EventKind::TimeAdvanced { ticks: 1 }
                },
            ]
        );
        assert_eq!(transition.state().event_log, transition.events());
        assert_eq!(transition.entropy_after(), &before.entropy);
        assert!(transition.entropy_draws().is_empty());
        assert!(legal_ids(transition.state(), &content).is_empty());
        let after = transition.state().clone();
        assert!(matches!(
            step(&after, &action, &content, &after.entropy),
            Err(KernelError::StaleAction { .. })
        ));
        assert_eq!(initial, before);
        assert_eq!(after, *transition.state());
    }

    #[test]
    fn recipe_filters_each_missing_input_and_capacity_without_partial_changes() {
        let content = recipe_content(
            vec![simple_action(
                "press",
                Condition::Always,
                vec![recipe_effect("craft.press")],
            )],
            vec![recipe(
                "craft.press",
                &[("item.clay", 2), ("item.mesh", 1)],
                &[("item.offcut", 1), ("item.repair", 1)],
            )],
        );
        for (clay, mesh, repair_count, expected_error) in [
            (1, 1, 1, "lacks 2 of item item.clay"),
            (2, 0, 1, "lacks 1 of item item.mesh"),
            (2, 1, u32::MAX, "cannot hold 1 more of item item.repair"),
        ] {
            let mut initial = state(&content);
            initial
                .character
                .inventory
                .insert("item.clay".to_owned(), clay);
            if mesh > 0 {
                initial
                    .character
                    .inventory
                    .insert("item.mesh".to_owned(), mesh);
            }
            initial
                .character
                .inventory
                .insert("item.repair".to_owned(), repair_count);
            let before = initial.clone();
            assert!(legal_ids(&initial, &content).is_empty());
            let fabricated = CanonicalAction::new(
                content.build_id(),
                initial.state_id(),
                "press",
                BTreeMap::new(),
            );
            assert!(matches!(
                step(&initial, &fabricated, &content, &initial.entropy),
                Err(KernelError::IllegalAction(_))
            ));
            let mut entropy = initial.entropy.clone();
            let mut draws = Vec::new();
            let mut events = Vec::new();
            let result = apply_effect(
                &mut initial,
                &recipe_effect("craft.press"),
                &BTreeMap::new(),
                &content,
                &mut entropy,
                &mut draws,
                &mut events,
            );
            assert!(
                matches!(result, Err(KernelError::InvalidAction(message)) if message.contains(expected_error))
            );
            assert_eq!(initial, before);
            assert_eq!(entropy, before.entropy);
            assert!(events.is_empty());
            assert!(draws.is_empty());
        }
    }

    #[test]
    fn recipe_capacity_is_checked_after_same_item_consumption_and_install_can_have_no_output() {
        let content = recipe_content(
            vec![
                simple_action("grow", Condition::Always, vec![recipe_effect("craft.grow")]),
                simple_action(
                    "install",
                    Condition::Always,
                    vec![recipe_effect("craft.install")],
                ),
                simple_action(
                    "rework",
                    Condition::Always,
                    vec![recipe_effect("craft.rework")],
                ),
            ],
            vec![
                recipe("craft.grow", &[("item.clay", 2)], &[("item.clay", 3)]),
                recipe("craft.install", &[("item.clay", u32::MAX)], &[]),
                recipe(
                    "craft.rework",
                    &[("item.clay", 2)],
                    &[("item.clay", 2), ("item.offcut", 1)],
                ),
            ],
        );
        let mut initial = state(&content);
        initial
            .character
            .inventory
            .insert("item.clay".to_owned(), u32::MAX);
        assert_eq!(legal_ids(&initial, &content), vec!["install", "rework"]);
        for action in enumerate_legal_actions(&initial, &content).unwrap() {
            let transition = step(&initial, &action, &content, &initial.entropy).unwrap();
            if action.definition_id == "install" {
                assert!(
                    !transition
                        .state()
                        .character
                        .inventory
                        .contains_key("item.clay")
                );
                assert!(
                    matches!(&transition.events()[0].kind, EventKind::RecipeApplied { outputs, .. } if outputs.is_empty())
                );
            } else {
                assert_eq!(
                    transition.state().character.inventory["item.clay"],
                    u32::MAX
                );
            }
        }
    }

    #[test]
    fn sequential_transfers_and_recipes_follow_inventory_order() {
        let take = transfer_effect(StringRef::Literal("sava".to_owned()), "item.ore", 2);
        let content = recipe_content(
            vec![
                simple_action(
                    "chain",
                    Condition::Always,
                    vec![
                        take.clone(),
                        recipe_effect("craft.smelt"),
                        recipe_effect("craft.brace"),
                    ],
                ),
                simple_action(
                    "reversed",
                    Condition::Always,
                    vec![recipe_effect("craft.smelt"), take],
                ),
                simple_action(
                    "twice",
                    Condition::Always,
                    vec![
                        transfer_effect(StringRef::Literal("sava".to_owned()), "item.ore", 2),
                        recipe_effect("craft.smelt"),
                        recipe_effect("craft.smelt"),
                    ],
                ),
            ],
            vec![
                recipe("craft.smelt", &[("item.ore", 2)], &[("item.ingot", 1)]),
                recipe("craft.brace", &[("item.ingot", 1)], &[("item.brace", 1)]),
            ],
        );
        let mut initial = state(&content);
        initial
            .world
            .npcs
            .get_mut("sava")
            .unwrap()
            .inventory
            .insert("item.ore".to_owned(), 2);
        assert_eq!(legal_ids(&initial, &content), vec!["chain"]);
        let action = enumerate_legal_actions(&initial, &content)
            .unwrap()
            .remove(0);
        let transition = step(&initial, &action, &content, &initial.entropy).unwrap();
        assert!(
            !transition.state().world.npcs["sava"]
                .inventory
                .contains_key("item.ore")
        );
        assert!(
            !transition
                .state()
                .character
                .inventory
                .contains_key("item.ore")
        );
        assert!(
            !transition
                .state()
                .character
                .inventory
                .contains_key("item.ingot")
        );
        assert_eq!(transition.state().character.inventory["item.brace"], 1);
        assert!(matches!(transition.events(), [
            Event { kind: EventKind::NpcItemTransferredToCharacter { .. }, .. },
            Event { kind: EventKind::RecipeApplied { recipe: first, .. }, .. },
            Event { kind: EventKind::RecipeApplied { recipe: second, .. }, .. },
        ] if first == "craft.smelt" && second == "craft.brace"));
    }

    #[test]
    fn recipe_consumption_can_free_transfer_capacity_but_later_consumption_cannot() {
        let take = transfer_effect(StringRef::Literal("sava".to_owned()), "item.ore", 2);
        let consume = recipe_effect("craft.consume");
        let content = recipe_content(
            vec![
                simple_action(
                    "consume-first",
                    Condition::Always,
                    vec![consume.clone(), take.clone()],
                ),
                simple_action("take-first", Condition::Always, vec![take, consume]),
            ],
            vec![recipe("craft.consume", &[("item.ore", 2)], &[])],
        );
        let mut initial = state(&content);
        initial
            .world
            .npcs
            .get_mut("sava")
            .unwrap()
            .inventory
            .insert("item.ore".to_owned(), 2);
        initial
            .character
            .inventory
            .insert("item.ore".to_owned(), u32::MAX);
        assert_eq!(legal_ids(&initial, &content), vec!["consume-first"]);
        let action = enumerate_legal_actions(&initial, &content)
            .unwrap()
            .remove(0);
        let transition = step(&initial, &action, &content, &initial.entropy).unwrap();
        assert_eq!(transition.state().character.inventory["item.ore"], u32::MAX);
        assert!(
            !transition.state().world.npcs["sava"]
                .inventory
                .contains_key("item.ore")
        );
        assert!(matches!(
            transition.events(),
            [
                Event {
                    kind: EventKind::RecipeApplied { .. },
                    ..
                },
                Event {
                    kind: EventKind::NpcItemTransferredToCharacter { .. },
                    ..
                },
            ]
        ));
    }

    #[test]
    fn reachable_random_recipe_paths_all_need_stock_and_output_capacity() {
        let random = |success_percent, on_success, on_failure| Effect::RandomChance {
            success_percent,
            on_success: Box::new(on_success),
            on_failure: Box::new(on_failure),
        };
        let content = recipe_content(
            vec![
                simple_action(
                    "both-fit",
                    Condition::Always,
                    vec![
                        random(50, recipe_effect("craft.one"), recipe_effect("craft.two")),
                        recipe_effect("craft.finish"),
                    ],
                ),
                simple_action(
                    "missing-branch",
                    Condition::Always,
                    vec![
                        random(50, recipe_effect("craft.one"), Effect::Noop),
                        recipe_effect("craft.finish"),
                    ],
                ),
                simple_action(
                    "overflow-branch",
                    Condition::Always,
                    vec![
                        random(50, recipe_effect("craft.finish"), Effect::Noop),
                        transfer_effect(StringRef::Literal("sava".to_owned()), "item.ingot", 1),
                    ],
                ),
                simple_action(
                    "zero",
                    Condition::Always,
                    vec![random(
                        0,
                        recipe_effect("craft.missing"),
                        recipe_effect("craft.one"),
                    )],
                ),
                simple_action(
                    "hundred",
                    Condition::Always,
                    vec![random(
                        100,
                        recipe_effect("craft.one"),
                        recipe_effect("craft.missing"),
                    )],
                ),
            ],
            vec![
                recipe("craft.one", &[("item.ore", 2)], &[("item.ingot", 1)]),
                recipe("craft.two", &[("item.ore", 2)], &[("item.ingot", 2)]),
                recipe("craft.finish", &[("item.ingot", 1)], &[("item.brace", 1)]),
                recipe("craft.missing", &[("item.missing", 1)], &[]),
            ],
        );
        let mut initial = state(&content);
        initial.character.inventory.insert("item.ore".to_owned(), 2);
        initial
            .world
            .npcs
            .get_mut("sava")
            .unwrap()
            .inventory
            .insert("item.ingot".to_owned(), 1);
        let mut produced = BTreeSet::new();
        for seed in 0..16 {
            initial.entropy = EntropyState::new(seed);
            assert_eq!(
                legal_ids(&initial, &content),
                vec!["both-fit", "hundred", "zero"]
            );
            for action in enumerate_legal_actions(&initial, &content).unwrap() {
                let transition = step(&initial, &action, &content, &initial.entropy).unwrap();
                assert_eq!(transition.entropy_draws().len(), 1);
                assert_eq!(transition.state().entropy.cursor, 1);
                if action.definition_id == "both-fit" {
                    assert_eq!(transition.state().character.inventory["item.brace"], 1);
                    produced.insert(
                        transition
                            .state()
                            .character
                            .inventory
                            .get("item.ingot")
                            .copied()
                            .unwrap_or_default(),
                    );
                }
            }
        }
        assert_eq!(produced, BTreeSet::from([0, 1]));
        initial
            .character
            .inventory
            .insert("item.ingot".to_owned(), u32::MAX);
        assert!(!legal_ids(&initial, &content).contains(&"overflow-branch".to_owned()));
    }

    #[test]
    fn random_transfers_then_recipe_use_post_branch_minimum_stock() {
        let content = recipe_content(
            vec![simple_action(
                "random-press",
                Condition::Always,
                vec![
                    Effect::RandomChance {
                        success_percent: 50,
                        on_success: Box::new(transfer_effect(
                            StringRef::Literal("sava".to_owned()),
                            "item.ore",
                            1,
                        )),
                        on_failure: Box::new(transfer_effect(
                            StringRef::Literal("sava".to_owned()),
                            "item.ore",
                            2,
                        )),
                    },
                    recipe_effect("craft.press"),
                ],
            )],
            vec![recipe(
                "craft.press",
                &[("item.ore", 2)],
                &[("item.brace", 1)],
            )],
        );
        let mut initial = state(&content);
        initial
            .world
            .npcs
            .get_mut("sava")
            .unwrap()
            .inventory
            .insert("item.ore".to_owned(), 2);
        assert!(legal_ids(&initial, &content).is_empty());
        initial.character.inventory.insert("item.ore".to_owned(), 1);
        for seed in 0..8 {
            initial.entropy = EntropyState::new(seed);
            let action = enumerate_legal_actions(&initial, &content)
                .unwrap()
                .remove(0);
            let transition = step(&initial, &action, &content, &initial.entropy).unwrap();
            assert_eq!(transition.state().character.inventory["item.brace"], 1);
            let held = transition
                .state()
                .character
                .inventory
                .get("item.ore")
                .copied()
                .unwrap_or_default();
            let retained = transition.state().world.npcs["sava"]
                .inventory
                .get("item.ore")
                .copied()
                .unwrap_or_default();
            assert_eq!(held + retained, 1);
        }
    }

    #[test]
    fn later_failure_rolls_back_recipe_stock_transfer_flags_and_entropy() {
        let content = recipe_content(
            vec![simple_action(
                "fail-late",
                Condition::Always,
                vec![
                    transfer_effect(StringRef::Literal("sava".to_owned()), "item.ore", 2),
                    Effect::RandomChance {
                        success_percent: 50,
                        on_success: Box::new(recipe_effect("craft.press")),
                        on_failure: Box::new(recipe_effect("craft.press")),
                    },
                    Effect::SetWorldFlag {
                        flag: "pressed".to_owned(),
                        value: true,
                    },
                    Effect::AdjustResource {
                        resource: "coin".to_owned(),
                        amount: 1,
                    },
                ],
            )],
            vec![recipe(
                "craft.press",
                &[("item.ore", 2)],
                &[("item.brace", 1)],
            )],
        );
        let mut initial = state(&content);
        initial
            .character
            .resources
            .insert("coin".to_owned(), i64::MAX);
        initial
            .world
            .npcs
            .get_mut("sava")
            .unwrap()
            .inventory
            .insert("item.ore".to_owned(), 2);
        let before = initial.clone();
        let action = enumerate_legal_actions(&initial, &content)
            .unwrap()
            .remove(0);
        let result = step(&initial, &action, &content, &initial.entropy);
        assert!(
            matches!(result, Err(KernelError::InvalidState(message)) if message.contains("resource coin overflow"))
        );
        assert_eq!(initial, before);
        assert_eq!(initial.entropy.cursor, 0);
        assert_eq!(
            enumerate_legal_actions(&initial, &content).unwrap(),
            vec![action]
        );
    }

    #[test]
    fn recipe_parameter_catalog_remains_complete_above_256_actions() {
        let mut action = simple_action(
            "press",
            Condition::Always,
            vec![
                transfer_effect(StringRef::Parameter("source".to_owned()), "item.ore", 2),
                recipe_effect("craft.press"),
            ],
        );
        action.parameters = vec![crate::ParameterSpec {
            name: "source".to_owned(),
            domain: ParameterDomain::NpcsAtCurrentLocation,
        }];
        let mut source = draft(vec![action]);
        source.recipes = vec![recipe(
            "craft.press",
            &[("item.ore", 2)],
            &[("item.brace", 1)],
        )];
        for index in 0..300 {
            source.npcs.push(NpcDefinition {
                id: format!("worker-{index:03}"),
                name: "Worker".to_owned(),
                location: "gate".to_owned(),
                goals: BTreeSet::new(),
                values: BTreeSet::new(),
                tags: BTreeSet::new(),
                inventory: BTreeMap::from([("item.ore".to_owned(), 2)]),
            });
        }
        let content = CompiledContent::try_compile(source).unwrap();
        let initial = state(&content);
        let all = enumerate_legal_actions(&initial, &content).unwrap();
        assert_eq!(all.len(), 300);
        let expected: Vec<_> = all.iter().map(|action| action.action_id.clone()).collect();
        let digest = legal_action_digest(&all).unwrap();
        let mut paged = Vec::new();
        let mut offset = 0;
        loop {
            let page = content.action_page(&initial, offset, 17).unwrap();
            assert_eq!(page.total, 300);
            assert_eq!(page.digest, digest);
            paged.extend(page.actions.into_iter().map(|action| action.action_id));
            let Some(next) = page.next_offset else {
                break;
            };
            offset = next;
        }
        assert_eq!(paged, expected);
        for action in [&all[0], &all[149], &all[299]] {
            let transition = step(&initial, action, &content, &initial.entropy).unwrap();
            assert_eq!(transition.state().character.inventory["item.brace"], 1);
            let after_page = content.action_page(transition.state(), 0, 17).unwrap();
            assert_eq!(after_page.total, 299);
            assert!(
                enumerate_legal_actions(transition.state(), &content)
                    .unwrap()
                    .iter()
                    .all(|candidate| candidate.parameters["source"] != action.parameters["source"])
            );
            let stale = step(
                transition.state(),
                &all[299],
                &content,
                &transition.state().entropy,
            );
            assert!(matches!(stale, Err(KernelError::StaleAction { .. })));
        }
    }

    #[test]
    fn deferred_schedule_is_relative_late_and_one_shot_after_resolution() {
        let content = batch_content();
        let initial = batch_state(&content);
        assert!(initial.world.scheduled_events.is_empty());
        let late = record_test_action(&initial, &content, "late").into_state();
        let lit = record_test_action(&late, &content, "ignite");
        assert_eq!(lit.state().world.time, 131);
        assert_eq!(
            lit.state().world.scheduled_events,
            vec![
                crate::ScheduledEvent {
                    id: "batch.ready".to_owned(),
                    due_time: 132,
                    event_kind: "Work".to_owned()
                },
                crate::ScheduledEvent {
                    id: "batch.spoil".to_owned(),
                    due_time: 135,
                    event_kind: "Work".to_owned()
                },
            ]
        );
        let scheduling: Vec<_> = lit
            .events()
            .iter()
            .filter(|event| matches!(event.kind, EventKind::EventScheduled { .. }))
            .cloned()
            .collect();
        assert_eq!(
            scheduling,
            vec![
                Event {
                    turn: 130,
                    kind: EventKind::EventScheduled {
                        event_id: "batch.ready".to_owned(),
                        event_kind: "Work".to_owned(),
                        due_time: 132
                    }
                },
                Event {
                    turn: 130,
                    kind: EventKind::EventScheduled {
                        event_id: "batch.spoil".to_owned(),
                        event_kind: "Work".to_owned(),
                        due_time: 135
                    }
                },
            ]
        );
        assert!(!legal_ids(lit.state(), &content).contains(&"reschedule".to_owned()));
        let ready = record_test_action(lit.state(), &content, "wait");
        assert_eq!(ready.state().world.time, 132);
        assert!(ready.state().world.flags.contains("batch.ready"));
        assert!(legal_ids(ready.state(), &content).contains(&"draw".to_owned()));
        assert!(!legal_ids(ready.state(), &content).contains(&"reschedule".to_owned()));
        let forged = CanonicalAction::new(
            content.build_id(),
            ready.state().state_id(),
            "reschedule",
            BTreeMap::new(),
        );
        let before = ready.state().clone();
        assert!(matches!(
            step(ready.state(), &forged, &content, &ready.state().entropy),
            Err(KernelError::IllegalAction(_))
        ));
        assert_eq!(ready.state(), &before);
        let mut staged = before.clone();
        let mut entropy = staged.entropy.clone();
        let mut events = Vec::new();
        assert!(
            apply_effect(
                &mut staged,
                &schedule("batch.ready"),
                &BTreeMap::new(),
                &content,
                &mut entropy,
                &mut Vec::new(),
                &mut events
            )
            .is_err()
        );
        assert_eq!(staged, before);
        assert!(events.is_empty());
    }

    #[test]
    fn deferred_draw_first_and_last_windows_and_remote_spoil_consume_owned_claim() {
        let content = batch_content();
        let lit = record_test_action(&batch_state(&content), &content, "ignite").into_state();
        assert!(!legal_ids(&lit, &content).contains(&"draw".to_owned()));
        for draw_time in [2, 4] {
            let mut current = lit.clone();
            while current.world.time < draw_time {
                current = record_test_action(&current, &content, "wait").into_state();
            }
            current = record_test_action(&current, &content, "draw").into_state();
            while current.world.time < 6 {
                current = record_test_action(&current, &content, "wait").into_state();
            }
            assert_eq!(current.character.inventory["batch.filter"], 1);
            assert!(!current.character.inventory.contains_key("batch.claim"));
            assert!(!current.character.inventory.contains_key("batch.spoiled"));
            assert!(current.world.scheduled_events.is_empty());
            let spoil: Vec<_> = current.event_log.iter().filter(|event| matches!(&event.kind, EventKind::ScheduledEventResolved { event_id, .. } if event_id == "batch.spoil")).collect();
            assert_eq!(spoil.len(), 1);
            assert!(matches!(
                spoil[0].kind,
                EventKind::ScheduledEventResolved { applied: false, .. }
            ));
        }
        let mut remote = record_test_action(&lit, &content, "leave").into_state();
        while remote.world.time < 5 {
            remote = record_test_action(&remote, &content, "wait").into_state();
        }
        assert_eq!(remote.world.current_location, "yard");
        assert_eq!(remote.character.inventory["batch.spoiled"], 1);
        assert!(!remote.character.inventory.contains_key("batch.claim"));
        assert!(!remote.character.inventory.contains_key("batch.filter"));
        assert!(!legal_ids(&remote, &content).contains(&"draw".to_owned()));
        assert!(remote.world.scheduled_events.is_empty());
        assert!(remote.event_log.iter().any(|event| event.turn == 5 && matches!(&event.kind,
            EventKind::RecipeApplied { recipe, inputs, outputs }
                if recipe == "batch.spoil" && inputs == &BTreeMap::from([("batch.claim".to_owned(), 1)])
                    && outputs == &BTreeMap::from([("batch.spoiled".to_owned(), 1)]))));
    }

    #[test]
    fn deferred_duplicate_schedule_filter_composes_sequential_and_random_paths() {
        let random = |percent, success, failure| Effect::RandomChance {
            success_percent: percent,
            on_success: Box::new(success),
            on_failure: Box::new(failure),
        };
        let mut source = draft(vec![
            simple_action(
                "double",
                Condition::Always,
                vec![schedule("work.finish"), schedule("work.finish")],
            ),
            simple_action(
                "maybe-double",
                Condition::Always,
                vec![
                    random(50, schedule("work.finish"), Effect::Noop),
                    schedule("work.finish"),
                ],
            ),
            simple_action(
                "single-branch",
                Condition::Always,
                vec![random(50, schedule("work.finish"), schedule("work.finish"))],
            ),
            simple_action(
                "zero",
                Condition::Always,
                vec![
                    random(0, schedule("work.finish"), Effect::Noop),
                    schedule("work.finish"),
                ],
            ),
            simple_action(
                "hundred",
                Condition::Always,
                vec![
                    random(100, Effect::Noop, schedule("work.finish")),
                    schedule("work.finish"),
                ],
            ),
        ]);
        source.deferred_events = vec![deferred(
            "work.finish",
            2,
            Condition::Always,
            vec![Effect::SetWorldFlag {
                flag: "done".to_owned(),
                value: true,
            }],
        )];
        let content = CompiledContent::try_compile(source).unwrap();
        let mut initial = state(&content);
        for seed in 0..8 {
            initial.entropy = EntropyState::new(seed);
            let before = initial.clone();
            assert_eq!(
                legal_ids(&initial, &content),
                vec!["hundred", "single-branch", "zero"]
            );
            assert_eq!(initial, before);
            for action in enumerate_legal_actions(&initial, &content).unwrap() {
                let transition = step(&initial, &action, &content, &initial.entropy).unwrap();
                assert_eq!(transition.state().world.scheduled_events.len(), 1);
                assert_eq!(transition.entropy_draws().len(), 1);
            }
        }
        let mut staged = initial.clone();
        let mut entropy = staged.entropy.clone();
        let mut events = Vec::new();
        apply_effect(
            &mut staged,
            &schedule("work.finish"),
            &BTreeMap::new(),
            &content,
            &mut entropy,
            &mut Vec::new(),
            &mut events,
        )
        .unwrap();
        staged.world.scheduled_events.clear();
        let before = staged.clone();
        let before_events = events.clone();
        assert!(
            apply_effect(
                &mut staged,
                &schedule("work.finish"),
                &BTreeMap::new(),
                &content,
                &mut entropy,
                &mut Vec::new(),
                &mut events
            )
            .is_err()
        );
        assert_eq!(staged, before);
        assert_eq!(events, before_events);
    }

    #[test]
    fn deferred_schedule_overflow_uses_effect_order_and_all_reachable_time_bounds() {
        let random_time = |percent| Effect::RandomChance {
            success_percent: percent,
            on_success: Box::new(Effect::AdvanceTime { ticks: 3 }),
            on_failure: Box::new(Effect::Noop),
        };
        let mut source = draft(vec![
            simple_action(
                "late",
                Condition::Always,
                vec![Effect::AdvanceTime {
                    ticks: u64::MAX - 3,
                }],
            ),
            simple_action(
                "schedule-first",
                Condition::Always,
                vec![schedule("work.finish"), Effect::AdvanceTime { ticks: 2 }],
            ),
            simple_action(
                "time-first",
                Condition::Always,
                vec![Effect::AdvanceTime { ticks: 2 }, schedule("work.finish")],
            ),
            simple_action(
                "late-overflow",
                Condition::Always,
                vec![schedule("work.finish"), Effect::AdvanceTime { ticks: 4 }],
            ),
            simple_action(
                "random-overflow",
                Condition::Always,
                vec![random_time(50), schedule("work.finish")],
            ),
            simple_action(
                "zero",
                Condition::Always,
                vec![random_time(0), schedule("work.finish")],
            ),
        ]);
        source.deferred_events = vec![deferred(
            "work.finish",
            2,
            Condition::Always,
            vec![Effect::SetWorldFlag {
                flag: "done".to_owned(),
                value: true,
            }],
        )];
        let content = CompiledContent::try_compile(source).unwrap();
        let late = record_test_action(&state(&content), &content, "late").into_state();
        let ids = legal_ids(&late, &content);
        assert!(ids.contains(&"schedule-first".to_owned()));
        assert!(ids.contains(&"zero".to_owned()));
        for absent in ["time-first", "late-overflow", "random-overflow"] {
            assert!(!ids.contains(&absent.to_owned()));
        }
        let transition = record_test_action(&late, &content, "schedule-first");
        assert_eq!(transition.state().world.time, u64::MAX - 1);
        assert!(transition.state().world.flags.contains("done"));
        assert!(transition.state().world.scheduled_events.is_empty());
        assert!(
            matches!(&transition.events()[0].kind, EventKind::EventScheduled { due_time, .. } if *due_time == u64::MAX - 1)
        );
    }

    #[test]
    fn deferred_and_absolute_events_keep_due_then_id_order_on_one_timeline() {
        let mut source = draft(vec![
            simple_action(
                "ignite",
                Condition::Always,
                vec![
                    schedule("work.ready"),
                    schedule("work.spoil"),
                    Effect::AdvanceTime { ticks: 1 },
                ],
            ),
            simple_action(
                "jump",
                Condition::Always,
                vec![Effect::AdvanceTime { ticks: 4 }],
            ),
        ]);
        source.recipes = vec![recipe(
            "work.spoil",
            &[("work.claim", 1)],
            &[("work.spoiled", 1)],
        )];
        source.deferred_events = vec![
            deferred(
                "work.ready",
                2,
                Condition::Always,
                vec![Effect::SetWorldFlag {
                    flag: "ready".to_owned(),
                    value: true,
                }],
            ),
            deferred(
                "work.spoil",
                5,
                Condition::Always,
                vec![recipe_effect("work.spoil")],
            ),
        ];
        source.timed_events = vec![TimedEventDefinition {
            id: "tide.surge".to_owned(),
            due_time: 5,
            event_kind: "Tide".to_owned(),
            label: "Tide due".to_owned(),
            result: "The tide rises.".to_owned(),
            condition: Condition::Always,
            effects: vec![Effect::SetWorldFlag {
                flag: "flooded".to_owned(),
                value: true,
            }],
        }];
        let content = CompiledContent::try_compile(source).unwrap();
        let mut initial = state(&content);
        initial
            .character
            .inventory
            .insert("work.claim".to_owned(), 1);
        initial.world.scheduled_events = vec![crate::ScheduledEvent {
            id: "tide.surge".to_owned(),
            due_time: 5,
            event_kind: "Tide".to_owned(),
        }];
        let lit = record_test_action(&initial, &content, "ignite").into_state();
        let transition = record_test_action(&lit, &content, "jump");
        let ids: Vec<_> = transition
            .events()
            .iter()
            .filter_map(|event| match &event.kind {
                EventKind::ScheduledEventResolved { event_id, .. } => Some(event_id.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(ids, vec!["work.ready", "tide.surge", "work.spoil"]);
        assert!(
            transition
                .events()
                .iter()
                .filter(|event| matches!(event.kind, EventKind::ScheduledEventResolved { .. }))
                .all(|event| event.turn == 5)
        );
        assert_eq!(transition.state().character.inventory["work.spoiled"], 1);
        assert!(transition.state().world.flags.contains("flooded"));
        assert!(transition.state().world.flags.contains("ready"));
        assert!(transition.state().world.scheduled_events.is_empty());
    }

    fn guarded_recipe_content(guard: Condition, random_outcome: bool) -> CompiledContent {
        let random = Effect::RandomChance {
            success_percent: 50,
            on_success: Box::new(recipe_effect("work.retire")),
            on_failure: Box::new(Effect::SetWorldFlag {
                flag: "retired".to_owned(),
                value: true,
            }),
        };
        let mut source = draft(vec![
            simple_action("schedule", Condition::Always, vec![schedule("work.spoil")]),
            simple_action(
                "random",
                Condition::Always,
                vec![random, Effect::AdvanceTime { ticks: 2 }],
            ),
            simple_action(
                "wait",
                Condition::Always,
                vec![Effect::AdvanceTime { ticks: 2 }],
            ),
            simple_action(
                "free-space",
                Condition::Always,
                vec![
                    recipe_effect("work.clear"),
                    Effect::AdvanceTime { ticks: 2 },
                ],
            ),
        ]);
        source.recipes = vec![
            recipe("work.retire", &[("work.claim", 1)], &[("work.safe", 1)]),
            recipe("work.spoil", &[("work.claim", 1)], &[("work.spoiled", 1)]),
            recipe("work.clear", &[("work.spoiled", 1)], &[]),
        ];
        let event_effect = if random_outcome {
            Effect::RandomChance {
                success_percent: 50,
                on_success: Box::new(recipe_effect("work.spoil")),
                on_failure: Box::new(recipe_effect("work.retire")),
            }
        } else {
            recipe_effect("work.spoil")
        };
        source.deferred_events = vec![deferred("work.spoil", 2, guard, vec![event_effect])];
        CompiledContent::try_compile(source).unwrap()
    }

    #[test]
    fn deferred_recipe_preflight_preserves_guard_inventory_correlations_without_entropy_draws() {
        let content = guarded_recipe_content(
            Condition::HasItem {
                item: "work.claim".to_owned(),
                count: 1,
            },
            false,
        );
        let mut initial = state(&content);
        initial
            .character
            .inventory
            .insert("work.claim".to_owned(), 1);
        let pending = record_test_action(&initial, &content, "schedule").into_state();
        let mut outcomes = BTreeSet::new();
        for seed in 0..16 {
            let mut current = pending.clone();
            current.entropy = EntropyState::new(seed);
            let before = current.clone();
            let actions = enumerate_legal_actions(&current, &content).unwrap();
            assert_eq!(current, before);
            let action = actions
                .into_iter()
                .find(|action| action.definition_id == "random")
                .unwrap();
            let transition = step(&current, &action, &content, &current.entropy).unwrap();
            assert_eq!(transition.entropy_draws().len(), 1);
            assert!(
                !transition
                    .state()
                    .character
                    .inventory
                    .contains_key("work.claim")
            );
            if transition
                .state()
                .character
                .inventory
                .contains_key("work.safe")
            {
                outcomes.insert("safe");
            }
            if transition
                .state()
                .character
                .inventory
                .contains_key("work.spoiled")
            {
                outcomes.insert("spoiled");
            }
        }
        assert_eq!(outcomes, BTreeSet::from(["safe", "spoiled"]));
        let bad_content = guarded_recipe_content(
            Condition::Not {
                condition: Box::new(Condition::WorldFlag {
                    flag: "retired".to_owned(),
                }),
            },
            false,
        );
        let mut bad = state(&bad_content);
        bad.character.inventory.insert("work.claim".to_owned(), 1);
        bad = record_test_action(&bad, &bad_content, "schedule").into_state();
        for seed in 0..8 {
            bad.entropy = EntropyState::new(seed);
            assert!(!legal_ids(&bad, &bad_content).contains(&"random".to_owned()));
        }
    }

    #[test]
    fn deferred_recipe_preflight_checks_timed_random_branches_and_post_action_capacity() {
        for random_outcome in [false, true] {
            let content = guarded_recipe_content(
                Condition::HasItem {
                    item: "work.claim".to_owned(),
                    count: 1,
                },
                random_outcome,
            );
            let mut initial = state(&content);
            initial
                .character
                .inventory
                .insert("work.claim".to_owned(), 1);
            initial
                .character
                .inventory
                .insert("work.spoiled".to_owned(), u32::MAX);
            let pending = record_test_action(&initial, &content, "schedule").into_state();
            let before = pending.clone();
            assert!(!legal_ids(&pending, &content).contains(&"wait".to_owned()));
            assert_eq!(pending, before);
            let transition = record_test_action(&pending, &content, "free-space");
            assert!(
                !transition
                    .state()
                    .character
                    .inventory
                    .contains_key("work.claim")
            );
            assert!(transition.state().character.inventory["work.spoiled"] >= u32::MAX - 1);
            assert!(transition.state().world.scheduled_events.is_empty());
        }
    }

    #[test]
    fn deferred_due_guard_observes_earlier_event_changes() {
        let mut source = draft(vec![simple_action(
            "cross",
            Condition::Always,
            vec![schedule("work.spoil"), Effect::AdvanceTime { ticks: 2 }],
        )]);
        source.recipes = vec![recipe(
            "work.spoil",
            &[("work.claim", 1)],
            &[("work.spoiled", 1)],
        )];
        source.deferred_events = vec![deferred(
            "work.spoil",
            2,
            Condition::Not {
                condition: Box::new(Condition::WorldFlag {
                    flag: "retired".to_owned(),
                }),
            },
            vec![recipe_effect("work.spoil")],
        )];
        source.timed_events = vec![TimedEventDefinition {
            id: "absolute.retire".to_owned(),
            due_time: 2,
            event_kind: "Work".to_owned(),
            label: "Retire work".to_owned(),
            result: "The work stops.".to_owned(),
            condition: Condition::Always,
            effects: vec![Effect::SetWorldFlag {
                flag: "retired".to_owned(),
                value: true,
            }],
        }];
        let content = CompiledContent::try_compile(source).unwrap();
        let mut initial = state(&content);
        initial.world.scheduled_events = vec![crate::ScheduledEvent {
            id: "absolute.retire".to_owned(),
            due_time: 2,
            event_kind: "Work".to_owned(),
        }];
        let transition = record_test_action(&initial, &content, "cross");
        assert!(
            !transition
                .state()
                .character
                .inventory
                .contains_key("work.spoiled")
        );
        let resolutions: Vec<_> = transition
            .events()
            .iter()
            .filter_map(|event| match &event.kind {
                EventKind::ScheduledEventResolved {
                    event_id, applied, ..
                } => Some((event_id.as_str(), *applied)),
                _ => None,
            })
            .collect();
        assert_eq!(
            resolutions,
            vec![("absolute.retire", true), ("work.spoil", false)]
        );
    }

    #[test]
    fn deferred_recipe_late_failure_rolls_back_transition_schedule_stock_and_entropy() {
        let content = guarded_recipe_content(
            Condition::Not {
                condition: Box::new(Condition::WorldFlag {
                    flag: "retired".to_owned(),
                }),
            },
            false,
        );
        let mut initial = state(&content);
        initial
            .character
            .inventory
            .insert("work.claim".to_owned(), 1);
        initial = record_test_action(&initial, &content, "schedule").into_state();
        // The public catalog already rejects this uncertain program. Exercise
        // its immutable reduction directly to prove failure stays transactional.
        let mut failures = 0;
        for seed in 0..16 {
            initial.entropy = EntropyState::new(seed);
            let before = initial.clone();
            let action = CanonicalAction::new(
                content.build_id(),
                initial.state_id(),
                "random",
                BTreeMap::new(),
            );
            assert!(matches!(
                step(&initial, &action, &content, &initial.entropy),
                Err(KernelError::IllegalAction(_))
            ));
            if let Err(error) =
                reduce_validated_action(&initial, &action, &content, &initial.entropy)
            {
                assert!(
                    matches!(error, KernelError::InvalidAction(message) if message.contains("lacks 1 of item work.claim"))
                );
                failures += 1;
            }
            assert_eq!(initial, before);
        }
        assert!(failures > 0);
    }

    #[test]
    fn deferred_schedule_catalog_pages_preserve_more_than_256_canonical_candidates() {
        let mut action = simple_action(
            "schedule",
            Condition::Always,
            vec![
                transfer_effect(StringRef::Parameter("source".to_owned()), "work.token", 1),
                schedule("work.finish"),
            ],
        );
        action.parameters = vec![crate::ParameterSpec {
            name: "source".to_owned(),
            domain: ParameterDomain::NpcsAtCurrentLocation,
        }];
        let mut source = draft(vec![action]);
        source.deferred_events = vec![deferred(
            "work.finish",
            2,
            Condition::Always,
            vec![Effect::SetWorldFlag {
                flag: "done".to_owned(),
                value: true,
            }],
        )];
        for index in 0..300 {
            source.npcs.push(NpcDefinition {
                id: format!("worker-{index:03}"),
                name: "Worker".to_owned(),
                location: "gate".to_owned(),
                goals: BTreeSet::new(),
                values: BTreeSet::new(),
                tags: BTreeSet::new(),
                inventory: BTreeMap::from([("work.token".to_owned(), 1)]),
            });
        }
        let content = CompiledContent::try_compile(source).unwrap();
        let initial = state(&content);
        let actions = enumerate_legal_actions(&initial, &content).unwrap();
        assert_eq!(actions.len(), 300);
        let digest = legal_action_digest(&actions).unwrap();
        let mut ids = Vec::new();
        for offset in (0..300).step_by(17) {
            let page = content.action_page(&initial, offset, 17).unwrap();
            assert_eq!(page.total, 300);
            assert_eq!(page.digest, digest);
            ids.extend(page.actions.into_iter().map(|view| view.action_id));
        }
        assert_eq!(
            ids,
            actions
                .iter()
                .map(|action| action.action_id.clone())
                .collect::<Vec<_>>()
        );
        let transition = step(&initial, &actions[299], &content, &initial.entropy).unwrap();
        assert_eq!(transition.state().world.scheduled_events.len(), 1);
        assert!(
            enumerate_legal_actions(transition.state(), &content)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn parameter_domains_are_complete_and_semantically_sorted() {
        let action = ActionDefinition {
            id: "move-and-greet".to_owned(),
            label: "Move and greet".to_owned(),
            category: "Social".to_owned(),
            result: "The exchange is complete.".to_owned(),
            result_variants: Vec::new(),
            locations: vec!["gate".to_owned()],
            condition: Condition::Always,
            effects: vec![
                Effect::MoveCharacter {
                    location: StringRef::Parameter("destination".to_owned()),
                },
                Effect::AdjustNpcRelationship {
                    npc: StringRef::Parameter("npc".to_owned()),
                    amount: 1,
                },
            ],
            parameters: vec![
                crate::ParameterSpec {
                    name: "destination".to_owned(),
                    domain: ParameterDomain::Values(vec!["yard".to_owned(), "gate".to_owned()]),
                },
                crate::ParameterSpec {
                    name: "npc".to_owned(),
                    domain: ParameterDomain::NpcsAtCurrentLocation,
                },
            ],
            meaningful: true,
            movement: true,
        };
        let compiled = content(vec![action]);
        let actions = enumerate_legal_actions(&state(&compiled), &compiled).unwrap();
        assert_eq!(actions.len(), 2);
        assert_eq!(
            actions
                .iter()
                .map(|action| action.parameters["destination"].as_str())
                .collect::<Vec<_>>(),
            vec!["gate", "yard"]
        );
        let mut reversed = actions.clone();
        reversed.reverse();
        assert_eq!(
            legal_action_digest(&actions).unwrap(),
            legal_action_digest(&reversed).unwrap()
        );
    }

    #[test]
    fn combinatorial_conditions_and_knowledge_gate() {
        let mut initial = state(&content(vec![simple_action(
            "secret",
            Condition::All {
                conditions: vec![
                    Condition::Any {
                        conditions: vec![
                            Condition::FacetEquals {
                                axis: "background".to_owned(),
                                value: crate::FacetValue::Text("clerk".to_owned()),
                            },
                            Condition::HasTag {
                                tag: "lock-runner".to_owned(),
                            },
                        ],
                    },
                    Condition::NpcKnows {
                        npc: "sava".to_owned(),
                        knowledge_id: "forged-order".to_owned(),
                    },
                ],
            },
            vec![Effect::Noop],
        )]));
        assert!(
            enumerate_legal_actions(&initial, &content(vec![])).is_err(),
            "a mismatched content pack is never accepted"
        );
        let content = content(vec![simple_action(
            "secret",
            Condition::All {
                conditions: vec![
                    Condition::FacetEquals {
                        axis: "background".to_owned(),
                        value: crate::FacetValue::Text("clerk".to_owned()),
                    },
                    Condition::NpcKnows {
                        npc: "sava".to_owned(),
                        knowledge_id: "forged-order".to_owned(),
                    },
                ],
            },
            vec![Effect::Noop],
        )]);
        // Rebuild the state against the same content identity for the actual gate check.
        initial = state(&content);
        assert!(
            enumerate_legal_actions(&initial, &content)
                .unwrap()
                .is_empty()
        );
        initial
            .world
            .npcs
            .get_mut("sava")
            .unwrap()
            .knowledge
            .insert(
                "forged-order".to_owned(),
                crate::Knowledge {
                    id: "forged-order".to_owned(),
                    subject: "The order is forged".to_owned(),
                    turn: 0,
                    provenance: crate::KnowledgeProvenance::Witnessed,
                },
            );
        assert_eq!(
            enumerate_legal_actions(&initial, &content).unwrap().len(),
            1
        );
    }

    #[test]
    fn enumerates_exactly_256_unique_actions_and_pages_cover_the_vector() {
        let destinations: Vec<_> = (0..256).map(|index| format!("stress-{index:03}")).collect();
        let stress = ActionDefinition {
            id: "stress".to_owned(),
            label: "Stress".to_owned(),
            category: "Travel".to_owned(),
            result: "The route is open.".to_owned(),
            result_variants: Vec::new(),
            locations: vec!["gate".to_owned()],
            condition: Condition::Always,
            effects: vec![Effect::MoveCharacter {
                location: StringRef::Parameter("destination".to_owned()),
            }],
            parameters: vec![crate::ParameterSpec {
                name: "destination".to_owned(),
                domain: ParameterDomain::LocationsAdjacent,
            }],
            meaningful: true,
            movement: true,
        };
        let mut locations = vec![LocationDefinition {
            id: "gate".to_owned(),
            name: "Gate".to_owned(),
            description: "Many roads start here.".to_owned(),
            description_variants: Vec::new(),
            exits: destinations.clone(),
            terminal: true,
        }];
        locations.extend(destinations.iter().map(|id| LocationDefinition {
            id: id.clone(),
            name: id.clone(),
            description: "A marked test point waits here.".to_owned(),
            description_variants: Vec::new(),
            exits: vec!["gate".to_owned()],
            terminal: true,
        }));
        let content = CompiledContent::try_compile(ContentDraft {
            schema_version: "forge-schema-v9".to_owned(),
            rules_version: "forge-rules-v7".to_owned(),
            world_id: "world-1".to_owned(),
            contract: crate::ContentContract::Fixture,
            start_location: "gate".to_owned(),
            character_presets: Vec::new(),
            character_creation: None,
            supply_labels: Default::default(),
            recipes: Vec::new(),
            locations,
            npcs: vec![NpcDefinition {
                id: "sava".to_owned(),
                name: "Sava".to_owned(),
                location: "gate".to_owned(),
                goals: BTreeSet::new(),
                values: BTreeSet::new(),
                tags: BTreeSet::new(),
                inventory: BTreeMap::new(),
            }],
            timed_events: Vec::new(),
            deferred_events: Vec::new(),
            actions: vec![stress],
        })
        .unwrap();
        let all = enumerate_legal_actions(&state(&content), &content).unwrap();
        assert_eq!(all.len(), 256);

        let unique_ids: BTreeSet<_> = all.iter().map(|action| action.action_id.as_str()).collect();
        assert_eq!(unique_ids.len(), 256);
        let ordered_destinations: Vec<_> = all
            .iter()
            .map(|action| action.parameters["destination"].as_str())
            .collect();
        assert_eq!(
            ordered_destinations,
            destinations.iter().map(String::as_str).collect::<Vec<_>>()
        );

        let mut paged = Vec::new();
        for page in all.chunks(17) {
            paged.extend_from_slice(page);
        }
        assert_eq!(paged, all);
        assert_eq!(
            legal_action_digest(&all).unwrap(),
            legal_action_digest(&paged).unwrap()
        );
    }

    #[test]
    fn cardinality_overflow_and_reservation_failure_are_fallible() {
        let overflow = checked_combination_count([usize::MAX, 2], "overflow");
        assert!(matches!(overflow, Err(KernelError::ResourceExhausted(_))));

        let zero = checked_combination_count([usize::MAX, 0, usize::MAX], "zero").unwrap();
        assert_eq!(zero, 0);

        let mut output: Vec<u8> = Vec::new();
        let error = try_reserve(&mut output, usize::MAX, "test reservation").unwrap_err();
        assert!(matches!(error, KernelError::ResourceExhausted(_)));
        assert!(output.is_empty());
    }

    #[test]
    fn legal_action_digest_has_a_golden_value_and_is_order_independent() {
        let action = |id: &str| CanonicalAction {
            action_id: id.to_owned(),
            build_id: "build".to_owned(),
            pre_state_id: "state".to_owned(),
            definition_id: id.to_owned(),
            parameters: BTreeMap::new(),
        };
        let a = action("a");
        let b = action("b");
        let expected = "0473ef2dc0d324ab659d3580c1134e9d812035905c4781fdd6d529b0c6860e13";
        assert_eq!(
            legal_action_digest(&[a.clone(), b.clone()]).unwrap(),
            expected
        );
        assert_eq!(legal_action_digest(&[b, a]).unwrap(), expected);
        assert_ne!(
            legal_action_digest(&[action("a"), action("c")]).unwrap(),
            expected
        );
    }
}
