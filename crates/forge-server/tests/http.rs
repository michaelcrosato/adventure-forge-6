use axum::{
    Router,
    body::{Body, to_bytes},
    http::{HeaderMap, HeaderName, HeaderValue, Method, Request, StatusCode, header},
};
use forge_content::parse_and_compile_production;
use forge_kernel::{
    CharacterChoiceSelection, CharacterSelection, CompiledContent, enumerate_legal_actions,
};
use forge_replay::{PlayerTrace, Session, resume_player_trace};
use forge_server::{
    SessionView,
    http::{HTTP_RESUME_BODY_BYTES, HTTP_SAVE_BYTES, router},
};
use serde::Serialize;
use serde_json::{Value, json};
use std::sync::Arc;
use tower::ServiceExt;

const CONTENT: &str = include_str!("../../../content/split-tide.json");
const PORT: u16 = 38_123;
const HOST: &str = "127.0.0.1:38123";
const ORIGIN: &str = "http://127.0.0.1:38123";

fn content() -> Arc<CompiledContent> {
    Arc::new(parse_and_compile_production(CONTENT).expect("production content compiles"))
}

fn app() -> Router {
    router(content(), PORT).expect("HTTP router builds")
}

async fn request(
    app: &Router,
    method: Method,
    path: &str,
    token: Option<&str>,
    origin: Option<&str>,
    body: Vec<u8>,
) -> (StatusCode, HeaderMap, Vec<u8>) {
    let mut headers = HeaderMap::new();
    headers.insert(header::HOST, HeaderValue::from_static(HOST));
    headers.insert(
        HeaderName::from_static("sec-fetch-site"),
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(
        HeaderName::from_static("sec-fetch-dest"),
        HeaderValue::from_static("empty"),
    );
    if let Some(token) = token {
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}"))
                .expect("bearer header is valid ASCII"),
        );
    }
    if let Some(origin) = origin {
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_str(origin).expect("origin header is valid ASCII"),
        );
    }
    if method == Method::POST {
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
    }
    request_with_headers(app, method, path, headers, body).await
}

async fn request_with_headers(
    app: &Router,
    method: Method,
    path: &str,
    headers: HeaderMap,
    body: Vec<u8>,
) -> (StatusCode, HeaderMap, Vec<u8>) {
    let mut request = Request::new(Body::from(body));
    *request.method_mut() = method;
    *request.uri_mut() = path.parse().expect("request URI");
    *request.headers_mut() = headers;
    let response = app.clone().oneshot(request).await.expect("router response");
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(
        response.into_body(),
        HTTP_RESUME_BODY_BYTES + HTTP_SAVE_BYTES,
    )
    .await
    .expect("bounded response body");
    (status, headers, bytes.to_vec())
}

fn api_headers(token: &str, origin: Option<&str>) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::HOST, HeaderValue::from_static(HOST));
    headers.insert(
        HeaderName::from_static("sec-fetch-site"),
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(
        HeaderName::from_static("sec-fetch-dest"),
        HeaderValue::from_static("empty"),
    );
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {token}")).expect("bearer header is valid ASCII"),
    );
    if let Some(origin) = origin {
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_str(origin).expect("origin header is valid ASCII"),
        );
    }
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    headers
}

async fn bootstrap(app: &Router) -> String {
    let (status, headers, body) =
        request(app, Method::GET, "/api/bootstrap", None, None, Vec::new()).await;
    assert_eq!(status, StatusCode::OK);
    assert_secure_no_cors(&headers);
    let value: Value = serde_json::from_slice(&body).expect("bootstrap JSON");
    value["token"].as_str().expect("bootstrap token").to_owned()
}

fn json_body(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).expect("JSON request")
}

fn parse_json(body: &[u8]) -> Value {
    serde_json::from_slice(body).expect("JSON response")
}

fn view(value: &Value) -> &Value {
    value.get("view").unwrap_or(value)
}

fn session_id(value: &Value) -> String {
    value["session_id"]
        .as_str()
        .expect("session response ID")
        .to_owned()
}

fn error_code(body: &[u8], expected: &str) {
    let value = parse_json(body);
    assert_eq!(value, json!({"error": expected}));
}

fn assert_secure_no_cors(headers: &HeaderMap) {
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(
        headers[header::CONTENT_SECURITY_POLICY],
        "default-src 'none'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'"
    );
    assert!(!headers.contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN));
    assert!(!headers.contains_key(header::ACCESS_CONTROL_ALLOW_CREDENTIALS));
}

fn pad_to(mut bytes: Vec<u8>, target: usize) -> Vec<u8> {
    assert!(
        bytes.len() <= target,
        "test fixture exceeds target boundary"
    );
    bytes.resize(target, b' ');
    bytes
}

fn assert_wire_numbers_are_strings(value: &Value) {
    match value {
        Value::Number(number) => panic!("HTTP public number was not stringified: {number}"),
        Value::Array(values) => values.iter().for_each(assert_wire_numbers_are_strings),
        Value::Object(values) => values.values().for_each(assert_wire_numbers_are_strings),
        Value::Null | Value::Bool(_) | Value::String(_) => {}
    }
}

fn wire_value<T: Serialize>(value: &T) -> Value {
    fn stringify(value: &mut Value) {
        match value {
            Value::Number(number) => *value = Value::String(number.to_string()),
            Value::Array(values) => values.iter_mut().for_each(stringify),
            Value::Object(values) => values.values_mut().for_each(stringify),
            Value::Null | Value::Bool(_) | Value::String(_) => {}
        }
    }
    let mut value = serde_json::to_value(value).expect("canonical value serializes");
    stringify(&mut value);
    value
}

fn expected_view(session: &Session<'_>, content: &CompiledContent) -> SessionView {
    SessionView {
        revision: u64::try_from(session.trace().steps.len()).expect("reference revision"),
        observation: session
            .trace()
            .steps
            .last()
            .map(|step| step.observation.clone())
            .unwrap_or_else(|| session.trace().initial_observation.clone()),
        catalog: content
            .action_page(session.state(), 0, 32)
            .expect("reference page"),
    }
}

fn custom_selection(content: &CompiledContent) -> CharacterSelection {
    let creation = content.character_creation().expect("custom creation");
    CharacterSelection {
        name: "HTTP Custom".to_owned(),
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

fn canonical_action(
    session: &Session<'_>,
    content: &CompiledContent,
    id: &str,
) -> forge_kernel::CanonicalAction {
    enumerate_legal_actions(session.state(), content)
        .expect("canonical catalog")
        .into_iter()
        .find(|action| action.action_id == id)
        .expect("HTTP action is in canonical catalog")
}

fn action_ids(page: &Value) -> Vec<String> {
    page["actions"]
        .as_array()
        .expect("catalog actions")
        .iter()
        .map(|action| action["action_id"].as_str().expect("action ID").to_owned())
        .collect()
}

#[tokio::test]
async fn preset_and_custom_max_seed_match_canonical_public_views() {
    for (creation_id, start, reference) in [
        (
            "http-preset-max",
            json!({
                "kind": "preset",
                "character_preset_id": "rook",
                "seed": "18446744073709551615"
            }),
            "preset",
        ),
        (
            "http-custom-max",
            json!({
                "kind": "custom",
                "selection": serde_json::to_value(custom_selection(&content())).expect("selection"),
                "seed": "18446744073709551615"
            }),
            "custom",
        ),
    ] {
        let compiled = content();
        let app = router(compiled.clone(), PORT).expect("HTTP router builds");
        let token = bootstrap(&app).await;
        let body = json_body(&json!({"creation_id": creation_id, "start": start}));
        let (status, _, response_body) = request(
            &app,
            Method::POST,
            "/api/sessions",
            Some(&token),
            Some(ORIGIN),
            body,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let response = parse_json(&response_body);
        assert_wire_numbers_are_strings(&response);
        let reference_session = if reference == "preset" {
            Session::new_game("rook", u64::MAX, &compiled).expect("preset reference")
        } else {
            let selection = custom_selection(&compiled);
            Session::new_custom_game(&selection, u64::MAX, &compiled).expect("custom reference")
        };
        let expected = wire_value(&expected_view(&reference_session, &compiled));
        assert_eq!(view(&response), &expected);
        let id = session_id(&response);
        assert_eq!(id.len(), 64);
        assert!(
            id.bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        );

        let invalid_seed = json!({
            "creation_id": "invalid-seed",
            "start": {"kind": "preset", "character_preset_id": "rook", "seed": 71}
        });
        let (status, _, body) = request(
            &app,
            Method::POST,
            "/api/sessions",
            Some(&token),
            Some(ORIGIN),
            json_body(&invalid_seed),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        error_code(&body, "invalid_input");

        let (status, _, observed_body) = request(
            &app,
            Method::GET,
            &format!("/api/sessions/{id}"),
            Some(&token),
            None,
            Vec::new(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(view(&parse_json(&observed_body)), view(&response));

        let (status, save_headers, save_body) = request(
            &app,
            Method::GET,
            &format!("/api/sessions/{id}/save"),
            Some(&token),
            None,
            Vec::new(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(save_headers[header::CONTENT_TYPE], "application/json");
        let trace = PlayerTrace::from_json(std::str::from_utf8(&save_body).unwrap())
            .expect("max-seed save parses");
        let replayed = resume_player_trace(&trace, &compiled).expect("max-seed save replays");
        assert_eq!(replayed.state(), reference_session.state());
        assert_eq!(
            replayed.trace().final_state_id,
            reference_session.trace().final_state_id
        );
        assert_eq!(
            replayed.trace().final_receipt,
            reference_session.trace().final_receipt
        );
    }
}

#[tokio::test]
async fn http_catalog_pages_are_complete_and_numeric_requests_are_strict() {
    let compiled = content();
    let app = router(compiled.clone(), PORT).expect("HTTP router builds");
    let token = bootstrap(&app).await;
    let (status, _, body) = request(
        &app,
        Method::POST,
        "/api/sessions",
        Some(&token),
        Some(ORIGIN),
        json_body(&json!({
            "creation_id": "http-catalog",
            "start": {"kind": "preset", "character_preset_id": "rook", "seed": "71"}
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let created = parse_json(&body);
    let id = session_id(&created);
    let state_id = view(&created)["observation"]["state_id"]
        .as_str()
        .expect("state ID")
        .to_owned();

    let mut actual_ids = Vec::new();
    let mut offset = 0usize;
    let mut total = None;
    let mut digest = None;
    loop {
        let (status, _, page_body) = request(
            &app,
            Method::POST,
            &format!("/api/sessions/{id}/catalog"),
            Some(&token),
            Some(ORIGIN),
            json_body(&json!({
                "expected_state_id": state_id,
                "offset": offset.to_string(),
                "page_size": "1"
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let page = parse_json(&page_body);
        assert_wire_numbers_are_strings(&page);
        assert_eq!(page["state_id"].as_str(), Some(state_id.as_str()));
        total.get_or_insert(
            page["total"]
                .as_str()
                .expect("total")
                .parse::<usize>()
                .unwrap(),
        );
        digest.get_or_insert(page["digest"].as_str().expect("digest").to_owned());
        actual_ids.extend(action_ids(&page));
        match page["next_offset"].as_str() {
            Some(next) => offset = next.parse().expect("next offset"),
            None => break,
        }
    }

    let reference = Session::new_game("rook", 71, &compiled).expect("reference session");
    let legal = enumerate_legal_actions(reference.state(), &compiled).expect("reference actions");
    let expected_ids: Vec<_> = legal
        .iter()
        .map(|action| action.action_id.clone())
        .collect();
    assert_eq!(actual_ids, expected_ids);
    assert_eq!(total, Some(expected_ids.len()));
    assert_eq!(
        digest,
        Some(forge_kernel::legal_action_digest(&legal).unwrap())
    );

    let (status, _, body) = request(
        &app,
        Method::POST,
        &format!("/api/sessions/{id}/catalog"),
        Some(&token),
        Some(ORIGIN),
        json_body(&json!({
            "expected_state_id": state_id,
            "offset": 0,
            "page_size": "1"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    error_code(&body, "invalid_input");

    let (status, _, body) = request(
        &app,
        Method::POST,
        &format!("/api/sessions/{id}/catalog"),
        Some(&token),
        Some(ORIGIN),
        json_body(&json!({
            "expected_state_id": "0".repeat(64),
            "offset": "0",
            "page_size": "1"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    error_code(&body, "stale_state");
}

#[tokio::test]
async fn http_security_attack_matrix_is_inert_and_responses_are_private() {
    let app = app();

    let (status, headers, root_body) =
        request(&app, Method::GET, "/", None, None, Vec::new()).await;
    assert_eq!(status, StatusCode::OK);
    assert_secure_no_cors(&headers);
    let root_text = String::from_utf8(root_body).expect("root HTML");
    assert!(!root_text.contains("token"));

    let token = bootstrap(&app).await;
    let mut bootstrap_base = HeaderMap::new();
    bootstrap_base.insert(header::HOST, HeaderValue::from_static(HOST));
    bootstrap_base.insert(
        HeaderName::from_static("sec-fetch-site"),
        HeaderValue::from_static("same-origin"),
    );
    bootstrap_base.insert(
        HeaderName::from_static("sec-fetch-dest"),
        HeaderValue::from_static("empty"),
    );
    let mut bootstrap_cases = Vec::new();
    let mut case = bootstrap_base.clone();
    case.remove("sec-fetch-site");
    bootstrap_cases.push((
        "bootstrap-missing-site",
        StatusCode::FORBIDDEN,
        "forbidden",
        case,
    ));
    let mut case = bootstrap_base.clone();
    case.insert(
        HeaderName::from_static("sec-fetch-site"),
        HeaderValue::from_static("cross-site"),
    );
    bootstrap_cases.push((
        "bootstrap-cross-site",
        StatusCode::FORBIDDEN,
        "forbidden",
        case,
    ));
    let mut case = bootstrap_base.clone();
    case.insert(
        HeaderName::from_static("sec-fetch-dest"),
        HeaderValue::from_static("script"),
    );
    bootstrap_cases.push(("bootstrap-script", StatusCode::FORBIDDEN, "forbidden", case));
    let mut case = bootstrap_base.clone();
    case.insert(
        HeaderName::from_static("sec-fetch-dest"),
        HeaderValue::from_static("document"),
    );
    bootstrap_cases.push((
        "bootstrap-document",
        StatusCode::FORBIDDEN,
        "forbidden",
        case,
    ));
    let mut case = bootstrap_base.clone();
    case.insert(
        header::AUTHORIZATION,
        HeaderValue::from_static("Bearer unexpected"),
    );
    bootstrap_cases.push((
        "bootstrap-auth",
        StatusCode::UNAUTHORIZED,
        "unauthorized",
        case,
    ));
    for (label, expected_status, expected_error, headers) in bootstrap_cases {
        let (status, response_headers, body) =
            request_with_headers(&app, Method::GET, "/api/bootstrap", headers, Vec::new()).await;
        assert_eq!(status, expected_status, "{label}");
        assert_secure_no_cors(&response_headers);
        error_code(&body, expected_error);
        assert!(!String::from_utf8_lossy(&body).contains(&token));
    }

    let (status, headers, created_body) = request(
        &app,
        Method::POST,
        "/api/sessions",
        Some(&token),
        Some(ORIGIN),
        json_body(&json!({
            "creation_id": "http-security",
            "start": {"kind": "preset", "character_preset_id": "rook", "seed": "71"}
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_secure_no_cors(&headers);
    let created = parse_json(&created_body);
    let id = session_id(&created);
    let initial_view = view(&created).clone();
    let action_id = initial_view["catalog"]["actions"][0]["action_id"]
        .as_str()
        .expect("current action")
        .to_owned();
    let state_id = initial_view["observation"]["state_id"]
        .as_str()
        .expect("current state")
        .to_owned();
    let action_body = json_body(&json!({
        "command_id": "security-valid-action",
        "expected_revision": "0",
        "expected_state_id": state_id,
        "action_id": action_id
    }));
    let (status, headers, baseline_view_body) = request(
        &app,
        Method::GET,
        &format!("/api/sessions/{id}"),
        Some(&token),
        None,
        Vec::new(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_secure_no_cors(&headers);
    assert_eq!(view(&parse_json(&baseline_view_body)), &initial_view);

    let (status, headers, baseline_save_body) = request(
        &app,
        Method::GET,
        &format!("/api/sessions/{id}/save"),
        Some(&token),
        None,
        Vec::new(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_secure_no_cors(&headers);
    assert_eq!(headers[header::CONTENT_TYPE], "application/json");

    let mut attacks: Vec<(&str, StatusCode, HeaderMap, String)> = Vec::new();
    let mut attack = api_headers(&token, Some(ORIGIN));
    attack.remove(header::HOST);
    attacks.push((
        "missing-host",
        StatusCode::FORBIDDEN,
        attack,
        "/api/sessions/".to_owned() + &id + "/actions",
    ));
    let mut attack = api_headers(&token, Some(ORIGIN));
    attack.insert(header::HOST, HeaderValue::from_static("localhost:38123"));
    attacks.push((
        "wrong-host",
        StatusCode::FORBIDDEN,
        attack,
        format!("/api/sessions/{id}/actions"),
    ));
    let mut attack = api_headers(&token, Some(ORIGIN));
    attack.append(header::HOST, HeaderValue::from_static(HOST));
    attacks.push((
        "duplicate-host",
        StatusCode::FORBIDDEN,
        attack,
        format!("/api/sessions/{id}/actions"),
    ));
    let attack = api_headers(&token, None);
    attacks.push((
        "missing-origin",
        StatusCode::FORBIDDEN,
        attack,
        format!("/api/sessions/{id}/actions"),
    ));
    let attack = api_headers(&token, Some("http://evil.test"));
    attacks.push((
        "wrong-origin",
        StatusCode::FORBIDDEN,
        attack,
        format!("/api/sessions/{id}/actions"),
    ));
    let mut attack = api_headers(&token, Some(ORIGIN));
    attack.remove(header::AUTHORIZATION);
    attacks.push((
        "missing-bearer",
        StatusCode::UNAUTHORIZED,
        attack,
        format!("/api/sessions/{id}/actions"),
    ));
    let mut attack = api_headers(&token, Some(ORIGIN));
    attack.insert(
        header::AUTHORIZATION,
        HeaderValue::from_static("Bearer wrong"),
    );
    attacks.push((
        "wrong-bearer",
        StatusCode::UNAUTHORIZED,
        attack,
        format!("/api/sessions/{id}/actions"),
    ));
    let mut attack = api_headers(&token, Some(ORIGIN));
    attack.append(
        header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {token}")).expect("bearer header"),
    );
    attacks.push((
        "duplicate-bearer",
        StatusCode::UNAUTHORIZED,
        attack,
        format!("/api/sessions/{id}/actions"),
    ));
    let mut attack = api_headers(&token, Some(ORIGIN));
    attack.insert("sec-fetch-site", HeaderValue::from_static("cross-site"));
    attacks.push((
        "cross-site",
        StatusCode::FORBIDDEN,
        attack,
        format!("/api/sessions/{id}/actions"),
    ));
    let mut attack = api_headers(&token, Some(ORIGIN));
    attack.insert("sec-fetch-site", HeaderValue::from_static("same-site"));
    attacks.push((
        "same-site",
        StatusCode::FORBIDDEN,
        attack,
        format!("/api/sessions/{id}/actions"),
    ));
    let mut attack = api_headers(&token, Some(ORIGIN));
    attack.remove("sec-fetch-site");
    attacks.push((
        "missing-fetch-site",
        StatusCode::FORBIDDEN,
        attack,
        format!("/api/sessions/{id}/actions"),
    ));
    let mut attack = api_headers(&token, Some(ORIGIN));
    attack.insert("sec-fetch-dest", HeaderValue::from_static("script"));
    attacks.push((
        "script-dest",
        StatusCode::FORBIDDEN,
        attack,
        format!("/api/sessions/{id}/actions"),
    ));
    let mut attack = api_headers(&token, Some(ORIGIN));
    attack.insert("sec-fetch-dest", HeaderValue::from_static("document"));
    attacks.push((
        "document-dest",
        StatusCode::FORBIDDEN,
        attack,
        format!("/api/sessions/{id}/actions"),
    ));
    let mut attack = api_headers(&token, Some(ORIGIN));
    attack.remove("sec-fetch-dest");
    attacks.push((
        "missing-fetch-dest",
        StatusCode::FORBIDDEN,
        attack,
        format!("/api/sessions/{id}/actions"),
    ));
    let mut attack = api_headers(&token, Some(ORIGIN));
    attack.insert("x-forwarded-host", HeaderValue::from_static("evil.test"));
    attacks.push((
        "x-forwarded",
        StatusCode::FORBIDDEN,
        attack,
        format!("/api/sessions/{id}/actions"),
    ));
    let mut attack = api_headers(&token, Some(ORIGIN));
    attack.insert("forwarded", HeaderValue::from_static("for=evil"));
    attacks.push((
        "forwarded",
        StatusCode::FORBIDDEN,
        attack,
        format!("/api/sessions/{id}/actions"),
    ));
    attacks.push((
        "query",
        StatusCode::FORBIDDEN,
        api_headers(&token, Some(ORIGIN)),
        format!("/api/sessions/{id}/actions?x=1"),
    ));
    let mut attack = api_headers(&token, Some(ORIGIN));
    attack.insert(header::CONTENT_TYPE, HeaderValue::from_static("text/plain"));
    attacks.push((
        "text-plain",
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        attack,
        format!("/api/sessions/{id}/actions"),
    ));
    let mut attack = api_headers(&token, Some(ORIGIN));
    attack.remove(header::CONTENT_TYPE);
    attacks.push((
        "missing-content-type",
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        attack,
        format!("/api/sessions/{id}/actions"),
    ));
    let mut attack = api_headers(&token, Some(ORIGIN));
    attack.append(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    attacks.push((
        "duplicate-content-type",
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        attack,
        format!("/api/sessions/{id}/actions"),
    ));
    let mut attack = api_headers(&token, Some(ORIGIN));
    attack.insert(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
    attacks.push((
        "compressed-body",
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        attack,
        format!("/api/sessions/{id}/actions"),
    ));

    for (label, expected_status, headers, path) in attacks {
        let (status, response_headers, body) =
            request_with_headers(&app, Method::POST, &path, headers, action_body.clone()).await;
        assert_eq!(status, expected_status, "{label}");
        assert_secure_no_cors(&response_headers);
        let expected_error = match expected_status {
            StatusCode::UNAUTHORIZED => "unauthorized",
            StatusCode::UNSUPPORTED_MEDIA_TYPE => "invalid_content_type",
            _ => "forbidden",
        };
        error_code(&body, expected_error);
        assert!(!String::from_utf8_lossy(&body).contains(&token));

        let (status, _, view_body) = request(
            &app,
            Method::GET,
            &format!("/api/sessions/{id}"),
            Some(&token),
            None,
            Vec::new(),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{label} changed session view");
        assert_eq!(
            view_body, baseline_view_body,
            "{label} changed session view"
        );
        let (status, _, save_body) = request(
            &app,
            Method::GET,
            &format!("/api/sessions/{id}/save"),
            Some(&token),
            None,
            Vec::new(),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{label} changed save status");
        assert_eq!(save_body, baseline_save_body, "{label} changed save");
    }

    let (status, response_headers, valid_body) = request(
        &app,
        Method::POST,
        &format!("/api/sessions/{id}/actions"),
        Some(&token),
        Some(ORIGIN),
        action_body,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_secure_no_cors(&response_headers);
    let valid_response = parse_json(&valid_body);
    let valid_view = view(&valid_response);
    assert_eq!(valid_view["revision"], "1");
    assert_ne!(
        valid_view["observation"]["state_id"],
        initial_view["observation"]["state_id"]
    );
}

#[tokio::test]
async fn http_action_retry_conflict_stale_and_canonical_save_are_atomic() {
    let compiled = content();
    let app = app();
    let token = bootstrap(&app).await;
    let (status, _, body) = request(
        &app,
        Method::POST,
        "/api/sessions",
        Some(&token),
        Some(ORIGIN),
        json_body(&json!({
            "creation_id": "http-actions",
            "start": {"kind": "preset", "character_preset_id": "rook", "seed": "71"}
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let created = parse_json(&body);
    let id = session_id(&created);
    let initial = view(&created).clone();
    let state_id = initial["observation"]["state_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let action_id = initial["catalog"]["actions"][0]["action_id"]
        .as_str()
        .expect("first action")
        .to_owned();

    let duplicate_action = format!(
        "{{\"command_id\":\"duplicate\",\"expected_revision\":\"0\",\"expected_state_id\":\"{state_id}\",\"action_id\":\"{action_id}\",\"action_id\":\"{action_id}\"}}"
    );
    let (status, _, body) = request(
        &app,
        Method::POST,
        &format!("/api/sessions/{id}/actions"),
        Some(&token),
        Some(ORIGIN),
        duplicate_action.into_bytes(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    error_code(&body, "invalid_input");

    let mut unknown_action = json!({
        "command_id": "unknown-field",
        "expected_revision": "0",
        "expected_state_id": state_id,
        "action_id": action_id
    });
    unknown_action["unexpected"] = json!(true);
    let (status, _, body) = request(
        &app,
        Method::POST,
        &format!("/api/sessions/{id}/actions"),
        Some(&token),
        Some(ORIGIN),
        json_body(&unknown_action),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    error_code(&body, "invalid_input");

    let numeric_revision = json!({
        "command_id": "numeric-revision",
        "expected_revision": 0,
        "expected_state_id": state_id,
        "action_id": action_id
    });
    let (status, _, body) = request(
        &app,
        Method::POST,
        &format!("/api/sessions/{id}/actions"),
        Some(&token),
        Some(ORIGIN),
        json_body(&numeric_revision),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    error_code(&body, "invalid_input");

    let (status, _, body) = request(
        &app,
        Method::GET,
        &format!("/api/sessions/{id}"),
        Some(&token),
        None,
        Vec::new(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(view(&parse_json(&body)), &initial);

    let mut reference = Session::new_game("rook", 71, &compiled).expect("reference session");
    let canonical = canonical_action(&reference, &compiled, &action_id);
    let action_request = json!({
        "command_id": "http-command-1",
        "expected_revision": "0",
        "expected_state_id": state_id,
        "action_id": action_id
    });
    let (status, _, accepted_body) = request(
        &app,
        Method::POST,
        &format!("/api/sessions/{id}/actions"),
        Some(&token),
        Some(ORIGIN),
        json_body(&action_request),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    reference.record(&canonical).expect("reference action");
    let expected = wire_value(&expected_view(&reference, &compiled));
    assert_eq!(view(&parse_json(&accepted_body)), &expected);

    let (status, _, retry_body) = request(
        &app,
        Method::POST,
        &format!("/api/sessions/{id}/actions"),
        Some(&token),
        Some(ORIGIN),
        json_body(&action_request),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(retry_body, accepted_body);

    let current = parse_json(&accepted_body);
    let current_view = view(&current);
    let second_action_id = current_view["catalog"]["actions"][0]["action_id"]
        .as_str()
        .expect("second action")
        .to_owned();
    let changed = json!({
        "command_id": "http-command-1",
        "expected_revision": "1",
        "expected_state_id": current_view["observation"]["state_id"],
        "action_id": second_action_id
    });
    let (status, _, body) = request(
        &app,
        Method::POST,
        &format!("/api/sessions/{id}/actions"),
        Some(&token),
        Some(ORIGIN),
        json_body(&changed),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    error_code(&body, "idempotency_conflict");

    let stale = json!({
        "command_id": "http-command-stale",
        "expected_revision": "0",
        "expected_state_id": state_id,
        "action_id": second_action_id
    });
    let (status, _, body) = request(
        &app,
        Method::POST,
        &format!("/api/sessions/{id}/actions"),
        Some(&token),
        Some(ORIGIN),
        json_body(&stale),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    error_code(&body, "stale_state");

    let (status, _, body) = request(
        &app,
        Method::POST,
        &format!("/api/sessions/{id}/actions"),
        Some(&token),
        Some(ORIGIN),
        json_body(&json!({
            "command_id": "http-command-invalid",
            "expected_revision": "1",
            "expected_state_id": current_view["observation"]["state_id"],
            "action_id": "f".repeat(64)
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    error_code(&body, "invalid_action");

    let (status, save_headers, save_body) = request(
        &app,
        Method::GET,
        &format!("/api/sessions/{id}/save"),
        Some(&token),
        None,
        Vec::new(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(save_headers[header::CONTENT_TYPE], "application/json");
    assert!(save_body.len() <= HTTP_SAVE_BYTES);
    let player_trace = PlayerTrace::from_json(std::str::from_utf8(&save_body).unwrap())
        .expect("HTTP save is a player trace");
    let reference_save = reference
        .player_trace()
        .expect("reference save")
        .to_json()
        .unwrap();
    assert_eq!(std::str::from_utf8(&save_body).unwrap(), reference_save);
    assert_eq!(player_trace.action_count(), 1);
}

#[tokio::test]
async fn http_resume_close_and_strict_wire_limits_preserve_state() {
    let compiled = content();
    let app = router(compiled.clone(), PORT).expect("HTTP router builds");
    let token = bootstrap(&app).await;
    let start = json!({
        "creation_id": "http-close",
        "start": {"kind": "preset", "character_preset_id": "rook", "seed": "71"}
    });
    let (status, _, body) = request(
        &app,
        Method::POST,
        "/api/sessions",
        Some(&token),
        Some(ORIGIN),
        json_body(&start),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let created = parse_json(&body);
    let id = session_id(&created);
    let initial_view = view(&created).clone();

    let (status, _, save_body) = request(
        &app,
        Method::GET,
        &format!("/api/sessions/{id}/save"),
        Some(&token),
        None,
        Vec::new(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let save = String::from_utf8(save_body.clone()).expect("save UTF-8");
    let trace = PlayerTrace::from_json(&save).expect("save parses");
    let mut reference = resume_player_trace(&trace, &compiled).expect("reference save replays");
    assert_eq!(
        wire_value(&expected_view(&reference, &compiled)),
        initial_view
    );

    let (status, _, body) = request(
        &app,
        Method::POST,
        &format!("/api/sessions/{id}/close"),
        Some(&token),
        Some(ORIGIN),
        json_body(&json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(parse_json(&body), json!({"closed": true}));

    let (status, _, body) = request(
        &app,
        Method::GET,
        &format!("/api/sessions/{id}"),
        Some(&token),
        None,
        Vec::new(),
    )
    .await;
    assert_eq!(status, StatusCode::GONE);
    error_code(&body, "session_closed");

    let (status, _, closed_save) = request(
        &app,
        Method::GET,
        &format!("/api/sessions/{id}/save"),
        Some(&token),
        None,
        Vec::new(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(closed_save, save_body);

    let resume_body = json_body(&json!({"creation_id": "http-resume", "save_json": save}));
    let (status, _, body) = request(
        &app,
        Method::POST,
        "/api/resume",
        Some(&token),
        Some(ORIGIN),
        resume_body,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resumed = parse_json(&body);
    let resumed_id = session_id(&resumed);
    assert_eq!(view(&resumed), &initial_view);

    let resumed_action = view(&resumed)["catalog"]["actions"][0]["action_id"]
        .as_str()
        .expect("resumed action")
        .to_owned();
    let resumed_request = json!({
        "command_id": "http-resumed-action",
        "expected_revision": "0",
        "expected_state_id": view(&resumed)["observation"]["state_id"],
        "action_id": resumed_action
    });
    let canonical = canonical_action(
        &reference,
        &compiled,
        resumed_request["action_id"].as_str().expect("action ID"),
    );
    let (status, _, body) = request(
        &app,
        Method::POST,
        &format!("/api/sessions/{resumed_id}/actions"),
        Some(&token),
        Some(ORIGIN),
        json_body(&resumed_request),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    reference
        .record(&canonical)
        .expect("resumed reference action");
    assert_eq!(
        view(&parse_json(&body)),
        &wire_value(&expected_view(&reference, &compiled))
    );

    let too_large_start = {
        let mut body = json_body(&start);
        body.resize(128 * 1024 + 1, b' ');
        body
    };
    let (status, _, body) = request(
        &app,
        Method::POST,
        "/api/sessions",
        Some(&token),
        Some(ORIGIN),
        too_large_start,
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    error_code(&body, "resource_limit");

    let too_large_resume = format!(
        "{{\"creation_id\":\"oversized\",\"save_json\":\"{}\"}}",
        "x".repeat(HTTP_RESUME_BODY_BYTES)
    )
    .into_bytes();
    let (status, _, body) = request(
        &app,
        Method::POST,
        "/api/resume",
        Some(&token),
        Some(ORIGIN),
        too_large_resume,
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    error_code(&body, "resource_limit");

    let (status, _, body) = request(
        &app,
        Method::POST,
        "/api/resume",
        Some(&token),
        Some(ORIGIN),
        br#"{"creation_id":"bad","save_json":"{}","save_json":"{}"}"#.to_vec(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    error_code(&body, "invalid_input");

    let (status, _, body) = request(
        &app,
        Method::POST,
        &format!("/api/sessions/{resumed_id}/close"),
        Some(&token),
        Some(ORIGIN),
        json_body(&json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(parse_json(&body), json!({"closed": true}));

    let (status, _, final_save) = request(
        &app,
        Method::GET,
        &format!("/api/sessions/{resumed_id}/save"),
        Some(&token),
        None,
        Vec::new(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        String::from_utf8(final_save).expect("final save UTF-8"),
        reference
            .player_trace()
            .expect("final reference save")
            .to_json()
            .expect("final reference JSON")
    );
}

#[tokio::test]
async fn http_public_responses_and_save_omit_hidden_state() {
    let app = app();
    let token = bootstrap(&app).await;
    let (status, _, options_body) = request(
        &app,
        Method::GET,
        "/api/options",
        Some(&token),
        None,
        Vec::new(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let options = parse_json(&options_body);
    assert_wire_numbers_are_strings(&options);
    let options_text = String::from_utf8(options_body).unwrap();
    for forbidden in [
        "\"character\":",
        "\"patch\":",
        "\"inventory\":",
        "\"knowledge\":",
    ] {
        assert!(
            !options_text.contains(forbidden),
            "options leaked {forbidden}"
        );
    }

    let (status, _, body) = request(
        &app,
        Method::POST,
        "/api/sessions",
        Some(&token),
        Some(ORIGIN),
        json_body(&json!({
            "creation_id": "http-privacy",
            "start": {"kind": "preset", "character_preset_id": "rook", "seed": "71"}
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let created = parse_json(&body);
    assert_wire_numbers_are_strings(&created);
    let id = session_id(&created);
    let view_text = serde_json::to_string(view(&created)).expect("view JSON");
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
        assert!(!view_text.contains(forbidden), "view leaked {forbidden}");
    }
    let (status, _, save_body) = request(
        &app,
        Method::GET,
        &format!("/api/sessions/{id}/save"),
        Some(&token),
        None,
        Vec::new(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let save_text = String::from_utf8(save_body).unwrap();
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
        assert!(!save_text.contains(forbidden), "save leaked {forbidden}");
    }
}

#[tokio::test]
async fn http_exact_request_and_save_boundaries_preserve_retries() {
    let app = app();
    let token = bootstrap(&app).await;
    let start = json!({
        "creation_id": "boundary-start",
        "start": {"kind": "preset", "character_preset_id": "rook", "seed": "71"}
    });

    let exact_start = pad_to(json_body(&start), forge_server::MAX_REQUEST_BYTES);
    assert_eq!(exact_start.len(), forge_server::MAX_REQUEST_BYTES);
    let (status, _, first_body) = request(
        &app,
        Method::POST,
        "/api/sessions",
        Some(&token),
        Some(ORIGIN),
        exact_start.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let first = parse_json(&first_body);
    let first_id = session_id(&first);

    let (status, _, retry_body) = request(
        &app,
        Method::POST,
        "/api/sessions",
        Some(&token),
        Some(ORIGIN),
        exact_start.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(retry_body, first_body);

    let mut over_start = exact_start;
    over_start.push(b' ');
    let (status, headers, body) = request(
        &app,
        Method::POST,
        "/api/sessions",
        Some(&token),
        Some(ORIGIN),
        over_start,
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_secure_no_cors(&headers);
    error_code(&body, "resource_limit");

    let (status, _, retry_after_oversize) = request(
        &app,
        Method::POST,
        "/api/sessions",
        Some(&token),
        Some(ORIGIN),
        json_body(&start),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(retry_after_oversize, first_body);

    let (status, _, save_body) = request(
        &app,
        Method::GET,
        &format!("/api/sessions/{first_id}/save"),
        Some(&token),
        None,
        Vec::new(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let save = String::from_utf8(save_body.clone()).expect("save UTF-8");

    let (status, _, body) = request(
        &app,
        Method::POST,
        &format!("/api/sessions/{first_id}/close"),
        Some(&token),
        Some(ORIGIN),
        json_body(&json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(parse_json(&body), json!({"closed": true}));

    let exact_resume = pad_to(
        json_body(&json!({
            "creation_id": "boundary-envelope",
            "save_json": save
        })),
        HTTP_RESUME_BODY_BYTES,
    );
    assert_eq!(exact_resume.len(), HTTP_RESUME_BODY_BYTES);
    let (status, _, resumed_body) = request(
        &app,
        Method::POST,
        "/api/resume",
        Some(&token),
        Some(ORIGIN),
        exact_resume.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let resumed = parse_json(&resumed_body);
    let resumed_id = session_id(&resumed);

    let mut over_resume = exact_resume.clone();
    over_resume.push(b' ');
    let (status, headers, body) = request(
        &app,
        Method::POST,
        "/api/resume",
        Some(&token),
        Some(ORIGIN),
        over_resume,
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_secure_no_cors(&headers);
    error_code(&body, "resource_limit");

    let (status, _, retry_resume) = request(
        &app,
        Method::POST,
        "/api/resume",
        Some(&token),
        Some(ORIGIN),
        exact_resume,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(retry_resume, resumed_body);

    let (status, _, body) = request(
        &app,
        Method::POST,
        &format!("/api/sessions/{resumed_id}/close"),
        Some(&token),
        Some(ORIGIN),
        json_body(&json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(parse_json(&body), json!({"closed": true}));

    let padded_raw_save = pad_to(save_body.clone(), HTTP_SAVE_BYTES);
    assert_eq!(padded_raw_save.len(), HTTP_SAVE_BYTES);
    let padded_raw_text = String::from_utf8(padded_raw_save.clone()).expect("padded save UTF-8");
    let raw_resume = json_body(&json!({
        "creation_id": "boundary-raw-padded",
        "save_json": padded_raw_text
    }));
    assert!(raw_resume.len() < HTTP_RESUME_BODY_BYTES);
    let (status, _, raw_body) = request(
        &app,
        Method::POST,
        "/api/resume",
        Some(&token),
        Some(ORIGIN),
        raw_resume,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let raw_response = parse_json(&raw_body);
    assert_eq!(view(&raw_response), view(&first));
    let raw_id = session_id(&raw_response);
    let (status, _, canonical_raw_save) = request(
        &app,
        Method::GET,
        &format!("/api/sessions/{raw_id}/save"),
        Some(&token),
        None,
        Vec::new(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(canonical_raw_save, save_body);

    let (status, _, body) = request(
        &app,
        Method::POST,
        &format!("/api/sessions/{raw_id}/close"),
        Some(&token),
        Some(ORIGIN),
        json_body(&json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(parse_json(&body), json!({"closed": true}));

    let oversized_raw_save = pad_to(save_body.clone(), HTTP_SAVE_BYTES + 1);
    let oversized_raw_resume = json_body(&json!({
        "creation_id": "boundary-raw-too-large",
        "save_json": String::from_utf8(oversized_raw_save).expect("oversized save UTF-8")
    }));
    assert!(oversized_raw_resume.len() < HTTP_RESUME_BODY_BYTES);
    let (status, headers, body) = request(
        &app,
        Method::POST,
        "/api/resume",
        Some(&token),
        Some(ORIGIN),
        oversized_raw_resume,
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_secure_no_cors(&headers);
    error_code(&body, "resource_limit");

    let valid_raw_retry = json_body(&json!({
        "creation_id": "boundary-raw-too-large",
        "save_json": String::from_utf8(padded_raw_save).expect("padded save UTF-8")
    }));
    let (status, _, retry_raw_body) = request(
        &app,
        Method::POST,
        "/api/resume",
        Some(&token),
        Some(ORIGIN),
        valid_raw_retry,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let retry_raw = parse_json(&retry_raw_body);
    let retry_raw_id = session_id(&retry_raw);
    assert_eq!(view(&retry_raw), view(&first));

    let (status, _, body) = request(
        &app,
        Method::POST,
        &format!("/api/sessions/{retry_raw_id}/close"),
        Some(&token),
        Some(ORIGIN),
        json_body(&json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(parse_json(&body), json!({"closed": true}));
}
