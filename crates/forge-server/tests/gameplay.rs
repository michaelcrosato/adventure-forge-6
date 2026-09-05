use forge_content::parse_and_compile_production;
use forge_kernel::{
    ActionPage, ActionView, CanonicalAction, CharacterChoiceSelection, CharacterSelection,
    CompiledContent, enumerate_legal_actions, legal_action_digest,
};
use forge_replay::{PlayerTrace, Session, resume_player_trace};
use forge_server::{
    ActionRequest, ServiceError, ServiceLimits, SessionService, SessionView, StartRequest,
};
use serde_json::{Value, json};
use std::sync::{Arc, Barrier};

const SPLIT_TIDE: &str = include_str!("../../../content/split-tide.json");

fn content() -> Arc<CompiledContent> {
    Arc::new(parse_and_compile_production(SPLIT_TIDE).expect("production content compiles"))
}

fn limits() -> ServiceLimits {
    ServiceLimits::default()
}

fn custom_selection(content: &CompiledContent) -> CharacterSelection {
    let creation = content.character_creation().expect("custom creation");
    CharacterSelection {
        name: "Server Custom".to_owned(),
        choices: creation
            .slots
            .iter()
            .map(|slot| CharacterChoiceSelection {
                slot_id: slot.id.clone(),
                choice_id: slot.choices[0].id.clone(),
            })
            .collect(),
    }
}

fn reference_view(
    session: &Session<'_>,
    content: &CompiledContent,
    page_size: usize,
) -> SessionView {
    SessionView {
        revision: u64::try_from(session.trace().steps.len()).expect("reference revision"),
        observation: session
            .trace()
            .steps
            .last()
            .map(|step| step.observation.clone())
            .unwrap_or_else(|| session.trace().initial_observation.clone()),
        catalog: content
            .action_page(session.state(), 0, page_size)
            .expect("reference catalog"),
    }
}

fn assert_view_matches(
    view: &SessionView,
    session: &Session<'_>,
    content: &CompiledContent,
    page_size: usize,
) {
    let expected = reference_view(session, content, page_size);
    assert_eq!(view, &expected);
}

fn action_on_page(
    page: &ActionPage,
    definition_id: &str,
    parameter: Option<(&str, &str)>,
) -> Option<ActionView> {
    page.actions
        .iter()
        .find(|action| {
            action.definition_id == definition_id
                && parameter.is_none_or(|(name, value)| {
                    action
                        .parameters
                        .get(name)
                        .is_some_and(|found| found == value)
                })
        })
        .cloned()
}

fn find_action(
    service: &SessionService,
    state_id: &str,
    limits: &ServiceLimits,
    definition_id: &str,
    parameter: Option<(&str, &str)>,
) -> ActionView {
    let mut offset = 0;
    loop {
        let page = service
            .catalog(state_id, offset, limits.max_page_size)
            .expect("service catalog page");
        if let Some(action) = action_on_page(&page, definition_id, parameter) {
            return action;
        }
        offset = page
            .next_offset
            .unwrap_or_else(|| panic!("missing legal action {definition_id}"));
    }
}

fn reference_action(
    session: &Session<'_>,
    content: &CompiledContent,
    definition_id: &str,
    parameter: Option<(&str, &str)>,
) -> CanonicalAction {
    enumerate_legal_actions(session.state(), content)
        .expect("reference legal catalog")
        .into_iter()
        .find(|action| {
            action.definition_id == definition_id
                && parameter.is_none_or(|(name, value)| {
                    action
                        .parameters
                        .get(name)
                        .is_some_and(|found| found == value)
                })
        })
        .unwrap_or_else(|| panic!("reference missing legal action {definition_id}"))
}

fn action_request(view: &SessionView, command_id: &str, action: &ActionView) -> ActionRequest {
    ActionRequest {
        command_id: command_id.to_owned(),
        expected_revision: view.revision,
        expected_state_id: view.observation.state_id.clone(),
        action_id: action.action_id.clone(),
    }
}

fn act_and_compare(
    service: &SessionService,
    reference: &mut Session<'_>,
    content: &CompiledContent,
    limits: &ServiceLimits,
    command_id: &str,
    definition_id: &str,
    parameter: Option<(&str, &str)>,
) -> SessionView {
    let before = service
        .observe()
        .expect("service observation before action");
    assert_eq!(before.observation.state_id, reference.state().state_id());
    let action = find_action(
        service,
        &before.observation.state_id,
        limits,
        definition_id,
        parameter,
    );
    let request = action_request(&before, command_id, &action);
    let canonical = reference_action(reference, content, definition_id, parameter);
    assert_eq!(action.action_id, canonical.action_id);
    let view = service.act(request).expect("service action");
    reference.record(&canonical).expect("reference action");
    assert_view_matches(&view, reference, content, limits.default_page_size);
    view
}

fn assert_save_checkpoint(
    service: &SessionService,
    reference: &Session<'_>,
    content: &Arc<CompiledContent>,
    limits: &ServiceLimits,
) {
    let view = service.observe().expect("checkpoint observation");
    let save = service.save().expect("service save");
    let reference_save = reference
        .player_trace()
        .expect("reference player save")
        .to_json()
        .expect("reference save JSON");
    assert_eq!(save, reference_save);

    let resumed = SessionService::resume(content.clone(), &save, limits.clone())
        .expect("service save resumes");
    assert_eq!(resumed.observe().expect("resumed observation"), view);
    assert_eq!(resumed.save().expect("resumed save"), save);

    let decoded = PlayerTrace::from_json(&save).expect("safe save parses");
    let mut resumed_reference = resume_player_trace(&decoded, content).expect("safe save replays");
    assert_eq!(resumed_reference.state(), reference.state());
    assert_eq!(
        resumed_reference.trace().final_state_id,
        reference.trace().final_state_id
    );
    assert_eq!(
        resumed_reference.trace().final_receipt,
        reference.trace().final_receipt
    );

    let resumed_before = resumed.observe().expect("resumed probe observation");
    let resumed_wait = find_action(
        &resumed,
        &resumed_before.observation.state_id,
        limits,
        "wait_tide",
        None,
    );
    let direct_wait = reference_action(&resumed_reference, content, "wait_tide", None);
    assert_eq!(resumed_wait.action_id, direct_wait.action_id);
    let resumed_after = resumed
        .act(action_request(
            &resumed_before,
            "resume-wait-probe",
            &resumed_wait,
        ))
        .expect("resumed service wait");
    resumed_reference
        .record(&direct_wait)
        .expect("resumed reference wait");
    assert_view_matches(
        &resumed_after,
        &resumed_reference,
        content,
        limits.default_page_size,
    );

    let resumed_save = resumed.save().expect("resumed save after probe");
    let resumed_reference_save = resumed_reference
        .player_trace()
        .expect("resumed reference save")
        .to_json()
        .expect("resumed reference save JSON");
    assert_eq!(resumed_save, resumed_reference_save);
    let resumed_decoded = PlayerTrace::from_json(&resumed_save).expect("resumed probe save parses");
    let replayed =
        resume_player_trace(&resumed_decoded, content).expect("resumed probe save replays");
    assert_eq!(replayed.state(), resumed_reference.state());
    assert_eq!(
        replayed.trace().final_state_id,
        resumed_reference.trace().final_state_id
    );
    assert_eq!(
        replayed.trace().final_receipt,
        resumed_reference.trace().final_receipt
    );
}

#[test]
fn preset_and_custom_starts_match_independent_kernel_views() {
    let content = content();
    let limits = limits();

    let preset = SessionService::start(
        content.clone(),
        StartRequest::Preset {
            character_preset_id: "ilyan".to_owned(),
            seed: 71,
        },
        limits.clone(),
    )
    .expect("preset service starts");
    let preset_reference = Session::new_game("ilyan", 71, &content).expect("preset reference");
    let preset_view = preset.observe().expect("preset observation");
    assert_view_matches(
        &preset_view,
        &preset_reference,
        &content,
        limits.default_page_size,
    );

    let selection = custom_selection(&content);
    let custom = SessionService::start(
        content.clone(),
        StartRequest::Custom {
            selection: selection.clone(),
            seed: 71,
        },
        limits.clone(),
    )
    .expect("custom service starts");
    let custom_reference =
        Session::new_custom_game(&selection, 71, &content).expect("custom reference");
    let custom_view = custom.observe().expect("custom observation");
    assert_view_matches(
        &custom_view,
        &custom_reference,
        &content,
        limits.default_page_size,
    );
    assert_ne!(
        preset_view.observation.state_id,
        custom_view.observation.state_id
    );
}

#[test]
fn paid_relief_route_is_kernel_bound_and_save_resume_parity_holds() {
    let content = content();
    let limits = limits();
    let service = SessionService::start(
        content.clone(),
        StartRequest::Preset {
            character_preset_id: "rook".to_owned(),
            seed: 71,
        },
        limits.clone(),
    )
    .expect("Rook service starts");
    let mut reference = Session::new_game("rook", 71, &content).expect("Rook reference");
    let initial = service.observe().expect("initial observation");
    assert_view_matches(&initial, &reference, &content, limits.default_page_size);

    let route = [
        ("travel_adjacent", Some(("destination", "lowsail.docks"))),
        ("docks.ring_warning", None),
        ("docks.rig_towline", None),
        ("levee.relay_warning", None),
        ("levee.culvert_path", None),
        ("floor.open_relief", None),
        ("travel_adjacent", Some(("destination", "red_sluice.top"))),
        ("top.divert_relief", None),
        ("world.enter_aftermath", None),
        ("return.move_inland", None),
    ];

    for (index, (definition_id, parameter)) in route.into_iter().enumerate() {
        let view = act_and_compare(
            &service,
            &mut reference,
            &content,
            &limits,
            &format!("paid-route-{index}"),
            definition_id,
            parameter,
        );
        if matches!(index, 2 | 7 | 8 | 9) {
            assert_save_checkpoint(&service, &reference, &content, &limits);
        }
        assert_eq!(view.observation.state_id, reference.state().state_id());
    }
}

#[test]
fn tide_key_split_route_is_kernel_bound_through_return_ending() {
    let content = content();
    let limits = limits();
    let service = SessionService::start(
        content.clone(),
        StartRequest::Preset {
            character_preset_id: "rook".to_owned(),
            seed: 71,
        },
        limits.clone(),
    )
    .expect("Rook service starts");
    let mut reference = Session::new_game("rook", 71, &content).expect("Rook reference");
    let initial = service.observe().expect("initial observation");
    assert_view_matches(&initial, &reference, &content, limits.default_page_size);

    let route = [
        ("travel_adjacent", Some(("destination", "lowsail.docks"))),
        ("docks.press_yara", None),
        ("docks.ask_oren", None),
        ("travel_adjacent", Some(("destination", "lowsail.levee"))),
        ("levee.culvert_path", None),
        ("floor.key_calibration", None),
        ("travel_adjacent", Some(("destination", "red_sluice.top"))),
        ("top.check_wheels", None),
        ("top.split_flow", None),
        ("world.enter_aftermath", None),
        ("return.share_water", None),
    ];

    for (index, (definition_id, parameter)) in route.into_iter().enumerate() {
        let view = act_and_compare(
            &service,
            &mut reference,
            &content,
            &limits,
            &format!("key-route-{index}"),
            definition_id,
            parameter,
        );
        if matches!(index, 1 | 5 | 8 | 9 | 10) {
            assert_save_checkpoint(&service, &reference, &content, &limits);
        }
        assert_eq!(view.observation.state_id, reference.state().state_id());
    }
}

#[test]
fn repeated_views_are_inert_and_command_retries_are_idempotent() {
    let content = content();
    let limits = limits();
    let service = SessionService::start(
        content.clone(),
        StartRequest::Preset {
            character_preset_id: "rook".to_owned(),
            seed: 71,
        },
        limits.clone(),
    )
    .expect("service starts");

    let first = service.observe().expect("first observation");
    let repeated = service.observe().expect("repeated observation");
    assert_eq!(first, repeated);
    let first_catalog = service
        .catalog(&first.observation.state_id, 0, limits.default_page_size)
        .expect("first catalog");
    let second_catalog = service
        .catalog(&first.observation.state_id, 0, limits.default_page_size)
        .expect("repeated catalog");
    assert_eq!(first_catalog, second_catalog);
    let first_save = service.save().expect("first save");
    assert_eq!(service.save().expect("repeated save"), first_save);

    let first_action = find_action(
        &service,
        &first.observation.state_id,
        &limits,
        "checkpoint.read_flag",
        None,
    );
    let first_request = action_request(&first, "same-command", &first_action);
    let first_result = service.act(first_request.clone()).expect("first command");

    let second_before = service.observe().expect("second observation");
    let second_action = find_action(
        &service,
        &second_before.observation.state_id,
        &limits,
        "wait_tide",
        None,
    );
    let second_request = action_request(&second_before, "later-command", &second_action);
    let _second_result = service.act(second_request).expect("later command");
    let after_later = service.observe().expect("observation after later command");
    let retry = service
        .act(first_request.clone())
        .expect("identical retry returns original result");
    assert_eq!(
        serde_json::to_vec(&retry).unwrap(),
        serde_json::to_vec(&first_result).unwrap()
    );
    assert_eq!(service.observe().unwrap(), after_later);
    assert_ne!(after_later.revision, first_result.revision);

    let now = service.observe().expect("current observation");
    let conflict = ActionRequest {
        command_id: first_request.command_id,
        expected_revision: now.revision,
        expected_state_id: now.observation.state_id,
        action_id: second_action.action_id,
    };
    assert_eq!(
        service.act(conflict),
        Err(ServiceError::IdempotencyConflict)
    );
}

#[test]
fn cloned_handles_share_one_session_but_separate_sessions_are_isolated() {
    let content = content();
    let limits = limits();
    let ilyan = SessionService::start(
        content.clone(),
        StartRequest::Preset {
            character_preset_id: "ilyan".to_owned(),
            seed: 71,
        },
        limits.clone(),
    )
    .expect("Ilyan starts");
    let rook = SessionService::start(
        content.clone(),
        StartRequest::Preset {
            character_preset_id: "rook".to_owned(),
            seed: 71,
        },
        limits.clone(),
    )
    .expect("Rook starts");
    let ilyan_clone = ilyan.clone();
    let rook_before = rook.observe().expect("Rook before");
    let ilyan_before = ilyan.observe().expect("Ilyan before");
    let action = find_action(
        &ilyan_clone,
        &ilyan_before.observation.state_id,
        &limits,
        "checkpoint.read_flag",
        None,
    );
    let request = action_request(&ilyan_before, "clone-command", &action);
    ilyan_clone.act(request).expect("cloned handle acts");
    assert_eq!(ilyan.observe().unwrap(), ilyan_clone.observe().unwrap());
    assert_eq!(rook.observe().unwrap(), rook_before);

    let closed_rook = rook.clone();
    closed_rook.close().expect("closing a handle");
    assert_eq!(rook.observe(), Err(ServiceError::SessionClosed));
    assert!(
        rook.save().is_ok(),
        "safe final save remains available after close"
    );
}

#[test]
fn two_concurrent_actions_from_one_view_commit_exactly_once() {
    let content = content();
    let limits = limits();
    let service = SessionService::start(
        content.clone(),
        StartRequest::Preset {
            character_preset_id: "rook".to_owned(),
            seed: 71,
        },
        limits.clone(),
    )
    .expect("service starts");
    let initial = service.observe().expect("initial observation");
    let read = find_action(
        &service,
        &initial.observation.state_id,
        &limits,
        "checkpoint.read_flag",
        None,
    );
    let ask = find_action(
        &service,
        &initial.observation.state_id,
        &limits,
        "checkpoint.ask_sava",
        None,
    );
    let read_request = action_request(&initial, "parallel-read", &read);
    let ask_request = action_request(&initial, "parallel-ask", &ask);
    let left = service.clone();
    let right = service.clone();
    let barrier = Arc::new(Barrier::new(2));
    let left_barrier = Arc::clone(&barrier);
    let right_barrier = Arc::clone(&barrier);
    let (read_result, ask_result) = std::thread::scope(|scope| {
        let read_handle = scope.spawn(move || {
            left_barrier.wait();
            left.act(read_request)
        });
        let ask_handle = scope.spawn(move || {
            right_barrier.wait();
            right.act(ask_request)
        });
        (read_handle.join().unwrap(), ask_handle.join().unwrap())
    });
    let results = [read_result, ask_result];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| **result == Err(ServiceError::StaleState))
            .count(),
        1
    );
    assert_eq!(service.observe().unwrap().revision, initial.revision + 1);

    let identical_service = SessionService::start(
        content,
        StartRequest::Preset {
            character_preset_id: "rook".to_owned(),
            seed: 71,
        },
        limits.clone(),
    )
    .expect("identical-command service starts");
    let identical_initial = identical_service.observe().expect("identical initial view");
    let identical_action = find_action(
        &identical_service,
        &identical_initial.observation.state_id,
        &limits,
        "checkpoint.read_flag",
        None,
    );
    let identical_request = action_request(&identical_initial, "same-parallel", &identical_action);
    let identical_left = identical_service.clone();
    let identical_right = identical_service.clone();
    let identical_barrier = Arc::new(Barrier::new(2));
    let identical_left_barrier = Arc::clone(&identical_barrier);
    let identical_right_barrier = Arc::clone(&identical_barrier);
    let (identical_first, identical_second) = std::thread::scope(|scope| {
        let first_request = identical_request.clone();
        let second_request = identical_request;
        let first_handle = scope.spawn(move || {
            identical_left_barrier.wait();
            identical_left.act(first_request)
        });
        let second_handle = scope.spawn(move || {
            identical_right_barrier.wait();
            identical_right.act(second_request)
        });
        (
            first_handle
                .join()
                .unwrap()
                .expect("first identical command"),
            second_handle
                .join()
                .unwrap()
                .expect("second identical command"),
        )
    });
    assert_eq!(
        serde_json::to_vec(&identical_first).unwrap(),
        serde_json::to_vec(&identical_second).unwrap()
    );
    assert_eq!(
        identical_service.observe().unwrap().revision,
        identical_initial.revision + 1
    );
    let identical_trace = PlayerTrace::from_json(&identical_service.save().unwrap())
        .expect("identical command save parses");
    assert_eq!(identical_trace.action_count(), 1);
}

fn large_catalog_content() -> Arc<CompiledContent> {
    let mut root: Value = serde_json::from_str(SPLIT_TIDE).expect("production JSON");
    let actions = root
        .get_mut("actions")
        .and_then(Value::as_array_mut)
        .expect("production actions");
    for index in 0..256 {
        let suffix = format!("{index:03}");
        let flag = format!("server_catalog.used_{suffix}");
        actions.push(json!({
            "id": format!("server_catalog.inspect_{suffix}"),
            "label": format!("Inspect {suffix}"),
            "category": "Inspect",
            "result": format!("Inspection {suffix} records a unique marker."),
            "locations": ["lowsail_market"],
            "condition": {
                "kind": "all",
                "conditions": [
                    {"kind": "not", "condition": {"kind": "world_flag", "flag": flag}},
                    {"kind": "not", "condition": {"kind": "world_flag", "flag": "sluice_outcome_chosen"}}
                ]
            },
            "effects": [{"kind": "set_world_flag", "flag": flag, "value": true}],
            "parameters": [],
            "meaningful": true,
            "movement": false
        }));
    }
    let encoded = serde_json::to_string(&root).expect("large fixture JSON");
    Arc::new(parse_and_compile_production(&encoded).expect("large fixture compiles"))
}

#[test]
fn public_catalog_pages_union_without_a_256_action_cap() {
    let content = large_catalog_content();
    let limits = limits();
    let service = SessionService::start(
        content.clone(),
        StartRequest::Preset {
            character_preset_id: "ilyan".to_owned(),
            seed: 71,
        },
        limits.clone(),
    )
    .expect("large-catalog service starts");
    let direct = Session::new_game("ilyan", 71, &content).expect("large-catalog reference");
    let initial = service.observe().expect("large-catalog observation");
    let expected = content
        .action_page(direct.state(), 0, usize::MAX)
        .expect("complete reference catalog");
    let legal = enumerate_legal_actions(direct.state(), &content)
        .expect("independent complete legal catalog");
    let legal_ids: Vec<_> = legal
        .iter()
        .map(|action| action.action_id.clone())
        .collect();
    let legal_digest = legal_action_digest(&legal).expect("independent legal digest");
    assert!(expected.total > 256);
    assert_eq!(expected.total, legal.len());
    assert_eq!(initial.catalog.total, expected.total);
    assert_eq!(initial.catalog.digest, expected.digest);
    assert_eq!(expected.digest, legal_digest);

    let mut offset = 0;
    let mut actions = Vec::new();
    loop {
        let page = service
            .catalog(&initial.observation.state_id, offset, 64)
            .expect("large-catalog page");
        assert_eq!(page.total, expected.total);
        assert_eq!(page.digest, expected.digest);
        assert_eq!(page.state_id, expected.state_id);
        actions.extend(page.actions);
        match page.next_offset {
            Some(next) => offset = next,
            None => break,
        }
    }
    assert_eq!(actions, expected.actions);
    let paged_ids: Vec<_> = actions
        .iter()
        .map(|action| action.action_id.clone())
        .collect();
    assert_eq!(paged_ids, legal_ids);
    assert_eq!(actions.len(), expected.total);
    assert!(
        actions
            .iter()
            .filter(|action| action.definition_id.starts_with("server_catalog.inspect_"))
            .count()
            >= 256
    );
}
