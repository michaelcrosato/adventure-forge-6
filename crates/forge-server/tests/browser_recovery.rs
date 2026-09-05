use axum::{
    Router,
    body::{Body, to_bytes},
    http::{HeaderMap, HeaderValue, Method, Request, StatusCode, header},
};
use forge_content::parse_and_compile_production;
use forge_server::http::{HTTP_RESUME_BODY_BYTES, HTTP_SAVE_BYTES, router};
use serde_json::{Value, json};
use std::sync::Arc;
use tower::ServiceExt;

const CONTENT: &str = include_str!("../../../content/split-tide.json");
const PORT: u16 = 38_124;
const HOST: &str = "127.0.0.1:38124";
const ORIGIN: &str = "http://127.0.0.1:38124";

fn content() -> Arc<forge_kernel::CompiledContent> {
    Arc::new(parse_and_compile_production(CONTENT).expect("production content compiles"))
}

fn headers(token: Option<&str>, origin: Option<&str>, site: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::HOST, HeaderValue::from_static(HOST));
    headers.insert(
        "sec-fetch-site",
        HeaderValue::from_str(site).expect("fetch site header"),
    );
    headers.insert("sec-fetch-dest", HeaderValue::from_static("empty"));
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
    headers
}

async fn request(
    app: &Router,
    method: Method,
    path: &str,
    token: Option<&str>,
    origin: Option<&str>,
    site: &str,
    body: Value,
) -> (StatusCode, Vec<u8>) {
    let mut request = Request::new(if method == Method::GET {
        Body::empty()
    } else {
        Body::from(serde_json::to_vec(&body).expect("request JSON"))
    });
    *request.method_mut() = method.clone();
    *request.uri_mut() = path.parse().expect("request URI");
    *request.headers_mut() = headers(token, origin, site);
    if method == Method::POST {
        request.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
    }
    let response = app.clone().oneshot(request).await.expect("router response");
    let status = response.status();
    let bytes = to_bytes(
        response.into_body(),
        HTTP_RESUME_BODY_BYTES + HTTP_SAVE_BYTES,
    )
    .await
    .expect("bounded response body");
    (status, bytes.to_vec())
}

fn parse(body: &[u8]) -> Value {
    serde_json::from_slice(body).expect("JSON response")
}

fn assert_wire_numbers_are_strings(value: &Value) {
    match value {
        Value::Number(number) => panic!("HTTP public number was not stringified: {number}"),
        Value::Array(values) => values.iter().for_each(assert_wire_numbers_are_strings),
        Value::Object(values) => values.values().for_each(assert_wire_numbers_are_strings),
        Value::Null | Value::Bool(_) | Value::String(_) => {}
    }
}

fn assert_hex64(value: &str) {
    assert_eq!(value.len(), 64);
    assert!(
        value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    );
}

#[tokio::test]
async fn current_recovers_live_session_without_mutating_or_reopening_it() {
    let app = router(content(), PORT).expect("HTTP router builds");
    let (status, bootstrap_body) = request(
        &app,
        Method::GET,
        "/api/bootstrap",
        None,
        None,
        "same-origin",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let bootstrap = parse(&bootstrap_body);
    let token = bootstrap["token"].as_str().expect("bootstrap token");
    let instance_id = bootstrap["instance_id"].as_str().expect("instance ID");
    assert_hex64(token);
    assert_hex64(instance_id);
    assert_ne!(token, instance_id);
    let mut bootstrap_keys: Vec<_> = bootstrap
        .as_object()
        .expect("bootstrap object")
        .keys()
        .map(String::as_str)
        .collect();
    bootstrap_keys.sort_unstable();
    assert_eq!(bootstrap_keys, vec!["instance_id", "token"]);

    let (status, body) = request(
        &app,
        Method::GET,
        "/api/current",
        Some(token),
        None,
        "same-origin",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(parse(&body), json!({"session": null}));

    let create_request = json!({
        "creation_id": "browser-recovery",
        "start": {"kind": "preset", "character_preset_id": "rook", "seed": "71"}
    });
    let (status, created_bytes) = request(
        &app,
        Method::POST,
        "/api/sessions",
        Some(token),
        Some(ORIGIN),
        "same-origin",
        create_request.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let created = parse(&created_bytes);
    assert_wire_numbers_are_strings(&created);
    let session_id = created["session_id"].as_str().expect("session ID");
    assert_hex64(session_id);
    let initial_view = created["view"].clone();

    let (status, current_bytes) = request(
        &app,
        Method::GET,
        "/api/current",
        Some(token),
        None,
        "same-origin",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let current = parse(&current_bytes);
    assert_wire_numbers_are_strings(&current);
    assert_eq!(current["session"]["session_id"], session_id);
    assert_eq!(current["session"]["view"], initial_view);

    let (status, _, before_action_save) = session_request(
        &app,
        token,
        &format!("/api/sessions/{session_id}/save"),
        Method::GET,
        None,
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let state_id = initial_view["catalog"]["state_id"]
        .as_str()
        .expect("initial state ID");
    let action_id = initial_view["catalog"]["actions"][0]["action_id"]
        .as_str()
        .expect("opening action ID");
    let action_request = json!({
        "command_id": "browser-recovery-action",
        "expected_revision": "0",
        "expected_state_id": state_id,
        "action_id": action_id
    });
    let (status, accepted_bytes) = request(
        &app,
        Method::POST,
        &format!("/api/sessions/{session_id}/actions"),
        Some(token),
        Some(ORIGIN),
        "same-origin",
        action_request.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let accepted = parse(&accepted_bytes);
    assert_wire_numbers_are_strings(&accepted);
    assert_eq!(accepted["revision"], "1");
    assert_ne!(accepted, initial_view);

    let (status, _, after_action_save) = session_request(
        &app,
        token,
        &format!("/api/sessions/{session_id}/save"),
        Method::GET,
        None,
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_ne!(after_action_save, before_action_save);

    let (status, current_after_bytes) = request(
        &app,
        Method::GET,
        "/api/current",
        Some(token),
        None,
        "same-origin",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let current_after = parse(&current_after_bytes);
    assert_wire_numbers_are_strings(&current_after);
    assert_eq!(current_after["session"]["session_id"], session_id);
    assert_eq!(current_after["session"]["view"], accepted);
    assert_ne!(current_after["session"]["view"], initial_view);

    let (status, repeated_current_bytes) = request(
        &app,
        Method::GET,
        "/api/current",
        Some(token),
        None,
        "same-origin",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(repeated_current_bytes, current_after_bytes);
    let (status, _, save_after_current) = session_request(
        &app,
        token,
        &format!("/api/sessions/{session_id}/save"),
        Method::GET,
        None,
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(save_after_current, after_action_save);

    let (status, close_body) = request(
        &app,
        Method::POST,
        &format!("/api/sessions/{session_id}/close"),
        Some(token),
        Some(ORIGIN),
        "same-origin",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(parse(&close_body), json!({"closed": true}));

    let (status, body) = request(
        &app,
        Method::GET,
        "/api/current",
        Some(token),
        None,
        "same-origin",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(parse(&body), json!({"session": null}));

    let (status, _, saved_after_close) = session_request(
        &app,
        token,
        &format!("/api/sessions/{session_id}/save"),
        Method::GET,
        None,
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved_after_close, after_action_save);

    let (status, retried_action) = request(
        &app,
        Method::POST,
        &format!("/api/sessions/{session_id}/actions"),
        Some(token),
        Some(ORIGIN),
        "same-origin",
        action_request,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(retried_action, accepted_bytes);

    let (status, retried_create) = request(
        &app,
        Method::POST,
        "/api/sessions",
        Some(token),
        Some(ORIGIN),
        "same-origin",
        create_request,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(retried_create, created_bytes);

    let (status, body) = request(
        &app,
        Method::GET,
        "/api/current",
        Some(token),
        None,
        "same-origin",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(parse(&body), json!({"session": null}));
}

async fn session_request(
    app: &Router,
    token: &str,
    path: &str,
    method: Method,
    origin: Option<&str>,
    body: Value,
) -> (StatusCode, HeaderMap, Vec<u8>) {
    let mut request = Request::new(if method == Method::GET {
        Body::empty()
    } else {
        Body::from(serde_json::to_vec(&body).expect("request JSON"))
    });
    *request.method_mut() = method.clone();
    *request.uri_mut() = path.parse().expect("request URI");
    *request.headers_mut() = headers(Some(token), origin, "same-origin");
    if method == Method::POST {
        request.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
    }
    let response = app.clone().oneshot(request).await.expect("router response");
    let status = response.status();
    let response_headers = response.headers().clone();
    let bytes = to_bytes(
        response.into_body(),
        HTTP_RESUME_BODY_BYTES + HTTP_SAVE_BYTES,
    )
    .await
    .expect("bounded response body");
    (status, response_headers, bytes.to_vec())
}

#[tokio::test]
async fn current_requires_capability_and_process_instance_ids_are_fresh() {
    let first_app = router(content(), PORT).expect("first router builds");
    let (_, first_bootstrap_body) = request(
        &first_app,
        Method::GET,
        "/api/bootstrap",
        None,
        None,
        "same-origin",
        json!({}),
    )
    .await;
    let first_bootstrap = parse(&first_bootstrap_body);
    let first_token = first_bootstrap["token"].as_str().unwrap();
    let first_instance = first_bootstrap["instance_id"].as_str().unwrap();

    let (status, body) = request(
        &first_app,
        Method::GET,
        "/api/current",
        None,
        None,
        "same-origin",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(parse(&body), json!({"error": "unauthorized"}));

    let (status, body) = request(
        &first_app,
        Method::GET,
        "/api/current",
        Some(first_token),
        None,
        "cross-site",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(parse(&body), json!({"error": "forbidden"}));

    let (status, body) = request(
        &first_app,
        Method::GET,
        "/api/current?session=old",
        Some(first_token),
        None,
        "same-origin",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(parse(&body), json!({"error": "forbidden"}));

    let (status, body) = request(
        &first_app,
        Method::GET,
        "/api/current",
        Some(first_token),
        Some("http://evil.test"),
        "same-origin",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(parse(&body), json!({"error": "forbidden"}));

    let second_app = router(content(), PORT).expect("fresh router builds");
    let (_, second_bootstrap_body) = request(
        &second_app,
        Method::GET,
        "/api/bootstrap",
        None,
        None,
        "same-origin",
        json!({}),
    )
    .await;
    let second_bootstrap = parse(&second_bootstrap_body);
    let second_token = second_bootstrap["token"].as_str().unwrap();
    let second_instance = second_bootstrap["instance_id"].as_str().unwrap();
    assert_hex64(first_token);
    assert_hex64(second_token);
    assert_hex64(first_instance);
    assert_hex64(second_instance);
    assert_ne!(first_token, second_token);
    assert_ne!(first_instance, second_instance);

    let (status, root_body) = request(
        &first_app,
        Method::GET,
        "/",
        None,
        None,
        "same-origin",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let root_text = String::from_utf8(root_body).expect("root is UTF-8");
    assert!(!root_text.contains(first_token));
    assert!(!root_text.contains(first_instance));
}
