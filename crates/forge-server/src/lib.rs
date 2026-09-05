//! Transport-independent, player-safe session services for the future browser.
//!
//! This library is not an HTTP server or an authentication boundary. Callers
//! own session handles and lifecycle; a future transport must authorize them.
//! Only the kernel and replay layer determine game truth. No hidden state,
//! authored character patches, filesystem paths, or detailed traces are public
//! response types here.

#![forbid(unsafe_code)]

use forge_kernel::{
    ActionPage, CharacterSelection, CompiledContent, ContentContract, Observation,
    validate_unique_json_keys,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::fmt::{Display, Formatter};

mod service;
pub use service::SessionService;

/// Transport input ceiling, not a limit on the kernel's legal catalog.
pub const MAX_REQUEST_BYTES: usize = 128 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum StartRequest {
    Preset {
        character_preset_id: String,
        seed: u64,
    },
    Custom {
        selection: CharacterSelection,
        seed: u64,
    },
}

impl StartRequest {
    pub fn from_json(input: &str) -> Result<Self, ServiceError> {
        decode_request(input)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActionRequest {
    pub command_id: String,
    pub expected_revision: u64,
    pub expected_state_id: String,
    pub action_id: String,
}

impl ActionRequest {
    pub fn from_json(input: &str) -> Result<Self, ServiceError> {
        decode_request(input)
    }
}

fn decode_request<T: DeserializeOwned>(input: &str) -> Result<T, ServiceError> {
    if input.len() > MAX_REQUEST_BYTES {
        return Err(ServiceError::ResourceLimit);
    }
    validate_unique_json_keys(input).map_err(|_| ServiceError::InvalidInput)?;
    serde_json::from_str(input).map_err(|_| ServiceError::InvalidInput)
}

/// The current or historically acknowledged public view. Retried commands
/// return their original view even if later commands advanced the session.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SessionView {
    pub revision: u64,
    pub observation: Observation,
    pub catalog: ActionPage,
}

/// Checked serialized-byte budgets, not a hard process-memory or CPU sandbox.
/// A full portable save is exported separately, never in cached action replies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceLimits {
    pub max_save_bytes: usize,
    pub max_response_bytes: usize,
    pub max_idempotency_bytes: usize,
    pub default_page_size: usize,
    pub max_page_size: usize,
}

impl Default for ServiceLimits {
    fn default() -> Self {
        Self {
            max_save_bytes: 16 * 1024 * 1024,
            max_response_bytes: 128 * 1024,
            max_idempotency_bytes: 16 * 1024 * 1024,
            default_page_size: 32,
            max_page_size: 128,
        }
    }
}

/// Stable public failures intentionally omit kernel/replay/host diagnostics.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceError {
    InvalidInput,
    InvalidContent,
    InvalidSave,
    InvalidAction,
    StaleState,
    IdempotencyConflict,
    SessionClosed,
    ResourceLimit,
    Unavailable,
    Internal,
}

impl Display for ServiceError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidInput => "invalid_input",
            Self::InvalidContent => "invalid_content",
            Self::InvalidSave => "invalid_save",
            Self::InvalidAction => "invalid_action",
            Self::StaleState => "stale_state",
            Self::IdempotencyConflict => "idempotency_conflict",
            Self::SessionClosed => "session_closed",
            Self::ResourceLimit => "resource_limit",
            Self::Unavailable => "unavailable",
            Self::Internal => "internal",
        })
    }
}

impl std::error::Error for ServiceError {}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PresetOption {
    pub id: String,
    pub display_name: String,
    pub summary: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ChoiceOption {
    pub id: String,
    pub display_name: String,
    pub summary: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CreationSlot {
    pub id: String,
    pub order: u16,
    pub display_name: String,
    pub choices: Vec<ChoiceOption>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StartOptions {
    pub build_id: String,
    pub presets: Vec<PresetOption>,
    pub creation_slots: Vec<CreationSlot>,
}

/// Deliberate allow-list projection: never serialize preset characters or
/// creation base/choice patches to a player.
pub fn start_options(content: &CompiledContent) -> Result<StartOptions, ServiceError> {
    if content.contract() != ContentContract::Production {
        return Err(ServiceError::InvalidContent);
    }
    let options = StartOptions {
        build_id: content.build_id().to_owned(),
        presets: content
            .character_presets()
            .map(|(_, preset)| PresetOption {
                id: preset.id.clone(),
                display_name: preset.display_name.clone(),
                summary: preset.summary.clone(),
            })
            .collect(),
        creation_slots: content
            .character_creation()
            .map_or_else(Vec::new, |creation| {
                creation
                    .slots
                    .iter()
                    .map(|slot| CreationSlot {
                        id: slot.id.clone(),
                        order: slot.order,
                        display_name: slot.display_name.clone(),
                        choices: slot
                            .choices
                            .iter()
                            .map(|choice| ChoiceOption {
                                id: choice.id.clone(),
                                display_name: choice.display_name.clone(),
                                summary: choice.summary.clone(),
                            })
                            .collect(),
                    })
                    .collect()
            }),
    };
    let bytes = serde_json::to_vec(&options).map_err(|_| ServiceError::Internal)?;
    if bytes.len() > MAX_REQUEST_BYTES {
        return Err(ServiceError::ResourceLimit);
    }
    Ok(options)
}
