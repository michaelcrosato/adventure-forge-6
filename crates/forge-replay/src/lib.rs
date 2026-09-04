//! Deterministic session recording and replay.
//!
//! This crate is deliberately an in-memory boundary.  It does not read files,
//! consult a clock, use ambient randomness, or contact a service.  Callers
//! provide a validated kernel state and the exact compiled content.  Every
//! recorded action is passed through `forge-kernel`; the trace is only a
//! serialized witness of that execution.

use forge_kernel::{
    CanonicalAction, CharacterSelection, CharacterStart, CompiledContent, ContentContract,
    EntropyDraw, EntropyState, Event, GameState, HashError, KernelError, Observation,
    enumerate_legal_actions, legal_action_digest, sha256_json, step, validate_unique_json_keys,
};
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display, Formatter};

/// Version the serialized trace format and receipt domain.
pub const TRACE_FORMAT_VERSION: &str = "forge-replay-v2";

/// Version for the player-safe, portable replay record. Unlike [`Trace`], this
/// format deliberately omits authoritative state, events, entropy, and
/// observations. A trusted process reconstructs those claims from the start
/// specification and the chosen canonical action identities.
pub const PLAYER_TRACE_FORMAT_VERSION: &str = "forge-player-trace-v2";

/// Declares how the trace's genesis state was obtained. Production evidence
/// must name an authored preset or canonical custom recipe and seed so a
/// verifier can reconstruct that state without a caller-supplied sheet.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TraceStart {
    CharacterPreset {
        character_preset_id: String,
        seed: u64,
    },
    CharacterCreation {
        selection: CharacterSelection,
        seed: u64,
    },
    FixtureState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplayError {
    Kernel(KernelError),
    Hash(HashError),
    Json(String),
    InvalidTrace(String),
    ResourceExhausted(String),
    Mismatch {
        path: String,
        expected: String,
        actual: String,
    },
}

impl Display for ReplayError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Kernel(error) => Display::fmt(error, f),
            Self::Hash(error) => Display::fmt(error, f),
            Self::Json(error) => write!(f, "trace JSON error: {error}"),
            Self::InvalidTrace(error) => write!(f, "invalid trace: {error}"),
            Self::ResourceExhausted(error) => write!(f, "trace resource exhausted: {error}"),
            Self::Mismatch {
                path,
                expected,
                actual,
            } => write!(
                f,
                "trace mismatch at {path}: expected {expected}, got {actual}"
            ),
        }
    }
}

impl std::error::Error for ReplayError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Kernel(error) => Some(error),
            Self::Hash(error) => Some(error),
            Self::Json(_)
            | Self::InvalidTrace(_)
            | Self::ResourceExhausted(_)
            | Self::Mismatch { .. } => None,
        }
    }
}

impl From<KernelError> for ReplayError {
    fn from(value: KernelError) -> Self {
        Self::Kernel(value)
    }
}

impl From<HashError> for ReplayError {
    fn from(value: HashError) -> Self {
        Self::Hash(value)
    }
}

impl From<serde_json::Error> for ReplayError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value.to_string())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TraceStep {
    pub prior_receipt: String,
    pub build_id: String,
    pub step_index: u64,
    pub action: CanonicalAction,
    pub legal_action_set_digest: String,
    pub pre_state_id: String,
    pub events: Vec<Event>,
    pub events_hash: String,
    pub entropy_before: EntropyState,
    pub entropy_draws: Vec<EntropyDraw>,
    pub entropy_after: EntropyState,
    pub post_state_id: String,
    pub observation: Observation,
    pub observation_hash: String,
    pub receipt: String,
}

#[derive(Serialize)]
struct StepReceiptInput<'a> {
    trace_format_version: &'static str,
    prior_receipt: &'a str,
    build_id: &'a str,
    step_index: u64,
    action: &'a CanonicalAction,
    legal_action_set_digest: &'a str,
    pre_state_id: &'a str,
    events: &'a [Event],
    events_hash: &'a str,
    entropy_before: &'a EntropyState,
    entropy_draws: &'a [EntropyDraw],
    entropy_after: &'a EntropyState,
    post_state_id: &'a str,
    observation: &'a Observation,
    observation_hash: &'a str,
}

impl TraceStep {
    fn receipt_input(&self) -> StepReceiptInput<'_> {
        StepReceiptInput {
            trace_format_version: TRACE_FORMAT_VERSION,
            prior_receipt: &self.prior_receipt,
            build_id: &self.build_id,
            step_index: self.step_index,
            action: &self.action,
            legal_action_set_digest: &self.legal_action_set_digest,
            pre_state_id: &self.pre_state_id,
            events: &self.events,
            events_hash: &self.events_hash,
            entropy_before: &self.entropy_before,
            entropy_draws: &self.entropy_draws,
            entropy_after: &self.entropy_after,
            post_state_id: &self.post_state_id,
            observation: &self.observation,
            observation_hash: &self.observation_hash,
        }
    }

    /// Recompute the collision-resistant receipt over every step field except
    /// the receipt itself.
    pub fn recomputed_receipt(&self) -> Result<String, HashError> {
        sha256_json(&self.receipt_input())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Trace {
    pub format_version: String,
    pub build_id: String,
    pub start: TraceStart,
    pub initial_state: GameState,
    pub initial_state_id: String,
    pub initial_observation: Observation,
    pub initial_observation_hash: String,
    pub initial_receipt: String,
    pub steps: Vec<TraceStep>,
    pub final_state_id: String,
    pub final_receipt: String,
}

/// A player-safe save and replay record.
///
/// The detailed [`Trace`] remains an internal verification witness because it
/// contains hidden state and events. This portable form contains only public
/// start inputs, player-selected opaque action identities, and opaque final
/// commitments. Loading it always reconstructs every step through the kernel.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlayerTrace {
    format_version: String,
    build_id: String,
    start: TraceStart,
    action_ids: Vec<String>,
    final_state_id: String,
    final_receipt: String,
}

impl PlayerTrace {
    /// Encode a player-safe trace for a caller-managed save boundary.
    pub fn to_json(&self) -> Result<String, ReplayError> {
        serde_json::to_string(self).map_err(ReplayError::from)
    }

    /// Decode without accepting the record. [`resume_player_trace`] must
    /// reconstruct it against the exact compiled content before use.
    pub fn from_json(input: &str) -> Result<Self, ReplayError> {
        validate_unique_json_keys(input).map_err(|error| ReplayError::Json(error.to_string()))?;
        serde_json::from_str(input).map_err(ReplayError::from)
    }

    pub fn action_count(&self) -> usize {
        self.action_ids.len()
    }
}

#[derive(Serialize)]
struct InitialReceiptInput<'a> {
    trace_format_version: &'static str,
    build_id: &'a str,
    start: &'a TraceStart,
    initial_state_id: &'a str,
    initial_state: &'a GameState,
    initial_observation: &'a Observation,
    initial_observation_hash: &'a str,
}

impl Trace {
    fn initial_receipt_input(&self) -> InitialReceiptInput<'_> {
        InitialReceiptInput {
            trace_format_version: TRACE_FORMAT_VERSION,
            build_id: &self.build_id,
            start: &self.start,
            initial_state_id: &self.initial_state_id,
            initial_state: &self.initial_state,
            initial_observation: &self.initial_observation,
            initial_observation_hash: &self.initial_observation_hash,
        }
    }

    /// Recompute the genesis receipt over the full initial state and its ID.
    pub fn recomputed_initial_receipt(&self) -> Result<String, HashError> {
        sha256_json(&self.initial_receipt_input())
    }

    /// Encode a trace for a caller-managed save or transport boundary.
    pub fn to_json(&self) -> Result<String, ReplayError> {
        serde_json::to_string(self).map_err(ReplayError::from)
    }

    /// Decode a trace without accepting it as valid.  `verify` or `resume`
    /// must still be called with the exact compiled content.
    pub fn from_json(input: &str) -> Result<Self, ReplayError> {
        validate_unique_json_keys(input).map_err(|error| ReplayError::Json(error.to_string()))?;
        serde_json::from_str(input).map_err(ReplayError::from)
    }
}

/// A live in-memory session.  The content reference is immutable and every
/// state change is delegated to the kernel reducer.
pub struct Session<'content> {
    content: &'content CompiledContent,
    state: GameState,
    trace: Trace,
}

impl<'content> Session<'content> {
    /// Start recording from a state that is valid for `content`.
    pub fn new(
        initial_state: GameState,
        content: &'content CompiledContent,
    ) -> Result<Self, ReplayError> {
        validate_initial_state(&initial_state, content)?;
        let start = identify_start(&initial_state, content)?;
        let trace = new_trace(&initial_state, start, content)?;
        Ok(Self {
            content,
            state: initial_state,
            trace,
        })
    }

    /// Start a production-ready session from an authored character preset and
    /// explicit seed. The verifier reconstructs this genesis independently.
    pub fn new_game(
        character_preset_id: &str,
        seed: u64,
        content: &'content CompiledContent,
    ) -> Result<Self, ReplayError> {
        let state = content
            .new_game(character_preset_id, seed)
            .map_err(|error| ReplayError::InvalidTrace(error.to_string()))?;
        Self::new(state, content)
    }

    /// Start a production-ready session from a canonicalizable public
    /// selection. The kernel owns all derived character fields.
    pub fn new_custom_game(
        selection: &CharacterSelection,
        seed: u64,
        content: &'content CompiledContent,
    ) -> Result<Self, ReplayError> {
        let state = content
            .new_custom_game(selection, seed)
            .map_err(|error| ReplayError::InvalidTrace(error.to_string()))?;
        Self::new(state, content)
    }

    pub fn state(&self) -> &GameState {
        &self.state
    }

    pub fn trace(&self) -> &Trace {
        &self.trace
    }

    pub fn into_trace(self) -> Trace {
        self.trace
    }

    /// Produce a portable save without exposing hidden state or events.
    pub fn player_trace(&self) -> Result<PlayerTrace, ReplayError> {
        match &self.trace.start {
            TraceStart::CharacterPreset { .. } | TraceStart::CharacterCreation { .. } => {}
            TraceStart::FixtureState => {
                return Err(ReplayError::InvalidTrace(
                    "only authored character starts can become player traces".to_owned(),
                ));
            }
        };
        let mut action_ids = Vec::new();
        action_ids
            .try_reserve(self.trace.steps.len())
            .map_err(|error| ReplayError::ResourceExhausted(error.to_string()))?;
        action_ids.extend(
            self.trace
                .steps
                .iter()
                .map(|step| step.action.action_id.clone()),
        );
        Ok(PlayerTrace {
            format_version: PLAYER_TRACE_FORMAT_VERSION.to_owned(),
            build_id: self.trace.build_id.clone(),
            start: self.trace.start.clone(),
            action_ids,
            final_state_id: self.trace.final_state_id.clone(),
            final_receipt: self.trace.final_receipt.clone(),
        })
    }

    /// Apply one caller-selected canonical action and append its witness.
    /// Invalid, stale, or wrong-build actions leave both state and trace
    /// contents unchanged.
    pub fn record(&mut self, action: &CanonicalAction) -> Result<TraceStep, ReplayError> {
        let legal = enumerate_legal_actions(&self.state, self.content)?;
        let legal_digest = legal_action_digest(&legal)?;
        let step_index = u64::try_from(self.trace.steps.len()).map_err(|_| {
            ReplayError::ResourceExhausted("trace step index exceeds u64".to_owned())
        })?;

        let prior_receipt = self
            .trace
            .steps
            .last()
            .map_or(self.trace.initial_receipt.as_str(), |previous| {
                previous.receipt.as_str()
            })
            .to_owned();

        let transition = step(&self.state, action, self.content, &self.state.entropy)?;
        let trace_step = trace_step_from_transition(
            step_index,
            &prior_receipt,
            &self.trace,
            &legal_digest,
            &transition,
            self.content,
        )?;

        // Reserve only after every validation, reduction, presentation, and
        // hashing step succeeds, preserving even observable vector capacity
        // when an action is rejected.
        self.trace
            .steps
            .try_reserve(1)
            .map_err(|error| ReplayError::ResourceExhausted(error.to_string()))?;

        self.state = transition.into_state();
        self.trace.steps.push(trace_step.clone());
        self.trace.final_state_id = self.state.state_id();
        self.trace.final_receipt = trace_step.receipt.clone();
        Ok(trace_step)
    }

    /// Alias emphasizing that the input is an action identity, not free text.
    pub fn record_action(&mut self, action: &CanonicalAction) -> Result<TraceStep, ReplayError> {
        self.record(action)
    }

    /// Resume a verified trace and keep recording against the same content.
    pub fn resume(trace: &Trace, content: &'content CompiledContent) -> Result<Self, ReplayError> {
        resume(trace, content)
    }
}

/// Compute the canonical hash of the ordered event vector emitted by a step.
pub fn canonical_events_hash(events: &[Event]) -> Result<String, HashError> {
    sha256_json(&events)
}

/// Verify every trace claim by replaying from its full initial state.
///
/// The returned state is the reconstructed final state.  A successful result
/// means the trace's build, receipt chain, actions, legal-set digests, events,
/// entropy, state IDs, and final claims all agree with the supplied content.
pub fn verify(trace: &Trace, content: &CompiledContent) -> Result<GameState, ReplayError> {
    if trace.format_version != TRACE_FORMAT_VERSION {
        return Err(ReplayError::InvalidTrace(format!(
            "unsupported format version {}",
            trace.format_version
        )));
    }
    if !content.has_valid_build_id() {
        return Err(ReplayError::InvalidTrace(
            "compiled content build identity is invalid".to_owned(),
        ));
    }
    check_equal("build_id", content.build_id(), trace.build_id.as_str())?;
    validate_initial_state(&trace.initial_state, content)?;
    validate_start(trace, content)?;

    let actual_initial_state_id = trace.initial_state.state_id();
    check_equal(
        "initial_state_id",
        &actual_initial_state_id,
        &trace.initial_state_id,
    )?;
    let actual_initial_observation = content
        .observe(&trace.initial_state)
        .map_err(|error| ReplayError::InvalidTrace(error.to_string()))?;
    check_equal(
        "initial_observation",
        &actual_initial_observation,
        &trace.initial_observation,
    )?;
    let actual_initial_observation_hash = sha256_json(&actual_initial_observation)?;
    check_equal(
        "initial_observation_hash",
        &actual_initial_observation_hash,
        &trace.initial_observation_hash,
    )?;
    let actual_initial_receipt = trace.recomputed_initial_receipt()?;
    check_equal(
        "initial_receipt",
        &actual_initial_receipt,
        &trace.initial_receipt,
    )?;

    let mut state = trace.initial_state.clone();
    let mut prior_receipt = trace.initial_receipt.clone();
    for (position, claimed) in trace.steps.iter().enumerate() {
        let expected_index = u64::try_from(position).map_err(|_| {
            ReplayError::ResourceExhausted("trace step index exceeds u64".to_owned())
        })?;
        let path = |field: &str| format!("steps[{position}].{field}");
        check_equal(&path("step_index"), &expected_index, &claimed.step_index)?;
        check_equal(
            &path("prior_receipt"),
            &prior_receipt,
            &claimed.prior_receipt,
        )?;
        check_equal(
            &path("build_id"),
            content.build_id(),
            claimed.build_id.as_str(),
        )?;

        let actual_pre_state_id = state.state_id();
        check_equal(
            &path("pre_state_id"),
            &actual_pre_state_id,
            &claimed.pre_state_id,
        )?;
        check_equal(
            &path("entropy_before"),
            &state.entropy,
            &claimed.entropy_before,
        )?;

        let legal = enumerate_legal_actions(&state, content)?;
        let actual_legal_digest = legal_action_digest(&legal)?;
        check_equal(
            &path("legal_action_set_digest"),
            &actual_legal_digest,
            &claimed.legal_action_set_digest,
        )?;

        let transition = step(&state, &claimed.action, content, &state.entropy)?;
        check_equal(&path("action"), transition.action(), &claimed.action)?;
        check_equal(
            &path("pre_state_id"),
            transition.pre_state_id(),
            &claimed.pre_state_id,
        )?;
        check_equal(&path("events"), transition.events(), &claimed.events)?;
        let actual_events_hash = canonical_events_hash(transition.events())?;
        check_equal(
            &path("events_hash"),
            &actual_events_hash,
            &claimed.events_hash,
        )?;
        check_equal(
            &path("entropy_before"),
            transition.entropy_before(),
            &claimed.entropy_before,
        )?;
        check_equal(
            &path("entropy_draws"),
            transition.entropy_draws(),
            &claimed.entropy_draws,
        )?;
        check_equal(
            &path("entropy_after"),
            transition.entropy_after(),
            &claimed.entropy_after,
        )?;
        check_equal(
            &path("post_state_id"),
            transition.post_state_id(),
            &claimed.post_state_id,
        )?;
        let actual_observation = content
            .observe_after_transition(&transition)
            .map_err(|error| ReplayError::InvalidTrace(error.to_string()))?;
        check_equal(
            &path("observation"),
            &actual_observation,
            &claimed.observation,
        )?;
        let actual_observation_hash = sha256_json(&actual_observation)?;
        check_equal(
            &path("observation_hash"),
            &actual_observation_hash,
            &claimed.observation_hash,
        )?;
        let actual_receipt = claimed.recomputed_receipt()?;
        check_equal(&path("receipt"), &actual_receipt, &claimed.receipt)?;

        state = transition.into_state();
        prior_receipt = claimed.receipt.clone();
    }

    let actual_final_state_id = state.state_id();
    check_equal(
        "final_state_id",
        &actual_final_state_id,
        &trace.final_state_id,
    )?;
    check_equal("final_receipt", &prior_receipt, &trace.final_receipt)?;
    Ok(state)
}

/// Alias for callers that prefer an explicit verb.
pub fn verify_trace(trace: &Trace, content: &CompiledContent) -> Result<GameState, ReplayError> {
    verify(trace, content)
}

/// Verify and reconstruct a trace, returning a session positioned at its end.
pub fn resume<'content>(
    trace: &Trace,
    content: &'content CompiledContent,
) -> Result<Session<'content>, ReplayError> {
    let state = verify(trace, content)?;
    Ok(Session {
        content,
        state,
        trace: trace.clone(),
    })
}

/// Reconstruct a player-safe record through current kernel enumeration.
///
/// No serialized action is trusted: every opaque identity must match one
/// action in the complete legal set at that exact reconstructed state.
pub fn resume_player_trace<'content>(
    player_trace: &PlayerTrace,
    content: &'content CompiledContent,
) -> Result<Session<'content>, ReplayError> {
    if player_trace.format_version != PLAYER_TRACE_FORMAT_VERSION {
        return Err(ReplayError::InvalidTrace(format!(
            "unsupported player trace format version {}",
            player_trace.format_version
        )));
    }
    check_equal(
        "build_id",
        content.build_id(),
        player_trace.build_id.as_str(),
    )?;
    let mut session = match &player_trace.start {
        TraceStart::CharacterPreset {
            character_preset_id,
            seed,
        } => Session::new_game(character_preset_id, *seed, content)?,
        TraceStart::CharacterCreation { selection, seed } => {
            Session::new_custom_game(selection, *seed, content)?
        }
        TraceStart::FixtureState => {
            return Err(ReplayError::InvalidTrace(
                "player trace cannot use an arbitrary fixture genesis".to_owned(),
            ));
        }
    };
    check_equal("start", &session.trace.start, &player_trace.start)?;
    for (position, action_id) in player_trace.action_ids.iter().enumerate() {
        let action = enumerate_legal_actions(session.state(), content)?
            .into_iter()
            .find(|action| action.action_id == *action_id)
            .ok_or_else(|| {
                ReplayError::InvalidTrace(format!(
                    "action_ids[{position}] is not legal in its reconstructed state"
                ))
            })?;
        session.record(&action)?;
    }
    check_equal(
        "final_state_id",
        &session.trace.final_state_id,
        &player_trace.final_state_id,
    )?;
    check_equal(
        "final_receipt",
        &session.trace.final_receipt,
        &player_trace.final_receipt,
    )?;
    Ok(session)
}

fn identify_start(state: &GameState, content: &CompiledContent) -> Result<TraceStart, ReplayError> {
    match &state.character_start {
        CharacterStart::Preset {
            character_preset_id,
        } => {
            let expected = content
                .new_game(character_preset_id, state.entropy.seed)
                .map_err(|error| ReplayError::InvalidTrace(error.to_string()))?;
            check_equal("initial_state", &expected, state)?;
            Ok(TraceStart::CharacterPreset {
                character_preset_id: character_preset_id.clone(),
                seed: state.entropy.seed,
            })
        }
        CharacterStart::Custom { selection } => {
            let expected = content
                .new_custom_game(selection, state.entropy.seed)
                .map_err(|error| ReplayError::InvalidTrace(error.to_string()))?;
            check_equal("initial_state", &expected, state)?;
            Ok(TraceStart::CharacterCreation {
                selection: selection.clone(),
                seed: state.entropy.seed,
            })
        }
        CharacterStart::Fixture if content.contract() == ContentContract::Fixture => {
            Ok(TraceStart::FixtureState)
        }
        CharacterStart::Fixture => Err(ReplayError::InvalidTrace(
            "production session must begin from an authored character start".to_owned(),
        )),
    }
}

fn validate_start(trace: &Trace, content: &CompiledContent) -> Result<(), ReplayError> {
    match &trace.start {
        TraceStart::CharacterPreset {
            character_preset_id,
            seed,
        } => {
            let expected = content
                .new_game(character_preset_id, *seed)
                .map_err(|error| ReplayError::InvalidTrace(error.to_string()))?;
            check_equal("initial_state", &expected, &trace.initial_state)
        }
        TraceStart::CharacterCreation { selection, seed } => {
            let canonical = content
                .canonical_character_selection(selection)
                .map_err(|error| ReplayError::InvalidTrace(error.to_string()))?;
            check_equal("start.selection", &canonical, selection)?;
            let expected = content
                .new_custom_game(selection, *seed)
                .map_err(|error| ReplayError::InvalidTrace(error.to_string()))?;
            check_equal("initial_state", &expected, &trace.initial_state)
        }
        TraceStart::FixtureState if content.contract() == ContentContract::Fixture => Ok(()),
        TraceStart::FixtureState => Err(ReplayError::InvalidTrace(
            "production trace cannot use an arbitrary fixture genesis".to_owned(),
        )),
    }
}

fn validate_initial_state(state: &GameState, content: &CompiledContent) -> Result<(), ReplayError> {
    if !content.has_valid_build_id() {
        return Err(ReplayError::InvalidTrace(
            "compiled content build identity is invalid".to_owned(),
        ));
    }
    if state.build_id != content.build_id() {
        return Err(KernelError::WrongBuild {
            expected: content.build_id().to_owned(),
            actual: state.build_id.clone(),
        }
        .into());
    }
    state.entropy.validate().map_err(KernelError::from)?;
    content
        .validate_state(state)
        .map_err(|error| KernelError::InvalidState(error.to_string()))?;
    Ok(())
}

fn new_trace(
    initial_state: &GameState,
    start: TraceStart,
    content: &CompiledContent,
) -> Result<Trace, ReplayError> {
    let initial_state_id = initial_state.state_id();
    let initial_observation = content
        .observe(initial_state)
        .map_err(|error| ReplayError::InvalidTrace(error.to_string()))?;
    let initial_observation_hash = sha256_json(&initial_observation)?;
    let mut trace = Trace {
        format_version: TRACE_FORMAT_VERSION.to_owned(),
        build_id: content.build_id().to_owned(),
        start,
        initial_state: initial_state.clone(),
        initial_state_id,
        initial_observation,
        initial_observation_hash,
        initial_receipt: String::new(),
        steps: Vec::new(),
        final_state_id: String::new(),
        final_receipt: String::new(),
    };
    trace.initial_receipt = trace.recomputed_initial_receipt()?;
    trace.final_state_id = trace.initial_state_id.clone();
    trace.final_receipt = trace.initial_receipt.clone();
    Ok(trace)
}

fn trace_step_from_transition(
    step_index: u64,
    prior_receipt: &str,
    trace: &Trace,
    legal_digest: &str,
    transition: &forge_kernel::Transition,
    content: &CompiledContent,
) -> Result<TraceStep, ReplayError> {
    let events_hash = canonical_events_hash(transition.events())?;
    let observation = content
        .observe_after_transition(transition)
        .map_err(|error| ReplayError::InvalidTrace(error.to_string()))?;
    let observation_hash = sha256_json(&observation)?;
    let mut trace_step = TraceStep {
        prior_receipt: prior_receipt.to_owned(),
        build_id: trace.build_id.clone(),
        step_index,
        action: transition.action().clone(),
        legal_action_set_digest: legal_digest.to_owned(),
        pre_state_id: transition.pre_state_id().to_owned(),
        events: transition.events().to_vec(),
        events_hash,
        entropy_before: transition.entropy_before().clone(),
        entropy_draws: transition.entropy_draws().to_vec(),
        entropy_after: transition.entropy_after().clone(),
        post_state_id: transition.post_state_id().to_owned(),
        observation,
        observation_hash,
        receipt: String::new(),
    };
    trace_step.receipt = trace_step.recomputed_receipt()?;
    Ok(trace_step)
}

fn check_equal<T: PartialEq + Debug + ?Sized>(
    path: &str,
    expected: &T,
    actual: &T,
) -> Result<(), ReplayError> {
    if expected == actual {
        Ok(())
    } else {
        Err(ReplayError::Mismatch {
            path: path.to_owned(),
            expected: format!("{expected:?}"),
            actual: format!("{actual:?}"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_kernel::{
        ActionDefinition, CharacterCreationChoice, CharacterCreationDefinition,
        CharacterCreationSlot, CharacterPatch, CharacterPreset, Condition, ContentDraft, Effect,
        EntropyError, LocationDefinition, ParameterSpec, WorldState,
    };
    use std::collections::{BTreeMap, BTreeSet};

    fn content_draft() -> ContentDraft {
        ContentDraft {
            schema_version: "forge-schema-v3".to_owned(),
            rules_version: "forge-rules-v1".to_owned(),
            world_id: "world".to_owned(),
            contract: Default::default(),
            start_location: "start".to_owned(),
            character_presets: Vec::new(),
            character_creation: None,
            locations: vec![
                LocationDefinition {
                    id: "start".to_owned(),
                    name: "Start".to_owned(),
                    description: "A marked test room stands here.".to_owned(),
                    description_variants: Vec::new(),
                    exits: vec!["end".to_owned()],
                    terminal: true,
                },
                LocationDefinition {
                    id: "end".to_owned(),
                    name: "End".to_owned(),
                    description: "A quiet test room waits ahead.".to_owned(),
                    description_variants: Vec::new(),
                    exits: vec!["start".to_owned()],
                    terminal: true,
                },
            ],
            npcs: Vec::new(),
            actions: vec![
                ActionDefinition {
                    id: "mark".to_owned(),
                    label: "Mark".to_owned(),
                    category: "test".to_owned(),
                    result: "Marked.".to_owned(),
                    result_variants: Vec::new(),
                    locations: vec!["start".to_owned()],
                    condition: Condition::Always,
                    effects: vec![Effect::SetFlag {
                        flag: "marked".to_owned(),
                        value: true,
                    }],
                    parameters: Vec::new(),
                    meaningful: true,
                    movement: false,
                },
                ActionDefinition {
                    id: "roll".to_owned(),
                    label: "Roll".to_owned(),
                    category: "test".to_owned(),
                    result: "Rolled.".to_owned(),
                    result_variants: Vec::new(),
                    locations: vec!["start".to_owned()],
                    condition: Condition::Always,
                    effects: vec![Effect::RandomChance {
                        success_percent: 100,
                        on_success: Box::new(Effect::SetFlag {
                            flag: "rolled".to_owned(),
                            value: true,
                        }),
                        on_failure: Box::new(Effect::SetFlag {
                            flag: "failed".to_owned(),
                            value: true,
                        }),
                    }],
                    parameters: Vec::new(),
                    meaningful: true,
                    movement: false,
                },
                ActionDefinition {
                    id: "wait".to_owned(),
                    label: "Wait".to_owned(),
                    category: "test".to_owned(),
                    result: "Waited.".to_owned(),
                    result_variants: Vec::new(),
                    locations: vec!["start".to_owned()],
                    condition: Condition::Always,
                    effects: vec![Effect::AdvanceTime { ticks: 1 }],
                    parameters: Vec::new(),
                    meaningful: true,
                    movement: false,
                },
            ],
        }
    }

    fn content() -> CompiledContent {
        CompiledContent::try_compile(content_draft()).expect("test content is valid")
    }

    fn state(content: &CompiledContent) -> GameState {
        GameState::new(
            content.build_id().to_owned(),
            WorldState::new(
                content.world_id(),
                "start",
                content.empty_location_runtime(),
                BTreeMap::new(),
            ),
            forge_kernel::Character {
                id: "hero".to_owned(),
                lineage: "fenborn".to_owned(),
                origin: "start".to_owned(),
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
            },
            EntropyState::new(71),
        )
    }

    fn production_content() -> CompiledContent {
        let mut draft = content_draft();
        let fixture = content();
        let mut ilyan = state(&fixture).character;
        ilyan.id = "ilyan".to_owned();
        ilyan.background = "ledger_clerk".to_owned();
        let mut rook = ilyan.clone();
        rook.id = "rook".to_owned();
        rook.background = "lock_runner".to_owned();
        draft.contract = ContentContract::Production;
        draft.character_presets = vec![
            CharacterPreset {
                id: "ilyan".to_owned(),
                display_name: "Ilyan Vale".to_owned(),
                summary: "A careful clerk reads the law.".to_owned(),
                character: ilyan,
            },
            CharacterPreset {
                id: "rook".to_owned(),
                display_name: "Rook Ash".to_owned(),
                summary: "A wanted runner finds hidden routes.".to_owned(),
                character: rook,
            },
        ];
        draft.character_creation = Some(CharacterCreationDefinition {
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
                                origin: Some("start".to_owned()),
                                background: Some("clerk".to_owned()),
                                ..CharacterPatch::default()
                            },
                        },
                        CharacterCreationChoice {
                            id: "runner".to_owned(),
                            display_name: "Runner".to_owned(),
                            summary: "A runner from the locks.".to_owned(),
                            patch: CharacterPatch {
                                origin: Some("locks".to_owned()),
                                background: Some("runner".to_owned()),
                                ..CharacterPatch::default()
                            },
                        },
                    ],
                },
            ],
        });
        CompiledContent::try_compile(draft).expect("production test content is valid")
    }

    fn action_for(
        state: &GameState,
        content: &CompiledContent,
        definition_id: &str,
    ) -> CanonicalAction {
        enumerate_legal_actions(state, content)
            .expect("test state enumerates")
            .into_iter()
            .find(|action| action.definition_id == definition_id)
            .expect("requested test action is legal")
    }

    fn two_step_trace(content: &CompiledContent) -> Trace {
        let mut session = Session::new(state(content), content).expect("session starts");
        let first = action_for(session.state(), content, "roll");
        session.record(&first).expect("roll records");
        let second = action_for(session.state(), content, "wait");
        session.record(&second).expect("wait records");
        session.into_trace()
    }

    fn two_step_production_session(content: &CompiledContent) -> Session<'_> {
        let mut session = Session::new_game("ilyan", 71, content).expect("session starts");
        let first = action_for(session.state(), content, "roll");
        session.record(&first).expect("roll records");
        let second = action_for(session.state(), content, "wait");
        session.record(&second).expect("wait records");
        session
    }

    fn custom_selection() -> CharacterSelection {
        CharacterSelection {
            name: "Mara Venn".to_owned(),
            choices: vec![
                forge_kernel::CharacterChoiceSelection {
                    slot_id: "path".to_owned(),
                    choice_id: "clerk".to_owned(),
                },
                forge_kernel::CharacterChoiceSelection {
                    slot_id: "lineage".to_owned(),
                    choice_id: "fenborn".to_owned(),
                },
            ],
        }
    }

    #[test]
    fn receipts_are_deterministic_and_bind_the_full_chain() {
        let content = content();
        let left = two_step_trace(&content);
        let right = two_step_trace(&content);
        assert_eq!(left, right);
        assert_eq!(
            left.initial_receipt,
            left.recomputed_initial_receipt().unwrap()
        );
        for step in &left.steps {
            assert_eq!(step.receipt, step.recomputed_receipt().unwrap());
        }
        assert_ne!(left.initial_receipt, left.steps[0].receipt);
        assert_eq!(left.final_receipt, left.steps[1].receipt);
        assert_eq!(
            left.initial_observation_hash,
            sha256_json(&left.initial_observation).unwrap()
        );
        for step in &left.steps {
            assert_eq!(
                step.observation_hash,
                sha256_json(&step.observation).unwrap()
            );
            assert!(step.observation.result.is_some());
        }
    }

    #[test]
    fn multi_step_session_replays_and_resumes_with_uninterrupted_parity() {
        let content = content();
        let initial = state(&content);
        let mut uninterrupted = Session::new(initial.clone(), &content).unwrap();
        let first = action_for(uninterrupted.state(), &content, "roll");
        uninterrupted.record(&first).unwrap();
        let second = action_for(uninterrupted.state(), &content, "wait");
        uninterrupted.record(&second).unwrap();
        let expected_state = uninterrupted.state().clone();
        let trace = uninterrupted.into_trace();

        let verified_state = verify(&trace, &content).unwrap();
        assert_eq!(verified_state, expected_state);

        let prefix = Trace {
            final_state_id: trace.steps[0].post_state_id.clone(),
            final_receipt: trace.steps[0].receipt.clone(),
            steps: vec![trace.steps[0].clone()],
            ..trace.clone()
        };
        let mut resumed = resume(&prefix, &content).unwrap();
        resumed.record(&second).unwrap();
        assert_eq!(resumed.state(), &expected_state);
        assert_eq!(resumed.trace(), &trace);
    }

    #[test]
    fn save_json_round_trip_preserves_verified_trace() {
        let content = content();
        let trace = two_step_trace(&content);
        let json = trace.to_json().unwrap();
        let decoded = Trace::from_json(&json).unwrap();
        assert_eq!(decoded, trace);
        assert_eq!(
            verify(&decoded, &content).unwrap(),
            verify(&trace, &content).unwrap()
        );
    }

    #[test]
    fn player_trace_omits_hidden_claims_and_reconstructs_exactly() {
        let content = production_content();
        let session = two_step_production_session(&content);
        let expected = session.trace().clone();
        let player_trace = session.player_trace().unwrap();
        let json = player_trace.to_json().unwrap();
        let document: serde_json::Value = serde_json::from_str(&json).unwrap();
        let mut keys: Vec<_> = document
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "action_ids",
                "build_id",
                "final_receipt",
                "final_state_id",
                "format_version",
                "start",
            ]
        );

        let decoded = PlayerTrace::from_json(&json).unwrap();
        let reconstructed = resume_player_trace(&decoded, &content).unwrap();
        assert_eq!(reconstructed.trace(), &expected);
        assert_eq!(reconstructed.state(), &verify(&expected, &content).unwrap());
    }

    #[test]
    fn custom_player_trace_round_trips_and_binds_the_canonical_recipe() {
        let content = production_content();
        let mut session = Session::new_custom_game(&custom_selection(), 71, &content).unwrap();
        let action = action_for(session.state(), &content, "wait");
        session.record(&action).unwrap();
        let expected = session.trace().clone();
        assert!(matches!(
            expected.start,
            TraceStart::CharacterCreation { .. }
        ));

        let player_trace = session.player_trace().unwrap();
        let json = player_trace.to_json().unwrap();
        for hidden in [
            "initial_state",
            "observation",
            "events",
            "entropy",
            "knowledge",
            "aptitudes",
        ] {
            assert!(!json.contains(hidden), "custom save leaked {hidden}");
        }
        let reconstructed =
            resume_player_trace(&PlayerTrace::from_json(&json).unwrap(), &content).unwrap();
        assert_eq!(reconstructed.trace(), &expected);

        let mut changed = player_trace.clone();
        let TraceStart::CharacterCreation { selection, .. } = &mut changed.start else {
            panic!("expected custom start")
        };
        selection.choices[0].choice_id = "missing".to_owned();
        assert!(resume_player_trace(&changed, &content).is_err());

        let mut changed = player_trace.clone();
        let TraceStart::CharacterCreation { selection, .. } = &mut changed.start else {
            panic!("expected custom start")
        };
        selection.choices.reverse();
        assert!(resume_player_trace(&changed, &content).is_err());

        let mut changed = player_trace.clone();
        let TraceStart::CharacterCreation { seed, .. } = &mut changed.start else {
            panic!("expected custom start")
        };
        *seed += 1;
        assert!(resume_player_trace(&changed, &content).is_err());
    }

    #[test]
    fn replay_decoders_reject_duplicate_json_keys_before_deserialization() {
        let content = production_content();
        let session = Session::new_game("ilyan", 71, &content).unwrap();
        let json = session.player_trace().unwrap().to_json().unwrap();
        let duplicate = json.replacen(
            "\"format_version\":",
            "\"format_version\":\"shadow\",\"format_version\":",
            1,
        );
        assert!(PlayerTrace::from_json(&duplicate).is_err());

        let detailed = session.trace().to_json().unwrap();
        let duplicate = detailed.replacen(
            "\"format_version\":",
            "\"format_version\":\"shadow\",\"format_version\":",
            1,
        );
        assert!(Trace::from_json(&duplicate).is_err());
    }

    #[test]
    fn player_trace_rejects_tampering_and_fixture_genesis() {
        let production = production_content();
        let session = two_step_production_session(&production);
        let player_trace = session.player_trace().unwrap();

        let mut changed = player_trace.clone();
        changed.action_ids[0].push('x');
        assert!(resume_player_trace(&changed, &production).is_err());

        let mut changed = player_trace.clone();
        changed.final_receipt.push('x');
        assert!(resume_player_trace(&changed, &production).is_err());

        let fixture = content();
        let fixture_session = Session::new(state(&fixture), &fixture).unwrap();
        assert!(fixture_session.player_trace().is_err());
    }

    #[test]
    fn tampering_each_claimed_field_is_rejected() {
        let content = content();
        let original = two_step_trace(&content);

        let mut cases = Vec::new();
        let mut changed = original.clone();
        changed.build_id.push('x');
        cases.push(("build", changed));

        let mut changed = original.clone();
        changed.initial_state.world.time += 1;
        cases.push(("initial state", changed));

        let mut changed = original.clone();
        changed.initial_state.entropy.seed += 1;
        cases.push(("seed", changed));

        let mut changed = original.clone();
        changed.start = TraceStart::CharacterPreset {
            character_preset_id: "missing".to_owned(),
            seed: 71,
        };
        cases.push(("start specification", changed));

        let mut changed = original.clone();
        changed.initial_observation.text.push_str(" altered");
        cases.push(("initial observation", changed));

        let mut changed = original.clone();
        changed.initial_observation_hash.push('x');
        cases.push(("initial observation hash", changed));

        let mut changed = original.clone();
        changed.steps[0].action.definition_id = "wait".to_owned();
        cases.push(("action", changed));

        let mut changed = original.clone();
        changed.steps[0]
            .action
            .parameters
            .insert("unexpected".to_owned(), "value".to_owned());
        cases.push(("parameters", changed));

        let mut changed = original.clone();
        changed.steps[0].entropy_before.seed += 1;
        cases.push(("entropy before", changed));

        let mut changed = original.clone();
        changed.steps[0].entropy_draws[0].value ^= 1;
        cases.push(("entropy draw", changed));

        let mut changed = original.clone();
        changed.steps[0].entropy_after.cursor += 1;
        cases.push(("entropy after", changed));

        let mut changed = original.clone();
        changed.steps[0].events[0].turn += 1;
        cases.push(("events", changed));

        let mut changed = original.clone();
        changed.steps[0].events_hash.push('x');
        cases.push(("events hash", changed));

        let mut changed = original.clone();
        changed.steps[0].observation.text.push_str(" altered");
        cases.push(("observation", changed));

        let mut changed = original.clone();
        changed.steps[0].observation_hash.push('x');
        cases.push(("observation hash", changed));

        let mut changed = original.clone();
        changed.steps[0].legal_action_set_digest.push('x');
        cases.push(("legal digest", changed));

        let mut changed = original.clone();
        changed.steps[0].pre_state_id.push('x');
        cases.push(("intermediate pre ID", changed));

        let mut changed = original.clone();
        changed.steps[1].post_state_id.push('x');
        cases.push(("intermediate post ID", changed));

        let mut changed = original.clone();
        changed.steps[0].prior_receipt.push('x');
        cases.push(("prior receipt", changed));

        let mut changed = original.clone();
        changed.steps[0].receipt.push('x');
        cases.push(("step receipt", changed));

        let mut changed = original.clone();
        changed.final_state_id.push('x');
        cases.push(("final state ID", changed));

        let mut changed = original.clone();
        changed.final_receipt.push('x');
        cases.push(("final receipt", changed));

        for (label, changed) in cases {
            assert!(
                verify(&changed, &content).is_err(),
                "tamper must fail: {label}"
            );
        }
    }

    #[test]
    fn stale_and_wrong_build_actions_are_rejected_without_session_mutation() {
        let content = content();
        let initial = state(&content);
        let mut pristine = Session::new(initial.clone(), &content).unwrap();
        let mut initially_wrong = action_for(pristine.state(), &content, "wait");
        initially_wrong.build_id = "wrong-build".to_owned();
        let pristine_state = pristine.state().clone();
        let pristine_trace = pristine.trace().clone();
        let pristine_capacity = pristine.trace().steps.capacity();
        assert!(matches!(
            pristine.record(&initially_wrong),
            Err(ReplayError::Kernel(KernelError::WrongBuild { .. }))
        ));
        assert_eq!(pristine.state(), &pristine_state);
        assert_eq!(pristine.trace(), &pristine_trace);
        assert_eq!(pristine.trace().steps.capacity(), pristine_capacity);

        let mut session = Session::new(initial, &content).unwrap();
        let action = action_for(session.state(), &content, "wait");
        session.record(&action).unwrap();
        let state_before = session.state().clone();
        let trace_before = session.trace().clone();

        assert!(matches!(
            session.record(&action),
            Err(ReplayError::Kernel(KernelError::StaleAction { .. }))
        ));
        assert_eq!(session.state(), &state_before);
        assert_eq!(session.trace(), &trace_before);

        let mut wrong_build = action_for(session.state(), &content, "wait");
        wrong_build.build_id = "wrong-build".to_owned();
        assert!(matches!(
            session.record(&wrong_build),
            Err(ReplayError::Kernel(KernelError::WrongBuild { .. }))
        ));
        assert_eq!(session.state(), &state_before);
        assert_eq!(session.trace(), &trace_before);
    }

    #[test]
    fn wrong_content_is_rejected_even_when_a_trace_is_well_formed() {
        let content = content();
        let trace = two_step_trace(&content);
        let other_draft = ContentDraft {
            schema_version: "forge-schema-v3".to_owned(),
            rules_version: "forge-rules-v1".to_owned(),
            world_id: "world".to_owned(),
            contract: Default::default(),
            start_location: "start".to_owned(),
            character_presets: Vec::new(),
            character_creation: None,
            locations: vec![
                LocationDefinition {
                    id: "start".to_owned(),
                    name: "Start".to_owned(),
                    description: "A marked test room stands here.".to_owned(),
                    description_variants: Vec::new(),
                    exits: vec!["end".to_owned()],
                    terminal: true,
                },
                LocationDefinition {
                    id: "end".to_owned(),
                    name: "End".to_owned(),
                    description: "A quiet test room waits ahead.".to_owned(),
                    description_variants: Vec::new(),
                    exits: vec!["start".to_owned()],
                    terminal: true,
                },
            ],
            npcs: Vec::new(),
            actions: vec![ActionDefinition {
                id: "different".to_owned(),
                label: "Different".to_owned(),
                category: "test".to_owned(),
                result: "Different.".to_owned(),
                result_variants: Vec::new(),
                locations: vec!["start".to_owned()],
                condition: Condition::Always,
                effects: vec![Effect::AdvanceTime { ticks: 1 }],
                parameters: Vec::<ParameterSpec>::new(),
                meaningful: true,
                movement: false,
            }],
        };
        let wrong = CompiledContent::try_compile(other_draft.clone()).unwrap();
        assert!(verify(&trace, &wrong).is_err());
    }

    #[test]
    fn invalid_initial_entropy_is_rejected_as_typed_kernel_error() {
        let content = content();
        let mut invalid = state(&content);
        invalid.entropy.algorithm = "wrong".to_owned();
        assert!(matches!(
            Session::new(invalid, &content),
            Err(ReplayError::Kernel(KernelError::Entropy(
                EntropyError::UnsupportedAlgorithm { .. }
            )))
        ));
    }

    #[test]
    fn production_trace_reconstructs_authored_preset_genesis() {
        let content = production_content();
        let session = Session::new_game("ilyan", 91, &content).unwrap();
        let trace = session.into_trace();
        assert_eq!(
            trace.start,
            TraceStart::CharacterPreset {
                character_preset_id: "ilyan".to_owned(),
                seed: 91,
            }
        );
        assert_eq!(verify(&trace, &content).unwrap(), trace.initial_state);

        let mut forged_state = trace.initial_state.clone();
        forged_state
            .character
            .resources
            .insert("unearned_coin".to_owned(), 999);
        assert!(Session::new(forged_state.clone(), &content).is_err());

        // Even a self-consistent replacement genesis and receipt cannot pass:
        // production verification reconstructs the named authored start.
        let mut forged = trace.clone();
        forged.initial_state = forged_state;
        forged.initial_state_id = forged.initial_state.state_id();
        forged.initial_observation = content.observe(&forged.initial_state).unwrap();
        forged.initial_observation_hash = sha256_json(&forged.initial_observation).unwrap();
        forged.initial_receipt = forged.recomputed_initial_receipt().unwrap();
        forged.final_state_id = forged.initial_state_id.clone();
        forged.final_receipt = forged.initial_receipt.clone();
        assert!(verify(&forged, &content).is_err());
    }
}
