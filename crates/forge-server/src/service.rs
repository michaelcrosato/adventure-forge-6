use crate::{
    ActionRequest, MAX_REQUEST_BYTES, ServiceError, ServiceLimits, SessionView, StartRequest,
};
use forge_kernel::{
    ActionPage, CompiledContent, ContentContract, KernelError, enumerate_legal_actions, sha256_json,
};
use forge_replay::{PlayerTrace, ReplayError, Session, resume_player_trace};
use serde::Serialize;
use std::sync::{Arc, Mutex, MutexGuard};

/// A transport-independent, player-safe session handle.
///
/// The service deliberately stores a verified player trace instead of a live
/// replay session.  Each mutating request reconstructs that trace against the
/// immutable compiled content, which keeps the ownership boundary simple and
/// makes save/resume semantics identical to ordinary replay.
#[derive(Clone)]
pub struct SessionService {
    content: Arc<CompiledContent>,
    record: Arc<Mutex<Record>>,
    limits: ServiceLimits,
}

struct Record {
    player_trace: PlayerTrace,
    save_json: String,
    view: SessionView,
    idempotency: Vec<LedgerEntry>,
    idempotency_bytes: usize,
    closed: bool,
}

struct LedgerEntry {
    command_id: String,
    request_fingerprint: String,
    view: SessionView,
    serialized_bytes: usize,
}

#[derive(Serialize)]
struct LedgerEntryWire<'a> {
    command_id: &'a str,
    request_fingerprint: &'a str,
    view: &'a SessionView,
}

impl SessionService {
    /// Start a new production session from an authored preset or canonical
    /// character-creation selection.
    pub fn start(
        content: Arc<CompiledContent>,
        start: StartRequest,
        limits: ServiceLimits,
    ) -> Result<Self, ServiceError> {
        require_production(&content)?;
        validate_limits(&limits)?;
        ensure_serialized_request(&start)?;

        let session_content = Arc::clone(&content);
        let session = match &start {
            StartRequest::Preset {
                character_preset_id,
                seed,
            } => Session::new_game(character_preset_id, *seed, &session_content),
            StartRequest::Custom { selection, seed } => {
                Session::new_custom_game(selection, *seed, &session_content)
            }
        }
        .map_err(map_start_error)?;

        Self::from_session(content, session, limits)
    }

    /// Resume a verified player-safe save against the exact compiled build.
    pub fn resume(
        content: Arc<CompiledContent>,
        save_json: &str,
        limits: ServiceLimits,
    ) -> Result<Self, ServiceError> {
        require_production(&content)?;
        validate_limits(&limits)?;
        if save_json.len() > limits.max_save_bytes {
            return Err(ServiceError::ResourceLimit);
        }

        let player_trace = PlayerTrace::from_json(save_json).map_err(map_save_error)?;
        let session_content = Arc::clone(&content);
        let session =
            resume_player_trace(&player_trace, &session_content).map_err(map_save_error)?;
        Self::from_session(content, session, limits)
    }

    /// Return the cached player view.  It contains the complete first catalog
    /// page produced when the current trace endpoint was accepted.
    pub fn observe(&self) -> Result<SessionView, ServiceError> {
        let record = self.lock_record()?;
        if record.closed {
            return Err(ServiceError::SessionClosed);
        }
        Ok(record.view.clone())
    }

    /// Return one bounded page of the complete current kernel catalog.
    pub fn catalog(
        &self,
        expected_state_id: &str,
        offset: usize,
        page_size: usize,
    ) -> Result<ActionPage, ServiceError> {
        let record = self.lock_record()?;
        if record.closed {
            return Err(ServiceError::SessionClosed);
        }
        validate_state_id(expected_state_id)?;
        if page_size == 0 {
            return Err(ServiceError::InvalidInput);
        }
        if page_size > self.limits.max_page_size {
            return Err(ServiceError::ResourceLimit);
        }
        if expected_state_id != record.view.catalog.state_id {
            return Err(ServiceError::StaleState);
        }
        if offset > record.view.catalog.total {
            return Err(ServiceError::InvalidInput);
        }

        let session = self.resume_current(&record)?;
        let page = self
            .content
            .action_page(session.state(), offset, page_size)
            .map_err(|_| ServiceError::Internal)?;
        ensure_response_size(&page, self.limits.max_response_bytes)?;
        Ok(page)
    }

    /// Apply one opaque canonical action with serialized idempotency.
    pub fn act(&self, request: ActionRequest) -> Result<SessionView, ServiceError> {
        validate_action_request_shape(&request)?;
        let request_fingerprint = fingerprint(&request)?;
        let mut record = self.lock_record()?;

        // Idempotency is intentionally checked before closed/stale checks: a
        // retry must receive the exact historical response it originally got.
        if let Some(entry) = record
            .idempotency
            .iter()
            .find(|entry| entry.command_id == request.command_id)
        {
            if entry.request_fingerprint == request_fingerprint {
                return Ok(entry.view.clone());
            }
            return Err(ServiceError::IdempotencyConflict);
        }

        if record.closed {
            return Err(ServiceError::SessionClosed);
        }
        let current_revision = record.player_trace.action_count();
        let current_revision =
            u64::try_from(current_revision).map_err(|_| ServiceError::ResourceLimit)?;
        if request.expected_revision != current_revision
            || request.expected_state_id != record.view.catalog.state_id
        {
            return Err(ServiceError::StaleState);
        }

        let mut session = self.resume_current(&record)?;
        let legal = enumerate_legal_actions(session.state(), &self.content)
            .map_err(|_| ServiceError::Internal)?;
        let action = legal
            .into_iter()
            .find(|candidate| candidate.action_id == request.action_id)
            .ok_or(ServiceError::InvalidAction)?;
        session.record(&action).map_err(map_action_error)?;

        let next_player_trace = session.player_trace().map_err(map_action_error)?;
        let next_save_json = next_player_trace.to_json().map_err(map_action_error)?;
        if next_save_json.len() > self.limits.max_save_bytes {
            return Err(ServiceError::ResourceLimit);
        }
        let next_view = view_for_session(
            &self.content,
            &session,
            self.limits.default_page_size,
            self.limits.max_response_bytes,
        )?;
        let ledger_bytes =
            ledger_entry_size(&request.command_id, &request_fingerprint, &next_view)?;
        let recorded_idempotency_bytes =
            record.idempotency.iter().try_fold(0usize, |total, entry| {
                total
                    .checked_add(entry.serialized_bytes)
                    .ok_or(ServiceError::ResourceLimit)
            })?;
        if recorded_idempotency_bytes != record.idempotency_bytes {
            return Err(ServiceError::Internal);
        }
        let new_idempotency_bytes = recorded_idempotency_bytes
            .checked_add(ledger_bytes)
            .ok_or(ServiceError::ResourceLimit)?;
        if new_idempotency_bytes > self.limits.max_idempotency_bytes {
            return Err(ServiceError::ResourceLimit);
        }
        record
            .idempotency
            .try_reserve(1)
            .map_err(|_| ServiceError::ResourceLimit)?;

        // All fallible work is complete.  The remaining assignments and push
        // cannot reject, so one accepted request advances every record field.
        let returned_view = next_view.clone();
        let stored_view = returned_view.clone();
        let ledger_entry = LedgerEntry {
            command_id: request.command_id,
            request_fingerprint,
            view: next_view,
            serialized_bytes: ledger_bytes,
        };
        record.player_trace = next_player_trace;
        record.save_json = next_save_json;
        record.view = stored_view;
        record.idempotency_bytes = new_idempotency_bytes;
        record.idempotency.push(ledger_entry);
        Ok(returned_view)
    }

    /// Return the latest verified player-safe save.  Saving remains available
    /// after transport closure so callers can persist a final checkpoint.
    pub fn save(&self) -> Result<String, ServiceError> {
        let record = self.lock_record()?;
        Ok(record.save_json.clone())
    }

    /// Close the transport handle.  This is idempotent and does not alter the
    /// authoritative trace, cached view, or idempotency history.
    pub fn close(&self) -> Result<(), ServiceError> {
        let mut record = self.lock_record()?;
        record.closed = true;
        Ok(())
    }

    fn from_session(
        content: Arc<CompiledContent>,
        session: Session<'_>,
        limits: ServiceLimits,
    ) -> Result<Self, ServiceError> {
        let player_trace = session.player_trace().map_err(map_start_error)?;
        let save_json = player_trace.to_json().map_err(map_start_error)?;
        if save_json.len() > limits.max_save_bytes {
            return Err(ServiceError::ResourceLimit);
        }
        let view = view_for_session(
            &content,
            &session,
            limits.default_page_size,
            limits.max_response_bytes,
        )?;
        Ok(Self {
            content,
            record: Arc::new(Mutex::new(Record {
                player_trace,
                save_json,
                view,
                idempotency: Vec::new(),
                idempotency_bytes: 0,
                closed: false,
            })),
            limits,
        })
    }

    fn lock_record(&self) -> Result<MutexGuard<'_, Record>, ServiceError> {
        self.record.lock().map_err(|_| ServiceError::Unavailable)
    }

    fn resume_current<'a>(&'a self, record: &'a Record) -> Result<Session<'a>, ServiceError> {
        resume_player_trace(&record.player_trace, &self.content).map_err(map_internal_error)
    }
}

fn require_production(content: &CompiledContent) -> Result<(), ServiceError> {
    if content.contract() == ContentContract::Production {
        Ok(())
    } else {
        Err(ServiceError::InvalidContent)
    }
}

fn validate_limits(limits: &ServiceLimits) -> Result<(), ServiceError> {
    if limits.max_save_bytes == 0
        || limits.max_response_bytes == 0
        || limits.max_idempotency_bytes == 0
        || limits.default_page_size == 0
        || limits.max_page_size == 0
        || limits.default_page_size > limits.max_page_size
    {
        return Err(ServiceError::InvalidInput);
    }
    Ok(())
}

fn ensure_serialized_request<T: Serialize>(request: &T) -> Result<(), ServiceError> {
    let bytes = serde_json::to_vec(request).map_err(|_| ServiceError::Internal)?;
    if bytes.len() > MAX_REQUEST_BYTES {
        return Err(ServiceError::ResourceLimit);
    }
    Ok(())
}

fn ensure_response_size<T: Serialize>(value: &T, max_bytes: usize) -> Result<(), ServiceError> {
    let bytes = serde_json::to_vec(value).map_err(|_| ServiceError::Internal)?;
    if bytes.len() > max_bytes {
        return Err(ServiceError::ResourceLimit);
    }
    Ok(())
}

fn view_for_session(
    content: &CompiledContent,
    session: &Session<'_>,
    page_size: usize,
    max_response_bytes: usize,
) -> Result<SessionView, ServiceError> {
    let trace = session.trace();
    let revision = u64::try_from(trace.steps.len()).map_err(|_| ServiceError::ResourceLimit)?;
    let observation = trace
        .steps
        .last()
        .map(|step| step.observation.clone())
        .unwrap_or_else(|| trace.initial_observation.clone());
    let catalog = content
        .action_page(session.state(), 0, page_size)
        .map_err(|_| ServiceError::Internal)?;
    let view = SessionView {
        revision,
        observation,
        catalog,
    };
    ensure_response_size(&view, max_response_bytes)?;
    Ok(view)
}

fn validate_command_id(command_id: &str) -> Result<(), ServiceError> {
    if command_id.is_empty()
        || command_id.len() > 128
        || !command_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
    {
        return Err(ServiceError::InvalidInput);
    }
    Ok(())
}

fn validate_lower_hex_id(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_state_id(value: &str) -> Result<(), ServiceError> {
    if validate_lower_hex_id(value) {
        Ok(())
    } else {
        Err(ServiceError::InvalidInput)
    }
}

fn validate_action_request_shape(request: &ActionRequest) -> Result<(), ServiceError> {
    validate_command_id(&request.command_id)?;
    validate_state_id(&request.expected_state_id)?;
    if !validate_lower_hex_id(&request.action_id) {
        return Err(ServiceError::InvalidInput);
    }
    Ok(())
}

fn fingerprint(request: &ActionRequest) -> Result<String, ServiceError> {
    sha256_json(request).map_err(|_| ServiceError::Internal)
}

fn ledger_entry_size(
    command_id: &str,
    request_fingerprint: &str,
    view: &SessionView,
) -> Result<usize, ServiceError> {
    let wire = LedgerEntryWire {
        command_id,
        request_fingerprint,
        view,
    };
    serde_json::to_vec(&wire)
        .map(|bytes| bytes.len())
        .map_err(|_| ServiceError::Internal)
}

fn map_start_error(error: ReplayError) -> ServiceError {
    match error {
        ReplayError::ResourceExhausted(_) => ServiceError::ResourceLimit,
        ReplayError::Kernel(KernelError::InvalidContent(_)) => ServiceError::InvalidContent,
        ReplayError::InvalidTrace(_) => ServiceError::InvalidInput,
        ReplayError::Kernel(_)
        | ReplayError::Hash(_)
        | ReplayError::Json(_)
        | ReplayError::Mismatch { .. } => ServiceError::Internal,
    }
}

fn map_save_error(error: ReplayError) -> ServiceError {
    match error {
        ReplayError::ResourceExhausted(_) => ServiceError::ResourceLimit,
        _ => ServiceError::InvalidSave,
    }
}

fn map_internal_error(error: ReplayError) -> ServiceError {
    match error {
        ReplayError::ResourceExhausted(_) => ServiceError::ResourceLimit,
        _ => ServiceError::Internal,
    }
}

fn map_action_error(error: ReplayError) -> ServiceError {
    match error {
        ReplayError::ResourceExhausted(_) => ServiceError::ResourceLimit,
        ReplayError::Kernel(
            KernelError::UnknownAction(_)
            | KernelError::IllegalAction(_)
            | KernelError::InvalidAction(_)
            | KernelError::StaleAction { .. },
        ) => ServiceError::InvalidAction,
        _ => ServiceError::Internal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_content::parse_and_compile_production;
    use forge_kernel::ActionView;
    use serde_json::to_vec;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    const SPLIT_TIDE: &str = include_str!("../../../content/split-tide.json");

    fn content() -> Arc<CompiledContent> {
        Arc::new(
            parse_and_compile_production(SPLIT_TIDE)
                .expect("the production pack must compile for service tests"),
        )
    }

    fn preset() -> StartRequest {
        StartRequest::Preset {
            character_preset_id: "rook".to_owned(),
            seed: 71,
        }
    }

    fn action(view: &SessionView, definition_id: &str) -> ActionView {
        view.catalog
            .actions
            .iter()
            .find(|candidate| candidate.definition_id == definition_id)
            .cloned()
            .unwrap_or_else(|| panic!("missing test action {definition_id}"))
    }

    fn request(view: &SessionView, command_id: &str, action: &ActionView) -> ActionRequest {
        ActionRequest {
            command_id: command_id.to_owned(),
            expected_revision: view.revision,
            expected_state_id: view.observation.state_id.clone(),
            action_id: action.action_id.clone(),
        }
    }

    fn candidate_view(service: &SessionService, request: &ActionRequest) -> SessionView {
        let record = service.record.lock().expect("probe record is not poisoned");
        let mut session = service
            .resume_current(&record)
            .expect("probe trace must resume");
        let action = enumerate_legal_actions(session.state(), &service.content)
            .expect("probe state must enumerate")
            .into_iter()
            .find(|candidate| candidate.action_id == request.action_id)
            .expect("probe action must be legal");
        session.record(&action).expect("probe action must record");
        view_for_session(
            &service.content,
            &session,
            service.limits.default_page_size,
            service.limits.max_response_bytes,
        )
        .expect("probe view must render")
    }

    #[test]
    fn exact_first_ledger_quota_keeps_failed_candidate_atomic_and_retryable() {
        let probe = SessionService::start(content(), preset(), ServiceLimits::default())
            .expect("probe service starts");
        let initial = probe.observe().expect("probe observes");
        let first_action = action(&initial, "wait_tide");
        let first_request = request(&initial, "exact-first", &first_action);
        let next_view = candidate_view(&probe, &first_request);
        let request_fingerprint = fingerprint(&first_request).expect("request hashes");
        let exact_quota =
            ledger_entry_size(&first_request.command_id, &request_fingerprint, &next_view)
                .expect("ledger probe serializes");

        let limits = ServiceLimits {
            max_idempotency_bytes: exact_quota,
            ..ServiceLimits::default()
        };
        let service = SessionService::start(content(), preset(), limits)
            .expect("exact first-entry quota permits start");
        let accepted = service
            .act(first_request.clone())
            .expect("first entry should fit exactly");
        assert_eq!(accepted, next_view);

        let before_failed = service.observe().expect("post-accept observation");
        let before_failed_save = service.save().expect("post-accept save");
        let second_action = action(&before_failed, "wait_tide");
        let second_request = request(&before_failed, "over-quota", &second_action);
        assert_eq!(
            service.act(second_request.clone()),
            Err(ServiceError::ResourceLimit)
        );
        assert_eq!(
            service.observe().expect("failed candidate is inert"),
            before_failed
        );
        assert_eq!(
            service.save().expect("failed save is inert"),
            before_failed_save
        );

        let retry = service
            .act(first_request)
            .expect("the accepted command remains idempotent");
        assert_eq!(
            to_vec(&retry).expect("retry serializes"),
            to_vec(&accepted).unwrap()
        );

        let unpoisoned_retry = ActionRequest {
            action_id: action(&before_failed, "checkpoint.ask_sava").action_id,
            ..second_request
        };
        assert_eq!(
            service.act(unpoisoned_retry),
            Err(ServiceError::ResourceLimit),
            "the failed command must not reserve an idempotency key"
        );
    }

    #[test]
    fn poisoned_record_fails_closed_for_every_session_operation() {
        let service = SessionService::start(content(), preset(), ServiceLimits::default())
            .expect("service starts");
        let initial = service.observe().expect("initial observation");
        let action = action(&initial, "wait_tide");
        let action_request = request(&initial, "poisoned-command", &action);
        let state_id = initial.observation.state_id.clone();

        let panic_result = catch_unwind(AssertUnwindSafe(|| {
            let _guard = service.record.lock().expect("record is initially healthy");
            panic!("deliberately poison the private test mutex");
        }));
        assert!(panic_result.is_err());

        assert_eq!(service.observe(), Err(ServiceError::Unavailable));
        assert_eq!(
            service.catalog(&state_id, 0, 1),
            Err(ServiceError::Unavailable)
        );
        assert_eq!(service.save(), Err(ServiceError::Unavailable));
        assert_eq!(service.close(), Err(ServiceError::Unavailable));
        assert_eq!(service.act(action_request), Err(ServiceError::Unavailable));
    }
}
