//! The deterministic authority for Adventure Forge.
//!
//! The kernel deliberately has no I/O, clocks, networking, or model calls.  A
//! caller gives it an immutable state, a compiled content pack, a canonical
//! action, and an explicit entropy state.  It returns a new state and a
//! replayable transition.

mod build_manifest;
mod content;
mod entropy;
mod hash;
mod model;
mod reducer;

pub use build_manifest::BuildManifest;
pub use content::{
    ActionDefinition, ActionPage, ActionTimeCost, ActionView, CharacterCreationChoice,
    CharacterCreationDefinition, CharacterCreationSlot, CharacterPatch, CharacterPreset,
    CompiledContent, Condition, ContentContract, ContentDraft, ContentValidationError, Effect,
    ItemView, LocationDefinition, NpcDefinition, Observation, ParameterDomain, ParameterSpec,
    RecipeDefinition, ResourceView, StringRef, SupplyLabels, SupplyView, TextVariant,
    TimedEventDefinition, TimedEventView,
};
pub use entropy::{
    ENTROPY_ALGORITHM_VERSION, EntropyDraw, EntropyError, EntropyState, MAX_ENTROPY_CURSOR,
};
pub use hash::{
    HashError, canonical_json_bytes, sha256_hex_bytes, sha256_json, validate_unique_json_keys,
};
pub use model::{
    ActionId, BuildId, Character, CharacterChoiceSelection, CharacterId, CharacterSelection,
    CharacterStart, Event, EventKind, FacetValue, GameState, Knowledge, KnowledgeProvenance,
    KnowledgeProvenanceKind, LocationId, LocationRuntime, Memory, NpcId, NpcState, ScheduledEvent,
    WorldId, WorldState,
};
pub use reducer::{
    CanonicalAction, KernelError, Transition, enumerate_legal_actions, legal_action_digest, step,
    validate_action,
};

/// Return the identity already computed from trusted build provenance and
/// validated content. Independent checkers use the serialized manifest and
/// content pack to recompute the same value.
pub fn compute_build_id(content: &CompiledContent) -> BuildId {
    content.build_id().to_owned()
}
