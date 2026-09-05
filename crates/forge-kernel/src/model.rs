use crate::EntropyState;
use crate::hash::sha256_json;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub type BuildId = String;
pub type WorldId = String;
pub type LocationId = String;
pub type CharacterId = String;
pub type NpcId = String;
pub type ActionId = String;

/// One public, authored choice in a custom character selection.
///
/// A vector is used instead of a map so duplicate slot selections survive
/// deserialization and can be rejected explicitly by the kernel.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct CharacterChoiceSelection {
    pub slot_id: String,
    pub choice_id: String,
}

/// Player-supplied public inputs for an authored custom character.
/// Materialized character fields are never accepted from the player.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct CharacterSelection {
    pub name: String,
    pub choices: Vec<CharacterChoiceSelection>,
}

/// Immutable provenance for the character at the root of a state.
/// Production states must be reconstructable from one of the authored forms.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CharacterStart {
    Fixture,
    Preset { character_preset_id: String },
    Custom { selection: CharacterSelection },
}

/// Extensible values for character facets.  Built-in axes are represented by
/// named fields on `Character`; this map lets content add new axes without
/// changing the kernel's state shape.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum FacetValue {
    Text(String),
    Number(i64),
    Boolean(bool),
    Tags(BTreeSet<String>),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Character {
    pub id: CharacterId,
    pub lineage: String,
    pub origin: String,
    pub background: String,
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

impl Character {
    /// Resolve both built-in and extensible axes for data-driven conditions.
    pub fn facet_value(&self, axis: &str) -> Option<FacetValue> {
        match axis {
            "lineage" => Some(FacetValue::Text(self.lineage.clone())),
            "origin" => Some(FacetValue::Text(self.origin.clone())),
            "background" => Some(FacetValue::Text(self.background.clone())),
            "aptitudes" | "skills" | "values" | "traits" | "flaws" | "appearance"
            | "affiliations" | "reputation" | "knowledge" | "injuries" | "deeds" | "promises"
            | "discoveries" => None,
            _ => {
                if let Some(name) = axis.strip_prefix("aptitude.") {
                    self.aptitudes.get(name).copied().map(FacetValue::Number)
                } else if let Some(name) = axis.strip_prefix("affiliation.") {
                    self.affiliations.get(name).copied().map(FacetValue::Number)
                } else if let Some(name) = axis.strip_prefix("reputation.") {
                    self.reputation.get(name).copied().map(FacetValue::Number)
                } else if let Some(name) = axis.strip_prefix("appearance.") {
                    self.appearance.get(name).cloned().map(FacetValue::Text)
                } else {
                    self.facets.get(axis).cloned()
                }
            }
        }
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.skills.contains(tag)
            || self.values.contains(tag)
            || self.traits.contains(tag)
            || self.flaws.contains(tag)
            || self.knowledge.contains(tag)
            || self.injuries.contains(tag)
            || self.deeds.contains(tag)
            || self.promises.contains(tag)
            || self.discoveries.contains(tag)
            || self
                .facets
                .values()
                .any(|value| matches!(value, FacetValue::Tags(tags) if tags.contains(tag)))
    }
}

/// Facet keys that would collide with authoritative typed character axes.
pub(crate) fn is_reserved_character_facet_axis(axis: &str) -> bool {
    matches!(
        axis,
        "lineage"
            | "origin"
            | "background"
            | "aptitudes"
            | "skills"
            | "values"
            | "traits"
            | "flaws"
            | "appearance"
            | "affiliations"
            | "reputation"
            | "knowledge"
            | "inventory"
            | "resources"
            | "injuries"
            | "deeds"
            | "promises"
            | "discoveries"
            | "facets"
    ) || ["aptitude.", "affiliation.", "reputation.", "appearance."]
        .iter()
        .any(|prefix| axis.starts_with(prefix))
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum KnowledgeProvenance {
    Witnessed,
    Told { by: NpcId },
    Read { source: String },
    Inferred { from: String },
    Rumor { from: Option<NpcId> },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeProvenanceKind {
    Witnessed,
    Told,
    Read,
    Inferred,
    Rumor,
}

impl KnowledgeProvenance {
    pub fn kind(&self) -> KnowledgeProvenanceKind {
        match self {
            Self::Witnessed => KnowledgeProvenanceKind::Witnessed,
            Self::Told { .. } => KnowledgeProvenanceKind::Told,
            Self::Read { .. } => KnowledgeProvenanceKind::Read,
            Self::Inferred { .. } => KnowledgeProvenanceKind::Inferred,
            Self::Rumor { .. } => KnowledgeProvenanceKind::Rumor,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Memory {
    pub id: String,
    pub subject: String,
    pub turn: u64,
    pub provenance: KnowledgeProvenance,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Knowledge {
    pub id: String,
    pub subject: String,
    pub turn: u64,
    pub provenance: KnowledgeProvenance,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NpcState {
    pub id: NpcId,
    pub location: LocationId,
    pub goals: BTreeSet<String>,
    pub values: BTreeSet<String>,
    pub tags: BTreeSet<String>,
    pub relationships: BTreeMap<String, i64>,
    pub memories: BTreeMap<String, Memory>,
    pub knowledge: BTreeMap<String, Knowledge>,
    pub inventory: BTreeMap<String, u32>,
    pub suspicion: i64,
}

impl NpcState {
    pub fn knows(&self, knowledge_id: &str) -> bool {
        self.knowledge.contains_key(knowledge_id)
    }

    pub fn remembers(&self, memory_id: &str) -> bool {
        self.memories.contains_key(memory_id)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LocationRuntime {
    pub flags: BTreeSet<String>,
    pub entities: BTreeSet<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ScheduledEvent {
    pub id: String,
    pub due_time: u64,
    pub event_kind: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorldState {
    pub id: WorldId,
    pub time: u64,
    pub current_location: LocationId,
    pub locations: BTreeMap<LocationId, LocationRuntime>,
    pub npcs: BTreeMap<NpcId, NpcState>,
    pub flags: BTreeSet<String>,
    pub scheduled_events: Vec<ScheduledEvent>,
}

impl WorldState {
    pub fn new(
        id: impl Into<WorldId>,
        current_location: impl Into<LocationId>,
        locations: BTreeMap<LocationId, LocationRuntime>,
        npcs: BTreeMap<NpcId, NpcState>,
    ) -> Self {
        Self {
            id: id.into(),
            time: 0,
            current_location: current_location.into(),
            locations,
            npcs,
            flags: BTreeSet::new(),
            scheduled_events: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GameState {
    pub build_id: BuildId,
    pub world: WorldState,
    pub character: Character,
    pub character_start: CharacterStart,
    pub entropy: EntropyState,
    pub event_log: Vec<Event>,
}

impl GameState {
    pub fn new(
        build_id: impl Into<BuildId>,
        world: WorldState,
        character: Character,
        entropy: EntropyState,
    ) -> Self {
        Self {
            build_id: build_id.into(),
            world,
            character,
            character_start: CharacterStart::Fixture,
            entropy,
            event_log: Vec::new(),
        }
    }

    /// Hash only canonical serialized authoritative state.  Derived indexes
    /// must not be added here unless they are themselves authoritative.
    pub fn state_id(&self) -> String {
        sha256_json(self).expect("kernel state must always be serializable")
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub turn: u64,
    pub kind: EventKind,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EventKind {
    FlagSet {
        flag: String,
        value: bool,
    },
    WorldFlagSet {
        flag: String,
        value: bool,
    },
    LocationFlagSet {
        location: LocationId,
        flag: String,
        value: bool,
    },
    ResourceAdjusted {
        resource: String,
        amount: i64,
    },
    Moved {
        from: LocationId,
        to: LocationId,
    },
    NpcMoved {
        npc: NpcId,
        from: LocationId,
        to: LocationId,
    },
    NpcRelationshipAdjusted {
        npc: NpcId,
        amount: i64,
    },
    NpcMemoryAdded {
        npc: NpcId,
        memory: String,
    },
    NpcKnowledgeAdded {
        npc: NpcId,
        knowledge: String,
    },
    NpcItemTransferredToCharacter {
        npc: NpcId,
        item: String,
        count: u32,
    },
    RecipeApplied {
        recipe: String,
        inputs: BTreeMap<String, u32>,
        outputs: BTreeMap<String, u32>,
    },
    TimeAdvanced {
        ticks: u64,
    },
    ScheduledEventResolved {
        event_id: String,
        event_kind: String,
        applied: bool,
    },
    EventScheduled {
        event_id: String,
        event_kind: String,
        due_time: u64,
    },
    RandomDraw {
        algorithm: String,
        cursor: u64,
        value: u64,
    },
}
