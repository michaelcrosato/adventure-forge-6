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

#[derive(Default)]
struct ItemTransferRequirements {
    source: BTreeMap<(String, String), u64>,
    received: BTreeMap<String, u64>,
    impossible: bool,
}

impl ItemTransferRequirements {
    fn add_source(&mut self, source: String, item: String, count: u64) {
        let key = (source, item);
        let current = self.source.get(&key).copied().unwrap_or_default();
        let Some(total) = current.checked_add(count) else {
            self.impossible = true;
            return;
        };
        self.source.insert(key, total);
    }

    fn add_received(&mut self, item: String, count: u64) {
        let current = self.received.get(&item).copied().unwrap_or_default();
        let Some(total) = current.checked_add(count) else {
            self.impossible = true;
            return;
        };
        self.received.insert(item, total);
    }

    fn add_sequential(&mut self, other: Self) {
        self.impossible |= other.impossible;
        for ((source, item), count) in other.source {
            self.add_source(source, item, count);
        }
        for (item, count) in other.received {
            self.add_received(item, count);
        }
    }

    fn pointwise_max(&mut self, other: Self) {
        self.impossible |= other.impossible;
        for (key, count) in other.source {
            self.source
                .entry(key)
                .and_modify(|current| *current = (*current).max(count))
                .or_insert(count);
        }
        for (item, count) in other.received {
            self.received
                .entry(item)
                .and_modify(|current| *current = (*current).max(count))
                .or_insert(count);
        }
    }
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
            content.build_id(),
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
    build_id: &str,
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
        if item_transfer_candidate_available(definition, &parameters, state)? {
            output.push(CanonicalAction::new(
                build_id.to_owned(),
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

fn item_transfer_candidate_available(
    definition: &crate::ActionDefinition,
    parameters: &BTreeMap<String, String>,
    state: &GameState,
) -> Result<bool, KernelError> {
    if !contains_item_transfer(&definition.effects) {
        return Ok(true);
    }
    let requirements = item_transfer_requirements(&definition.effects, parameters)?;
    if requirements.impossible {
        return Ok(false);
    }
    for ((source, item), required) in requirements.source {
        let available = state
            .world
            .npcs
            .get(&source)
            .and_then(|npc| npc.inventory.get(&item))
            .copied()
            .map(u64::from)
            .unwrap_or_default();
        if available < required {
            return Ok(false);
        }
    }
    for (item, received) in requirements.received {
        let current = state
            .character
            .inventory
            .get(&item)
            .copied()
            .map(u64::from)
            .unwrap_or_default();
        let Some(total) = current.checked_add(received) else {
            return Ok(false);
        };
        if total > u64::from(u32::MAX) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn contains_item_transfer(effects: &[Effect]) -> bool {
    effects.iter().any(|effect| match effect {
        Effect::TransferNpcItemToCharacter { .. } => true,
        Effect::RandomChance {
            on_success,
            on_failure,
            ..
        } => {
            contains_item_transfer(std::slice::from_ref(on_success))
                || contains_item_transfer(std::slice::from_ref(on_failure))
        }
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
        | Effect::AddCharacterDeed { .. }
        | Effect::AdvanceTime { .. } => false,
    })
}

fn item_transfer_requirements(
    effects: &[Effect],
    parameters: &BTreeMap<String, String>,
) -> Result<ItemTransferRequirements, KernelError> {
    let mut requirements = ItemTransferRequirements::default();
    for effect in effects {
        requirements.add_sequential(item_transfer_effect_requirements(effect, parameters)?);
    }
    Ok(requirements)
}

fn item_transfer_effect_requirements(
    effect: &Effect,
    parameters: &BTreeMap<String, String>,
) -> Result<ItemTransferRequirements, KernelError> {
    match effect {
        Effect::TransferNpcItemToCharacter { npc, item, count } => {
            let source = resolve_ref(npc, parameters)?;
            let mut requirements = ItemTransferRequirements::default();
            requirements.add_source(source, item.clone(), u64::from(*count));
            requirements.add_received(item.clone(), u64::from(*count));
            Ok(requirements)
        }
        Effect::RandomChance {
            success_percent,
            on_success,
            on_failure,
        } => match success_percent {
            0 => item_transfer_effect_requirements(on_failure, parameters),
            100 => item_transfer_effect_requirements(on_success, parameters),
            _ => {
                let mut requirements = item_transfer_effect_requirements(on_success, parameters)?;
                requirements
                    .pointwise_max(item_transfer_effect_requirements(on_failure, parameters)?);
                Ok(requirements)
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
        | Effect::AddCharacterDeed { .. }
        | Effect::AdvanceTime { .. } => Ok(ItemTransferRequirements::default()),
    }
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
        let definition = content.timed_event(&scheduled.id).ok_or_else(|| {
            KernelError::InvalidState(format!("unknown timed event {}", scheduled.id))
        })?;
        if definition.due_time != scheduled.due_time
            || definition.event_kind != scheduled.event_kind
        {
            return Err(KernelError::InvalidState(format!(
                "timed event {} differs from compiled content",
                scheduled.id
            )));
        }
        let applied = definition.condition.evaluate(state);
        events.push(Event {
            turn: state.world.time,
            kind: EventKind::ScheduledEventResolved {
                event_id: scheduled.id,
                event_kind: scheduled.event_kind,
                applied,
            },
        });
        if applied {
            for effect in &definition.effects {
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
            schema_version: "forge-schema-v6".to_owned(),
            rules_version: "forge-rules-v4".to_owned(),
            world_id: "world-1".to_owned(),
            contract: crate::ContentContract::Fixture,
            start_location: "gate".to_owned(),
            character_presets: Vec::new(),
            character_creation: None,
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
            schema_version: "forge-schema-v6".to_owned(),
            rules_version: "forge-rules-v4".to_owned(),
            world_id: "world-1".to_owned(),
            contract: crate::ContentContract::Fixture,
            start_location: "gate".to_owned(),
            character_presets: Vec::new(),
            character_creation: None,
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
