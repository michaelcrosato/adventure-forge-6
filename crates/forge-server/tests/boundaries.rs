use forge_content::parse_and_compile_production;
use forge_kernel::{ActionPage, CompiledContent, ContentContract};
use forge_server::{
    ActionRequest, MAX_REQUEST_BYTES, ServiceError, ServiceLimits, SessionService, SessionView,
    StartRequest, start_options,
};
use serde_json::{Value, json};
use std::sync::Arc;

const SPLIT_TIDE: &str = include_str!("../../../content/split-tide.json");

fn content() -> Arc<CompiledContent> {
    Arc::new(parse_and_compile_production(SPLIT_TIDE).expect("the production pack must compile"))
}

fn preset(id: &str) -> StartRequest {
    StartRequest::Preset {
        character_preset_id: id.to_owned(),
        seed: 71,
    }
}

fn service(id: &str) -> SessionService {
    SessionService::start(content(), preset(id), ServiceLimits::default())
        .expect("the authored preset must start")
}

fn all_actions(service: &SessionService, view: &SessionView) -> ActionPage {
    service
        .catalog(&view.observation.state_id, 0, 128)
        .expect("the current catalog must be readable")
}

fn action_id(
    service: &SessionService,
    view: &SessionView,
    definition_id: &str,
    destination: Option<&str>,
) -> String {
    all_actions(service, view)
        .actions
        .into_iter()
        .find(|action| {
            action.definition_id == definition_id
                && destination.is_none_or(|destination| {
                    action
                        .parameters
                        .get("destination")
                        .is_some_and(|value| value == destination)
                })
        })
        .map(|action| action.action_id)
        .unwrap_or_else(|| panic!("missing legal action {definition_id} {destination:?}"))
}

fn request(view: &SessionView, command_id: &str, action_id: String) -> ActionRequest {
    ActionRequest {
        command_id: command_id.to_owned(),
        expected_revision: view.revision,
        expected_state_id: view.observation.state_id.clone(),
        action_id,
    }
}

fn act(
    service: &SessionService,
    view: &SessionView,
    command_id: &str,
    action_id: String,
) -> SessionView {
    service
        .act(request(view, command_id, action_id))
        .expect("the selected current action must succeed")
}

fn json_string<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("public values must serialize")
}

fn assert_no_private_fields(json: &str) {
    for forbidden in [
        "\"character\":",
        "\"patch\":",
        "\"inventory\":",
        "\"knowledge\":",
        "\"memories\":",
        "\"flags\":",
        "\"entropy\":",
        "\"event_log\":",
        "\"initial_state\":",
        "\"steps\":",
    ] {
        assert!(
            !json.contains(forbidden),
            "public boundary leaked {forbidden}: {json}"
        );
    }
}

#[test]
fn request_decoders_are_strict_and_bounded() {
    assert_eq!(
        StartRequest::from_json(
            r#"{"kind":"preset","character_preset_id":"rook","seed":71,"extra":true}"#,
        ),
        Err(ServiceError::InvalidInput)
    );
    assert_eq!(
        StartRequest::from_json(
            r#"{"kind":"preset","character_preset_id":"rook","seed":71,"seed":72}"#,
        ),
        Err(ServiceError::InvalidInput)
    );
    assert_eq!(
        StartRequest::from_json(
            r#"{"kind":"preset","character_preset_id":"rook","seed":71,"character":{"id":"forged"}}"#,
        ),
        Err(ServiceError::InvalidInput)
    );
    assert_eq!(
        StartRequest::from_json(
            r#"{"kind":"custom","selection":{"name":"Rook","choices":[],"patch":{}},"seed":71}"#,
        ),
        Err(ServiceError::InvalidInput)
    );
    assert_eq!(
        ActionRequest::from_json(
            r#"{"command_id":"c","expected_revision":0,"expected_state_id":"s","action_id":"a","rawCharacter":{}}"#,
        ),
        Err(ServiceError::InvalidInput)
    );
    assert_eq!(
        ActionRequest::from_json(
            r#"{"command_id":"c","expected_revision":0,"expected_state_id":"s","action_id":"a","action_id":"b"}"#,
        ),
        Err(ServiceError::InvalidInput)
    );
    let oversized = "x".repeat(MAX_REQUEST_BYTES + 1);
    assert_eq!(
        StartRequest::from_json(&oversized),
        Err(ServiceError::ResourceLimit)
    );

    let valid_start = r#"{"kind":"preset","character_preset_id":"rook","seed":71}"#;
    let padded_start = format!(
        "{valid_start}{}",
        " ".repeat(MAX_REQUEST_BYTES - valid_start.len())
    );
    assert_eq!(padded_start.len(), MAX_REQUEST_BYTES);
    assert!(StartRequest::from_json(&padded_start).is_ok());
    assert_eq!(
        StartRequest::from_json(&format!("{padded_start} ")),
        Err(ServiceError::ResourceLimit)
    );

    let valid_action = r#"{"command_id":"boundary","expected_revision":0,"expected_state_id":"0000000000000000000000000000000000000000000000000000000000000000","action_id":"1111111111111111111111111111111111111111111111111111111111111111"}"#;
    let padded_action = format!(
        "{valid_action}{}",
        " ".repeat(MAX_REQUEST_BYTES - valid_action.len())
    );
    assert_eq!(padded_action.len(), MAX_REQUEST_BYTES);
    assert!(ActionRequest::from_json(&padded_action).is_ok());
    assert_eq!(
        ActionRequest::from_json(&format!("{padded_action} ")),
        Err(ServiceError::ResourceLimit)
    );
}

#[test]
fn start_options_are_an_allowlisted_ordered_public_projection() {
    let compiled = content();
    let options = start_options(&compiled).expect("production start options must be available");
    assert_eq!(
        options
            .presets
            .iter()
            .map(|preset| preset.id.as_str())
            .collect::<Vec<_>>(),
        ["ilyan", "rook"]
    );
    assert_eq!(
        options
            .creation_slots
            .iter()
            .map(|slot| slot.id.as_str())
            .collect::<Vec<_>>(),
        ["lineage", "origin", "calling", "value", "burden", "history"]
    );
    let json = json_string(&options);
    for forbidden in ["\"character\":", "\"patch\":", "\"npcs\":", "Yara Dene"] {
        assert!(
            !json.contains(forbidden),
            "start options leaked {forbidden}"
        );
    }
    assert!(json.contains("Ilyan Vale"));
    assert!(json.contains("Kilnborn"));
}

#[test]
fn invalid_limits_start_recipes_and_catalog_arguments_have_stable_codes() {
    let zero_limit = ServiceLimits {
        max_save_bytes: 0,
        ..ServiceLimits::default()
    };
    assert!(matches!(
        SessionService::start(content(), preset("rook"), zero_limit),
        Err(ServiceError::InvalidInput)
    ));

    let defaults = ServiceLimits::default();
    let reversed_pages = ServiceLimits {
        default_page_size: defaults.max_page_size + 1,
        ..defaults
    };
    assert!(matches!(
        SessionService::start(content(), preset("rook"), reversed_pages),
        Err(ServiceError::InvalidInput)
    ));

    assert!(matches!(
        SessionService::start(
            content(),
            preset("missing-preset"),
            ServiceLimits::default()
        ),
        Err(ServiceError::InvalidInput)
    ));

    let mut fixture_source = forge_content::parse(SPLIT_TIDE).expect("production JSON parses");
    fixture_source.contract = ContentContract::Fixture;
    let fixture_content = Arc::new(
        forge_content::compile(fixture_source).expect("the same source compiles as a fixture"),
    );
    assert!(matches!(
        SessionService::start(fixture_content, preset("rook"), ServiceLimits::default()),
        Err(ServiceError::InvalidContent)
    ));

    let game = service("rook");
    let view = game.observe().expect("initial view");
    assert_eq!(
        game.catalog(&view.observation.state_id, 0, 0),
        Err(ServiceError::InvalidInput)
    );
    assert_eq!(
        game.catalog(&view.observation.state_id, 0, 129),
        Err(ServiceError::ResourceLimit)
    );
    assert_eq!(
        game.catalog(&view.observation.state_id, view.catalog.total + 1, 1),
        Err(ServiceError::InvalidInput)
    );
    assert_eq!(
        game.catalog(&"0".repeat(64), 0, 1),
        Err(ServiceError::StaleState)
    );
}

#[test]
fn views_and_exports_exclude_hidden_state_but_show_transferred_owned_key() {
    let game = service("rook");
    let initial = game.observe().expect("initial view");
    assert_no_private_fields(&json_string(&initial));
    let initial_save = game.save().expect("initial safe save");
    assert_no_private_fields(&initial_save);
    assert!(!initial_save.contains("split_tide.tide_key"));
    assert!(
        initial
            .observation
            .supplies
            .items
            .iter()
            .all(|item| item.id != "split_tide.tide_key")
    );

    let at_docks = act(
        &game,
        &initial,
        "boundary-travel-docks",
        action_id(&game, &initial, "travel_adjacent", Some("lowsail.docks")),
    );
    let after_travel_json = json_string(&at_docks);
    assert_no_private_fields(&after_travel_json);
    assert!(
        at_docks
            .observation
            .supplies
            .items
            .iter()
            .all(|item| item.id != "split_tide.tide_key")
    );

    let after_key = act(
        &game,
        &at_docks,
        "boundary-take-key",
        action_id(&game, &at_docks, "docks.press_yara", None),
    );
    let key = after_key
        .observation
        .supplies
        .items
        .iter()
        .find(|item| item.id == "split_tide.tide_key")
        .expect("the transferred key must enter player supplies");
    assert_eq!(key.name, "Tide Key");
    assert_eq!(key.count, 1);
    assert_no_private_fields(&json_string(&after_key));
    assert_no_private_fields(&game.save().expect("post-transfer safe save"));
    assert!(json_string(&after_key).contains("Tide Key"));
}

#[test]
fn malformed_or_tampered_resume_is_inert_to_existing_sessions() {
    let subject = service("rook");
    let before_subject = subject.observe().expect("subject view");
    let subject_action = request(
        &before_subject,
        "resume-seed-action",
        action_id(
            &subject,
            &before_subject,
            "travel_adjacent",
            Some("lowsail.docks"),
        ),
    );
    subject
        .act(subject_action)
        .expect("subject action must succeed");
    let valid = subject.save().expect("subject save");

    let other = service("ilyan");
    let other_view = other.observe().expect("other view");
    let other_save = other.save().expect("other save");
    let mut cases = vec![
        "{".to_owned(),
        r#"{"format_version":"forge-player-trace-v2","format_version":"duplicate"}"#.to_owned(),
    ];

    let mut wrong_build = serde_json::from_str::<Value>(&valid).expect("valid save JSON");
    wrong_build["build_id"] = json!("0".repeat(64));
    cases.push(json_string(&wrong_build));

    let mut bad_start = serde_json::from_str::<Value>(&valid).expect("valid save JSON");
    bad_start["start"] = json!({"kind":"fixture_state"});
    cases.push(json_string(&bad_start));

    let mut changed_preset = serde_json::from_str::<Value>(&valid).expect("valid save JSON");
    changed_preset["start"]["character_preset_id"] = json!("ilyan");
    cases.push(json_string(&changed_preset));

    let mut changed_seed = serde_json::from_str::<Value>(&valid).expect("valid save JSON");
    changed_seed["start"]["seed"] = json!(72);
    cases.push(json_string(&changed_seed));

    let mut bad_action = serde_json::from_str::<Value>(&valid).expect("valid save JSON");
    bad_action["action_ids"][0] = json!("0".repeat(64));
    cases.push(json_string(&bad_action));

    let mut bad_receipt = serde_json::from_str::<Value>(&valid).expect("valid save JSON");
    bad_receipt["final_receipt"] = json!("0".repeat(64));
    cases.push(json_string(&bad_receipt));

    for bad in cases {
        assert!(
            matches!(
                SessionService::resume(content(), &bad, ServiceLimits::default()),
                Err(ServiceError::InvalidSave)
            ),
            "bad save must fail closed: {bad}"
        );
        assert_eq!(
            other.observe().expect("other view remains readable"),
            other_view
        );
        assert_eq!(
            other.save().expect("other save remains readable"),
            other_save
        );
    }
}

#[test]
fn fake_stale_and_foreign_state_actions_do_not_mutate() {
    let ilyan = service("ilyan");
    let rook = service("rook");
    let ilyan_view = ilyan.observe().expect("Ilyan view");
    let rook_view = rook.observe().expect("Rook view");
    let rook_save = rook.save().expect("Rook save");

    let audit = action_id(&ilyan, &ilyan_view, "checkpoint.audit_order", None);
    assert_eq!(
        rook.act(request(&rook_view, "cross-session", audit)),
        Err(ServiceError::InvalidAction)
    );
    assert_eq!(rook.save().expect("Rook remains unchanged"), rook_save);

    let fake = request(&rook_view, "fake-action", "0".repeat(64));
    assert_eq!(rook.act(fake), Err(ServiceError::InvalidAction));
    assert_eq!(rook.save().expect("fake action is inert"), rook_save);

    let malformed_action = request(&rook_view, "malformed-action-id", "not-a-hash".to_owned());
    assert_eq!(rook.act(malformed_action), Err(ServiceError::InvalidInput));
    assert_eq!(
        rook.save().expect("malformed action id is inert"),
        rook_save
    );

    let malformed_state = ActionRequest {
        command_id: "malformed-state-id".to_owned(),
        expected_revision: rook_view.revision,
        expected_state_id: "not-a-state-hash".to_owned(),
        action_id: "0".repeat(64),
    };
    assert_eq!(rook.act(malformed_state), Err(ServiceError::InvalidInput));
    assert_eq!(rook.save().expect("malformed state id is inert"), rook_save);

    let accepted_request = request(
        &rook_view,
        "accepted-before-stale",
        action_id(&rook, &rook_view, "wait_tide", None),
    );
    let after = rook
        .act(accepted_request.clone())
        .expect("the first command must succeed");
    let after_save = rook.save().expect("accepted action save");
    let stale = ActionRequest {
        command_id: "new-stale-command".to_owned(),
        expected_revision: rook_view.revision,
        expected_state_id: rook_view.observation.state_id.clone(),
        action_id: accepted_request.action_id,
    };
    assert_eq!(rook.act(stale), Err(ServiceError::StaleState));
    assert_eq!(rook.observe().expect("post-stale view"), after);
    assert_eq!(rook.save().expect("post-stale save"), after_save);

    let wrong_state = ActionRequest {
        command_id: "wrong-state".to_owned(),
        expected_revision: after.revision,
        expected_state_id: "0".repeat(64),
        action_id: action_id(&rook, &after, "wait_tide", None),
    };
    assert_eq!(rook.act(wrong_state), Err(ServiceError::StaleState));
    assert_eq!(rook.observe().expect("wrong-state is inert"), after);
    assert_eq!(rook.save().expect("post-wrong-state save"), after_save);

    let wrong_revision = ActionRequest {
        command_id: "wrong-revision".to_owned(),
        expected_revision: u64::MAX,
        expected_state_id: after.observation.state_id.clone(),
        action_id: action_id(&rook, &after, "wait_tide", None),
    };
    assert_eq!(rook.act(wrong_revision), Err(ServiceError::StaleState));
    assert_eq!(rook.observe().expect("wrong-revision is inert"), after);
    assert_eq!(rook.save().expect("post-wrong-revision save"), after_save);
}

enum ChangedCandidateOutcome {
    ResourceLimit,
    Accepted,
}

fn assert_candidate_budget_is_atomic(
    limits: ServiceLimits,
    command_id: &str,
    alternate_action_id: Option<String>,
    expected_changed_outcome: ChangedCandidateOutcome,
) {
    let game = SessionService::start(content(), preset("rook"), limits.clone())
        .expect("the initial view must fit the selected budget");
    let before = game.observe().expect("initial view");
    let before_save = game.save().expect("initial save");
    let first = request(
        &before,
        command_id,
        action_id(&game, &before, "wait_tide", None),
    );
    assert_eq!(game.act(first.clone()), Err(ServiceError::ResourceLimit));
    assert_eq!(game.observe().expect("failed candidate is inert"), before);
    assert_eq!(
        game.save().expect("failed candidate save is inert"),
        before_save
    );

    let second_action_id = alternate_action_id.unwrap_or_else(|| {
        all_actions(&game, &before)
            .actions
            .into_iter()
            .find(|action| action.action_id != first.action_id)
            .map(|action| action.action_id)
            .expect("the starting catalog has another action")
    });
    let changed_payload = ActionRequest {
        command_id: first.command_id,
        expected_revision: first.expected_revision,
        expected_state_id: first.expected_state_id,
        action_id: second_action_id,
    };
    match expected_changed_outcome {
        ChangedCandidateOutcome::ResourceLimit => {
            assert_eq!(
                game.act(changed_payload),
                Err(ServiceError::ResourceLimit),
                "a failed {command_id} candidate must not poison the command id as a conflict"
            );
            assert_eq!(game.observe().expect("changed candidate is inert"), before);
            assert_eq!(
                game.save().expect("changed candidate save is inert"),
                before_save
            );
        }
        ChangedCandidateOutcome::Accepted => {
            let accepted = game
                .act(changed_payload)
                .expect("a fitting changed candidate must retry successfully");
            assert_ne!(accepted, before);
            let after_save = game.save().expect("accepted changed candidate save");
            assert_ne!(after_save, before_save);
            let resumed = SessionService::resume(content(), &after_save, limits)
                .expect("accepted changed candidate must resume");
            assert_eq!(
                resumed.observe().expect("resumed changed candidate"),
                accepted
            );
        }
    }
}

#[test]
fn candidate_resource_failure_does_not_poison_idempotency_or_state() {
    let probe = service("rook");
    let probe_before = probe.observe().expect("probe view");
    let probe_before_save = probe.save().expect("probe save");
    let probe_first_action_id = action_id(&probe, &probe_before, "wait_tide", None);
    let response_candidates = all_actions(&probe, &probe_before).actions;
    let probe_after = probe
        .act(request(
            &probe_before,
            "probe-size",
            probe_first_action_id.clone(),
        ))
        .expect("probe action");

    let response_alternate = response_candidates
        .into_iter()
        .filter(|candidate| candidate.action_id != probe_first_action_id)
        .find_map(|candidate| {
            let trial = service("rook");
            let trial_before = trial.observe().ok()?;
            let next = trial
                .act(request(
                    &trial_before,
                    "response-size-probe",
                    candidate.action_id.clone(),
                ))
                .ok()?;
            (json_string(&next).len() <= json_string(&probe_before).len())
                .then_some(candidate.action_id)
        })
        .expect("the response budget needs a second candidate fitting the old view");

    let save_limits = ServiceLimits {
        max_save_bytes: probe_before_save.len(),
        ..ServiceLimits::default()
    };
    assert_candidate_budget_is_atomic(
        save_limits,
        "save-budget",
        None,
        ChangedCandidateOutcome::ResourceLimit,
    );

    let response_limits = ServiceLimits {
        max_response_bytes: json_string(&probe_before).len(),
        ..ServiceLimits::default()
    };
    assert_candidate_budget_is_atomic(
        response_limits,
        "response-budget",
        Some(response_alternate),
        ChangedCandidateOutcome::Accepted,
    );

    let idempotency_limits = ServiceLimits {
        max_idempotency_bytes: 1,
        ..ServiceLimits::default()
    };
    assert_candidate_budget_is_atomic(
        idempotency_limits,
        "idempotency-budget",
        None,
        ChangedCandidateOutcome::ResourceLimit,
    );

    assert!(json_string(&probe_after).len() > json_string(&probe_before).len());
}

#[test]
fn close_is_idempotent_but_cached_acknowledgements_survive_transport_close() {
    let game = service("rook");
    let initial = game.observe().expect("initial view");
    let accepted_request = request(
        &initial,
        "close-cached-command",
        action_id(&game, &initial, "wait_tide", None),
    );
    let accepted = game
        .act(accepted_request.clone())
        .expect("the command must succeed");
    let next_action_id = action_id(&game, &accepted, "wait_tide", None);
    let saved = game.save().expect("save before close");

    let clone = game.clone();
    game.close().expect("first close");
    clone.close().expect("second close through clone");
    game.close().expect("close is idempotent");
    assert_eq!(game.save().expect("save after close"), saved);
    assert_eq!(game.observe(), Err(ServiceError::SessionClosed));
    assert_eq!(
        game.catalog(&accepted.observation.state_id, 0, 1),
        Err(ServiceError::SessionClosed)
    );
    assert_eq!(
        game.act(ActionRequest {
            command_id: "new-after-close".to_owned(),
            expected_revision: accepted.revision,
            expected_state_id: accepted.observation.state_id.clone(),
            action_id: next_action_id,
        }),
        Err(ServiceError::SessionClosed)
    );
    assert_eq!(game.act(accepted_request), Ok(accepted));
}
