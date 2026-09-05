use axum::{
    Router,
    body::{Body, to_bytes},
    http::{HeaderMap, HeaderValue, Method, Request, StatusCode, header},
};
use forge_content::parse_and_compile_production;
use forge_server::http::{HTTP_RESUME_BODY_BYTES, HTTP_SAVE_BYTES, router};
use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tower::ServiceExt;

const CONTENT: &str = include_str!("../../../content/split-tide.json");
const PORT: u16 = 38_125;
const HOST: &str = "127.0.0.1:38125";
const HTML_CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'";
const STRICT_CSP: &str = "default-src 'none'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'";

fn content() -> Arc<forge_kernel::CompiledContent> {
    Arc::new(parse_and_compile_production(CONTENT).expect("production content compiles"))
}

fn headers(host: &str, origin: Option<&str>, site: &str, destination: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::HOST,
        HeaderValue::from_str(host).expect("host header"),
    );
    headers.insert(
        "sec-fetch-site",
        HeaderValue::from_str(site).expect("fetch site header"),
    );
    headers.insert(
        "sec-fetch-dest",
        HeaderValue::from_str(destination).expect("fetch destination header"),
    );
    if let Some(origin) = origin {
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_str(origin).expect("origin header"),
        );
    }
    headers
}

async fn get(
    app: &Router,
    path: &str,
    host: &str,
    origin: Option<&str>,
    site: &str,
    destination: &str,
) -> (StatusCode, HeaderMap, Vec<u8>) {
    get_with_headers(app, path, headers(host, origin, site, destination)).await
}

async fn get_with_headers(
    app: &Router,
    path: &str,
    request_headers: HeaderMap,
) -> (StatusCode, HeaderMap, Vec<u8>) {
    let mut request = Request::new(Body::empty());
    *request.method_mut() = Method::GET;
    *request.uri_mut() = path.parse().expect("request URI");
    *request.headers_mut() = request_headers;
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

fn header_text(headers: &HeaderMap, name: &str) -> String {
    headers
        .get(name)
        .unwrap_or_else(|| panic!("missing response header {name}"))
        .to_str()
        .expect("response header is UTF-8")
        .to_owned()
}

fn assert_security_headers_with_csp(
    headers: &HeaderMap,
    expected_build: Option<&str>,
    expected_csp: &str,
) -> String {
    assert_eq!(
        header_text(headers, header::CACHE_CONTROL.as_str()),
        "no-store"
    );
    assert_eq!(
        header_text(headers, header::CONTENT_SECURITY_POLICY.as_str()),
        expected_csp
    );
    assert_eq!(
        header_text(headers, header::X_CONTENT_TYPE_OPTIONS.as_str()),
        "nosniff"
    );
    assert_eq!(header_text(headers, "x-frame-options"), "DENY");
    assert!(!headers.contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN));
    assert!(!headers.contains_key(header::ACCESS_CONTROL_ALLOW_CREDENTIALS));

    let build = header_text(headers, "x-forge-ui-build");
    assert_eq!(build.len(), 64);
    assert!(
        build
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    );
    if let Some(expected_build) = expected_build {
        assert_eq!(build, expected_build);
    }
    build
}

fn assert_security_headers(headers: &HeaderMap, expected_build: Option<&str>) -> String {
    assert_security_headers_with_csp(headers, expected_build, STRICT_CSP)
}

fn assert_html_security_headers(headers: &HeaderMap, expected_build: Option<&str>) -> String {
    assert_security_headers_with_csp(headers, expected_build, HTML_CSP)
}

fn linked_paths(html: &str, attribute: &str) -> Vec<String> {
    let needle = format!("{attribute}=\"");
    let mut paths = Vec::new();
    let mut remaining = html;
    while let Some(start) = remaining.find(&needle) {
        let value = &remaining[start + needle.len()..];
        let end = value.find('"').expect("quoted asset path");
        paths.push(value[..end].to_owned());
        remaining = &value[end + 1..];
    }
    paths
}

fn assert_asset_path(path: &str, extension: &str) {
    assert!(
        path.starts_with("/assets/"),
        "unexpected linked path: {path}"
    );
    assert!(
        path.ends_with(extension),
        "unexpected asset extension: {path}"
    );
    assert!(!path.contains(".."));
    for forbidden in ['%', '?', '#', '\\'] {
        assert!(!path.contains(forbidden));
    }
    assert!(!path.contains("//"));
}

#[tokio::test]
async fn serves_actual_embedded_bundle_with_exact_public_headers() {
    let app = router(content(), PORT).expect("HTTP router builds");
    let (status, root_headers, root_bytes) =
        get(&app, "/", HOST, None, "same-origin", "document").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        header_text(&root_headers, header::CONTENT_TYPE.as_str()),
        "text/html; charset=utf-8"
    );
    let build = assert_html_security_headers(&root_headers, None);
    let html = String::from_utf8(root_bytes).expect("embedded index is UTF-8");
    assert!(html.contains("<title>Adventure Forge"));
    assert!(html.contains("<div id=\"root\"></div>"));

    let (status, _, _) = get(&app, "/", HOST, None, "none", "document").await;
    assert_eq!(
        status,
        StatusCode::OK,
        "direct navigation must allow site=none"
    );

    let (status, _, _) = get(&app, "/", HOST, None, "same-origin", "empty").await;
    assert_eq!(
        status,
        StatusCode::OK,
        "same-origin public reads must remain allowed"
    );

    let scripts = linked_paths(&html, "src");
    let stylesheets = linked_paths(&html, "href");
    assert_eq!(scripts.len(), 1, "root should link one module bundle");
    assert_eq!(
        stylesheets.len(),
        1,
        "root should link one stylesheet bundle"
    );
    assert_asset_path(&scripts[0], ".js");
    assert_asset_path(&stylesheets[0], ".css");

    let (status, index_headers, _) = get(&app, "/index.html", HOST, None, "none", "document").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        header_text(&index_headers, header::CONTENT_TYPE.as_str()),
        "text/html; charset=utf-8"
    );
    assert_html_security_headers(&index_headers, Some(&build));

    let (status, _, _) = get(&app, "/index.html", HOST, None, "same-origin", "empty").await;
    assert_eq!(status, StatusCode::OK);

    let (status, script_headers, script_bytes) =
        get(&app, &scripts[0], HOST, None, "same-origin", "script").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        header_text(&script_headers, header::CONTENT_TYPE.as_str()),
        "text/javascript; charset=utf-8"
    );
    assert_security_headers(&script_headers, Some(&build));
    assert!(
        script_bytes.len() > 1_024,
        "module response is not the real bundle"
    );
    assert!(
        String::from_utf8_lossy(&script_bytes).contains("Adventure Forge"),
        "module response is not the built Adventure Forge player"
    );

    let (status, style_headers, style_bytes) =
        get(&app, &stylesheets[0], HOST, None, "same-origin", "style").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        header_text(&style_headers, header::CONTENT_TYPE.as_str()),
        "text/css; charset=utf-8"
    );
    assert_security_headers(&style_headers, Some(&build));
    assert!(
        style_bytes.len() > 32,
        "stylesheet response is not the real bundle"
    );
    let style_text = String::from_utf8_lossy(&style_bytes);
    assert!(style_text.contains("font-family:"));
    assert!(style_text.contains("background:"));

    let (status, repeat_headers, repeat_bytes) =
        get(&app, &scripts[0], HOST, None, "same-origin", "script").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(repeat_bytes, script_bytes);
    assert_security_headers(&repeat_headers, Some(&build));
}

#[tokio::test]
async fn html_allows_its_bundle_but_api_json_keeps_strict_csp() {
    let app = router(content(), PORT).expect("HTTP router builds");
    let (status, html_headers, _) = get(&app, "/", HOST, None, "same-origin", "document").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        header_text(&html_headers, header::CONTENT_SECURITY_POLICY.as_str()),
        HTML_CSP
    );
    assert!(
        header_text(&html_headers, header::CONTENT_SECURITY_POLICY.as_str())
            .contains("script-src 'self'")
    );
    assert!(
        header_text(&html_headers, header::CONTENT_SECURITY_POLICY.as_str())
            .contains("style-src 'self'")
    );

    let (status, bootstrap_headers, bootstrap_body) =
        get(&app, "/api/bootstrap", HOST, None, "same-origin", "empty").await;
    assert_eq!(status, StatusCode::OK);
    let build = assert_security_headers(&bootstrap_headers, None);
    let bootstrap: serde_json::Value =
        serde_json::from_slice(&bootstrap_body).expect("bootstrap JSON");
    let token = bootstrap["token"].as_str().expect("bootstrap token");
    let mut api_headers = headers(HOST, None, "same-origin", "empty");
    api_headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {token}")).expect("bearer header"),
    );
    let (status, options_headers, _) = get_with_headers(&app, "/api/options", api_headers).await;
    assert_eq!(status, StatusCode::OK);
    assert_security_headers(&options_headers, Some(&build));
    assert_eq!(
        header_text(&options_headers, header::CONTENT_TYPE.as_str()),
        "application/json"
    );
}

#[tokio::test]
async fn asset_routes_have_no_fallback_or_cross_site_bypass() {
    let app = router(content(), PORT).expect("HTTP router builds");
    let (status, root_headers, root_bytes) =
        get(&app, "/", HOST, None, "same-origin", "document").await;
    assert_eq!(status, StatusCode::OK);
    let build = assert_html_security_headers(&root_headers, None);
    let html = String::from_utf8(root_bytes).expect("embedded index is UTF-8");
    let script = linked_paths(&html, "src").pop().expect("script path");
    let stylesheet = linked_paths(&html, "href").pop().expect("stylesheet path");

    for path in [
        "/assets",
        "/assets/",
        "/assets/missing.js",
        "/assets/index.js.map",
        "/assets/../index.html",
        "/assets/%2e%2e/index.html",
        "/assets/%2Findex.js",
        "/assets\\index.js",
        "/assets/./index.js",
        "/assets//index.js",
        "/etc/passwd",
    ] {
        let (status, headers, body) = get(&app, path, HOST, None, "same-origin", "empty").await;
        assert_ne!(status, StatusCode::OK, "unexpected public fallback: {path}");
        assert!(
            status == StatusCode::NOT_FOUND || status == StatusCode::FORBIDDEN,
            "unexpected status for {path}: {status}"
        );
        assert_security_headers(&headers, Some(&build));
        assert_ne!(body, b"<!doctype html>".to_vec());
    }

    for path in [
        format!("{script}.map"),
        format!("{stylesheet}.map"),
        "/Cargo.toml".to_owned(),
        "/src/main.rs".to_owned(),
    ] {
        let (status, headers, body) = get(&app, &path, HOST, None, "same-origin", "empty").await;
        assert_ne!(status, StatusCode::OK, "unexpected fallback: {path}");
        assert_security_headers(&headers, Some(&build));
        assert!(body.len() < 1_024, "unexpected file fallback body: {path}");
    }

    let (status, query_headers, _) = get(
        &app,
        &format!("{script}?cache=1"),
        HOST,
        None,
        "same-origin",
        "empty",
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_security_headers(&query_headers, Some(&build));

    for path in ["/", "/index.html", script.as_str(), stylesheet.as_str()] {
        let (status, headers, _) =
            get(&app, path, "localhost:38125", None, "same-origin", "empty").await;
        assert_eq!(status, StatusCode::FORBIDDEN, "wrong host accepted: {path}");
        assert_security_headers(&headers, Some(&build));

        let (status, headers, _) = get(
            &app,
            path,
            HOST,
            Some("http://evil.test"),
            "cross-site",
            "empty",
        )
        .await;
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "cross-site origin accepted: {path}"
        );
        assert_security_headers(&headers, Some(&build));
    }

    for (path, destination) in [
        ("/", "document"),
        ("/index.html", "document"),
        (script.as_str(), "script"),
        (stylesheet.as_str(), "style"),
    ] {
        for site in ["cross-site", "same-site"] {
            let (status, headers, _) = get(&app, path, HOST, None, site, destination).await;
            assert_eq!(
                status,
                StatusCode::FORBIDDEN,
                "asset accepted fetch site {site}: {path}"
            );
            assert_security_headers(&headers, Some(&build));
        }

        let mut cross_site_without_metadata = HeaderMap::new();
        cross_site_without_metadata.insert(header::HOST, HeaderValue::from_static(HOST));
        cross_site_without_metadata
            .insert(header::ORIGIN, HeaderValue::from_static("http://evil.test"));
        let (status, response_headers, _) =
            get_with_headers(&app, path, cross_site_without_metadata).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_security_headers(&response_headers, Some(&build));

        let mut duplicate_site = headers(HOST, None, "same-origin", destination);
        duplicate_site.append("sec-fetch-site", HeaderValue::from_static("same-origin"));
        let (status, response_headers, _) = get_with_headers(&app, path, duplicate_site).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_security_headers(&response_headers, Some(&build));

        let mut duplicate_origin = headers(
            HOST,
            Some("http://127.0.0.1:38125"),
            "same-origin",
            destination,
        );
        duplicate_origin.append(
            header::ORIGIN,
            HeaderValue::from_static("http://127.0.0.1:38125"),
        );
        let (status, response_headers, _) = get_with_headers(&app, path, duplicate_origin).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_security_headers(&response_headers, Some(&build));
    }
}

struct ServerGuard {
    child: Child,
    directory: PathBuf,
}

impl Drop for ServerGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.directory);
    }
}

struct SocketResponse {
    status: u16,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
}

fn socket_get(port: u16, path: &str) -> SocketResponse {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("server accepts socket");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("read timeout");
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .expect("write timeout");
    let destination = if path.ends_with(".js") {
        "script"
    } else if path.ends_with(".css") {
        "style"
    } else {
        "document"
    };
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\nSec-Fetch-Site: same-origin\r\nSec-Fetch-Dest: {destination}\r\n\r\n"
    )
    .expect("write HTTP request");

    let mut reader = BufReader::new(stream);
    let mut status_line = String::new();
    reader
        .read_line(&mut status_line)
        .expect("read HTTP status");
    let status = status_line
        .split_whitespace()
        .nth(1)
        .expect("HTTP status code")
        .parse()
        .expect("numeric HTTP status code");
    let mut headers = BTreeMap::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).expect("read HTTP header");
        if line == "\r\n" {
            break;
        }
        let (name, value) = line.split_once(':').expect("well-formed HTTP header");
        headers.insert(name.to_ascii_lowercase(), value.trim().to_owned());
    }
    let mut body = Vec::new();
    if let Some(length) = headers
        .get("content-length")
        .map(|value| value.parse::<usize>().expect("content length"))
    {
        body.resize(length, 0);
        reader.read_exact(&mut body).expect("read HTTP body");
    } else {
        reader.read_to_end(&mut body).expect("read HTTP body");
    }
    SocketResponse {
        status,
        headers,
        body,
    }
}

#[test]
fn embedded_assets_work_from_a_different_process_working_directory() {
    let sequence = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "forge-server-browser-assets-{}-{sequence}",
        std::process::id()
    ));
    fs::create_dir(&directory).expect("unique child working directory");

    let mut child = Command::new(env!("CARGO_BIN_EXE_forge-server"))
        .args(["--port", "0"])
        .current_dir(&directory)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("forge-server test binary starts");
    let stdout = child.stdout.take().expect("server stdout");
    let _server = ServerGuard { child, directory };
    let mut line = String::new();
    BufReader::new(stdout)
        .read_line(&mut line)
        .expect("server startup line");
    let port = line
        .trim()
        .strip_prefix("Adventure Forge local API: http://127.0.0.1:")
        .expect("server announces loopback port")
        .parse::<u16>()
        .expect("announced port");

    let response = socket_get(port, "/");
    assert_eq!(response.status, 200);
    assert_eq!(
        response.headers.get("content-type").unwrap(),
        "text/html; charset=utf-8"
    );
    assert_eq!(response.headers.get("cache-control").unwrap(), "no-store");
    let html = String::from_utf8(response.body).expect("child index is UTF-8");
    assert!(html.contains("<div id=\"root\"></div>"));
    let script = linked_paths(&html, "src").pop().expect("child script path");

    let script_response = socket_get(port, &script);
    assert_eq!(script_response.status, 200);
    assert_eq!(
        script_response.headers.get("content-type").unwrap(),
        "text/javascript; charset=utf-8"
    );
    assert!(script_response.body.len() > 1_024);

    // The guard's Drop kills only this child and removes only the exact
    // temporary directory it created.
}
