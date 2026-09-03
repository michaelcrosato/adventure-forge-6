use crate::content::{CompiledContent, Effect, ParameterDomain, StringRef};
use crate::hash::{HashError, sha256_json};
use crate::model::{ActionId, Event, EventKind, GameState, Knowledge, Memory};
use crate::{EntropyDraw, EntropyError, EntropyState};
use serde::{Deserialize, Serialize};
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
/// product and then stably sorted by canonical identity.
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
    let mut total_actions = 0usize;

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
        total_actions = total_actions.checked_add(combinations).ok_or_else(|| {
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
    try_reserve(&mut actions, total_actions, "legal action list")?;
    for candidate in candidates {
        append_parameter_combinations(
            candidate.definition,
            &candidate.domains,
            candidate.combinations,
            content.build_id(),
            &pre_state_id,
            &mut actions,
        )?;
    }
    actions.sort_by(|left, right| left.action_id.cmp(&right.action_id));
    if actions.len() != total_actions {
        return Err(KernelError::InvalidContent(format!(
            "legal action enumeration count mismatch: expected {total_actions}, emitted {}",
            actions.len()
        )));
    }
    if actions
        .windows(2)
        .any(|window| window[0].action_id == window[1].action_id)
    {
        return Err(KernelError::InvalidContent(
            "legal action identity collision".to_owned(),
        ));
    }
    Ok(actions)
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
    output: &mut Vec<CanonicalAction>,
) -> Result<(), KernelError> {
    if expected == 0 {
        return Ok(());
    }

    let mut indexes = Vec::new();
    try_reserve(&mut indexes, domains.len(), "parameter enumeration indexes")?;
    indexes.extend(std::iter::repeat_n(0usize, domains.len()));

    let mut emitted = 0usize;
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
        output.push(CanonicalAction::new(
            build_id.to_owned(),
            pre_state_id.to_owned(),
            definition.id.clone(),
            parameters,
        ));
        emitted = emitted.checked_add(1).ok_or_else(|| {
            KernelError::ResourceExhausted(format!(
                "enumerated action count overflow for action {}",
                definition.id
            ))
        })?;

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

    if emitted != expected {
        return Err(KernelError::InvalidContent(format!(
            "parameter enumeration count mismatch for action {}: expected {expected}, emitted {emitted}",
            definition.id
        )));
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
    next.entropy = entropy_cursor.clone();
    next.event_log.extend(events.iter().cloned());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::{
        ActionDefinition, Condition, ContentDraft, Effect, LocationDefinition, NpcDefinition,
    };
    use crate::model::{Character, LocationRuntime, NpcState, WorldState};
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

    fn content(actions: Vec<ActionDefinition>) -> CompiledContent {
        CompiledContent::try_compile(ContentDraft {
            schema_version: "forge-schema-v2".to_owned(),
            rules_version: "forge-rules-v1".to_owned(),
            world_id: "world-1".to_owned(),
            contract: crate::ContentContract::Fixture,
            start_location: "gate".to_owned(),
            character_presets: Vec::new(),
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
            }],
            actions,
        })
        .unwrap()
    }

    fn state(content: &CompiledContent) -> GameState {
        let mut locations = content.empty_location_runtime();
        locations.insert("gate".to_owned(), LocationRuntime::default());
        locations
            .get_mut("gate")
            .unwrap()
            .entities
            .insert("sava".to_owned());
        let npcs = BTreeMap::from([(
            "sava".to_owned(),
            NpcState {
                id: "sava".to_owned(),
                location: "gate".to_owned(),
                goals: BTreeSet::new(),
                values: BTreeSet::new(),
                tags: BTreeSet::new(),
                relationships: BTreeMap::new(),
                memories: BTreeMap::new(),
                knowledge: BTreeMap::new(),
                inventory: BTreeMap::new(),
                suspicion: 0,
            },
        )]);
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
            vec![Effect::AdjustResource {
                resource: "coin".to_owned(),
                amount: 1,
            }],
        )]);
        let mut overflowing = state(&overflow_content);
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
    fn parameter_domains_are_complete_and_sorted() {
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
        assert!(
            actions
                .windows(2)
                .all(|window| window[0].action_id < window[1].action_id)
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
            schema_version: "forge-schema-v2".to_owned(),
            rules_version: "forge-rules-v1".to_owned(),
            world_id: "world-1".to_owned(),
            contract: crate::ContentContract::Fixture,
            start_location: "gate".to_owned(),
            character_presets: Vec::new(),
            locations,
            npcs: vec![NpcDefinition {
                id: "sava".to_owned(),
                name: "Sava".to_owned(),
                location: "gate".to_owned(),
                goals: BTreeSet::new(),
                values: BTreeSet::new(),
                tags: BTreeSet::new(),
            }],
            actions: vec![stress],
        })
        .unwrap();
        let all = enumerate_legal_actions(&state(&content), &content).unwrap();
        assert_eq!(all.len(), 256);

        let unique_ids: BTreeSet<_> = all.iter().map(|action| action.action_id.as_str()).collect();
        assert_eq!(unique_ids.len(), 256);

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
