use crate::build_manifest::BuildManifest;
use crate::hash::sha256_json;
use crate::model::{
    Character, CharacterChoiceSelection, CharacterSelection, CharacterStart, EventKind, FacetValue,
    GameState, KnowledgeProvenance, KnowledgeProvenanceKind, LocationId, NpcId, NpcState,
    ScheduledEvent, is_reserved_character_facet_axis,
};
use crate::{EntropyState, LocationRuntime};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::{Display, Formatter};

const MAX_CREATION_SLOTS: usize = 16;
const MAX_CREATION_CHOICES_PER_SLOT: usize = 64;
const MAX_TIMED_EVENTS: usize = 64;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LocationDefinition {
    pub id: LocationId,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub description_variants: Vec<TextVariant>,
    #[serde(default)]
    pub exits: Vec<LocationId>,
    #[serde(default)]
    pub terminal: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NpcDefinition {
    pub id: NpcId,
    pub name: String,
    pub location: LocationId,
    #[serde(default)]
    pub goals: BTreeSet<String>,
    #[serde(default)]
    pub values: BTreeSet<String>,
    #[serde(default)]
    pub tags: BTreeSet<String>,
}

/// A short, data-authored description selected against the authoritative
/// state. Variants are ordered by descending priority and then ascending ID.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TextVariant {
    pub id: String,
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub condition: Condition,
    pub text: String,
}

/// Whether this source is a complete production pack or a deliberately small
/// fixture. Production packs must carry the character-selection contract;
/// fixture is the serde default to keep focused kernel fixtures concise.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContentContract {
    #[default]
    Fixture,
    Production,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CharacterPreset {
    pub id: String,
    pub display_name: String,
    pub summary: String,
    pub character: Character,
}

/// A closed, typed contribution made by an authored creation choice.
/// Fields are merged by the kernel; clients never submit this structure.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct CharacterPatch {
    pub lineage: Option<String>,
    pub origin: Option<String>,
    pub background: Option<String>,
    pub aptitudes: BTreeMap<String, i64>,
    pub skills: BTreeSet<String>,
    pub values: BTreeSet<String>,
    pub traits: BTreeSet<String>,
    pub flaws: BTreeSet<String>,
    pub appearance: BTreeMap<String, String>,
    pub affiliations: BTreeMap<String, i64>,
    pub reputation: BTreeMap<String, i64>,
    pub knowledge: BTreeSet<String>,
    pub inventory: BTreeMap<String, u32>,
    pub resources: BTreeMap<String, i64>,
    pub injuries: BTreeSet<String>,
    pub deeds: BTreeSet<String>,
    pub promises: BTreeSet<String>,
    pub discoveries: BTreeSet<String>,
    pub facets: BTreeMap<String, FacetValue>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CharacterCreationChoice {
    pub id: String,
    pub display_name: String,
    pub summary: String,
    pub patch: CharacterPatch,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CharacterCreationSlot {
    pub id: String,
    pub order: u16,
    pub display_name: String,
    pub choices: Vec<CharacterCreationChoice>,
}

/// Finite authored axes from which the kernel can materialize custom starts.
/// The Cartesian product is not precompiled or trusted as client data.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CharacterCreationDefinition {
    #[serde(default)]
    pub base: CharacterPatch,
    pub slots: Vec<CharacterCreationSlot>,
}

/// A deterministic content-authored event resolved by the kernel after an
/// accepted canonical action advances world time to or beyond `due_time`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TimedEventDefinition {
    pub id: String,
    pub due_time: u64,
    pub event_kind: String,
    pub label: String,
    pub result: String,
    #[serde(default)]
    pub condition: Condition,
    #[serde(default)]
    pub effects: Vec<Effect>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TimedEventView {
    pub label: String,
    pub remaining_ticks: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub build_id: String,
    pub state_id: String,
    pub location_id: LocationId,
    pub title: String,
    /// Result-first, then the current location description.
    pub text: String,
    pub result: Option<String>,
    pub world_time: u64,
    pub upcoming_events: Vec<TimedEventView>,
    pub action_set_digest: String,
    pub action_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActionView {
    pub action_id: String,
    pub definition_id: String,
    pub label: String,
    pub category: String,
    pub time_cost: ActionTimeCost,
    /// Present only for decisions whose authored result is a consequence the
    /// player should see before committing (currently outcomes and endings).
    pub consequence_preview: Option<String>,
    /// Player-facing names for canonical parameter values. The canonical
    /// values remain in `parameters` and continue to define action identity.
    pub parameter_display_values: BTreeMap<String, String>,
    pub parameters: BTreeMap<String, String>,
}

/// The complete range of world-time advancement possible for one canonical
/// action. Most actions have an exact cost; randomized branches may expose a
/// bounded range without revealing which branch entropy will select.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActionTimeCost {
    pub minimum_ticks: u64,
    pub maximum_ticks: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActionPage {
    pub build_id: String,
    pub state_id: String,
    pub actions: Vec<ActionView>,
    pub total: usize,
    pub digest: String,
    pub offset: usize,
    pub next_offset: Option<usize>,
}

/// Untrusted authoring input. A `CompiledContent` can only be created by
/// `try_compile`, which performs all semantic checks in the kernel.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ContentDraft {
    pub schema_version: String,
    pub rules_version: String,
    pub world_id: String,
    #[serde(default)]
    pub contract: ContentContract,
    #[serde(default)]
    pub start_location: LocationId,
    #[serde(default)]
    pub character_presets: Vec<CharacterPreset>,
    #[serde(default)]
    pub character_creation: Option<CharacterCreationDefinition>,
    #[serde(default)]
    pub locations: Vec<LocationDefinition>,
    #[serde(default)]
    pub npcs: Vec<NpcDefinition>,
    #[serde(default)]
    pub timed_events: Vec<TimedEventDefinition>,
    #[serde(default)]
    pub actions: Vec<ActionDefinition>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentValidationError {
    pub issues: Vec<String>,
}

impl ContentValidationError {
    pub(crate) fn new() -> Self {
        Self { issues: Vec::new() }
    }

    pub(crate) fn push(&mut self, message: impl Into<String>) {
        self.issues.push(message.into());
    }
}

impl Display for ContentValidationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for (index, issue) in self.issues.iter().enumerate() {
            if index != 0 {
                f.write_str("; ")?;
            }
            f.write_str(issue)?;
        }
        Ok(())
    }
}

impl std::error::Error for ContentValidationError {}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ParameterSpec {
    pub name: String,
    pub domain: ParameterDomain,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    content = "values",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ParameterDomain {
    Values(Vec<String>),
    InventoryItems,
    NpcsAtCurrentLocation,
    LocationsAdjacent,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActionDefinition {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub result: String,
    #[serde(default)]
    pub result_variants: Vec<TextVariant>,
    #[serde(default)]
    pub locations: Vec<LocationId>,
    #[serde(default)]
    pub condition: Condition,
    #[serde(default)]
    pub effects: Vec<Effect>,
    #[serde(default)]
    pub parameters: Vec<ParameterSpec>,
    pub meaningful: bool,
    pub movement: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Condition {
    #[default]
    Always,
    Never,
    All {
        conditions: Vec<Condition>,
    },
    Any {
        conditions: Vec<Condition>,
    },
    Not {
        condition: Box<Condition>,
    },
    FacetEquals {
        axis: String,
        value: FacetValue,
    },
    FacetAtLeast {
        axis: String,
        value: i64,
    },
    HasTag {
        tag: String,
    },
    HasItem {
        item: String,
        count: u32,
    },
    ResourceAtLeast {
        resource: String,
        amount: i64,
    },
    CharacterKnows {
        knowledge_id: String,
    },
    CharacterHasDeed {
        deed_id: String,
    },
    WorldFlag {
        flag: String,
    },
    LocationFlag {
        location: LocationId,
        flag: String,
    },
    AtLocation {
        location: LocationId,
    },
    NpcKnows {
        npc: NpcId,
        knowledge_id: String,
    },
    NpcKnowsWithProvenance {
        npc: NpcId,
        knowledge_id: String,
        provenance: KnowledgeProvenanceKind,
    },
    NpcRemembers {
        npc: NpcId,
        memory_id: String,
    },
    NpcRelationshipAtLeast {
        npc: NpcId,
        amount: i64,
    },
}

impl Condition {
    pub fn evaluate(&self, state: &GameState) -> bool {
        match self {
            Self::Always => true,
            Self::Never => false,
            Self::All { conditions } => {
                conditions.iter().all(|condition| condition.evaluate(state))
            }
            Self::Any { conditions } => {
                conditions.iter().any(|condition| condition.evaluate(state))
            }
            Self::Not { condition } => !condition.evaluate(state),
            Self::FacetEquals { axis, value } => {
                state.character.facet_value(axis).as_ref() == Some(value)
            }
            Self::FacetAtLeast { axis, value } => state
                .character
                .facet_value(axis)
                .and_then(|facet| match facet {
                    FacetValue::Number(number) => Some(number),
                    _ => None,
                })
                .is_some_and(|number| number >= *value),
            Self::HasTag { tag } => state.character.has_tag(tag),
            Self::HasItem { item, count } => state
                .character
                .inventory
                .get(item)
                .copied()
                .is_some_and(|available| available >= *count),
            Self::ResourceAtLeast { resource, amount } => {
                state
                    .character
                    .resources
                    .get(resource)
                    .copied()
                    .unwrap_or_default()
                    >= *amount
            }
            Self::CharacterKnows { knowledge_id } => {
                state.character.knowledge.contains(knowledge_id)
            }
            Self::CharacterHasDeed { deed_id } => state.character.deeds.contains(deed_id),
            Self::WorldFlag { flag } => state.world.flags.contains(flag),
            Self::LocationFlag { location, flag } => state
                .world
                .locations
                .get(location)
                .is_some_and(|runtime| runtime.flags.contains(flag)),
            Self::AtLocation { location } => state.world.current_location == *location,
            Self::NpcKnows { npc, knowledge_id } => state
                .world
                .npcs
                .get(npc)
                .is_some_and(|npc_state| npc_state.knows(knowledge_id)),
            Self::NpcKnowsWithProvenance {
                npc,
                knowledge_id,
                provenance,
            } => state
                .world
                .npcs
                .get(npc)
                .and_then(|npc_state| npc_state.knowledge.get(knowledge_id))
                .is_some_and(|knowledge| knowledge.provenance.kind() == *provenance),
            Self::NpcRemembers { npc, memory_id } => state
                .world
                .npcs
                .get(npc)
                .is_some_and(|npc_state| npc_state.remembers(memory_id)),
            Self::NpcRelationshipAtLeast { npc, amount } => {
                state
                    .world
                    .npcs
                    .get(npc)
                    .and_then(|npc_state| npc_state.relationships.get("player"))
                    .copied()
                    .unwrap_or_default()
                    >= *amount
            }
        }
    }

    fn is_obviously_never(&self) -> bool {
        self.is_obviously_never_at_depth(0)
    }

    fn is_obviously_never_at_depth(&self, depth: usize) -> bool {
        // Validation rejects deeper trees. Keep this helper bounded too, so a
        // malformed draft cannot force an unbounded recursive walk while the
        // area contract is being checked.
        if depth > 64 {
            return false;
        }
        match self {
            Self::Never => true,
            Self::Always
            | Self::FacetEquals { .. }
            | Self::FacetAtLeast { .. }
            | Self::HasTag { .. }
            | Self::HasItem { .. }
            | Self::ResourceAtLeast { .. }
            | Self::CharacterKnows { .. }
            | Self::CharacterHasDeed { .. }
            | Self::WorldFlag { .. }
            | Self::LocationFlag { .. }
            | Self::AtLocation { .. }
            | Self::NpcKnows { .. }
            | Self::NpcKnowsWithProvenance { .. }
            | Self::NpcRemembers { .. }
            | Self::NpcRelationshipAtLeast { .. } => false,
            Self::All { conditions } => conditions
                .iter()
                .any(|condition| condition.is_obviously_never_at_depth(depth + 1)),
            Self::Any { conditions } => {
                !conditions.is_empty()
                    && conditions
                        .iter()
                        .all(|condition| condition.is_obviously_never_at_depth(depth + 1))
            }
            Self::Not { condition } => matches!(condition.as_ref(), Self::Always),
        }
    }

    fn contains_never(&self) -> bool {
        let mut pending = vec![self];
        while let Some(condition) = pending.pop() {
            match condition {
                Self::Never => return true,
                Self::All { conditions } | Self::Any { conditions } => {
                    pending.extend(conditions.iter());
                }
                Self::Not { condition } => pending.push(condition),
                Self::Always
                | Self::FacetEquals { .. }
                | Self::FacetAtLeast { .. }
                | Self::HasTag { .. }
                | Self::HasItem { .. }
                | Self::ResourceAtLeast { .. }
                | Self::CharacterKnows { .. }
                | Self::CharacterHasDeed { .. }
                | Self::WorldFlag { .. }
                | Self::LocationFlag { .. }
                | Self::AtLocation { .. }
                | Self::NpcKnows { .. }
                | Self::NpcKnowsWithProvenance { .. }
                | Self::NpcRemembers { .. }
                | Self::NpcRelationshipAtLeast { .. } => {}
            }
        }
        false
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum StringRef {
    Literal(String),
    Parameter(String),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Effect {
    Noop,
    SetFlag {
        flag: String,
        value: bool,
    },
    SetWorldFlag {
        flag: String,
        value: bool,
    },
    SetLocationFlag {
        location: StringRef,
        flag: String,
        value: bool,
    },
    AdjustResource {
        resource: String,
        amount: i64,
    },
    MoveCharacter {
        location: StringRef,
    },
    AdjustNpcRelationship {
        npc: StringRef,
        amount: i64,
    },
    AddNpcMemory {
        npc: StringRef,
        memory_id: String,
        subject: String,
        provenance: KnowledgeProvenance,
    },
    TeachNpc {
        npc: StringRef,
        knowledge_id: String,
        subject: String,
        provenance: KnowledgeProvenance,
    },
    AddCharacterDeed {
        deed_id: String,
    },
    AdvanceTime {
        ticks: u64,
    },
    RandomChance {
        success_percent: u8,
        on_success: Box<Effect>,
        on_failure: Box<Effect>,
    },
}

impl Effect {
    fn changes_state(&self) -> bool {
        match self {
            Self::Noop => false,
            // A chance with no state-changing branch is not a meaningful
            // authored action, even though a later runtime may record a draw.
            Self::RandomChance {
                on_success,
                on_failure,
                ..
            } => on_success.changes_state() || on_failure.changes_state(),
            Self::SetFlag { .. }
            | Self::SetWorldFlag { .. }
            | Self::SetLocationFlag { .. }
            | Self::AdjustResource { .. }
            | Self::MoveCharacter { .. }
            | Self::AdjustNpcRelationship { .. }
            | Self::AddNpcMemory { .. }
            | Self::TeachNpc { .. }
            | Self::AddCharacterDeed { .. }
            | Self::AdvanceTime { .. } => true,
        }
    }
}

fn effect_time_cost(effect: &Effect) -> Option<ActionTimeCost> {
    match effect {
        Effect::AdvanceTime { ticks } => Some(ActionTimeCost {
            minimum_ticks: *ticks,
            maximum_ticks: *ticks,
        }),
        Effect::RandomChance {
            success_percent,
            on_success,
            on_failure,
        } => {
            let success = effect_time_cost(on_success)?;
            let failure = effect_time_cost(on_failure)?;
            match success_percent {
                0 => Some(failure),
                100 => Some(success),
                _ => Some(ActionTimeCost {
                    minimum_ticks: success.minimum_ticks.min(failure.minimum_ticks),
                    maximum_ticks: success.maximum_ticks.max(failure.maximum_ticks),
                }),
            }
        }
        Effect::Noop
        | Effect::SetFlag { .. }
        | Effect::SetWorldFlag { .. }
        | Effect::SetLocationFlag { .. }
        | Effect::AdjustResource { .. }
        | Effect::MoveCharacter { .. }
        | Effect::AdjustNpcRelationship { .. }
        | Effect::AddNpcMemory { .. }
        | Effect::TeachNpc { .. }
        | Effect::AddCharacterDeed { .. } => Some(ActionTimeCost {
            minimum_ticks: 0,
            maximum_ticks: 0,
        }),
    }
}

fn action_time_cost(effects: &[Effect]) -> Option<ActionTimeCost> {
    effects.iter().try_fold(
        ActionTimeCost {
            minimum_ticks: 0,
            maximum_ticks: 0,
        },
        |total, effect| {
            let cost = effect_time_cost(effect)?;
            Some(ActionTimeCost {
                minimum_ticks: total.minimum_ticks.checked_add(cost.minimum_ticks)?,
                maximum_ticks: total.maximum_ticks.checked_add(cost.maximum_ticks)?,
            })
        },
    )
}

#[derive(Serialize)]
struct ContentIdentity<'a> {
    manifest: &'a BuildManifest,
    contract: ContentContract,
    world_id: &'a str,
    start_location: &'a LocationId,
    character_presets: &'a BTreeMap<String, CharacterPreset>,
    character_creation: &'a Option<CharacterCreationDefinition>,
    locations: &'a BTreeMap<LocationId, LocationDefinition>,
    npcs: &'a BTreeMap<NpcId, NpcDefinition>,
    timed_events: &'a BTreeMap<String, TimedEventDefinition>,
    actions: &'a BTreeMap<String, ActionDefinition>,
}

/// Validated, normalized content. Fields are private and this type has no
/// `Deserialize` implementation, forcing all construction through the kernel.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct CompiledContent {
    manifest: BuildManifest,
    contract: ContentContract,
    world_id: String,
    start_location: LocationId,
    character_presets: BTreeMap<String, CharacterPreset>,
    character_creation: Option<CharacterCreationDefinition>,
    locations: BTreeMap<LocationId, LocationDefinition>,
    npcs: BTreeMap<NpcId, NpcDefinition>,
    timed_events: BTreeMap<String, TimedEventDefinition>,
    actions: BTreeMap<String, ActionDefinition>,
    build_id: String,
}

impl CompiledContent {
    pub fn try_compile(draft: ContentDraft) -> Result<Self, ContentValidationError> {
        let manifest = BuildManifest::generated();
        validate_draft(&draft, &manifest)?;
        let start_location = if draft.start_location.trim().is_empty() {
            draft
                .locations
                .iter()
                .map(|location| location.id.clone())
                .min()
                .unwrap_or_default()
        } else {
            draft.start_location.clone()
        };
        let contract = draft.contract;
        let character_presets = draft
            .character_presets
            .into_iter()
            .map(|preset| (preset.id.clone(), preset))
            .collect::<BTreeMap<_, _>>();
        let character_creation = draft.character_creation.map(|mut definition| {
            definition.slots.sort_by(|left, right| {
                left.order
                    .cmp(&right.order)
                    .then_with(|| left.id.cmp(&right.id))
            });
            for slot in &mut definition.slots {
                slot.choices.sort_by(|left, right| left.id.cmp(&right.id));
            }
            definition
        });
        let locations = draft
            .locations
            .into_iter()
            .map(|mut location| {
                location.exits.sort();
                sort_variants(&mut location.description_variants);
                (location.id.clone(), location)
            })
            .collect::<BTreeMap<_, _>>();
        let npcs = draft
            .npcs
            .into_iter()
            .map(|npc| (npc.id.clone(), npc))
            .collect::<BTreeMap<_, _>>();
        let timed_events = draft
            .timed_events
            .into_iter()
            .map(|event| (event.id.clone(), event))
            .collect::<BTreeMap<_, _>>();
        let actions = draft
            .actions
            .into_iter()
            .map(|mut action| {
                if action.category.trim().is_empty() {
                    action.category = "Action".to_owned();
                }
                if action.result.trim().is_empty() {
                    action.result = "The action is complete.".to_owned();
                }
                sort_variants(&mut action.result_variants);
                action.locations.sort();
                action
                    .parameters
                    .sort_by(|left, right| left.name.cmp(&right.name));
                for parameter in &mut action.parameters {
                    if let ParameterDomain::Values(values) = &mut parameter.domain {
                        values.sort();
                    }
                }
                (action.id.clone(), action)
            })
            .collect::<BTreeMap<_, _>>();
        let world_id = draft.world_id;
        let identity = ContentIdentity {
            manifest: &manifest,
            contract,
            world_id: &world_id,
            start_location: &start_location,
            character_presets: &character_presets,
            character_creation: &character_creation,
            locations: &locations,
            npcs: &npcs,
            timed_events: &timed_events,
            actions: &actions,
        };
        let build_id = sha256_json(&identity).expect("validated content must be serializable");
        Ok(Self {
            manifest,
            contract,
            world_id,
            start_location,
            character_presets,
            character_creation,
            locations,
            npcs,
            timed_events,
            actions,
            build_id,
        })
    }

    pub fn build_id(&self) -> &str {
        &self.build_id
    }

    pub fn world_id(&self) -> &str {
        &self.world_id
    }

    pub fn contract(&self) -> ContentContract {
        self.contract
    }

    pub fn start_location(&self) -> &LocationId {
        &self.start_location
    }

    pub fn schema_version(&self) -> &str {
        self.manifest.schema_abi_version()
    }

    pub fn rules_version(&self) -> &str {
        self.manifest.rules_abi_version()
    }

    pub fn manifest(&self) -> &BuildManifest {
        &self.manifest
    }

    pub fn character_preset(&self, id: &str) -> Option<&CharacterPreset> {
        self.character_presets.get(id)
    }

    pub fn character_presets(&self) -> impl Iterator<Item = (&String, &CharacterPreset)> {
        self.character_presets.iter()
    }

    pub fn character_creation(&self) -> Option<&CharacterCreationDefinition> {
        self.character_creation.as_ref()
    }

    pub fn location(&self, id: &str) -> Option<&LocationDefinition> {
        self.locations.get(id)
    }

    pub fn npc(&self, id: &str) -> Option<&NpcDefinition> {
        self.npcs.get(id)
    }

    pub fn action(&self, id: &str) -> Option<&ActionDefinition> {
        self.actions.get(id)
    }

    pub fn timed_event(&self, id: &str) -> Option<&TimedEventDefinition> {
        self.timed_events.get(id)
    }

    pub fn locations(&self) -> impl Iterator<Item = (&LocationId, &LocationDefinition)> {
        self.locations.iter()
    }

    pub fn npcs(&self) -> impl Iterator<Item = (&NpcId, &NpcDefinition)> {
        self.npcs.iter()
    }

    pub fn actions(&self) -> impl Iterator<Item = (&String, &ActionDefinition)> {
        self.actions.iter()
    }

    pub fn timed_events(&self) -> impl Iterator<Item = (&String, &TimedEventDefinition)> {
        self.timed_events.iter()
    }

    pub fn has_location(&self, id: &str) -> bool {
        self.locations.contains_key(id)
    }

    pub fn has_npc(&self, id: &str) -> bool {
        self.npcs.contains_key(id)
    }

    pub fn has_valid_build_id(&self) -> bool {
        let identity = ContentIdentity {
            manifest: &self.manifest,
            contract: self.contract,
            world_id: &self.world_id,
            start_location: &self.start_location,
            character_presets: &self.character_presets,
            character_creation: &self.character_creation,
            locations: &self.locations,
            npcs: &self.npcs,
            timed_events: &self.timed_events,
            actions: &self.actions,
        };
        sha256_json(&identity).is_ok_and(|digest| digest == self.build_id)
    }

    pub fn empty_location_runtime(&self) -> BTreeMap<LocationId, LocationRuntime> {
        self.locations
            .keys()
            .cloned()
            .map(|id| (id, LocationRuntime::default()))
            .collect()
    }

    /// Construct the authoritative starting state from content definitions.
    /// NPC placement and location entity indexes are derived here so clients
    /// cannot accidentally create a different world before the first action.
    pub fn new_game(
        &self,
        character_id: &str,
        seed: u64,
    ) -> Result<GameState, ContentValidationError> {
        let Some(preset) = self.character_preset(character_id) else {
            return Err(single_validation_error(format!(
                "unknown character preset {character_id}"
            )));
        };
        self.new_game_with_character(
            preset.character.clone(),
            CharacterStart::Preset {
                character_preset_id: character_id.to_owned(),
            },
            seed,
        )
    }

    /// Normalize and validate a player selection against the compiled slots.
    /// Input ordering is presentation-only; the returned order is canonical.
    pub fn canonical_character_selection(
        &self,
        selection: &CharacterSelection,
    ) -> Result<CharacterSelection, ContentValidationError> {
        let definition = self.character_creation.as_ref().ok_or_else(|| {
            single_validation_error("compiled content does not offer custom character creation")
        })?;
        let name = canonical_character_name(&selection.name)?;
        if selection.choices.len() != definition.slots.len() {
            return Err(single_validation_error(format!(
                "character selection has {} choices; expected {}",
                selection.choices.len(),
                definition.slots.len()
            )));
        }
        let mut supplied = BTreeMap::new();
        for selected in &selection.choices {
            if supplied
                .insert(selected.slot_id.as_str(), selected.choice_id.as_str())
                .is_some()
            {
                return Err(single_validation_error(format!(
                    "character selection repeats slot {}",
                    selected.slot_id
                )));
            }
        }
        let mut choices = Vec::new();
        choices
            .try_reserve(definition.slots.len())
            .map_err(|error| single_validation_error(format!("character selection: {error:?}")))?;
        for slot in &definition.slots {
            let choice_id = supplied.get(slot.id.as_str()).ok_or_else(|| {
                single_validation_error(format!("character selection is missing slot {}", slot.id))
            })?;
            if !slot.choices.iter().any(|choice| choice.id == **choice_id) {
                return Err(single_validation_error(format!(
                    "character selection has unknown choice {} for slot {}",
                    choice_id, slot.id
                )));
            }
            choices.push(CharacterChoiceSelection {
                slot_id: slot.id.clone(),
                choice_id: (*choice_id).to_owned(),
            });
        }
        for slot_id in supplied.keys() {
            if !definition.slots.iter().any(|slot| slot.id == **slot_id) {
                return Err(single_validation_error(format!(
                    "character selection has unknown slot {slot_id}"
                )));
            }
        }
        Ok(CharacterSelection { name, choices })
    }

    /// Materialize a custom character solely from compiled authored patches.
    pub fn custom_character(
        &self,
        selection: &CharacterSelection,
    ) -> Result<Character, ContentValidationError> {
        let canonical = self.canonical_character_selection(selection)?;
        self.custom_character_from_canonical(&canonical)
    }

    /// Construct an authoritative custom starting state from a public recipe.
    pub fn new_custom_game(
        &self,
        selection: &CharacterSelection,
        seed: u64,
    ) -> Result<GameState, ContentValidationError> {
        let canonical = self.canonical_character_selection(selection)?;
        let character = self.custom_character_from_canonical(&canonical)?;
        self.new_game_with_character(
            character,
            CharacterStart::Custom {
                selection: canonical,
            },
            seed,
        )
    }

    fn custom_character_from_canonical(
        &self,
        selection: &CharacterSelection,
    ) -> Result<Character, ContentValidationError> {
        #[derive(Serialize)]
        struct CustomCharacterIdentity<'a> {
            domain: &'static str,
            build_id: &'a str,
            selection: &'a CharacterSelection,
        }

        let definition = self.character_creation.as_ref().ok_or_else(|| {
            single_validation_error("compiled content does not offer custom character creation")
        })?;
        let digest = sha256_json(&CustomCharacterIdentity {
            domain: "forge-custom-character-v1",
            build_id: &self.build_id,
            selection,
        })
        .map_err(|error| single_validation_error(error.to_string()))?;
        let mut character = empty_character(format!("custom-{digest}"));
        apply_character_patch(&mut character, &definition.base, "creation base")?;
        for selected in &selection.choices {
            let slot = definition
                .slots
                .iter()
                .find(|slot| slot.id == selected.slot_id)
                .expect("canonical selection slot must be compiled");
            let choice = slot
                .choices
                .iter()
                .find(|choice| choice.id == selected.choice_id)
                .expect("canonical selection choice must be compiled");
            apply_character_patch(
                &mut character,
                &choice.patch,
                &format!("creation choice {}.{}", slot.id, choice.id),
            )?;
        }
        if character.lineage.is_empty()
            || character.origin.is_empty()
            || character.background.is_empty()
        {
            return Err(single_validation_error(
                "character creation does not define lineage, origin, and background",
            ));
        }
        Ok(character)
    }

    fn new_game_with_character(
        &self,
        character: Character,
        character_start: CharacterStart,
        seed: u64,
    ) -> Result<GameState, ContentValidationError> {
        let mut locations = self.empty_location_runtime();
        let mut npcs = BTreeMap::new();
        for (id, definition) in self.npcs() {
            let npc = NpcState {
                id: id.clone(),
                location: definition.location.clone(),
                goals: definition.goals.clone(),
                values: definition.values.clone(),
                tags: definition.tags.clone(),
                relationships: BTreeMap::new(),
                memories: BTreeMap::new(),
                knowledge: BTreeMap::new(),
                inventory: BTreeMap::new(),
                suspicion: 0,
            };
            locations
                .get_mut(&definition.location)
                .expect("validated NPC location must be compiled")
                .entities
                .insert(id.clone());
            npcs.insert(id.clone(), npc);
        }
        let mut state = GameState::new(
            self.build_id().to_owned(),
            crate::WorldState::new(
                self.world_id().to_owned(),
                self.start_location().clone(),
                locations,
                npcs,
            ),
            character,
            EntropyState::new(seed),
        );
        state.character_start = character_start;
        state.world.scheduled_events = self
            .timed_events
            .values()
            .map(|event| ScheduledEvent {
                id: event.id.clone(),
                due_time: event.due_time,
                event_kind: event.event_kind.clone(),
            })
            .collect();
        state.world.scheduled_events.sort_by(|left, right| {
            left.due_time
                .cmp(&right.due_time)
                .then_with(|| left.id.cmp(&right.id))
        });
        self.validate_state(&state)?;
        Ok(state)
    }

    /// Build the concise player-facing observation for a valid state.
    pub fn observe(&self, state: &GameState) -> Result<Observation, ContentValidationError> {
        self.observe_with_result(state, None)
    }

    /// Preview authored result text for a definition against a valid state.
    /// Authoritative play should use `observe_after_transition` so the result
    /// is bound to a reducer-produced transition.
    pub fn observe_action(
        &self,
        state: &GameState,
        definition_id: &str,
    ) -> Result<Observation, ContentValidationError> {
        let action = self.action(definition_id).ok_or_else(|| {
            single_validation_error(format!("unknown action definition {definition_id}"))
        })?;
        let result = select_variant_text(&action.result, &action.result_variants, state);
        self.observe_with_result(state, Some(result))
    }

    /// Render a result-first observation bound to a reducer transition.
    pub fn observe_after_transition(
        &self,
        transition: &crate::Transition,
    ) -> Result<Observation, ContentValidationError> {
        let action = transition.action();
        if action.build_id != self.build_id {
            return Err(single_validation_error(
                "action build identity does not match compiled content",
            ));
        }
        if action.action_id != action.recomputed_id() {
            return Err(single_validation_error(
                "action identity does not match its canonical fields",
            ));
        }
        if transition.post_state_id() != transition.state().state_id() {
            return Err(single_validation_error(
                "transition post-state identity does not match its state",
            ));
        }
        if transition.entropy_after() != &transition.state().entropy {
            return Err(single_validation_error(
                "transition entropy does not match its state",
            ));
        }
        if !transition.state().event_log.ends_with(transition.events()) {
            return Err(single_validation_error(
                "transition events are not the state event-log suffix",
            ));
        }
        let definition = self
            .action(&action.definition_id)
            .expect("validated transition action must be compiled");
        let mut result_parts = vec![select_variant_text(
            &definition.result,
            &definition.result_variants,
            transition.state(),
        )];
        for event in transition.events() {
            if let EventKind::ScheduledEventResolved {
                event_id,
                event_kind,
                applied: true,
            } = &event.kind
            {
                let timed = self.timed_event(event_id).ok_or_else(|| {
                    single_validation_error("transition resolved an unknown timed event")
                })?;
                if timed.event_kind != *event_kind {
                    return Err(single_validation_error(
                        "transition timed-event kind does not match compiled content",
                    ));
                }
                result_parts.push(timed.result.clone());
            }
        }
        self.observe_with_result(transition.state(), Some(result_parts.join(" ")))
    }

    pub fn location_description(
        &self,
        state: &GameState,
    ) -> Result<String, ContentValidationError> {
        self.validate_state(state)?;
        let location = self
            .location(&state.world.current_location)
            .expect("validated current location must be compiled");
        Ok(select_variant_text(
            &location.description,
            &location.description_variants,
            state,
        ))
    }

    pub fn action_result(
        &self,
        state: &GameState,
        definition_id: &str,
    ) -> Result<String, ContentValidationError> {
        self.validate_state(state)?;
        let action = self.action(definition_id).ok_or_else(|| {
            single_validation_error(format!("unknown action definition {definition_id}"))
        })?;
        Ok(select_variant_text(
            &action.result,
            &action.result_variants,
            state,
        ))
    }

    /// Return a deterministic page of the complete legal action catalog.
    /// `page_size` is presentation-only; legality and the digest come from
    /// the full kernel enumeration before slicing.
    pub fn action_page(
        &self,
        state: &GameState,
        offset: usize,
        page_size: usize,
    ) -> Result<ActionPage, ContentValidationError> {
        if page_size == 0 {
            return Err(single_validation_error("action page size must be positive"));
        }
        let actions = crate::enumerate_legal_actions(state, self)
            .map_err(|error| single_validation_error(error.to_string()))?;
        let total = actions.len();
        if offset > total {
            return Err(single_validation_error(format!(
                "action page offset {offset} exceeds total {total}"
            )));
        }
        let digest = crate::legal_action_digest(&actions)
            .map_err(|error| single_validation_error(error.to_string()))?;
        let end = offset.saturating_add(page_size).min(total);
        let page_actions = &actions[offset..end];
        let mut views = Vec::new();
        views
            .try_reserve(page_actions.len())
            .map_err(|error| single_validation_error(format!("action page: {error:?}")))?;
        for action in page_actions {
            let definition = self
                .action(&action.definition_id)
                .expect("enumerated action definition must be compiled");
            let parameter_display_values = action
                .parameters
                .iter()
                .map(|(name, value)| {
                    let display_value = definition
                        .parameters
                        .iter()
                        .find(|parameter| parameter.name == *name)
                        .map_or_else(
                            || value.clone(),
                            |parameter| match &parameter.domain {
                                ParameterDomain::LocationsAdjacent => {
                                    self.location(value).map_or_else(
                                        || value.clone(),
                                        |location| location.name.clone(),
                                    )
                                }
                                ParameterDomain::NpcsAtCurrentLocation => self
                                    .npc(value)
                                    .map_or_else(|| value.clone(), |npc| npc.name.clone()),
                                ParameterDomain::Values(_) | ParameterDomain::InventoryItems => {
                                    value.clone()
                                }
                            },
                        );
                    (name.clone(), display_value)
                })
                .collect();
            views.push(ActionView {
                action_id: action.action_id.clone(),
                definition_id: action.definition_id.clone(),
                label: definition.label.clone(),
                category: definition.category.clone(),
                time_cost: action_time_cost(&definition.effects)
                    .expect("validated action time cost must fit in world time"),
                consequence_preview: matches!(definition.category.as_str(), "Outcome" | "Ending")
                    .then(|| definition.result.clone()),
                parameter_display_values,
                parameters: action.parameters.clone(),
            });
        }
        let next_offset = (end < total).then_some(end);
        Ok(ActionPage {
            build_id: self.build_id.clone(),
            state_id: state.state_id(),
            actions: views,
            total,
            digest,
            offset,
            next_offset,
        })
    }

    fn observe_with_result(
        &self,
        state: &GameState,
        result: Option<String>,
    ) -> Result<Observation, ContentValidationError> {
        self.validate_state(state)?;
        let location = self
            .location(&state.world.current_location)
            .expect("validated current location must be compiled");
        let actions = crate::enumerate_legal_actions(state, self)
            .map_err(|error| single_validation_error(error.to_string()))?;
        let action_set_digest = crate::legal_action_digest(&actions)
            .map_err(|error| single_validation_error(error.to_string()))?;
        let location_text =
            select_variant_text(&location.description, &location.description_variants, state);
        let text = match &result {
            Some(result) => format!("{result} {location_text}"),
            None => location_text,
        };
        if word_count(&text) > 100 {
            return Err(single_validation_error(
                "combined routine observation exceeds 100 words",
            ));
        }
        let upcoming_events = state
            .world
            .scheduled_events
            .iter()
            .filter_map(|scheduled| {
                self.timed_event(&scheduled.id)
                    .filter(|definition| definition.condition.evaluate(state))
                    .map(|definition| TimedEventView {
                        label: definition.label.clone(),
                        remaining_ticks: scheduled.due_time.saturating_sub(state.world.time),
                    })
            })
            .collect();
        Ok(Observation {
            build_id: self.build_id.clone(),
            state_id: state.state_id(),
            location_id: state.world.current_location.clone(),
            title: location.name.clone(),
            text,
            result,
            world_time: state.world.time,
            upcoming_events,
            action_set_digest,
            action_count: actions.len(),
        })
    }

    pub fn validate_state(&self, state: &GameState) -> Result<(), ContentValidationError> {
        let mut errors = ContentValidationError::new();
        if state.build_id != self.build_id {
            errors.push("state build identity does not match compiled content");
        }
        if state.world.id != self.world_id {
            errors.push(format!(
                "state world id {} does not match content world {}",
                state.world.id, self.world_id
            ));
        }
        if state.character.id.trim().is_empty() {
            errors.push("character id cannot be empty");
        }
        if self.contract == ContentContract::Production {
            match &state.character_start {
                CharacterStart::Preset {
                    character_preset_id,
                } => match self.character_preset(character_preset_id) {
                    Some(preset)
                        if static_character_fields_match(&state.character, &preset.character) => {}
                    Some(_) => errors.push(
                        "production character identity fields differ from its compiled preset",
                    ),
                    None => errors.push(
                        "production character start does not name a compiled character preset",
                    ),
                },
                CharacterStart::Custom { selection } => {
                    match self.canonical_character_selection(selection) {
                        Ok(canonical) if &canonical != selection => errors.push(
                            "production custom character selection is not in canonical order",
                        ),
                        Ok(canonical) => match self.custom_character_from_canonical(&canonical) {
                            Ok(expected)
                                if static_character_fields_match(&state.character, &expected) => {}
                            Ok(_) => errors.push(
                                "production custom character fields differ from its compiled recipe",
                            ),
                            Err(error) => errors.push(format!(
                                "production custom character recipe is invalid: {error}"
                            )),
                        },
                        Err(error) => errors.push(format!(
                            "production custom character selection is invalid: {error}"
                        )),
                    }
                }
                CharacterStart::Fixture => {
                    errors.push("production character cannot use fixture provenance")
                }
            }
        }
        if state.world.locations.len() != self.locations.len() {
            errors.push("state location runtime keys are not exact");
        }
        for location in self.locations.keys() {
            if !state.world.locations.contains_key(location) {
                errors.push(format!("state is missing location runtime {location}"));
            }
        }
        for location in state.world.locations.keys() {
            if !self.locations.contains_key(location) {
                errors.push(format!("state has unknown location runtime {location}"));
            }
        }
        if !self.locations.contains_key(&state.world.current_location) {
            errors.push(format!(
                "state current location {} is unknown",
                state.world.current_location
            ));
        }
        for (key, npc) in &state.world.npcs {
            if key != &npc.id {
                errors.push(format!(
                    "NPC map key {key} does not match embedded id {}",
                    npc.id
                ));
            }
            if let Some(definition) = self.npcs.get(key) {
                if npc.goals != definition.goals
                    || npc.values != definition.values
                    || npc.tags != definition.tags
                {
                    errors.push(format!(
                        "NPC {key} static goals, values, or tags differ from compiled content"
                    ));
                }
            } else {
                errors.push(format!("state NPC {key} is not in the content registry"));
            }
            if !self.locations.contains_key(&npc.location) {
                errors.push(format!("NPC {key} has unknown location {}", npc.location));
            } else if state
                .world
                .locations
                .get(&npc.location)
                .is_none_or(|runtime| !runtime.entities.contains(key))
            {
                errors.push(format!(
                    "NPC {key} is missing from its location entity index {}",
                    npc.location
                ));
            }
            for (item, count) in &npc.inventory {
                if item.trim().is_empty() || *count == 0 {
                    errors.push(format!("NPC {key} has an invalid inventory entry"));
                }
            }
        }
        if state.world.npcs.len() != self.npcs.len() {
            errors.push("state NPC keys are not exact registered keys");
        }
        for npc in self.npcs.keys() {
            if !state.world.npcs.contains_key(npc) {
                errors.push(format!("state is missing registered NPC {npc}"));
            }
        }
        for (location_id, runtime) in &state.world.locations {
            for entity in &runtime.entities {
                match state.world.npcs.get(entity) {
                    Some(npc) if npc.location == *location_id => {}
                    Some(_) => errors.push(format!(
                        "entity {entity} is listed at {location_id} but its NPC location differs"
                    )),
                    None => errors.push(format!(
                        "location {location_id} references unknown entity {entity}"
                    )),
                }
            }
        }
        let mut expected_scheduled = self
            .timed_events
            .values()
            .filter(|definition| definition.due_time > state.world.time)
            .map(|definition| ScheduledEvent {
                id: definition.id.clone(),
                due_time: definition.due_time,
                event_kind: definition.event_kind.clone(),
            })
            .collect::<Vec<_>>();
        expected_scheduled.sort_by(|left, right| {
            left.due_time
                .cmp(&right.due_time)
                .then_with(|| left.id.cmp(&right.id))
        });
        if state.world.scheduled_events != expected_scheduled {
            errors.push("state pending timed events differ from compiled content and world time");
        }
        for event in &state.event_log {
            if let EventKind::ScheduledEventResolved {
                event_id,
                event_kind,
                ..
            } = &event.kind
            {
                match self.timed_event(event_id) {
                    Some(definition)
                        if definition.event_kind == *event_kind
                            && event.turn >= definition.due_time
                            && event.turn <= state.world.time => {}
                    Some(_) => errors.push(format!(
                        "resolved timed event {event_id} has invalid kind or turn"
                    )),
                    None => errors.push(format!(
                        "event log references unknown timed event {event_id}"
                    )),
                }
            }
        }
        for definition in self
            .timed_events
            .values()
            .filter(|definition| definition.due_time <= state.world.time)
        {
            let resolutions = state
                .event_log
                .iter()
                .filter(|event| {
                    matches!(
                        &event.kind,
                        EventKind::ScheduledEventResolved { event_id, .. }
                            if event_id == &definition.id
                    )
                })
                .count();
            if resolutions != 1 {
                errors.push(format!(
                    "timed event {} has {resolutions} resolutions; expected exactly one",
                    definition.id
                ));
            }
        }
        for (item, count) in &state.character.inventory {
            if item.trim().is_empty() || *count == 0 {
                errors.push("character has an invalid inventory entry");
            }
        }
        for resource in state.character.resources.keys() {
            if resource.trim().is_empty() {
                errors.push("character has an empty resource key");
            }
        }
        if state.entropy.algorithm != self.manifest.entropy_algorithm()
            || state.entropy.validate().is_err()
        {
            errors.push("state uses an unsupported entropy algorithm or exhausted cursor");
        }
        if errors.issues.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

fn validate_draft(
    draft: &ContentDraft,
    manifest: &BuildManifest,
) -> Result<(), ContentValidationError> {
    let mut errors = ContentValidationError::new();
    if draft.schema_version != manifest.schema_abi_version() {
        errors.push(format!(
            "schema version {} does not match trusted ABI {}",
            draft.schema_version,
            manifest.schema_abi_version()
        ));
    }
    if draft.rules_version != manifest.rules_abi_version() {
        errors.push(format!(
            "rules version {} does not match trusted ABI {}",
            draft.rules_version,
            manifest.rules_abi_version()
        ));
    }
    if draft.world_id.trim().is_empty() {
        errors.push("world_id cannot be empty");
    }
    let production = matches!(draft.contract, ContentContract::Production);
    let mut location_ids = BTreeSet::new();
    for location in &draft.locations {
        validate_location(location, &mut errors);
        if !location_ids.insert(&location.id) {
            errors.push(format!("duplicate location id {}", location.id));
        }
    }
    if location_ids.is_empty() {
        errors.push("content must define at least one location");
    }
    for location in &draft.locations {
        let mut exits = BTreeSet::new();
        for exit in &location.exits {
            if !location_ids.contains(exit) {
                errors.push(format!(
                    "location {} references unknown exit {}",
                    location.id, exit
                ));
            }
            if !exits.insert(exit) {
                errors.push(format!("location {} repeats exit {}", location.id, exit));
            }
        }
    }
    if !location_ids.is_empty() && !graph_connected(&draft.locations, &location_ids) {
        errors.push("location graph is not connected");
    }
    if draft.start_location.trim().is_empty() {
        if production {
            errors.push("production content must define a start_location");
        }
    } else if !location_ids.contains(&draft.start_location) {
        errors.push(format!(
            "start_location {} is not a registered location",
            draft.start_location
        ));
    }

    let mut npc_ids = BTreeSet::new();
    for npc in &draft.npcs {
        if npc.id.trim().is_empty() {
            errors.push("NPC id cannot be empty");
        }
        if npc.name.trim().is_empty() {
            errors.push(format!("NPC {} has an empty name", npc.id));
        }
        if !location_ids.contains(&npc.location) {
            errors.push(format!(
                "NPC {} references unknown location {}",
                npc.id, npc.location
            ));
        }
        if !npc_ids.insert(&npc.id) {
            errors.push(format!("duplicate NPC id {}", npc.id));
        }
    }

    let mut preset_ids = BTreeSet::new();
    let mut character_ids = BTreeSet::new();
    for preset in &draft.character_presets {
        if preset.id.trim().is_empty() {
            errors.push("character preset id cannot be empty");
        }
        if !preset_ids.insert(&preset.id) {
            errors.push(format!("duplicate character preset id {}", preset.id));
        }
        if preset.display_name.trim().is_empty() {
            errors.push(format!(
                "character preset {} has an empty display name",
                preset.id
            ));
        }
        if word_count(&preset.display_name) > 8 {
            errors.push(format!(
                "character preset {} display name exceeds 8 words",
                preset.id
            ));
        }
        if preset.summary.trim().is_empty() {
            errors.push(format!(
                "character preset {} has an empty summary",
                preset.id
            ));
        } else if split_sentences(&preset.summary).len() > 2
            || split_sentences(&preset.summary)
                .iter()
                .any(|sentence| word_count(sentence) > 18)
        {
            errors.push(format!(
                "character preset {} summary exceeds concise text limits",
                preset.id
            ));
        }
        if preset.character.id.trim().is_empty() {
            errors.push(format!(
                "character preset {} has an empty character id",
                preset.id
            ));
        }
        if !character_ids.insert(&preset.character.id) {
            errors.push(format!(
                "duplicate character id {} across presets",
                preset.character.id
            ));
        }
        for key in preset.character.facets.keys() {
            if is_reserved_character_facet_axis(key) {
                errors.push(format!(
                    "character preset {} uses reserved facet axis {key}",
                    preset.id
                ));
            }
        }
    }
    if production && draft.character_presets.len() < 2 {
        errors.push("production content requires at least two character presets");
    }
    match &draft.character_creation {
        Some(definition) => validate_character_creation(definition, &mut errors),
        None if production => {
            errors.push("production content requires authored character creation")
        }
        None => {}
    }
    for location in &draft.locations {
        validate_text_variants(
            &location.description_variants,
            &format!("location {} description", location.id),
            &location_ids,
            &npc_ids,
            &mut errors,
        );
    }

    if draft.timed_events.len() > MAX_TIMED_EVENTS {
        errors.push(format!("content exceeds {MAX_TIMED_EVENTS} timed events"));
    }
    let mut timed_event_ids = BTreeSet::new();
    for event in &draft.timed_events {
        validate_timed_event(event, &location_ids, &npc_ids, &mut errors);
        if !timed_event_ids.insert(&event.id) {
            errors.push(format!("duplicate timed event id {}", event.id));
        }
    }

    let mut action_ids = BTreeSet::new();
    for action in &draft.actions {
        validate_action(action, &location_ids, &npc_ids, production, &mut errors);
        if !action_ids.insert(&action.id) {
            errors.push(format!("duplicate action id {}", action.id));
        }
    }
    for location in &draft.locations {
        if location.terminal {
            continue;
        }
        let meaningful = draft
            .actions
            .iter()
            .filter(|action| {
                action.meaningful
                    && !action.movement
                    && !action.condition.is_obviously_never()
                    && condition_possible_at(&action.condition, &location.id)
                    && action.effects.iter().any(Effect::changes_state)
                    && (action.locations.is_empty()
                        || action.locations.iter().any(|id| id == &location.id))
                    && action
                        .parameters
                        .iter()
                        .all(|parameter| parameter_domain_possible_at(parameter, location, draft))
            })
            .count();
        if meaningful < 2 {
            errors.push(format!(
                "nonterminal location {} has {meaningful} obviously legal meaningful non-movement actions; expected at least 2",
                location.id
            ));
        }
    }
    if errors.issues.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_character_creation(
    definition: &CharacterCreationDefinition,
    errors: &mut ContentValidationError,
) {
    if definition.slots.len() < 2 {
        errors.push("character creation requires at least two slots");
    }
    if definition.slots.len() > MAX_CREATION_SLOTS {
        errors.push(format!(
            "character creation exceeds {MAX_CREATION_SLOTS} slots"
        ));
    }
    validate_character_patch(&definition.base, "character creation base", false, errors);

    let mut slot_ids = BTreeSet::new();
    let mut slot_orders = BTreeSet::new();
    let mut write_owners = BTreeMap::<String, String>::new();
    register_patch_writes(&definition.base, "base", &mut write_owners, errors);

    for slot in &definition.slots {
        if !valid_creation_id(&slot.id) {
            errors.push(format!(
                "character creation slot {} has an invalid id",
                slot.id
            ));
        }
        if !slot_ids.insert(&slot.id) {
            errors.push(format!("character creation repeats slot id {}", slot.id));
        }
        if !slot_orders.insert(slot.order) {
            errors.push(format!(
                "character creation repeats slot order {}",
                slot.order
            ));
        }
        if slot.display_name.trim().is_empty() || word_count(&slot.display_name) > 6 {
            errors.push(format!(
                "character creation slot {} has an invalid display name",
                slot.id
            ));
        }
        if slot.choices.len() < 2 {
            errors.push(format!(
                "character creation slot {} requires at least two choices",
                slot.id
            ));
        }
        if slot.choices.len() > MAX_CREATION_CHOICES_PER_SLOT {
            errors.push(format!(
                "character creation slot {} exceeds {MAX_CREATION_CHOICES_PER_SLOT} choices",
                slot.id
            ));
        }

        let mut choice_ids = BTreeSet::new();
        let mut slot_writes = BTreeSet::new();
        for (choice_index, choice) in slot.choices.iter().enumerate() {
            if !valid_creation_id(&choice.id) || !choice_ids.insert(&choice.id) {
                errors.push(format!(
                    "character creation slot {} has an invalid or duplicate choice id {}",
                    slot.id, choice.id
                ));
            }
            if choice.display_name.trim().is_empty() || word_count(&choice.display_name) > 8 {
                errors.push(format!(
                    "character creation choice {}.{} has an invalid display name",
                    slot.id, choice.id
                ));
            }
            let sentences = split_sentences(&choice.summary);
            if choice.summary.trim().is_empty()
                || sentences.len() > 2
                || sentences.iter().any(|sentence| word_count(sentence) > 18)
            {
                errors.push(format!(
                    "character creation choice {}.{} exceeds concise summary limits",
                    slot.id, choice.id
                ));
            }
            validate_character_patch(
                &choice.patch,
                &format!("character creation choice {}.{}", slot.id, choice.id),
                true,
                errors,
            );
            if slot.choices[..choice_index]
                .iter()
                .any(|earlier| earlier.patch == choice.patch)
            {
                errors.push(format!(
                    "character creation slot {} has mechanically identical choices",
                    slot.id
                ));
            }
            slot_writes.extend(character_patch_write_targets(&choice.patch));
        }
        for target in slot_writes {
            if let Some(previous) = write_owners.insert(target.clone(), slot.id.clone())
                && previous != slot.id
            {
                errors.push(format!(
                    "character creation {previous} and slot {} both write {target}",
                    slot.id
                ));
            }
        }
    }

    for (field, base_present, slot_covers) in [
        (
            "lineage",
            definition.base.lineage.is_some(),
            definition.slots.iter().any(|slot| {
                slot.choices
                    .iter()
                    .all(|choice| choice.patch.lineage.is_some())
            }),
        ),
        (
            "origin",
            definition.base.origin.is_some(),
            definition.slots.iter().any(|slot| {
                slot.choices
                    .iter()
                    .all(|choice| choice.patch.origin.is_some())
            }),
        ),
        (
            "background",
            definition.base.background.is_some(),
            definition.slots.iter().any(|slot| {
                slot.choices
                    .iter()
                    .all(|choice| choice.patch.background.is_some())
            }),
        ),
    ] {
        if !base_present && !slot_covers {
            errors.push(format!(
                "character creation does not define {field} for every selection"
            ));
        }
    }
}

fn validate_character_patch(
    patch: &CharacterPatch,
    owner: &str,
    choice: bool,
    errors: &mut ContentValidationError,
) {
    for (field, value) in [
        ("lineage", patch.lineage.as_deref()),
        ("origin", patch.origin.as_deref()),
        ("background", patch.background.as_deref()),
    ] {
        if value.is_some_and(|value| !valid_authored_value(value)) {
            errors.push(format!("{owner} has an invalid {field}"));
        }
    }
    if choice && !character_patch_is_mechanical(patch) {
        errors.push(format!("{owner} is cosmetic-only or empty"));
    }
    for (field, len) in [
        ("aptitudes", patch.aptitudes.len()),
        ("skills", patch.skills.len()),
        ("values", patch.values.len()),
        ("traits", patch.traits.len()),
        ("flaws", patch.flaws.len()),
        ("appearance", patch.appearance.len()),
        ("affiliations", patch.affiliations.len()),
        ("reputation", patch.reputation.len()),
        ("knowledge", patch.knowledge.len()),
        ("inventory", patch.inventory.len()),
        ("resources", patch.resources.len()),
        ("injuries", patch.injuries.len()),
        ("deeds", patch.deeds.len()),
        ("promises", patch.promises.len()),
        ("discoveries", patch.discoveries.len()),
        ("facets", patch.facets.len()),
    ] {
        if len > 128 {
            errors.push(format!("{owner} has more than 128 {field} entries"));
        }
    }
    for (key, value) in &patch.aptitudes {
        if !valid_authored_value(key) || !(0..=10).contains(value) {
            errors.push(format!("{owner} has invalid aptitude {key}"));
        }
    }
    for (field, values) in [
        ("skills", &patch.skills),
        ("values", &patch.values),
        ("traits", &patch.traits),
        ("flaws", &patch.flaws),
        ("knowledge", &patch.knowledge),
        ("injuries", &patch.injuries),
        ("deeds", &patch.deeds),
        ("promises", &patch.promises),
        ("discoveries", &patch.discoveries),
    ] {
        if values.iter().any(|value| !valid_authored_value(value)) {
            errors.push(format!("{owner} has an invalid {field} entry"));
        }
    }
    for (key, value) in &patch.appearance {
        if !valid_authored_value(key) || !valid_authored_value(value) {
            errors.push(format!("{owner} has invalid appearance {key}"));
        }
    }
    for (field, values) in [
        ("affiliations", &patch.affiliations),
        ("reputation", &patch.reputation),
        ("resources", &patch.resources),
    ] {
        for (key, value) in values {
            if !valid_authored_value(key) || !(-1_000_000..=1_000_000).contains(value) {
                errors.push(format!("{owner} has invalid {field} entry {key}"));
            }
        }
    }
    for (key, count) in &patch.inventory {
        if !valid_authored_value(key) || *count == 0 || *count > 1_000_000 {
            errors.push(format!("{owner} has invalid inventory entry {key}"));
        }
    }
    for (key, value) in &patch.facets {
        if is_reserved_character_facet_axis(key) {
            errors.push(format!("{owner} uses reserved facet axis {key}"));
        } else if !valid_authored_value(key) || !valid_facet_value(value) {
            errors.push(format!("{owner} has invalid facet {key}"));
        }
    }
}

fn valid_creation_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 48
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
}

fn valid_authored_value(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 96 && !value.chars().any(char::is_control)
}

fn valid_facet_value(value: &FacetValue) -> bool {
    match value {
        FacetValue::Text(value) => valid_authored_value(value),
        FacetValue::Number(value) => (-1_000_000..=1_000_000).contains(value),
        FacetValue::Boolean(_) => true,
        FacetValue::Tags(values) => {
            values.len() <= 128 && values.iter().all(|value| valid_authored_value(value))
        }
    }
}

fn character_patch_is_mechanical(patch: &CharacterPatch) -> bool {
    patch.lineage.is_some()
        || patch.origin.is_some()
        || patch.background.is_some()
        || !patch.aptitudes.is_empty()
        || !patch.skills.is_empty()
        || !patch.values.is_empty()
        || !patch.traits.is_empty()
        || !patch.flaws.is_empty()
        || !patch.affiliations.is_empty()
        || !patch.reputation.is_empty()
        || !patch.knowledge.is_empty()
        || !patch.inventory.is_empty()
        || !patch.resources.is_empty()
        || !patch.injuries.is_empty()
        || !patch.deeds.is_empty()
        || !patch.promises.is_empty()
        || !patch.discoveries.is_empty()
        || !patch.facets.is_empty()
}

fn register_patch_writes(
    patch: &CharacterPatch,
    owner: &str,
    owners: &mut BTreeMap<String, String>,
    errors: &mut ContentValidationError,
) {
    for target in character_patch_write_targets(patch) {
        if let Some(previous) = owners.insert(target.clone(), owner.to_owned())
            && previous != owner
        {
            errors.push(format!(
                "character creation {previous} and {owner} both write {target}"
            ));
        }
    }
}

fn character_patch_write_targets(patch: &CharacterPatch) -> BTreeSet<String> {
    let mut targets = BTreeSet::new();
    if patch.lineage.is_some() {
        targets.insert("lineage".to_owned());
    }
    if patch.origin.is_some() {
        targets.insert("origin".to_owned());
    }
    if patch.background.is_some() {
        targets.insert("background".to_owned());
    }
    extend_write_targets(&mut targets, "aptitudes", patch.aptitudes.keys());
    extend_write_targets(&mut targets, "appearance", patch.appearance.keys());
    extend_write_targets(&mut targets, "affiliations", patch.affiliations.keys());
    extend_write_targets(&mut targets, "reputation", patch.reputation.keys());
    extend_write_targets(&mut targets, "inventory", patch.inventory.keys());
    extend_write_targets(&mut targets, "resources", patch.resources.keys());
    extend_write_targets(&mut targets, "facets", patch.facets.keys());
    targets
}

fn extend_write_targets<'a>(
    targets: &mut BTreeSet<String>,
    field: &str,
    keys: impl Iterator<Item = &'a String>,
) {
    targets.extend(keys.map(|key| format!("{field}.{key}")));
}

fn condition_possible_at(condition: &Condition, location: &str) -> bool {
    condition_possible_at_depth(condition, location, 0)
}

fn condition_possible_at_depth(condition: &Condition, location: &str, depth: usize) -> bool {
    if depth > 64 {
        return true;
    }
    match condition {
        Condition::Never => false,
        Condition::Always
        | Condition::FacetEquals { .. }
        | Condition::FacetAtLeast { .. }
        | Condition::HasTag { .. }
        | Condition::HasItem { .. }
        | Condition::ResourceAtLeast { .. }
        | Condition::CharacterKnows { .. }
        | Condition::CharacterHasDeed { .. }
        | Condition::WorldFlag { .. }
        | Condition::LocationFlag { .. }
        | Condition::NpcKnows { .. }
        | Condition::NpcKnowsWithProvenance { .. }
        | Condition::NpcRemembers { .. }
        | Condition::NpcRelationshipAtLeast { .. } => true,
        Condition::AtLocation { location: target } => target == location,
        Condition::All { conditions } => conditions
            .iter()
            .all(|child| condition_possible_at_depth(child, location, depth + 1)),
        Condition::Any { conditions } => conditions
            .iter()
            .any(|child| condition_possible_at_depth(child, location, depth + 1)),
        Condition::Not { condition } => !matches!(condition.as_ref(), Condition::Always),
    }
}

fn parameter_domain_possible_at(
    parameter: &ParameterSpec,
    location: &LocationDefinition,
    draft: &ContentDraft,
) -> bool {
    match &parameter.domain {
        ParameterDomain::Values(values) => !values.is_empty(),
        ParameterDomain::InventoryItems => true,
        ParameterDomain::NpcsAtCurrentLocation => {
            draft.npcs.iter().any(|npc| npc.location == location.id)
        }
        ParameterDomain::LocationsAdjacent => location.exits.iter().any(|exit| {
            draft
                .locations
                .iter()
                .any(|candidate| candidate.id == *exit)
        }),
    }
}

fn validate_location(location: &LocationDefinition, errors: &mut ContentValidationError) {
    if location.id.trim().is_empty() {
        errors.push("location id cannot be empty");
    }
    if location.name.trim().is_empty() {
        errors.push(format!("location {} has an empty name", location.id));
    }
    let description = location.description.trim();
    if description.is_empty() {
        errors.push(format!("location {} has an empty description", location.id));
        return;
    }
    let sentences = split_sentences(description);
    if sentences.len() > 2 {
        errors.push(format!(
            "location {} description has {} sentences; expected at most 2",
            location.id,
            sentences.len()
        ));
    }
    for sentence in sentences {
        if word_count(sentence) > 18 {
            errors.push(format!(
                "location {} description sentence exceeds 18 words",
                location.id
            ));
        }
    }
}

fn validate_text_variants(
    variants: &[TextVariant],
    owner: &str,
    location_ids: &BTreeSet<&String>,
    npc_ids: &BTreeSet<&String>,
    errors: &mut ContentValidationError,
) {
    let mut ids = BTreeSet::new();
    for variant in variants {
        if variant.id.trim().is_empty() || !ids.insert(&variant.id) {
            errors.push(format!("{owner} has an empty or duplicate variant id"));
        }
        let text = variant.text.trim();
        if text.is_empty() {
            errors.push(format!("{owner} variant {} has empty text", variant.id));
        } else {
            let sentences = split_sentences(text);
            if sentences.len() > 1 || sentences.iter().any(|sentence| word_count(sentence) > 18) {
                errors.push(format!(
                    "{owner} variant {} must be one sentence of at most 18 words",
                    variant.id
                ));
            }
        }
        if variant.condition.contains_never() || variant.condition.is_obviously_never() {
            errors.push(format!(
                "{owner} variant {} contains or reduces to an impossible condition",
                variant.id
            ));
        }
        validate_condition(&variant.condition, owner, location_ids, npc_ids, errors, 0);
    }
}

fn validate_action(
    action: &ActionDefinition,
    location_ids: &BTreeSet<&String>,
    npc_ids: &BTreeSet<&String>,
    production: bool,
    errors: &mut ContentValidationError,
) {
    if action.id.trim().is_empty() {
        errors.push("action id cannot be empty");
    }
    let label = action.label.trim();
    if label.is_empty() {
        errors.push(format!("action {} has an empty label", action.id));
    } else if word_count(label) > 8 {
        errors.push(format!("action {} label exceeds 8 words", action.id));
    }
    let category = action.category.trim();
    if category.is_empty() {
        if production {
            errors.push(format!("action {} has an empty category", action.id));
        }
    } else if word_count(category) > 3 {
        errors.push(format!("action {} category exceeds 3 words", action.id));
    }
    let result = action.result.trim();
    if result.is_empty() {
        if production {
            errors.push(format!("action {} has an empty result", action.id));
        }
    } else {
        if word_count(result) > 60 {
            errors.push(format!("action {} result exceeds 60 words", action.id));
        }
        if split_sentences(result)
            .iter()
            .any(|sentence| word_count(sentence) > 18)
        {
            errors.push(format!(
                "action {} result sentence exceeds 18 words",
                action.id
            ));
        }
    }
    validate_text_variants(
        &action.result_variants,
        &format!("action {} result", action.id),
        location_ids,
        npc_ids,
        errors,
    );
    if action.condition.contains_never() || action.condition.is_obviously_never() {
        errors.push(format!(
            "action {} contains or reduces to an impossible condition",
            action.id
        ));
    }
    let mut action_locations = BTreeSet::new();
    for location in &action.locations {
        if !location_ids.contains(location) {
            errors.push(format!(
                "action {} references unknown location {}",
                action.id, location
            ));
        }
        if !action_locations.insert(location) {
            errors.push(format!(
                "action {} repeats location {}",
                action.id, location
            ));
        }
    }
    validate_condition(
        &action.condition,
        &action.id,
        location_ids,
        npc_ids,
        errors,
        0,
    );
    let mut parameters = BTreeSet::new();
    for parameter in &action.parameters {
        if parameter.name.trim().is_empty() || !parameters.insert(&parameter.name) {
            errors.push(format!(
                "action {} has duplicate or empty parameter name",
                action.id
            ));
        }
        if let ParameterDomain::Values(values) = &parameter.domain {
            if values.is_empty() {
                errors.push(format!(
                    "action {} parameter {} has an empty value domain",
                    action.id, parameter.name
                ));
            }
            let mut values_seen = BTreeSet::new();
            for value in values {
                if value.trim().is_empty() || !values_seen.insert(value) {
                    errors.push(format!(
                        "action {} parameter {} has an empty or duplicate value",
                        action.id, parameter.name
                    ));
                }
            }
        }
    }
    let mut roles: BTreeMap<String, BTreeSet<ReferenceRole>> = BTreeMap::new();
    validate_effects(
        &action.effects,
        action,
        location_ids,
        npc_ids,
        &parameters,
        &mut roles,
        errors,
        0,
    );
    let referenced_parameters: BTreeSet<_> = roles.keys().cloned().collect();
    for parameter in &action.parameters {
        if !referenced_parameters.contains(&parameter.name) {
            errors.push(format!(
                "action {} declares unused parameter {}",
                action.id, parameter.name
            ));
        }
    }
    for (name, expected_roles) in roles {
        let Some(parameter) = action
            .parameters
            .iter()
            .find(|parameter| parameter.name == name)
        else {
            continue;
        };
        if expected_roles.len() > 1 {
            errors.push(format!(
                "action {} parameter {} is used as incompatible reference types",
                action.id, name
            ));
        } else if let Some(role) = expected_roles.first()
            && !domain_compatible(&parameter.domain, *role, location_ids, npc_ids)
        {
            errors.push(format!(
                "action {} parameter {} domain is incompatible with {:?} reference",
                action.id, name, role
            ));
        }
    }
    if action.meaningful && !action.effects.iter().any(Effect::changes_state) {
        errors.push(format!(
            "meaningful action {} has no state-changing effect",
            action.id
        ));
    }
    if action_time_cost(&action.effects).is_none() {
        errors.push(format!(
            "action {} time cost overflows world time",
            action.id
        ));
    }
}

fn validate_timed_event(
    event: &TimedEventDefinition,
    location_ids: &BTreeSet<&String>,
    npc_ids: &BTreeSet<&String>,
    errors: &mut ContentValidationError,
) {
    let owner = format!("timed event {}", event.id);
    if event.id.trim().is_empty() {
        errors.push("timed event id cannot be empty");
    }
    if event.due_time == 0 {
        errors.push(format!("{owner} must be due after world time zero"));
    }
    if event.event_kind.trim().is_empty() {
        errors.push(format!("{owner} has an empty event kind"));
    }
    if event.label.trim().is_empty() || word_count(&event.label) > 8 {
        errors.push(format!("{owner} label must contain at most 8 words"));
    }
    let result_sentences = split_sentences(event.result.trim());
    if event.result.trim().is_empty()
        || word_count(&event.result) > 60
        || result_sentences
            .iter()
            .any(|sentence| word_count(sentence) > 18)
    {
        errors.push(format!("{owner} result exceeds concise text limits"));
    }
    if event.condition.contains_never() || event.condition.is_obviously_never() {
        errors.push(format!(
            "{owner} contains or reduces to an impossible condition"
        ));
    }
    validate_condition(&event.condition, &owner, location_ids, npc_ids, errors, 0);
    if event.effects.is_empty() || !event.effects.iter().any(Effect::changes_state) {
        errors.push(format!("{owner} has no state-changing effect"));
    }
    if event.effects.iter().any(effect_advances_time) {
        errors.push(format!("{owner} cannot advance time while resolving"));
    }

    let validation_action = ActionDefinition {
        id: owner,
        label: event.label.clone(),
        category: event.event_kind.clone(),
        result: event.result.clone(),
        result_variants: Vec::new(),
        locations: Vec::new(),
        condition: Condition::Always,
        effects: Vec::new(),
        parameters: Vec::new(),
        meaningful: true,
        movement: false,
    };
    let parameters = BTreeSet::new();
    let mut roles = BTreeMap::new();
    validate_effects(
        &event.effects,
        &validation_action,
        location_ids,
        npc_ids,
        &parameters,
        &mut roles,
        errors,
        0,
    );
}

fn effect_advances_time(effect: &Effect) -> bool {
    match effect {
        Effect::AdvanceTime { .. } => true,
        Effect::RandomChance {
            on_success,
            on_failure,
            ..
        } => effect_advances_time(on_success) || effect_advances_time(on_failure),
        Effect::Noop
        | Effect::SetFlag { .. }
        | Effect::SetWorldFlag { .. }
        | Effect::SetLocationFlag { .. }
        | Effect::AdjustResource { .. }
        | Effect::MoveCharacter { .. }
        | Effect::AdjustNpcRelationship { .. }
        | Effect::AddNpcMemory { .. }
        | Effect::TeachNpc { .. }
        | Effect::AddCharacterDeed { .. } => false,
    }
}

fn validate_condition(
    condition: &Condition,
    owner: &str,
    location_ids: &BTreeSet<&String>,
    npc_ids: &BTreeSet<&String>,
    errors: &mut ContentValidationError,
    depth: usize,
) {
    if depth > 64 {
        errors.push(format!("action {} condition AST exceeds depth 64", owner));
        return;
    }
    match condition {
        Condition::All { conditions } | Condition::Any { conditions } => {
            if conditions.is_empty() {
                errors.push(format!("action {} has an empty boolean condition", owner));
            }
            for child in conditions {
                validate_condition(child, owner, location_ids, npc_ids, errors, depth + 1);
            }
        }
        Condition::Not { condition } => {
            validate_condition(condition, owner, location_ids, npc_ids, errors, depth + 1)
        }
        Condition::LocationFlag { location, flag } => {
            validate_location_ref(location, owner, location_ids, errors);
            if flag.trim().is_empty() {
                errors.push(format!("action {} has an empty location flag", owner));
            }
        }
        Condition::AtLocation { location } => {
            validate_location_ref(location, owner, location_ids, errors)
        }
        Condition::NpcKnows { npc, knowledge_id }
        | Condition::NpcKnowsWithProvenance {
            npc, knowledge_id, ..
        } => {
            validate_npc_ref(npc, owner, npc_ids, errors);
            if knowledge_id.trim().is_empty() {
                errors.push(format!("action {} has an empty knowledge reference", owner));
            }
        }
        Condition::NpcRemembers { npc, memory_id } => {
            validate_npc_ref(npc, owner, npc_ids, errors);
            if memory_id.trim().is_empty() {
                errors.push(format!("action {} has an empty memory reference", owner));
            }
        }
        Condition::NpcRelationshipAtLeast { npc, .. } => {
            validate_npc_ref(npc, owner, npc_ids, errors)
        }
        Condition::FacetEquals { axis, .. } | Condition::FacetAtLeast { axis, .. } => {
            if axis.trim().is_empty() {
                errors.push(format!("action {} has an empty facet axis", owner));
            }
        }
        Condition::HasTag { tag }
        | Condition::HasItem { item: tag, .. }
        | Condition::ResourceAtLeast { resource: tag, .. }
        | Condition::CharacterKnows { knowledge_id: tag }
        | Condition::CharacterHasDeed { deed_id: tag }
        | Condition::WorldFlag { flag: tag } => {
            if tag.trim().is_empty() {
                errors.push(format!("action {} has an empty condition reference", owner));
            }
        }
        Condition::Always | Condition::Never => {}
    }
}

// The validator intentionally receives all registries and the shared issue
// sink so one walk can report every reference error in an action.
#[allow(clippy::too_many_arguments)]
fn validate_effects(
    effects: &[Effect],
    action: &ActionDefinition,
    location_ids: &BTreeSet<&String>,
    npc_ids: &BTreeSet<&String>,
    parameters: &BTreeSet<&String>,
    roles: &mut BTreeMap<String, BTreeSet<ReferenceRole>>,
    errors: &mut ContentValidationError,
    depth: usize,
) {
    if depth > 64 {
        errors.push(format!("action {} effect AST exceeds depth 64", action.id));
        return;
    }
    for effect in effects {
        match effect {
            Effect::SetFlag { flag, .. } | Effect::SetWorldFlag { flag, .. } => {
                if flag.trim().is_empty() {
                    errors.push(format!("action {} has an empty flag effect", action.id));
                }
            }
            Effect::SetLocationFlag { location, flag, .. } => {
                validate_reference(
                    location,
                    ReferenceRole::Location,
                    action,
                    location_ids,
                    npc_ids,
                    parameters,
                    roles,
                    errors,
                );
                if flag.trim().is_empty() {
                    errors.push(format!("action {} has an empty location flag", action.id));
                }
            }
            Effect::AdjustResource { resource, .. } => {
                if resource.trim().is_empty() {
                    errors.push(format!("action {} has an empty resource effect", action.id));
                }
            }
            Effect::MoveCharacter { location } => validate_reference(
                location,
                ReferenceRole::Location,
                action,
                location_ids,
                npc_ids,
                parameters,
                roles,
                errors,
            ),
            Effect::AdjustNpcRelationship { npc, .. } => validate_reference(
                npc,
                ReferenceRole::Npc,
                action,
                location_ids,
                npc_ids,
                parameters,
                roles,
                errors,
            ),
            Effect::AddNpcMemory {
                npc,
                memory_id,
                subject,
                provenance,
            } => {
                validate_reference(
                    npc,
                    ReferenceRole::Npc,
                    action,
                    location_ids,
                    npc_ids,
                    parameters,
                    roles,
                    errors,
                );
                if memory_id.trim().is_empty() || subject.trim().is_empty() {
                    errors.push(format!(
                        "action {} has an incomplete memory effect",
                        action.id
                    ));
                }
                validate_provenance(provenance, action, npc_ids, errors);
            }
            Effect::TeachNpc {
                npc,
                knowledge_id,
                subject,
                provenance,
            } => {
                validate_reference(
                    npc,
                    ReferenceRole::Npc,
                    action,
                    location_ids,
                    npc_ids,
                    parameters,
                    roles,
                    errors,
                );
                if knowledge_id.trim().is_empty() || subject.trim().is_empty() {
                    errors.push(format!(
                        "action {} has an incomplete knowledge effect",
                        action.id
                    ));
                }
                validate_provenance(provenance, action, npc_ids, errors);
            }
            Effect::AddCharacterDeed { deed_id } => {
                if deed_id.trim().is_empty() {
                    errors.push(format!("action {} has an empty deed effect", action.id));
                }
            }
            Effect::RandomChance {
                success_percent,
                on_success,
                on_failure,
            } => {
                if *success_percent > 100 {
                    errors.push(format!("action {} has chance above 100", action.id));
                }
                validate_effects(
                    std::slice::from_ref(on_success),
                    action,
                    location_ids,
                    npc_ids,
                    parameters,
                    roles,
                    errors,
                    depth + 1,
                );
                validate_effects(
                    std::slice::from_ref(on_failure),
                    action,
                    location_ids,
                    npc_ids,
                    parameters,
                    roles,
                    errors,
                    depth + 1,
                );
            }
            Effect::AdvanceTime { .. } | Effect::Noop => {}
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ReferenceRole {
    Npc,
    Location,
}

fn domain_compatible(
    domain: &ParameterDomain,
    role: ReferenceRole,
    location_ids: &BTreeSet<&String>,
    npc_ids: &BTreeSet<&String>,
) -> bool {
    match (domain, role) {
        (ParameterDomain::NpcsAtCurrentLocation, ReferenceRole::Npc) => true,
        (ParameterDomain::LocationsAdjacent, ReferenceRole::Location) => true,
        (ParameterDomain::Values(values), ReferenceRole::Npc) => values
            .iter()
            .all(|value| npc_ids.iter().any(|candidate| candidate.as_str() == value)),
        (ParameterDomain::Values(values), ReferenceRole::Location) => values.iter().all(|value| {
            location_ids
                .iter()
                .any(|candidate| candidate.as_str() == value)
        }),
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_reference(
    reference: &StringRef,
    role: ReferenceRole,
    action: &ActionDefinition,
    location_ids: &BTreeSet<&String>,
    npc_ids: &BTreeSet<&String>,
    parameters: &BTreeSet<&String>,
    roles: &mut BTreeMap<String, BTreeSet<ReferenceRole>>,
    errors: &mut ContentValidationError,
) {
    match reference {
        StringRef::Literal(value) => {
            if value.trim().is_empty() {
                errors.push(format!("action {} has an empty reference", action.id));
            }
            let known = match role {
                ReferenceRole::Location => location_ids.contains(value),
                ReferenceRole::Npc => npc_ids.contains(value),
            };
            if !known {
                errors.push(format!(
                    "action {} references unknown {:?} {}",
                    action.id, role, value
                ));
            }
        }
        StringRef::Parameter(name) => {
            if !parameters.contains(name) {
                errors.push(format!(
                    "action {} references undeclared parameter {}",
                    action.id, name
                ));
            }
            roles.entry(name.clone()).or_default().insert(role);
        }
    }
}

fn validate_provenance(
    provenance: &KnowledgeProvenance,
    action: &ActionDefinition,
    npc_ids: &BTreeSet<&String>,
    errors: &mut ContentValidationError,
) {
    let source = match provenance {
        KnowledgeProvenance::Told { by } => Some(by),
        KnowledgeProvenance::Rumor { from } => from.as_ref(),
        KnowledgeProvenance::Witnessed
        | KnowledgeProvenance::Read { .. }
        | KnowledgeProvenance::Inferred { .. } => None,
    };
    if let Some(source) = source
        && !npc_ids.iter().any(|candidate| candidate.as_str() == source)
    {
        errors.push(format!(
            "action {} provenance references unknown NPC {}",
            action.id, source
        ));
    }
}

fn validate_location_ref(
    location: &str,
    owner: &str,
    location_ids: &BTreeSet<&String>,
    errors: &mut ContentValidationError,
) {
    if !location_ids
        .iter()
        .any(|candidate| candidate.as_str() == location)
    {
        errors.push(format!(
            "action {} references unknown location {}",
            owner, location
        ));
    }
}

fn validate_npc_ref(
    npc: &str,
    owner: &str,
    npc_ids: &BTreeSet<&String>,
    errors: &mut ContentValidationError,
) {
    if !npc_ids.iter().any(|candidate| candidate.as_str() == npc) {
        errors.push(format!("action {} references unknown NPC {}", owner, npc));
    }
}

fn graph_connected(locations: &[LocationDefinition], ids: &BTreeSet<&String>) -> bool {
    let Some(first) = ids.iter().next() else {
        return true;
    };
    let mut neighbors: BTreeMap<&String, BTreeSet<&String>> =
        ids.iter().map(|id| (*id, BTreeSet::new())).collect();
    for location in locations {
        for exit in &location.exits {
            if ids.contains(exit) {
                neighbors.entry(&location.id).or_default().insert(exit);
                neighbors.entry(exit).or_default().insert(&location.id);
            }
        }
    }
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from([*first]);
    while let Some(id) = queue.pop_front() {
        if !seen.insert(id) {
            continue;
        }
        if let Some(next) = neighbors.get(id) {
            queue.extend(next.iter().copied());
        }
    }
    seen.len() == ids.len()
}

fn split_sentences(text: &str) -> Vec<&str> {
    let sentences: Vec<_> = text
        .split(['.', '!', '?'])
        .map(str::trim)
        .filter(|sentence| !sentence.is_empty())
        .collect();
    if sentences.is_empty() {
        vec![text.trim()]
    } else {
        sentences
    }
}

fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

fn canonical_character_name(name: &str) -> Result<String, ContentValidationError> {
    let canonical = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if canonical.is_empty() {
        return Err(single_validation_error("character name cannot be empty"));
    }
    if canonical.len() > 48 {
        return Err(single_validation_error(
            "character name exceeds the 48-byte limit",
        ));
    }
    if canonical.split_whitespace().count() > 4 {
        return Err(single_validation_error(
            "character name exceeds the 4-word limit",
        ));
    }
    if !canonical
        .chars()
        .all(|character| character.is_alphanumeric() || matches!(character, ' ' | '-' | '\''))
    {
        return Err(single_validation_error(
            "character name contains unsupported characters",
        ));
    }
    Ok(canonical)
}

fn empty_character(id: String) -> Character {
    Character {
        id,
        lineage: String::new(),
        origin: String::new(),
        background: String::new(),
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

fn apply_character_patch(
    character: &mut Character,
    patch: &CharacterPatch,
    owner: &str,
) -> Result<(), ContentValidationError> {
    merge_singleton(&mut character.lineage, &patch.lineage, owner, "lineage")?;
    merge_singleton(&mut character.origin, &patch.origin, owner, "origin")?;
    merge_singleton(
        &mut character.background,
        &patch.background,
        owner,
        "background",
    )?;
    merge_character_map(
        &mut character.aptitudes,
        &patch.aptitudes,
        owner,
        "aptitudes",
    )?;
    character.skills.extend(patch.skills.iter().cloned());
    character.values.extend(patch.values.iter().cloned());
    character.traits.extend(patch.traits.iter().cloned());
    character.flaws.extend(patch.flaws.iter().cloned());
    merge_character_map(
        &mut character.appearance,
        &patch.appearance,
        owner,
        "appearance",
    )?;
    merge_character_map(
        &mut character.affiliations,
        &patch.affiliations,
        owner,
        "affiliations",
    )?;
    merge_character_map(
        &mut character.reputation,
        &patch.reputation,
        owner,
        "reputation",
    )?;
    character.knowledge.extend(patch.knowledge.iter().cloned());
    merge_character_map(
        &mut character.inventory,
        &patch.inventory,
        owner,
        "inventory",
    )?;
    merge_character_map(
        &mut character.resources,
        &patch.resources,
        owner,
        "resources",
    )?;
    character.injuries.extend(patch.injuries.iter().cloned());
    character.deeds.extend(patch.deeds.iter().cloned());
    character.promises.extend(patch.promises.iter().cloned());
    character
        .discoveries
        .extend(patch.discoveries.iter().cloned());
    merge_character_map(&mut character.facets, &patch.facets, owner, "facets")?;
    Ok(())
}

fn merge_singleton(
    target: &mut String,
    value: &Option<String>,
    owner: &str,
    field: &str,
) -> Result<(), ContentValidationError> {
    let Some(value) = value else {
        return Ok(());
    };
    if !target.is_empty() {
        return Err(single_validation_error(format!(
            "{owner} conflicts on character {field}"
        )));
    }
    target.clone_from(value);
    Ok(())
}

fn merge_character_map<T: Clone>(
    target: &mut BTreeMap<String, T>,
    values: &BTreeMap<String, T>,
    owner: &str,
    field: &str,
) -> Result<(), ContentValidationError> {
    for (key, value) in values {
        if target.insert(key.clone(), value.clone()).is_some() {
            return Err(single_validation_error(format!(
                "{owner} conflicts on character {field}.{key}"
            )));
        }
    }
    Ok(())
}

/// Fields for which the current effect vocabulary has no mutation operation.
/// Mutable resources and deeds are intentionally excluded; their provenance
/// is established by a verified trace from an exact authored genesis.
fn static_character_fields_match(left: &Character, right: &Character) -> bool {
    left.id == right.id
        && left.lineage == right.lineage
        && left.origin == right.origin
        && left.background == right.background
        && left.aptitudes == right.aptitudes
        && left.skills == right.skills
        && left.values == right.values
        && left.traits == right.traits
        && left.flaws == right.flaws
        && left.appearance == right.appearance
        && left.affiliations == right.affiliations
        && left.reputation == right.reputation
        && left.knowledge == right.knowledge
        && left.inventory == right.inventory
        && left.injuries == right.injuries
        && left.promises == right.promises
        && left.discoveries == right.discoveries
        && left.facets == right.facets
}

fn single_validation_error(message: impl Into<String>) -> ContentValidationError {
    let mut error = ContentValidationError::new();
    error.push(message);
    error
}

fn sort_variants(variants: &mut [TextVariant]) {
    variants.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.id.cmp(&right.id))
    });
}

fn select_variant_text(fallback: &str, variants: &[TextVariant], state: &GameState) -> String {
    variants
        .iter()
        .filter(|variant| variant.condition.evaluate(state))
        .min_by(|left, right| {
            right
                .priority
                .cmp(&left.priority)
                .then_with(|| left.id.cmp(&right.id))
        })
        .map_or_else(|| fallback.to_owned(), |variant| variant.text.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Character, EntropyState, GameState, KnowledgeProvenance, NpcState, WorldState};

    fn draft(actions: Vec<ActionDefinition>) -> ContentDraft {
        let manifest = BuildManifest::generated();
        ContentDraft {
            schema_version: manifest.schema_abi_version().to_owned(),
            rules_version: manifest.rules_abi_version().to_owned(),
            world_id: "world-1".to_owned(),
            contract: ContentContract::Fixture,
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
            }],
            timed_events: Vec::new(),
            actions,
        }
    }

    fn action(id: &str, condition: Condition, effects: Vec<Effect>) -> ActionDefinition {
        ActionDefinition {
            id: id.to_owned(),
            label: "Use".to_owned(),
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

    fn compile(content: ContentDraft) -> Result<CompiledContent, ContentValidationError> {
        CompiledContent::try_compile(content)
    }

    fn character() -> Character {
        Character {
            id: "hero".to_owned(),
            lineage: "fenborn".to_owned(),
            origin: "lowsail".to_owned(),
            background: "clerk".to_owned(),
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

    fn preset(id: &str, background: &str) -> CharacterPreset {
        let mut character = character();
        character.id = id.to_owned();
        character.background = background.to_owned();
        CharacterPreset {
            id: id.to_owned(),
            display_name: id.to_owned(),
            summary: "A distinct starting path.".to_owned(),
            character,
        }
    }

    fn minimal_creation() -> CharacterCreationDefinition {
        CharacterCreationDefinition {
            base: CharacterPatch::default(),
            slots: vec![
                CharacterCreationSlot {
                    id: "lineage".to_owned(),
                    order: 10,
                    display_name: "Lineage".to_owned(),
                    choices: vec![
                        CharacterCreationChoice {
                            id: "fenborn".to_owned(),
                            display_name: "Fenborn".to_owned(),
                            summary: "A tide-aware lineage.".to_owned(),
                            patch: CharacterPatch {
                                lineage: Some("fenborn".to_owned()),
                                traits: BTreeSet::from(["tide-ear".to_owned()]),
                                ..CharacterPatch::default()
                            },
                        },
                        CharacterCreationChoice {
                            id: "kilnborn".to_owned(),
                            display_name: "Kilnborn".to_owned(),
                            summary: "A heat-aware lineage.".to_owned(),
                            patch: CharacterPatch {
                                lineage: Some("kilnborn".to_owned()),
                                traits: BTreeSet::from(["heat-sense".to_owned()]),
                                ..CharacterPatch::default()
                            },
                        },
                    ],
                },
                CharacterCreationSlot {
                    id: "path".to_owned(),
                    order: 20,
                    display_name: "Path".to_owned(),
                    choices: vec![
                        CharacterCreationChoice {
                            id: "clerk".to_owned(),
                            display_name: "Clerk".to_owned(),
                            summary: "A clerk from Lowsail.".to_owned(),
                            patch: CharacterPatch {
                                origin: Some("lowsail".to_owned()),
                                background: Some("clerk".to_owned()),
                                ..CharacterPatch::default()
                            },
                        },
                        CharacterCreationChoice {
                            id: "scout".to_owned(),
                            display_name: "Scout".to_owned(),
                            summary: "A scout from Red Sluice.".to_owned(),
                            patch: CharacterPatch {
                                origin: Some("red-sluice".to_owned()),
                                background: Some("scout".to_owned()),
                                ..CharacterPatch::default()
                            },
                        },
                    ],
                },
            ],
        }
    }

    fn production_draft(actions: Vec<ActionDefinition>) -> ContentDraft {
        let mut content = draft(actions);
        content.contract = ContentContract::Production;
        content.start_location = "gate".to_owned();
        content.character_presets = vec![preset("hero", "hero"), preset("scout", "scout")];
        content.character_creation = Some(minimal_creation());
        content.npcs[0].goals.insert("keep watch".to_owned());
        content.npcs[0].values.insert("order".to_owned());
        content.npcs[0].tags.insert("guard".to_owned());
        content
    }

    #[test]
    fn timed_events_validate_closed_effects_and_compile_into_identity() {
        let mut source = draft(vec![action(
            "wait",
            Condition::Always,
            vec![Effect::AdvanceTime { ticks: 1 }],
        )]);
        source.timed_events.push(TimedEventDefinition {
            id: "test.deadline".to_owned(),
            due_time: 2,
            event_kind: "deadline".to_owned(),
            label: "Test deadline".to_owned(),
            result: "The test deadline arrives.".to_owned(),
            condition: Condition::Always,
            effects: vec![Effect::SetWorldFlag {
                flag: "deadline_reached".to_owned(),
                value: true,
            }],
        });
        let compiled = compile(source.clone()).expect("valid timed event");
        assert_eq!(compiled.timed_event("test.deadline").unwrap().due_time, 2);

        let mut changed = source.clone();
        changed.timed_events[0].due_time = 3;
        let changed = compile(changed).expect("changed timed event");
        assert_ne!(compiled.build_id(), changed.build_id());

        let mut immediate = source.clone();
        immediate.timed_events[0].due_time = 0;
        assert!(
            compile(immediate)
                .unwrap_err()
                .to_string()
                .contains("due after world time zero")
        );

        let mut recursive_time = source;
        recursive_time.timed_events[0].effects = vec![Effect::RandomChance {
            success_percent: 50,
            on_success: Box::new(Effect::AdvanceTime { ticks: 1 }),
            on_failure: Box::new(Effect::Noop),
        }];
        assert!(
            compile(recursive_time)
                .unwrap_err()
                .to_string()
                .contains("cannot advance time while resolving")
        );
    }

    fn npc_state(id: &str, location: &str) -> NpcState {
        NpcState {
            id: id.to_owned(),
            location: location.to_owned(),
            goals: BTreeSet::new(),
            values: BTreeSet::new(),
            tags: BTreeSet::new(),
            relationships: BTreeMap::new(),
            memories: BTreeMap::new(),
            knowledge: BTreeMap::new(),
            inventory: BTreeMap::new(),
            suspicion: 0,
        }
    }

    fn state(content: &CompiledContent) -> GameState {
        let mut locations = content.empty_location_runtime();
        let npcs = content
            .npcs()
            .map(|(id, definition)| {
                locations
                    .get_mut(&definition.location)
                    .unwrap()
                    .entities
                    .insert(id.clone());
                (id.clone(), npc_state(id, &definition.location))
            })
            .collect();
        GameState::new(
            content.build_id().to_owned(),
            WorldState::new("world-1", "gate", locations, npcs),
            character(),
            EntropyState::new(9),
        )
    }

    #[test]
    fn rejects_untrusted_abi_and_world_mismatch_without_mutating_state() {
        let mut untrusted = draft(Vec::new());
        untrusted.schema_version = "untrusted-schema".to_owned();
        let error = compile(untrusted).unwrap_err();
        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.contains("trusted ABI"))
        );

        let content = compile(draft(Vec::new())).unwrap();
        let mut state = state(&content);
        state.world.id = "other-world".to_owned();
        let before = state.clone();
        assert!(content.validate_state(&state).is_err());
        assert_eq!(state, before);
    }

    #[test]
    fn requires_exact_location_and_npc_state_keys() {
        let content = compile(draft(Vec::new())).unwrap();

        let mut missing_location = state(&content);
        missing_location.world.locations.remove("yard");
        assert!(content.validate_state(&missing_location).is_err());

        let mut missing_npc_location = state(&content);
        missing_npc_location.world.locations.remove("gate");
        assert!(content.validate_state(&missing_npc_location).is_err());

        let mut unknown_npc = state(&content);
        unknown_npc
            .world
            .npcs
            .insert("ghost".to_owned(), npc_state("ghost", "gate"));
        assert!(content.validate_state(&unknown_npc).is_err());

        let mut mismatched_entity = state(&content);
        mismatched_entity
            .world
            .locations
            .get_mut("yard")
            .unwrap()
            .entities
            .insert("sava".to_owned());
        assert!(content.validate_state(&mismatched_entity).is_err());

        let mut missing_entity = state(&content);
        missing_entity
            .world
            .locations
            .get_mut("gate")
            .unwrap()
            .entities
            .remove("sava");
        assert!(content.validate_state(&missing_entity).is_err());

        let mut altered_static_npc = state(&content);
        altered_static_npc
            .world
            .npcs
            .get_mut("sava")
            .unwrap()
            .tags
            .insert("forged-tag".to_owned());
        assert!(content.validate_state(&altered_static_npc).is_err());
    }

    #[test]
    fn rejects_disconnected_graph() {
        let mut content = draft(Vec::new());
        for location in &mut content.locations {
            location.exits.clear();
        }
        let error = compile(content).unwrap_err();
        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.contains("graph is not connected"))
        );
    }

    #[test]
    fn rejects_parameter_reference_domain_mismatch_and_unknown_values() {
        let mut location_action = action(
            "move",
            Condition::Always,
            vec![Effect::MoveCharacter {
                location: StringRef::Parameter("target".to_owned()),
            }],
        );
        location_action.parameters = vec![ParameterSpec {
            name: "target".to_owned(),
            domain: ParameterDomain::Values(vec!["gate".to_owned(), "sava".to_owned()]),
        }];
        assert!(compile(draft(vec![location_action])).is_err());

        let mut npc_action = action(
            "befriend",
            Condition::Always,
            vec![Effect::AdjustNpcRelationship {
                npc: StringRef::Parameter("target".to_owned()),
                amount: 1,
            }],
        );
        npc_action.parameters = vec![ParameterSpec {
            name: "target".to_owned(),
            domain: ParameterDomain::Values(vec!["sava".to_owned(), "gate".to_owned()]),
        }];
        assert!(compile(draft(vec![npc_action])).is_err());

        let mut wrong_domain = action(
            "move-item-domain",
            Condition::Always,
            vec![Effect::MoveCharacter {
                location: StringRef::Parameter("target".to_owned()),
            }],
        );
        wrong_domain.parameters = vec![ParameterSpec {
            name: "target".to_owned(),
            domain: ParameterDomain::InventoryItems,
        }];
        assert!(compile(draft(vec![wrong_domain])).is_err());

        let mut valid_values = action(
            "move-values",
            Condition::Always,
            vec![Effect::MoveCharacter {
                location: StringRef::Parameter("target".to_owned()),
            }],
        );
        valid_values.parameters = vec![ParameterSpec {
            name: "target".to_owned(),
            domain: ParameterDomain::Values(vec!["yard".to_owned(), "gate".to_owned()]),
        }];
        assert!(compile(draft(vec![valid_values])).is_ok());
    }

    #[test]
    fn rejects_unknown_provenance_sources() {
        let error = compile(draft(vec![action(
            "teach",
            Condition::Always,
            vec![Effect::TeachNpc {
                npc: StringRef::Literal("sava".to_owned()),
                knowledge_id: "tide-key".to_owned(),
                subject: "The key is missing.".to_owned(),
                provenance: KnowledgeProvenance::Told {
                    by: "unknown-npc".to_owned(),
                },
            }],
        )]))
        .unwrap_err();
        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.contains("provenance references unknown NPC"))
        );
    }

    #[test]
    fn rejects_never_empty_booleans_and_meaningful_noop_chances() {
        let never = action(
            "never",
            Condition::All {
                conditions: vec![Condition::Always, Condition::Never],
            },
            vec![Effect::Noop],
        );
        let error = compile(draft(vec![never])).unwrap_err();
        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.contains("impossible condition"))
        );

        let not_always = action(
            "not-always",
            Condition::Not {
                condition: Box::new(Condition::Always),
            },
            vec![Effect::Noop],
        );
        let error = compile(draft(vec![not_always])).unwrap_err();
        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.contains("impossible condition"))
        );

        let empty = action(
            "empty",
            Condition::Any {
                conditions: Vec::new(),
            },
            vec![Effect::Noop],
        );
        assert!(compile(draft(vec![empty])).is_err());

        let mut chance = action(
            "coin",
            Condition::Always,
            vec![Effect::RandomChance {
                success_percent: 50,
                on_success: Box::new(Effect::Noop),
                on_failure: Box::new(Effect::Noop),
            }],
        );
        chance.meaningful = true;
        let error = compile(draft(vec![chance])).unwrap_err();
        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.contains("no state-changing effect"))
        );
    }

    #[test]
    fn area_contract_does_not_count_obviously_unavailable_actions() {
        let mut content = draft(vec![
            action(
                "wrong-place-one",
                Condition::AtLocation {
                    location: "yard".to_owned(),
                },
                vec![Effect::SetFlag {
                    flag: "one".to_owned(),
                    value: true,
                }],
            ),
            action(
                "wrong-place-two",
                Condition::AtLocation {
                    location: "yard".to_owned(),
                },
                vec![Effect::SetFlag {
                    flag: "two".to_owned(),
                    value: true,
                }],
            ),
        ]);
        content.locations[0].terminal = false;
        let error = compile(content).unwrap_err();
        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.contains("gate") && issue.contains("meaningful"))
        );
    }

    #[test]
    fn enforces_condition_ast_depth() {
        let mut shallow = Condition::Always;
        for _ in 0..64 {
            shallow = Condition::Not {
                condition: Box::new(shallow),
            };
        }
        assert!(compile(draft(vec![action("shallow", shallow, vec![Effect::Noop],)])).is_ok());

        let mut deep = Condition::Always;
        for _ in 0..65 {
            deep = Condition::Not {
                condition: Box::new(deep),
            };
        }
        let error = compile(draft(vec![action("deep", deep, vec![Effect::Noop])])).unwrap_err();
        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.contains("AST exceeds depth 64"))
        );
    }

    #[test]
    fn content_reordering_has_one_canonical_identity() {
        let mut first_action = action(
            "first",
            Condition::Always,
            vec![
                Effect::MoveCharacter {
                    location: StringRef::Parameter("zeta".to_owned()),
                },
                Effect::AdjustNpcRelationship {
                    npc: StringRef::Parameter("alpha".to_owned()),
                    amount: 1,
                },
            ],
        );
        first_action.parameters = vec![
            ParameterSpec {
                name: "zeta".to_owned(),
                domain: ParameterDomain::Values(vec!["yard".to_owned()]),
            },
            ParameterSpec {
                name: "alpha".to_owned(),
                domain: ParameterDomain::Values(vec!["sava".to_owned()]),
            },
        ];
        let second_action = action(
            "second",
            Condition::Always,
            vec![Effect::SetFlag {
                flag: "second".to_owned(),
                value: true,
            }],
        );
        let mut left = draft(vec![first_action.clone(), second_action.clone()]);
        let mut right = draft(vec![second_action, first_action]);
        left.locations.reverse();
        left.npcs.reverse();
        right.locations.reverse();
        right.npcs.reverse();
        right
            .actions
            .iter_mut()
            .find(|action| action.id == "first")
            .expect("reordered action must be present")
            .parameters
            .reverse();

        let left = compile(left).unwrap();
        let right = compile(right).unwrap();
        assert_eq!(left.build_id(), right.build_id());
    }

    #[test]
    fn rejects_parameters_that_only_manufacture_distinct_action_ids() {
        let mut unused = action(
            "fake-depth",
            Condition::Always,
            vec![Effect::SetFlag {
                flag: "same-result".to_owned(),
                value: true,
            }],
        );
        unused.parameters = vec![ParameterSpec {
            name: "unused".to_owned(),
            domain: ParameterDomain::Values(vec!["one".to_owned(), "two".to_owned()]),
        }];

        let error = compile(draft(vec![unused])).unwrap_err();
        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.contains("declares unused parameter"))
        );
    }

    #[test]
    fn new_game_is_deterministic_and_populates_npc_indexes_from_content() {
        let content = compile(production_draft(Vec::new())).unwrap();
        let left = content.new_game("hero", 123).unwrap();
        let right = content.new_game("hero", 123).unwrap();
        assert_eq!(left, right);
        assert_eq!(left.world.id, "world-1");
        assert_eq!(left.world.current_location, "gate");
        assert_eq!(left.character.id, "hero");
        assert_eq!(left.entropy, EntropyState::new(123));
        assert_eq!(
            left.world.npcs["sava"].goals,
            BTreeSet::from(["keep watch".to_owned()])
        );
        assert_eq!(
            left.world.npcs["sava"].values,
            BTreeSet::from(["order".to_owned()])
        );
        assert_eq!(
            left.world.npcs["sava"].tags,
            BTreeSet::from(["guard".to_owned()])
        );
        assert!(left.world.locations["gate"].entities.contains("sava"));
        assert!(content.new_game("missing", 123).is_err());

        let mut forged_identity = left.clone();
        forged_identity.character.background = "forged".to_owned();
        assert!(content.validate_state(&forged_identity).is_err());

        let mut progressed = left;
        progressed.character.resources.insert("coin".to_owned(), 99);
        progressed.character.deeds.insert("later-deed".to_owned());
        assert!(content.validate_state(&progressed).is_ok());
    }

    #[test]
    fn production_contract_requires_start_and_two_presets() {
        let mut content = production_draft(Vec::new());
        content.character_presets.pop();
        content.character_creation = None;
        content.start_location.clear();
        let error = compile(content).unwrap_err();
        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.contains("at least two character presets"))
        );
        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.contains("start_location"))
        );
        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.contains("authored character creation"))
        );
    }

    #[test]
    fn creation_schema_rejects_cosmetic_choices_conflicts_and_duplicate_order() {
        let mut cosmetic = production_draft(Vec::new());
        let creation = cosmetic.character_creation.as_mut().unwrap();
        creation.slots[0].choices[0].patch = CharacterPatch {
            appearance: BTreeMap::from([("marking".to_owned(), "blue".to_owned())]),
            ..CharacterPatch::default()
        };
        let error = compile(cosmetic).unwrap_err();
        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.contains("cosmetic-only"))
        );

        let mut conflicting = production_draft(Vec::new());
        let creation = conflicting.character_creation.as_mut().unwrap();
        creation.slots[1].choices[0].patch.lineage = Some("other".to_owned());
        let error = compile(conflicting).unwrap_err();
        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.contains("both write lineage"))
        );

        let mut duplicate_order = production_draft(Vec::new());
        let creation = duplicate_order.character_creation.as_mut().unwrap();
        creation.slots[1].order = creation.slots[0].order;
        let error = compile(duplicate_order).unwrap_err();
        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.contains("slot order"))
        );

        let mut shadowing = production_draft(Vec::new());
        shadowing.character_creation.as_mut().unwrap().slots[0].choices[0]
            .patch
            .facets
            .insert("lineage".to_owned(), FacetValue::Text("forged".to_owned()));
        let error = compile(shadowing).unwrap_err();
        assert!(
            error
                .issues
                .iter()
                .any(|issue| issue.contains("reserved facet axis lineage"))
        );

        let mut character = character();
        character.lineage = "authoritative".to_owned();
        character
            .facets
            .insert("lineage".to_owned(), FacetValue::Text("forged".to_owned()));
        assert_eq!(
            character.facet_value("lineage"),
            Some(FacetValue::Text("authoritative".to_owned()))
        );
    }

    #[test]
    fn creation_source_order_is_canonical_but_mechanical_changes_rebuild_identity() {
        let ordered = production_draft(Vec::new());
        let mut reordered = ordered.clone();
        let creation = reordered.character_creation.as_mut().unwrap();
        creation.slots.reverse();
        for slot in &mut creation.slots {
            slot.choices.reverse();
        }
        assert_eq!(
            compile(ordered.clone()).unwrap().build_id(),
            compile(reordered).unwrap().build_id()
        );

        let mut changed = ordered;
        changed.character_creation.as_mut().unwrap().slots[0].choices[0]
            .patch
            .traits
            .insert("new-mechanic".to_owned());
        assert_ne!(
            compile(production_draft(Vec::new())).unwrap().build_id(),
            compile(changed).unwrap().build_id()
        );
    }

    #[test]
    fn presentation_selects_conditions_with_stable_priority_and_tie_breaks() {
        let mut inspect = action(
            "inspect",
            Condition::Always,
            vec![Effect::SetFlag {
                flag: "inspected".to_owned(),
                value: true,
            }],
        );
        inspect.result = "The inspection is complete.".to_owned();
        inspect.result_variants = vec![
            TextVariant {
                id: "flagged".to_owned(),
                priority: 5,
                condition: Condition::WorldFlag {
                    flag: "special".to_owned(),
                },
                text: "The marked result appears.".to_owned(),
            },
            TextVariant {
                id: "facet".to_owned(),
                priority: 5,
                condition: Condition::FacetEquals {
                    axis: "background".to_owned(),
                    value: FacetValue::Text("hero".to_owned()),
                },
                text: "The skilled result appears.".to_owned(),
            },
            TextVariant {
                id: "base".to_owned(),
                priority: 1,
                condition: Condition::Always,
                text: "The ordinary result appears.".to_owned(),
            },
        ];
        let mut content = production_draft(vec![inspect]);
        content.locations[0].description_variants = vec![
            TextVariant {
                id: "zeta".to_owned(),
                priority: 3,
                condition: Condition::Always,
                text: "The later description wins only by tie.".to_owned(),
            },
            TextVariant {
                id: "alpha".to_owned(),
                priority: 3,
                condition: Condition::Always,
                text: "The earlier description wins by ID.".to_owned(),
            },
            TextVariant {
                id: "hero".to_owned(),
                priority: 4,
                condition: Condition::FacetEquals {
                    axis: "background".to_owned(),
                    value: FacetValue::Text("hero".to_owned()),
                },
                text: "The hero sees a bright gate.".to_owned(),
            },
        ];
        let content = compile(content).unwrap();
        let hero = content.new_game("hero", 1).unwrap();
        let scout = content.new_game("scout", 1).unwrap();
        assert_eq!(
            content.location_description(&hero).unwrap(),
            "The hero sees a bright gate."
        );
        assert_eq!(
            content.location_description(&scout).unwrap(),
            "The earlier description wins by ID."
        );
        assert_eq!(
            content.action_result(&scout, "inspect").unwrap(),
            "The ordinary result appears."
        );
        let mut flagged = scout.clone();
        flagged.world.flags.insert("special".to_owned());
        assert_eq!(
            content.action_result(&flagged, "inspect").unwrap(),
            "The marked result appears."
        );
        let observation = content.observe_action(&flagged, "inspect").unwrap();
        assert_eq!(
            observation.result.as_deref(),
            Some("The marked result appears.")
        );
        assert!(observation.text.starts_with("The marked result appears."));
        assert_eq!(observation.location_id, "gate");
        assert_eq!(observation.title, "Gate");
        assert!(observation.text.split_whitespace().count() <= 100);
    }

    #[test]
    fn validates_category_result_and_variant_sentence_limits() {
        let mut over_category = action("category", Condition::Always, vec![Effect::Noop]);
        over_category.category = "one two three four".to_owned();
        assert!(compile(draft(vec![over_category])).is_err());

        let mut over_result = action("result", Condition::Always, vec![Effect::Noop]);
        over_result.result = std::iter::repeat_n("word", 61)
            .collect::<Vec<_>>()
            .join(" ");
        assert!(compile(draft(vec![over_result])).is_err());

        let mut over_variant = action("variant", Condition::Always, vec![Effect::Noop]);
        over_variant.result_variants = vec![TextVariant {
            id: "too-long".to_owned(),
            priority: 1,
            condition: Condition::Always,
            text: "First sentence. Second sentence.".to_owned(),
        }];
        assert!(compile(draft(vec![over_variant])).is_err());
    }

    #[test]
    fn action_pages_expose_complete_time_cost_ranges_and_reject_overflow() {
        assert_eq!(
            effect_time_cost(&Effect::RandomChance {
                success_percent: 100,
                on_success: Box::new(Effect::AdvanceTime { ticks: 2 }),
                on_failure: Box::new(Effect::AdvanceTime { ticks: 9 }),
            }),
            Some(ActionTimeCost {
                minimum_ticks: 2,
                maximum_ticks: 2,
            })
        );
        let ranged = action(
            "ranged-time",
            Condition::Always,
            vec![
                Effect::AdvanceTime { ticks: 2 },
                Effect::RandomChance {
                    success_percent: 50,
                    on_success: Box::new(Effect::AdvanceTime { ticks: 1 }),
                    on_failure: Box::new(Effect::AdvanceTime { ticks: 4 }),
                },
            ],
        );
        let content = compile(draft(vec![ranged])).unwrap();
        let state = state(&content);
        let page = content.action_page(&state, 0, 1).unwrap();
        assert_eq!(
            page.actions[0].time_cost,
            ActionTimeCost {
                minimum_ticks: 3,
                maximum_ticks: 6,
            }
        );

        let overflow = action(
            "overflow-time",
            Condition::Always,
            vec![
                Effect::AdvanceTime { ticks: u64::MAX },
                Effect::AdvanceTime { ticks: 1 },
            ],
        );
        assert!(
            compile(draft(vec![overflow]))
                .unwrap_err()
                .to_string()
                .contains("time cost overflows world time")
        );
    }

    #[test]
    fn action_pages_union_to_the_full_catalog_and_keep_one_digest() {
        let destinations: Vec<_> = (0..256).map(|index| format!("page-{index:03}")).collect();
        let mut travel = action(
            "travel",
            Condition::Always,
            vec![Effect::MoveCharacter {
                location: StringRef::Parameter("destination".to_owned()),
            }],
        );
        travel.parameters = vec![ParameterSpec {
            name: "destination".to_owned(),
            domain: ParameterDomain::LocationsAdjacent,
        }];
        travel.meaningful = true;
        travel.movement = true;
        let mut content = draft(vec![travel]);
        content.locations = std::iter::once(LocationDefinition {
            id: "gate".to_owned(),
            name: "Gate".to_owned(),
            description: "A gate opens to many roads.".to_owned(),
            description_variants: Vec::new(),
            exits: destinations.clone(),
            terminal: true,
        })
        .chain(destinations.iter().map(|id| LocationDefinition {
            id: id.clone(),
            name: format!("Road {}", id.trim_start_matches("page-")),
            description: "A marked road ends here.".to_owned(),
            description_variants: Vec::new(),
            exits: vec!["gate".to_owned()],
            terminal: true,
        }))
        .collect();
        let content = compile(content).unwrap();
        let state = state(&content);
        let full = crate::enumerate_legal_actions(&state, &content).unwrap();
        assert_eq!(full.len(), 256);
        let expected_ids: Vec<_> = full.iter().map(|action| action.action_id.clone()).collect();
        let mut pages = Vec::new();
        let mut offset = 0;
        let mut digest = None;
        let mut saw_display_name = false;
        loop {
            let page = content.action_page(&state, offset, 17).unwrap();
            assert_eq!(page.build_id, content.build_id());
            assert_eq!(page.state_id, state.state_id());
            assert_eq!(page.total, 256);
            if let Some(expected) = &digest {
                assert_eq!(expected, &page.digest);
            } else {
                digest = Some(page.digest.clone());
            }
            if let Some(action) = page.actions.iter().find(|action| {
                action.parameters.get("destination").map(String::as_str) == Some("page-000")
            }) {
                assert_eq!(action.parameter_display_values["destination"], "Road 000");
                saw_display_name = true;
            }
            pages.extend(page.actions.iter().map(|action| action.action_id.clone()));
            match page.next_offset {
                Some(next) => offset = next,
                None => break,
            }
        }
        assert!(saw_display_name);
        assert_eq!(pages, expected_ids);
        let expected_digest = crate::legal_action_digest(&full).unwrap();
        assert_eq!(digest.as_deref(), Some(expected_digest.as_str()));
    }
}
