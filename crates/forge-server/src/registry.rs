//! In-process session lifetime and creation-idempotency registry.
//!
//! The registry is deliberately a thin owner of SessionService handles.  It
//! does not inspect or reproduce game state, and it never chooses a game
//! action.  Its only state is the bounded lifetime of transport handles and
//! the response needed to acknowledge a retried creation request.

use crate::{
    ActionRequest, MAX_REQUEST_BYTES, ServiceError, ServiceLimits, SessionService, SessionView,
    StartRequest,
};
use forge_kernel::{ActionPage, CompiledContent, sha256_json};
use getrandom::fill as fill_random;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter, Write};
use std::sync::{Arc, Mutex, MutexGuard};

const CREATE_RESPONSE_OVERHEAD_BYTES: usize = 512;
const MAX_RANDOM_ID_ATTEMPTS: usize = 4;

/// Lifetime and concurrent-active limits for a registry.
///
/// max_sessions counts every retained record, including records that have
/// been closed.  Closed records are never evicted, so a process restart is
/// the explicit reclamation boundary.  max_active_sessions only counts
/// records which have not been closed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistryLimits {
    pub max_sessions: usize,
    pub max_active_sessions: usize,
}

impl Default for RegistryLimits {
    fn default() -> Self {
        Self {
            max_sessions: 8,
            max_active_sessions: 1,
        }
    }
}

/// The initial public view and opaque handle returned by a successful create
/// or resume operation.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CreateResponse {
    pub session_id: String,
    pub view: SessionView,
}

/// Stable registry-level failures.  Service-specific player errors remain
/// wrapped so transports can preserve their existing public error codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegistryError {
    InvalidInput,
    UnknownSession,
    ResourceLimit,
    IdempotencyConflict,
    Unavailable,
    Internal,
    Service(ServiceError),
}

impl RegistryError {
    /// Stable snake-case code for a transport error body.
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::UnknownSession => "unknown_session",
            Self::ResourceLimit => "resource_limit",
            Self::IdempotencyConflict => "idempotency_conflict",
            Self::Unavailable => "unavailable",
            Self::Internal => "internal",
            Self::Service(error) => service_error_code(error),
        }
    }
}

impl Display for RegistryError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for RegistryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Service(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ServiceError> for RegistryError {
    fn from(error: ServiceError) -> Self {
        Self::Service(error)
    }
}

/// A clone shares the same bounded registry and all its retained handles.
#[derive(Clone)]
pub struct SessionRegistry {
    content: Arc<CompiledContent>,
    service_limits: ServiceLimits,
    registry_limits: RegistryLimits,
    state: Arc<Mutex<RegistryState>>,
}

struct RegistryState {
    records: Vec<SessionRecord>,
}

struct SessionRecord {
    session_id: String,
    creation_id: String,
    creation_fingerprint: String,
    create_response: CreateResponse,
    service: SessionService,
    closed: bool,
}

#[derive(Serialize)]
enum CreationFingerprint<'a> {
    Start { request: &'a StartRequest },
    Resume { save_json: &'a str },
}

impl SessionRegistry {
    /// Construct an empty registry for one immutable compiled content build.
    pub fn new(
        content: Arc<CompiledContent>,
        service_limits: ServiceLimits,
        registry_limits: RegistryLimits,
    ) -> Result<Self, RegistryError> {
        if registry_limits.max_sessions == 0
            || registry_limits.max_active_sessions == 0
            || registry_limits.max_active_sessions > registry_limits.max_sessions
        {
            return Err(RegistryError::InvalidInput);
        }

        Ok(Self {
            content,
            service_limits,
            registry_limits,
            state: Arc::new(Mutex::new(RegistryState {
                records: Vec::new(),
            })),
        })
    }

    /// Start a session, or return the exact original response for a retried
    /// creation ID and identical request.  The creation ID is not a game
    /// command and is never placed in the service action ledger.
    pub fn start(
        &self,
        creation_id: &str,
        start: StartRequest,
    ) -> Result<CreateResponse, RegistryError> {
        validate_creation_id(creation_id)?;
        ensure_start_size(&start)?;
        let creation_fingerprint = fingerprint_start(&start)?;
        let mut state = self.lock_state()?;

        if let Some(existing) = state
            .records
            .iter()
            .find(|record| record.creation_id == creation_id)
        {
            return if existing.creation_fingerprint == creation_fingerprint {
                Ok(existing.create_response.clone())
            } else {
                Err(RegistryError::IdempotencyConflict)
            };
        }

        self.reserve_creation_slot(&mut state)?;
        let service = SessionService::start(
            Arc::clone(&self.content),
            start,
            self.service_limits.clone(),
        )?;
        self.publish_new_record(&mut state, creation_id, creation_fingerprint, service)
    }

    /// Resume a player-safe save as a new session handle, with the same
    /// creation-idempotency behavior as start.
    pub fn resume(
        &self,
        creation_id: &str,
        save_json: &str,
    ) -> Result<CreateResponse, RegistryError> {
        validate_creation_id(creation_id)?;
        if save_json.len() > self.service_limits.max_save_bytes {
            return Err(RegistryError::ResourceLimit);
        }
        let creation_fingerprint = fingerprint_resume(save_json)?;
        let mut state = self.lock_state()?;

        if let Some(existing) = state
            .records
            .iter()
            .find(|record| record.creation_id == creation_id)
        {
            return if existing.creation_fingerprint == creation_fingerprint {
                Ok(existing.create_response.clone())
            } else {
                Err(RegistryError::IdempotencyConflict)
            };
        }

        self.reserve_creation_slot(&mut state)?;
        let service = SessionService::resume(
            Arc::clone(&self.content),
            save_json,
            self.service_limits.clone(),
        )?;
        self.publish_new_record(&mut state, creation_id, creation_fingerprint, service)
    }

    /// Return the current player-safe view for a retained session.
    pub fn observe(&self, session_id: &str) -> Result<SessionView, RegistryError> {
        self.lookup_service(session_id)?
            .observe()
            .map_err(Into::into)
    }

    /// Return the one active session for browser recovery.
    ///
    /// This endpoint is intentionally restricted to the single-active-session
    /// registry shape.  The registry lock remains held while the service is
    /// observed, so a concurrent close cannot make the returned handle and
    /// view disagree.  Observing is read-only and never advances the game.
    pub fn current(&self) -> Result<Option<CreateResponse>, RegistryError> {
        if self.registry_limits.max_active_sessions != 1 {
            return Err(RegistryError::InvalidInput);
        }
        let state = self.lock_state()?;
        let Some(record) = state.records.iter().find(|record| !record.closed) else {
            return Ok(None);
        };
        let view = record.service.observe()?;
        Ok(Some(CreateResponse {
            session_id: record.session_id.clone(),
            view,
        }))
    }

    /// Return one page from a retained session's complete legal catalog.
    pub fn catalog(
        &self,
        session_id: &str,
        expected_state_id: &str,
        offset: usize,
        page_size: usize,
    ) -> Result<ActionPage, RegistryError> {
        self.lookup_service(session_id)?
            .catalog(expected_state_id, offset, page_size)
            .map_err(Into::into)
    }

    /// Apply a canonical action through the existing service.
    pub fn act(
        &self,
        session_id: &str,
        request: ActionRequest,
    ) -> Result<SessionView, RegistryError> {
        self.lookup_service(session_id)?
            .act(request)
            .map_err(Into::into)
    }

    /// Export the latest player-safe save, including after close.
    pub fn save(&self, session_id: &str) -> Result<String, RegistryError> {
        self.lookup_service(session_id)?.save().map_err(Into::into)
    }

    /// Close a retained handle.  Closing never consumes a new record slot and
    /// is idempotent; the original service and creation acknowledgment remain
    /// retained for retries.
    pub fn close(&self, session_id: &str) -> Result<(), RegistryError> {
        validate_session_id(session_id)?;
        let mut state = self.lock_state()?;
        let record = state
            .records
            .iter_mut()
            .find(|record| record.session_id == session_id)
            .ok_or(RegistryError::UnknownSession)?;
        record.service.close()?;
        record.closed = true;
        Ok(())
    }

    fn lock_state(&self) -> Result<MutexGuard<'_, RegistryState>, RegistryError> {
        self.state.lock().map_err(|_| RegistryError::Unavailable)
    }

    fn reserve_creation_slot(&self, state: &mut RegistryState) -> Result<(), RegistryError> {
        if state.records.len() >= self.registry_limits.max_sessions
            || state.records.iter().filter(|record| !record.closed).count()
                >= self.registry_limits.max_active_sessions
        {
            return Err(RegistryError::ResourceLimit);
        }
        state
            .records
            .try_reserve(1)
            .map_err(|_| RegistryError::ResourceLimit)
    }

    fn publish_new_record(
        &self,
        state: &mut RegistryState,
        creation_id: &str,
        creation_fingerprint: String,
        service: SessionService,
    ) -> Result<CreateResponse, RegistryError> {
        let view = service.observe()?;
        let session_id = random_session_id(&state.records)?;
        let response = CreateResponse { session_id, view };
        stage_create_response(&response, self.service_limits.max_response_bytes)?;

        // Clone the complete cached acknowledgment before exposing the record.
        // All fallible work above has finished, and the vector slot was
        // reserved before the service was created.
        let cached_response = response.clone();
        state.records.push(SessionRecord {
            session_id: response.session_id.clone(),
            creation_id: creation_id.to_owned(),
            creation_fingerprint,
            create_response: cached_response,
            service,
            closed: false,
        });
        Ok(response)
    }

    fn lookup_service(&self, session_id: &str) -> Result<SessionService, RegistryError> {
        validate_session_id(session_id)?;
        let state = self.lock_state()?;
        state
            .records
            .iter()
            .find(|record| record.session_id == session_id)
            .map(|record| record.service.clone())
            .ok_or(RegistryError::UnknownSession)
    }
}

fn validate_creation_id(value: &str) -> Result<(), RegistryError> {
    if (1..=128).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
    {
        Ok(())
    } else {
        Err(RegistryError::InvalidInput)
    }
}

fn validate_session_id(value: &str) -> Result<(), RegistryError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(RegistryError::InvalidInput)
    }
}

fn fingerprint_start(start: &StartRequest) -> Result<String, RegistryError> {
    sha256_json(&CreationFingerprint::Start { request: start }).map_err(|_| RegistryError::Internal)
}

fn fingerprint_resume(save_json: &str) -> Result<String, RegistryError> {
    sha256_json(&CreationFingerprint::Resume { save_json }).map_err(|_| RegistryError::Internal)
}

fn random_session_id(records: &[SessionRecord]) -> Result<String, RegistryError> {
    for _ in 0..MAX_RANDOM_ID_ATTEMPTS {
        let mut bytes = [0_u8; 32];
        fill_random(&mut bytes).map_err(|_| RegistryError::Unavailable)?;
        let mut session_id = String::with_capacity(64);
        for byte in bytes {
            write!(&mut session_id, "{byte:02x}").map_err(|_| RegistryError::Internal)?;
        }
        if !records.iter().any(|record| record.session_id == session_id) {
            return Ok(session_id);
        }
    }
    Err(RegistryError::Unavailable)
}

fn ensure_start_size(start: &StartRequest) -> Result<(), RegistryError> {
    let bytes = serde_json::to_vec(start).map_err(|_| RegistryError::Internal)?;
    if bytes.len() > MAX_REQUEST_BYTES {
        return Err(RegistryError::ResourceLimit);
    }
    Ok(())
}

fn stage_create_response(
    response: &CreateResponse,
    max_response_bytes: usize,
) -> Result<(), RegistryError> {
    let maximum = max_response_bytes
        .checked_add(CREATE_RESPONSE_OVERHEAD_BYTES)
        .ok_or(RegistryError::ResourceLimit)?;
    let bytes = serde_json::to_vec(response).map_err(|_| RegistryError::Internal)?;
    if bytes.len() > maximum {
        return Err(RegistryError::ResourceLimit);
    }
    Ok(())
}

fn service_error_code(error: &ServiceError) -> &'static str {
    match error {
        ServiceError::InvalidInput => "invalid_input",
        ServiceError::InvalidContent => "invalid_content",
        ServiceError::InvalidSave => "invalid_save",
        ServiceError::InvalidAction => "invalid_action",
        ServiceError::StaleState => "stale_state",
        ServiceError::IdempotencyConflict => "idempotency_conflict",
        ServiceError::SessionClosed => "session_closed",
        ServiceError::ResourceLimit => "resource_limit",
        ServiceError::Unavailable => "unavailable",
        ServiceError::Internal => "internal",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_content::parse_and_compile_production;
    use std::collections::BTreeSet;
    use std::sync::{Arc, Barrier};
    use std::thread;

    const SPLIT_TIDE: &str = include_str!("../../../content/split-tide.json");

    fn content() -> Arc<CompiledContent> {
        Arc::new(
            parse_and_compile_production(SPLIT_TIDE)
                .expect("the production pack must compile for registry tests"),
        )
    }

    fn preset(seed: u64) -> StartRequest {
        StartRequest::Preset {
            character_preset_id: "rook".to_owned(),
            seed,
        }
    }

    fn registry(limits: RegistryLimits) -> SessionRegistry {
        SessionRegistry::new(content(), ServiceLimits::default(), limits)
            .expect("test registry limits are valid")
    }

    #[test]
    fn random_ids_are_exact_lower_hex_and_unique_in_sample() {
        let mut ids = BTreeSet::new();
        for _ in 0..32 {
            let id = random_session_id(&[]).expect("system randomness is available");
            assert_eq!(id.len(), 64);
            assert!(
                id.bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            );
            assert!(ids.insert(id));
        }
    }

    #[test]
    fn creation_retry_is_cached_and_bound_to_request_kind_and_payload() {
        let registry = registry(RegistryLimits::default());
        let first = registry
            .start("create-1", preset(71))
            .expect("first start succeeds");
        assert_eq!(registry.start("create-1", preset(71)), Ok(first.clone()));
        assert_eq!(
            registry.start("create-1", preset(72)),
            Err(RegistryError::IdempotencyConflict)
        );

        let save = registry
            .save(&first.session_id)
            .expect("initial save is available");
        assert_eq!(
            registry.resume("create-1", &save),
            Err(RegistryError::IdempotencyConflict)
        );
        registry.close(&first.session_id).expect("close succeeds");
        assert_eq!(
            registry.start("create-1", preset(71)),
            Ok(first.clone()),
            "the original create acknowledgment remains available after close"
        );
        assert_eq!(
            registry.observe(&first.session_id),
            Err(RegistryError::Service(ServiceError::SessionClosed))
        );
        assert!(registry.save(&first.session_id).is_ok());
    }

    #[test]
    fn active_limit_and_total_lifetime_cap_never_evict_records() {
        let registry = registry(RegistryLimits {
            max_sessions: 2,
            max_active_sessions: 1,
        });
        let first = registry
            .start("first", preset(71))
            .expect("first start succeeds");
        assert_eq!(
            registry.start("blocked", preset(71)),
            Err(RegistryError::ResourceLimit)
        );
        registry
            .close(&first.session_id)
            .expect("close is possible");

        let second = registry
            .start("blocked", preset(71))
            .expect("active-capacity failure did not reserve its creation key");
        registry
            .close(&second.session_id)
            .expect("second close is possible");
        assert_eq!(
            registry.start("third", preset(71)),
            Err(RegistryError::ResourceLimit),
            "closed records consume lifetime capacity and are never evicted"
        );
        registry
            .close(&first.session_id)
            .expect("repeated close never needs a new slot");
        assert_eq!(registry.start("first", preset(71)).unwrap(), first);
        assert_eq!(registry.start("blocked", preset(71)).unwrap(), second);
        assert!(registry.save(&first.session_id).is_ok());
        assert!(registry.save(&second.session_id).is_ok());
    }

    #[test]
    fn failed_creation_does_not_poison_creation_id() {
        let registry = registry(RegistryLimits::default());
        assert!(
            registry
                .start(
                    "recoverable",
                    StartRequest::Preset {
                        character_preset_id: "missing-preset".to_owned(),
                        seed: 71,
                    },
                )
                .is_err()
        );
        let created = registry
            .start("recoverable", preset(71))
            .expect("a failed create must not reserve its key");
        assert_eq!(created.view.revision, 0);

        registry
            .close(&created.session_id)
            .expect("the first session can release active capacity");
        assert_eq!(
            registry.resume("bad-resume", "not-json"),
            Err(RegistryError::Service(ServiceError::InvalidSave)),
            "malformed-save rejection must be reached, not hidden by active capacity"
        );
        assert!(
            registry.start("bad-resume", preset(71)).is_ok(),
            "a failed resume must not reserve its key"
        );
    }

    #[test]
    fn malformed_and_unknown_session_ids_are_distinct_errors() {
        let registry = registry(RegistryLimits::default());
        assert_eq!(
            registry.observe("not-a-session-id"),
            Err(RegistryError::InvalidInput)
        );
        assert_eq!(
            registry.observe(&"0".repeat(64)),
            Err(RegistryError::UnknownSession)
        );
        assert_eq!(
            registry.start("bad id", preset(71)),
            Err(RegistryError::InvalidInput)
        );
        assert_eq!(RegistryError::UnknownSession.code(), "unknown_session");
        assert_eq!(
            RegistryError::Service(ServiceError::SessionClosed).code(),
            "session_closed"
        );
    }

    #[test]
    fn cloned_handles_share_registry_and_concurrent_duplicate_start_is_one_record() {
        let registry = registry(RegistryLimits::default());
        let barrier = Arc::new(Barrier::new(8));
        let mut threads = Vec::new();
        for _ in 0..8 {
            let registry = registry.clone();
            let barrier = Arc::clone(&barrier);
            threads.push(thread::spawn(move || {
                barrier.wait();
                registry.start("concurrent", preset(71))
            }));
        }

        let responses: Vec<_> = threads
            .into_iter()
            .map(|thread| thread.join().expect("registry thread must not panic"))
            .collect();
        let first = responses[0].clone().expect("one create must succeed");
        for response in responses {
            assert_eq!(response.expect("all same retries must succeed"), first);
        }
        let state = registry.state.lock().expect("registry state is healthy");
        assert_eq!(state.records.len(), 1);
    }

    #[test]
    fn getters_delegate_service_and_close_preserves_save_but_blocks_new_actions() {
        let registry = registry(RegistryLimits::default());
        let created = registry
            .start("delegate", preset(71))
            .expect("start succeeds");
        let observed = registry
            .observe(&created.session_id)
            .expect("observe delegates");
        assert_eq!(observed, created.view);
        let page = registry
            .catalog(&created.session_id, &created.view.catalog.state_id, 0, 1)
            .expect("catalog delegates");
        assert_eq!(page.offset, 0);

        let request = ActionRequest {
            command_id: "before-close".into(),
            expected_revision: 0,
            expected_state_id: observed.catalog.state_id.clone(),
            action_id: observed.catalog.actions[0].action_id.clone(),
        };
        let accepted = registry.act(&created.session_id, request.clone()).unwrap();
        let save = registry.save(&created.session_id).unwrap();
        assert_eq!(registry.start("delegate", preset(71)).unwrap(), created);

        registry
            .close(&created.session_id)
            .expect("close delegates");
        registry
            .close(&created.session_id)
            .expect("close is idempotent");
        assert_eq!(registry.save(&created.session_id).unwrap(), save);
        assert_eq!(
            registry.act(&created.session_id, request.clone()).unwrap(),
            accepted
        );
        assert_eq!(registry.start("delegate", preset(71)).unwrap(), created);
        let mut new_command = request;
        new_command.command_id = "after-close".into();
        assert_eq!(
            registry.act(&created.session_id, new_command),
            Err(RegistryError::Service(ServiceError::SessionClosed))
        );
        assert_eq!(
            registry.observe(&created.session_id),
            Err(RegistryError::Service(ServiceError::SessionClosed))
        );
    }

    #[test]
    fn current_is_live_single_session_read_and_rejects_multi_active_shape() {
        let single = registry(RegistryLimits::default());
        assert_eq!(single.current().unwrap(), None);

        let created = single.start("current", preset(71)).expect("start succeeds");
        assert_eq!(single.current().unwrap(), Some(created.clone()));

        let action = created
            .view
            .catalog
            .actions
            .first()
            .expect("preset has an opening action");
        let request = ActionRequest {
            command_id: "current-action".into(),
            expected_revision: 0,
            expected_state_id: created.view.catalog.state_id.clone(),
            action_id: action.action_id.clone(),
        };
        let advanced = single
            .act(&created.session_id, request)
            .expect("opening action succeeds");
        let current = single
            .current()
            .unwrap()
            .expect("active session remains recoverable");
        assert_eq!(current.session_id, created.session_id);
        assert_eq!(current.view, advanced);
        assert_ne!(current.view, created.view);

        single.close(&created.session_id).unwrap();
        assert_eq!(single.current().unwrap(), None);

        let multi_active = registry(RegistryLimits {
            max_sessions: 2,
            max_active_sessions: 2,
        });
        assert_eq!(multi_active.current(), Err(RegistryError::InvalidInput));
    }

    #[test]
    fn bounded_creation_failures_leave_the_registry_and_keys_unpublished() {
        let registry = SessionRegistry::new(
            content(),
            ServiceLimits {
                max_response_bytes: 1,
                ..ServiceLimits::default()
            },
            RegistryLimits::default(),
        )
        .unwrap();
        for seed in [71, 72] {
            assert_eq!(
                registry.start("response-failure", preset(seed)),
                Err(RegistryError::Service(ServiceError::ResourceLimit))
            );
            assert!(registry.state.lock().unwrap().records.is_empty());
        }
        let registry = SessionRegistry::new(
            content(),
            ServiceLimits {
                max_save_bytes: 1024,
                ..ServiceLimits::default()
            },
            RegistryLimits::default(),
        )
        .unwrap();
        assert_eq!(
            registry.resume("bounded", &" ".repeat(1025)),
            Err(RegistryError::ResourceLimit)
        );
        assert!(registry.state.lock().unwrap().records.is_empty());
        let oversized = StartRequest::Preset {
            character_preset_id: "x".repeat(crate::MAX_REQUEST_BYTES),
            seed: 71,
        };
        assert_eq!(
            registry.start("bounded", oversized),
            Err(RegistryError::ResourceLimit)
        );
        assert!(registry.state.lock().unwrap().records.is_empty());
        assert!(registry.start("bounded", preset(71)).is_ok());
    }

    #[test]
    fn concurrent_distinct_creations_publish_one_active_session_and_allow_failed_key_retry() {
        let registry = registry(RegistryLimits::default());
        let barrier = Arc::new(Barrier::new(4));
        let tasks: Vec<_> = (0..4)
            .map(|index| {
                let registry = registry.clone();
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    let key = format!("race-{index}");
                    barrier.wait();
                    let response = registry.start(&key, preset(71));
                    (key, response)
                })
            })
            .collect();
        let results: Vec<_> = tasks.into_iter().map(|task| task.join().unwrap()).collect();
        assert_eq!(
            results.iter().filter(|(_, result)| result.is_ok()).count(),
            1
        );
        assert!(
            results
                .iter()
                .filter(|(_, result)| result.is_err())
                .all(|(_, result)| *result == Err(RegistryError::ResourceLimit))
        );
        let accepted = results
            .iter()
            .find_map(|(_, result)| result.as_ref().ok())
            .unwrap();
        let failed_key = &results
            .iter()
            .find(|(_, result)| result.is_err())
            .unwrap()
            .0;
        assert_eq!(registry.state.lock().unwrap().records.len(), 1);
        registry.close(&accepted.session_id).unwrap();
        assert!(registry.start(failed_key, preset(71)).is_ok());
    }
}
