//! Single-user, loopback-only browser transport. This is not Internet hosting.
//!
//! Every JSON number on the public HTTP surface is a decimal string. Portable
//! saves are the exception: they are opaque downloads, imported as exact text.

use crate::registry::{RegistryError, RegistryLimits, SessionRegistry};
use crate::{
    ActionRequest, MAX_REQUEST_BYTES, ServiceError, ServiceLimits, StartRequest, start_options,
};
use axum::{
    Router,
    body::{Body, to_bytes},
    extract::{Request, State},
    http::{HeaderValue, Method, StatusCode, header},
    response::Response,
};
use forge_kernel::{CharacterSelection, CompiledContent, validate_unique_json_keys};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

mod security;
use security::AccessPolicy;
mod assets;

pub const HTTP_SAVE_BYTES: usize = 256 * 1024;
// An imported save is a JSON string: a byte can expand to six escape bytes.
pub const HTTP_RESUME_BODY_BYTES: usize = 6 * HTTP_SAVE_BYTES + 4096;
const BODY_READ_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone)]
struct App {
    policy: Arc<AccessPolicy>,
    registry: SessionRegistry,
    options: Arc<Vec<u8>>,
    admissions: Arc<Semaphore>,
    worker: Arc<Semaphore>,
}

/// Build the adapter for a listener already bound to this exact loopback port.
/// The executable enforces the bind address; embedding callers must do so too.
pub fn router(content: Arc<CompiledContent>, port: u16) -> Result<Router, ServiceError> {
    Ok(Router::new()
        .fallback(handle)
        .with_state(app_state(content, port)?))
}

fn app_state(content: Arc<CompiledContent>, port: u16) -> Result<App, ServiceError> {
    let options = wire_bytes(&start_options(&content)?)?;
    let policy = AccessPolicy::new(port)?;
    let limits = ServiceLimits {
        max_save_bytes: HTTP_SAVE_BYTES,
        max_idempotency_bytes: 2 * 1024 * 1024,
        ..ServiceLimits::default()
    };
    let registry = SessionRegistry::new(content, limits, RegistryLimits::default())
        .map_err(|_| ServiceError::Internal)?;
    Ok(App {
        policy: Arc::new(policy),
        registry,
        options: Arc::new(options),
        admissions: Arc::new(Semaphore::new(2)),
        worker: Arc::new(Semaphore::new(1)),
    })
}

async fn handle(State(app): State<App>, request: Request) -> Response {
    let is_document_fetch = request
        .headers()
        .get("sec-fetch-dest")
        .is_none_or(|value| value == "document");
    let response = handle_inner(app, request).await;
    secure_response(response, is_document_fetch)
}

async fn handle_inner(app: App, request: Request) -> Response {
    if let Err(status) = app
        .policy
        .authorize(request.method(), request.uri(), request.headers())
    {
        let code = if status == StatusCode::UNAUTHORIZED {
            "unauthorized"
        } else {
            "forbidden"
        };
        return failure(status, code);
    }
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    if method == Method::GET {
        let asset_path = if path == "/" { "/index.html" } else { &path };
        if let Some(asset) = assets::get(asset_path) {
            let mut response = Response::new(Body::from(asset.bytes));
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static(asset.content_type),
            );
            return response;
        }
    }
    if method == Method::GET && path == "/api/bootstrap" {
        return plain_json(
            &json!({"token": app.policy.token(), "instance_id": app.policy.instance_id()}),
        );
    }
    if method == Method::GET && path == "/api/options" {
        return bytes_response(StatusCode::OK, "application/json", (*app.options).clone());
    }

    let operation = match Operation::parse(&method, &path) {
        Ok(operation) => operation,
        Err((status, code)) => return failure(status, code),
    };
    let admission = match Arc::clone(&app.admissions).try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => return failure(StatusCode::SERVICE_UNAVAILABLE, "busy"),
    };
    let body_limit = if matches!(operation, Operation::Resume) {
        HTTP_RESUME_BODY_BYTES
    } else if method == Method::GET {
        0
    } else {
        MAX_REQUEST_BYTES
    };
    if method == Method::POST {
        let mut content_types = request.headers().get_all(header::CONTENT_TYPE).iter();
        let valid = content_types.next().is_some_and(|value| {
            value == "application/json" || value == "application/json; charset=utf-8"
        }) && content_types.next().is_none();
        if !valid || request.headers().contains_key(header::CONTENT_ENCODING) {
            return failure(StatusCode::UNSUPPORTED_MEDIA_TYPE, "invalid_content_type");
        }
    }
    // A body deadline cannot cancel a mutation: no worker exists at this point.
    let bytes =
        match tokio::time::timeout(BODY_READ_TIMEOUT, to_bytes(request.into_body(), body_limit))
            .await
        {
            Ok(Ok(bytes)) => bytes,
            Ok(Err(_)) => return failure(StatusCode::PAYLOAD_TOO_LARGE, "resource_limit"),
            Err(_) => return failure(StatusCode::REQUEST_TIMEOUT, "body_timeout"),
        };
    let body = match std::str::from_utf8(&bytes) {
        Ok(body) => body.to_owned(),
        Err(_) => return failure(StatusCode::BAD_REQUEST, "invalid_input"),
    };
    let worker = match Arc::clone(&app.worker).acquire_owned().await {
        Ok(permit) => permit,
        Err(_) => return failure(StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
    };
    // Dropping the HTTP future detaches this task. Both permits stay with the
    // actual work, through commit and acknowledgment serialization. A retry is
    // the only way to resolve an uncertain/lost response; it is not a rollback.
    run_blocking(admission, worker, move || {
        execute(&app.registry, operation, &body)
    })
    .await
}

async fn run_blocking<F>(
    admission: OwnedSemaphorePermit,
    worker: OwnedSemaphorePermit,
    work: F,
) -> Response
where
    F: FnOnce() -> Response + Send + 'static,
{
    match tokio::task::spawn_blocking(move || {
        let _admission = admission;
        let _worker = worker;
        work()
    })
    .await
    {
        Ok(response) => response,
        Err(_) => failure(StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
    }
}

enum Operation {
    Current,
    Start,
    Resume,
    Observe(String),
    Catalog(String),
    Act(String),
    Save(String),
    Close(String),
}

impl Operation {
    fn parse(method: &Method, path: &str) -> Result<Self, (StatusCode, &'static str)> {
        if path == "/api/current" {
            return if method == Method::GET {
                Ok(Self::Current)
            } else {
                Err(method_not_allowed())
            };
        }
        if path == "/api/sessions" {
            return if method == Method::POST {
                Ok(Self::Start)
            } else {
                Err(method_not_allowed())
            };
        }
        if path == "/api/resume" {
            return if method == Method::POST {
                Ok(Self::Resume)
            } else {
                Err(method_not_allowed())
            };
        }
        if matches!(path, "/" | "/api/options" | "/api/bootstrap") {
            return Err(method_not_allowed());
        }
        let Some(suffix) = path.strip_prefix("/api/sessions/") else {
            return Err((StatusCode::NOT_FOUND, "not_found"));
        };
        let mut parts = suffix.split('/');
        let id = parts.next().unwrap_or_default().to_owned();
        if id.len() != 64
            || !id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err((StatusCode::BAD_REQUEST, "invalid_input"));
        }
        let tail = parts.next();
        if parts.next().is_some() {
            return Err((StatusCode::NOT_FOUND, "not_found"));
        }
        match (tail, method) {
            (None, &Method::GET) => Ok(Self::Observe(id)),
            (Some("catalog"), &Method::POST) => Ok(Self::Catalog(id)),
            (Some("actions"), &Method::POST) => Ok(Self::Act(id)),
            (Some("save"), &Method::GET) => Ok(Self::Save(id)),
            (Some("close"), &Method::POST) => Ok(Self::Close(id)),
            (None | Some("catalog" | "actions" | "save" | "close"), _) => Err(method_not_allowed()),
            _ => Err((StatusCode::NOT_FOUND, "not_found")),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateInput {
    creation_id: String,
    start: StartInput,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum StartInput {
    Preset {
        character_preset_id: String,
        seed: String,
    },
    Custom {
        selection: CharacterSelection,
        seed: String,
    },
}

impl StartInput {
    fn into_start(self) -> Result<StartRequest, ServiceError> {
        match self {
            Self::Preset {
                character_preset_id,
                seed,
            } => Ok(StartRequest::Preset {
                character_preset_id,
                seed: decimal(&seed)?,
            }),
            Self::Custom { selection, seed } => Ok(StartRequest::Custom {
                selection,
                seed: decimal(&seed)?,
            }),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResumeInput {
    creation_id: String,
    save_json: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogInput {
    expected_state_id: String,
    offset: String,
    page_size: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ActionInput {
    command_id: String,
    expected_revision: String,
    expected_state_id: String,
    action_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyInput {}

fn decode<T: DeserializeOwned>(body: &str) -> Result<T, ServiceError> {
    validate_unique_json_keys(body).map_err(|_| ServiceError::InvalidInput)?;
    serde_json::from_str(body).map_err(|_| ServiceError::InvalidInput)
}

fn decimal(text: &str) -> Result<u64, ServiceError> {
    if text.is_empty()
        || text.len() > 20
        || (text.len() > 1 && text.starts_with('0'))
        || !text.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(ServiceError::InvalidInput);
    }
    text.parse().map_err(|_| ServiceError::InvalidInput)
}

fn index(text: &str) -> Result<usize, ServiceError> {
    usize::try_from(decimal(text)?).map_err(|_| ServiceError::InvalidInput)
}

fn execute(registry: &SessionRegistry, operation: Operation, body: &str) -> Response {
    match execute_result(registry, operation, body) {
        Ok(response) => response,
        Err(error) => registry_failure(error),
    }
}

fn execute_result(
    registry: &SessionRegistry,
    operation: Operation,
    body: &str,
) -> Result<Response, RegistryError> {
    match operation {
        Operation::Current => Ok(public_json(&json!({"session": registry.current()?}))),
        Operation::Start => {
            let input: CreateInput = decode(body)?;
            let response = registry.start(&input.creation_id, input.start.into_start()?)?;
            Ok(public_json(&response))
        }
        Operation::Resume => {
            let input: ResumeInput = decode(body)?;
            let response = registry.resume(&input.creation_id, &input.save_json)?;
            Ok(public_json(&response))
        }
        Operation::Observe(id) => Ok(public_json(&registry.observe(&id)?)),
        Operation::Catalog(id) => {
            let input: CatalogInput = decode(body)?;
            Ok(public_json(&registry.catalog(
                &id,
                &input.expected_state_id,
                index(&input.offset)?,
                index(&input.page_size)?,
            )?))
        }
        Operation::Act(id) => {
            let input: ActionInput = decode(body)?;
            let action = ActionRequest {
                command_id: input.command_id,
                expected_revision: decimal(&input.expected_revision)?,
                expected_state_id: input.expected_state_id,
                action_id: input.action_id,
            };
            Ok(public_json(&registry.act(&id, action)?))
        }
        Operation::Save(id) => {
            let save = registry.save(&id)?;
            let mut response =
                bytes_response(StatusCode::OK, "application/json", save.into_bytes());
            response.headers_mut().insert(
                header::CONTENT_DISPOSITION,
                HeaderValue::from_static("attachment; filename=\"adventure-forge.trace.json\""),
            );
            Ok(response)
        }
        Operation::Close(id) => {
            let _: EmptyInput = decode(body)?;
            registry.close(&id)?;
            Ok(plain_json(&json!({"closed": true})))
        }
    }
}

fn registry_failure(error: RegistryError) -> Response {
    use RegistryError as R;
    match error {
        R::InvalidInput => failure(StatusCode::BAD_REQUEST, "invalid_input"),
        R::UnknownSession => failure(StatusCode::NOT_FOUND, "unknown_session"),
        R::ResourceLimit => failure(StatusCode::PAYLOAD_TOO_LARGE, "resource_limit"),
        R::IdempotencyConflict => failure(StatusCode::CONFLICT, "idempotency_conflict"),
        R::Unavailable => failure(StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
        R::Internal => failure(StatusCode::INTERNAL_SERVER_ERROR, "internal"),
        R::Service(error) => service_failure(error),
    }
}

fn service_failure(error: ServiceError) -> Response {
    use ServiceError as S;
    let status = match error {
        S::InvalidInput | S::InvalidSave | S::InvalidAction => StatusCode::BAD_REQUEST,
        S::StaleState | S::IdempotencyConflict => StatusCode::CONFLICT,
        S::SessionClosed => StatusCode::GONE,
        S::ResourceLimit => StatusCode::PAYLOAD_TOO_LARGE,
        S::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        S::InvalidContent | S::Internal => StatusCode::INTERNAL_SERVER_ERROR,
    };
    failure(status, &error.to_string())
}

fn method_not_allowed() -> (StatusCode, &'static str) {
    (StatusCode::METHOD_NOT_ALLOWED, "method_not_allowed")
}

fn failure(status: StatusCode, code: &str) -> Response {
    let mut response = plain_json(&json!({"error": code}));
    *response.status_mut() = status;
    response
}

fn public_json<T: Serialize>(value: &T) -> Response {
    match wire_bytes(value) {
        Ok(bytes) => bytes_response(StatusCode::OK, "application/json", bytes),
        Err(_) => failure(StatusCode::INTERNAL_SERVER_ERROR, "internal"),
    }
}

fn plain_json<T: Serialize>(value: &T) -> Response {
    match serde_json::to_vec(value) {
        Ok(bytes) => bytes_response(StatusCode::OK, "application/json", bytes),
        Err(_) => bytes_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "application/json",
            b"{\"error\":\"internal\"}".to_vec(),
        ),
    }
}

fn wire_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, ServiceError> {
    fn stringify_numbers(value: &mut Value) {
        match value {
            Value::Number(number) => *value = Value::String(number.to_string()),
            Value::Array(array) => array.iter_mut().for_each(stringify_numbers),
            Value::Object(object) => object.values_mut().for_each(stringify_numbers),
            _ => (),
        }
    }
    let mut value = serde_json::to_value(value).map_err(|_| ServiceError::Internal)?;
    stringify_numbers(&mut value);
    serde_json::to_vec(&value).map_err(|_| ServiceError::Internal)
}

fn bytes_response(status: StatusCode, content_type: &'static str, bytes: Vec<u8>) -> Response {
    let mut response = Response::new(Body::from(bytes));
    *response.status_mut() = status;
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    response
}

fn secure_response(mut response: Response, is_document_fetch: bool) -> Response {
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    let content_security_policy = if is_document_fetch
        && headers
            .get(header::CONTENT_TYPE)
            .is_some_and(|value| value == "text/html; charset=utf-8")
    {
        // Only the embedded HTML document needs to load the browser bundle.
        // JSON, errors, and non-HTML assets retain the stricter no-script policy.
        "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'"
    } else {
        "default-src 'none'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'"
    };
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(content_security_policy),
    );
    headers.insert(
        "x-forge-ui-build",
        HeaderValue::from_static(assets::build_id()),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_content::parse_and_compile_production;
    use std::sync::mpsc;

    fn request_for(app: &App, path: &str, body: Body) -> Request {
        Request::builder()
            .method(Method::POST)
            .uri(path)
            .header(header::HOST, "127.0.0.1:38123")
            .header(header::ORIGIN, "http://127.0.0.1:38123")
            .header("sec-fetch-site", "same-origin")
            .header("sec-fetch-dest", "empty")
            .header(header::CONTENT_TYPE, "application/json")
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", app.policy.token()),
            )
            .body(body)
            .unwrap()
    }

    fn test_app() -> App {
        let content = Arc::new(
            parse_and_compile_production(include_str!("../../../../content/split-tide.json"))
                .unwrap(),
        );
        app_state(content, 38123).unwrap()
    }

    #[tokio::test]
    async fn full_admission_rejects_without_publishing_or_poisoning_creation() {
        let app = test_app();
        let first = Arc::clone(&app.admissions).try_acquire_owned().unwrap();
        let second = Arc::clone(&app.admissions).try_acquire_owned().unwrap();
        let body = r#"{"creation_id":"busy-retry","start":{"kind":"preset","character_preset_id":"rook","seed":"71"}}"#;
        let rejected = handle_inner(
            app.clone(),
            request_for(&app, "/api/sessions", Body::from(body)),
        )
        .await;
        assert_eq!(rejected.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            to_bytes(rejected.into_body(), 1024).await.unwrap().as_ref(),
            b"{\"error\":\"busy\"}"
        );
        assert_eq!(app.worker.available_permits(), 1);
        drop(first);
        let accepted = handle_inner(
            app.clone(),
            request_for(&app, "/api/sessions", Body::from(body)),
        )
        .await;
        assert_eq!(accepted.status(), StatusCode::OK);
        assert_eq!(app.admissions.available_permits(), 1);
        drop(second);
    }

    #[tokio::test]
    async fn body_deadline_is_before_worker_admission_and_canonical_mutation() {
        struct PendingBody;
        impl futures_core::Stream for PendingBody {
            type Item = Result<axum::body::Bytes, std::io::Error>;
            fn poll_next(
                self: std::pin::Pin<&mut Self>,
                _: &mut std::task::Context<'_>,
            ) -> std::task::Poll<Option<Self::Item>> {
                std::task::Poll::Pending
            }
        }
        let app = test_app();
        let created = app
            .registry
            .start(
                "body-deadline",
                StartRequest::Preset {
                    character_preset_id: "rook".into(),
                    seed: 71,
                },
            )
            .unwrap();
        let before = app.registry.save(&created.session_id).unwrap();
        // Holding the sole worker ensures a timeout cannot come from dispatched
        // work. The body must expire before it ever waits for this permit.
        let worker = Arc::clone(&app.worker).try_acquire_owned().unwrap();
        let request = request_for(
            &app,
            &format!("/api/sessions/{}/actions", created.session_id),
            Body::from_stream(PendingBody),
        );
        let response =
            tokio::time::timeout(Duration::from_secs(7), handle_inner(app.clone(), request))
                .await
                .unwrap();
        assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
        assert_eq!(
            to_bytes(response.into_body(), 1024).await.unwrap().as_ref(),
            b"{\"error\":\"body_timeout\"}"
        );
        assert_eq!(app.admissions.available_permits(), 2);
        assert_eq!(
            app.registry.observe(&created.session_id).unwrap(),
            created.view
        );
        assert_eq!(app.registry.save(&created.session_id).unwrap(), before);
        drop(worker);
    }

    #[test]
    fn decimal_inputs_and_public_integer_output_preserve_exact_width() {
        for text in [
            "",
            "+1",
            "-1",
            "01",
            "1.0",
            "1e2",
            " 1",
            "18446744073709551616",
        ] {
            assert_eq!(decimal(text), Err(ServiceError::InvalidInput));
        }
        assert_eq!(decimal("0"), Ok(0));
        assert_eq!(decimal("18446744073709551615"), Ok(u64::MAX));
        let bytes = wire_bytes(
            &json!({"unsigned":u64::MAX,"signed":i64::MIN,"small":0,"nested":[1,null,true]}),
        )
        .unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&bytes).unwrap(),
            json!({"unsigned":"18446744073709551615","signed":"-9223372036854775808","small":"0","nested":["1",null,true]})
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn canceled_response_keeps_worker_permits_and_committed_acknowledgment() {
        let content = Arc::new(
            parse_and_compile_production(include_str!("../../../../content/split-tide.json"))
                .unwrap(),
        );
        let registry =
            SessionRegistry::new(content, ServiceLimits::default(), RegistryLimits::default())
                .unwrap();
        let created = registry
            .start(
                "create",
                StartRequest::Preset {
                    character_preset_id: "rook".into(),
                    seed: 71,
                },
            )
            .unwrap();
        let action = created
            .view
            .catalog
            .actions
            .iter()
            .find(|action| action.definition_id == "wait_tide")
            .unwrap();
        let command = ActionRequest {
            command_id: "dropped".into(),
            expected_revision: 0,
            expected_state_id: created.view.catalog.state_id.clone(),
            action_id: action.action_id.clone(),
        };
        let admissions = Arc::new(Semaphore::new(1));
        let workers = Arc::new(Semaphore::new(1));
        let admission = Arc::clone(&admissions).try_acquire_owned().unwrap();
        let worker = Arc::clone(&workers).try_acquire_owned().unwrap();
        let (started_send, started_recv) = tokio::sync::oneshot::channel();
        let (release_send, release_recv) = mpsc::channel();
        let (done_send, done_recv) = tokio::sync::oneshot::channel();
        let registry_task = registry.clone();
        let id = created.session_id.clone();
        let request = command.clone();
        let task = tokio::spawn(run_blocking(admission, worker, move || {
            let acknowledgment = registry_task.act(&id, request).unwrap();
            started_send.send(acknowledgment.clone()).unwrap();
            release_recv.recv_timeout(Duration::from_secs(5)).unwrap();
            done_send.send(()).unwrap();
            public_json(&acknowledgment)
        }));
        let accepted = tokio::time::timeout(Duration::from_secs(5), started_recv)
            .await
            .unwrap()
            .unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert!(Arc::clone(&admissions).try_acquire_owned().is_err());
        assert!(Arc::clone(&workers).try_acquire_owned().is_err());
        release_send.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(5), done_recv)
            .await
            .unwrap()
            .unwrap();
        let _returned_admission = tokio::time::timeout(
            Duration::from_secs(5),
            Arc::clone(&admissions).acquire_owned(),
        )
        .await
        .unwrap()
        .unwrap();
        let _returned_worker = Arc::clone(&workers).try_acquire_owned().unwrap();
        assert_eq!(
            registry.act(&created.session_id, command).unwrap(),
            accepted
        );
        assert_eq!(registry.observe(&created.session_id).unwrap().revision, 1);
        let save = registry.save(&created.session_id).unwrap();
        let trace = forge_replay::PlayerTrace::from_json(&save).unwrap();
        assert_eq!(trace.action_count(), 1);
    }
}
