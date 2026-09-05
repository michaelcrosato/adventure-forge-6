use crate::ServiceError;
use axum::http::{
    HeaderMap, HeaderName, Method, StatusCode, Uri,
    header::{AUTHORIZATION, HOST, ORIGIN},
};

const SAME_ORIGIN: &str = "same-origin";
const NONE_SITE: &str = "none";
const EMPTY_FETCH_DEST: &str = "empty";
const DOCUMENT_FETCH_DEST: &str = "document";
const BEARER_PREFIX: &str = "Bearer ";

#[derive(Clone, Copy)]
enum PublicAssetKind {
    Document,
    Script,
    Style,
    Image,
    Unsupported,
}

/// A process-scoped capability for the loopback adapter.
///
/// This is deliberately not user authentication. The token is only held in
/// memory and is never included in the debug representation or a URL.
pub(super) struct AccessPolicy {
    host: String,
    origin: String,
    token: String,
    instance_id: String,
}

impl AccessPolicy {
    pub(super) fn new(port: u16) -> Result<Self, ServiceError> {
        if port == 0 {
            return Err(ServiceError::InvalidInput);
        }

        let host = format!("127.0.0.1:{port}");
        let origin = format!("http://{host}");
        let mut secret = [0_u8; 32];
        getrandom::fill(&mut secret).map_err(|_| ServiceError::Internal)?;
        let token = secret.iter().map(|byte| format!("{byte:02x}")).collect();
        // Keep the process identity independent from the bearer capability.
        // It is safe public metadata used to bind a browser journal to the
        // process that owns its in-memory sessions.
        getrandom::fill(&mut secret).map_err(|_| ServiceError::Internal)?;
        let instance_id = secret.iter().map(|byte| format!("{byte:02x}")).collect();

        Ok(Self {
            host,
            origin,
            token,
            instance_id,
        })
    }

    /// Authorize one request before a router handler sees it.
    ///
    /// The caller must use the returned policy token only for the bootstrap
    /// response. Every other API request presents it as a Bearer credential.
    pub(super) fn authorize(
        &self,
        method: &Method,
        uri: &Uri,
        headers: &HeaderMap,
    ) -> Result<(), StatusCode> {
        reject_unsafe_uri(uri)?;
        reject_forwarding_headers(headers)?;
        self.require_host(headers)?;
        reject_duplicate_security_headers(headers)?;
        let authorization =
            singleton_header(headers, &AUTHORIZATION).map_err(|_| StatusCode::UNAUTHORIZED)?;

        match uri.path() {
            "/" => {
                if *method != Method::GET {
                    return Err(StatusCode::FORBIDDEN);
                }
                self.optional_origin(headers)?;
                require_document_navigation(headers)?;
                // The root document is public. A singleton credential is
                // ignored; duplicate credentials were rejected above.
                let _ = authorization;
                Ok(())
            }
            "/api/bootstrap" => {
                if *method != Method::GET {
                    return Err(StatusCode::FORBIDDEN);
                }
                require_fetch_metadata(headers)?;
                self.optional_origin(headers)?;
                // Bootstrap is intentionally unauthenticated. Reject even a
                // singleton credential so it cannot become an alternate API.
                if authorization.is_some() {
                    return Err(StatusCode::UNAUTHORIZED);
                }
                Ok(())
            }
            path if public_asset_kind(path).is_some() => {
                if *method != Method::GET {
                    return Err(StatusCode::FORBIDDEN);
                }
                self.optional_origin(headers)?;
                match public_asset_kind(path).expect("asset kind checked above") {
                    PublicAssetKind::Document => require_document_asset_fetch(headers)?,
                    PublicAssetKind::Script => require_asset_fetch(headers, "script")?,
                    PublicAssetKind::Style => require_asset_fetch(headers, "style")?,
                    PublicAssetKind::Image => require_asset_fetch(headers, "image")?,
                    PublicAssetKind::Unsupported => return Err(StatusCode::FORBIDDEN),
                }
                // Public assets do not need the API bearer capability. A
                // singleton credential is harmless and is intentionally
                // ignored, while duplicates were rejected above.
                let _ = authorization;
                Ok(())
            }
            path if path == "/api" || path.starts_with("/api/") => {
                require_fetch_metadata(headers)?;
                require_bearer(authorization, &self.token)?;
                if *method == Method::OPTIONS {
                    // There is no CORS preflight surface.
                    return Err(StatusCode::FORBIDDEN);
                }
                if *method != Method::GET && *method != Method::HEAD {
                    self.require_exact_origin(headers)?;
                } else {
                    self.optional_origin(headers)?;
                }
                Ok(())
            }
            // The root router owns all non-API 404s. They still pass through
            // the exact host and URI checks above.
            _ => Ok(()),
        }
    }

    pub(super) fn token(&self) -> &str {
        &self.token
    }

    pub(super) fn instance_id(&self) -> &str {
        &self.instance_id
    }

    fn require_host(&self, headers: &HeaderMap) -> Result<(), StatusCode> {
        let host = singleton_header(headers, &HOST)?;
        if host != Some(self.host.as_str()) {
            return Err(StatusCode::FORBIDDEN);
        }
        Ok(())
    }

    fn optional_origin(&self, headers: &HeaderMap) -> Result<(), StatusCode> {
        match singleton_header(headers, &ORIGIN)? {
            None => Ok(()),
            Some(origin) if origin == self.origin => Ok(()),
            Some(_) => Err(StatusCode::FORBIDDEN),
        }
    }

    fn require_exact_origin(&self, headers: &HeaderMap) -> Result<(), StatusCode> {
        match singleton_header(headers, &ORIGIN)? {
            Some(origin) if origin == self.origin => Ok(()),
            _ => Err(StatusCode::FORBIDDEN),
        }
    }
}

fn public_asset_kind(path: &str) -> Option<PublicAssetKind> {
    let asset = super::assets::get(path)?;
    match asset.content_type {
        "text/html; charset=utf-8" => Some(PublicAssetKind::Document),
        "text/javascript; charset=utf-8" => Some(PublicAssetKind::Script),
        "text/css; charset=utf-8" => Some(PublicAssetKind::Style),
        "image/svg+xml" | "image/x-icon" => Some(PublicAssetKind::Image),
        _ => Some(PublicAssetKind::Unsupported),
    }
}

fn reject_unsafe_uri(uri: &Uri) -> Result<(), StatusCode> {
    if uri.query().is_some() || uri.scheme().is_some() || uri.authority().is_some() {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(())
}

fn reject_forwarding_headers(headers: &HeaderMap) -> Result<(), StatusCode> {
    if headers.keys().any(|name| {
        name == HeaderName::from_static("forwarded") || name.as_str().starts_with("x-forwarded-")
    }) {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(())
}

fn reject_duplicate_security_headers(headers: &HeaderMap) -> Result<(), StatusCode> {
    for name in headers.keys() {
        let is_security_header = name == HOST
            || name == ORIGIN
            || name == AUTHORIZATION
            || name.as_str().starts_with("sec-fetch-");
        if is_security_header && headers.get_all(name).iter().count() > 1 {
            if name == AUTHORIZATION {
                return Err(StatusCode::UNAUTHORIZED);
            }
            return Err(StatusCode::FORBIDDEN);
        }
    }
    Ok(())
}

fn singleton_header<'a>(
    headers: &'a HeaderMap,
    name: &HeaderName,
) -> Result<Option<&'a str>, StatusCode> {
    let mut values = headers.get_all(name).iter();
    let Some(value) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() {
        return Err(if name == AUTHORIZATION {
            StatusCode::UNAUTHORIZED
        } else {
            StatusCode::FORBIDDEN
        });
    }
    value.to_str().map(Some).map_err(|_| {
        if name == AUTHORIZATION {
            StatusCode::UNAUTHORIZED
        } else {
            StatusCode::FORBIDDEN
        }
    })
}

fn require_fetch_metadata(headers: &HeaderMap) -> Result<(), StatusCode> {
    if singleton_header(headers, &HeaderName::from_static("sec-fetch-site"))? != Some(SAME_ORIGIN)
        || singleton_header(headers, &HeaderName::from_static("sec-fetch-dest"))?
            != Some(EMPTY_FETCH_DEST)
    {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(())
}

fn require_asset_fetch(headers: &HeaderMap, destination: &str) -> Result<(), StatusCode> {
    if singleton_header(headers, &HeaderName::from_static("sec-fetch-site"))? != Some(SAME_ORIGIN)
        || singleton_header(headers, &HeaderName::from_static("sec-fetch-dest"))?
            != Some(destination)
    {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(())
}

fn require_document_navigation(headers: &HeaderMap) -> Result<(), StatusCode> {
    let site = singleton_header(headers, &HeaderName::from_static("sec-fetch-site"))?;
    if let Some(site) = site
        && site != NONE_SITE
        && site != SAME_ORIGIN
    {
        return Err(StatusCode::FORBIDDEN);
    }
    if let Some(destination) =
        singleton_header(headers, &HeaderName::from_static("sec-fetch-dest"))?
    {
        let allowed = match site {
            Some(NONE_SITE) => destination == DOCUMENT_FETCH_DEST,
            Some(SAME_ORIGIN) => {
                destination == DOCUMENT_FETCH_DEST || destination == EMPTY_FETCH_DEST
            }
            None => destination == DOCUMENT_FETCH_DEST || destination == EMPTY_FETCH_DEST,
            _ => false,
        };
        if !allowed {
            return Err(StatusCode::FORBIDDEN);
        }
    }
    Ok(())
}

fn require_document_asset_fetch(headers: &HeaderMap) -> Result<(), StatusCode> {
    let site = singleton_header(headers, &HeaderName::from_static("sec-fetch-site"))?;
    if site != Some(NONE_SITE) && site != Some(SAME_ORIGIN) {
        return Err(StatusCode::FORBIDDEN);
    }
    let destination = singleton_header(headers, &HeaderName::from_static("sec-fetch-dest"))?;
    let allowed = match site {
        Some(NONE_SITE) => destination == Some(DOCUMENT_FETCH_DEST),
        Some(SAME_ORIGIN) => {
            destination == Some(DOCUMENT_FETCH_DEST) || destination == Some(EMPTY_FETCH_DEST)
        }
        _ => false,
    };
    if !allowed {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(())
}

fn require_bearer(authorization: Option<&str>, token: &str) -> Result<(), StatusCode> {
    let Some(authorization) = authorization else {
        return Err(StatusCode::UNAUTHORIZED);
    };
    let Some(candidate) = authorization.strip_prefix(BEARER_PREFIX) else {
        return Err(StatusCode::UNAUTHORIZED);
    };
    if constant_time_equal(candidate.as_bytes(), token.as_bytes()) {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    for index in 0..left.len().max(right.len()) {
        difference |= usize::from(left.get(index).copied().unwrap_or_default())
            ^ usize::from(right.get(index).copied().unwrap_or_default());
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    const PORT: u16 = 43_217;

    fn policy() -> AccessPolicy {
        AccessPolicy::new(PORT).expect("test policy token")
    }

    #[test]
    fn token_is_a_fresh_256_bit_hex_capability() {
        let first = policy();
        let second = policy();
        assert_eq!(first.token().len(), 64);
        assert!(
            first
                .token()
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        );
        assert_ne!(first.token(), second.token());
        assert_eq!(first.instance_id().len(), 64);
        assert!(
            first
                .instance_id()
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        );
        assert_ne!(first.instance_id(), second.instance_id());
        assert_ne!(first.token(), first.instance_id());
    }

    fn headers(policy: &AccessPolicy) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(HOST, HeaderValue::from_static("127.0.0.1:43217"));
        headers.insert("sec-fetch-site", HeaderValue::from_static("same-origin"));
        headers.insert("sec-fetch-dest", HeaderValue::from_static("empty"));
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", policy.token()))
                .expect("test bearer header"),
        );
        headers
    }

    fn uri(path: &str) -> Uri {
        path.parse().expect("valid test URI")
    }

    #[test]
    fn bootstrap_allows_same_origin_fetch_without_origin_but_not_metadata_bypass() {
        let policy = policy();
        let mut bootstrap = headers(&policy);
        bootstrap.remove(AUTHORIZATION);
        assert_eq!(
            policy.authorize(&Method::GET, &uri("/api/bootstrap"), &bootstrap),
            Ok(())
        );

        let mut missing_site = bootstrap.clone();
        missing_site.remove("sec-fetch-site");
        assert_eq!(
            policy.authorize(&Method::GET, &uri("/api/bootstrap"), &missing_site),
            Err(StatusCode::FORBIDDEN)
        );

        bootstrap.insert(ORIGIN, HeaderValue::from_static("http://evil.test"));
        assert_eq!(
            policy.authorize(&Method::GET, &uri("/api/bootstrap"), &bootstrap),
            Err(StatusCode::FORBIDDEN)
        );
        bootstrap.insert(ORIGIN, HeaderValue::from_static("http://127.0.0.1:43217"));
        assert_eq!(
            policy.authorize(&Method::GET, &uri("/api/bootstrap"), &bootstrap),
            Ok(())
        );

        let mut wrong_dest = bootstrap.clone();
        wrong_dest.insert("sec-fetch-dest", HeaderValue::from_static("script"));
        assert_eq!(
            policy.authorize(&Method::GET, &uri("/api/bootstrap"), &wrong_dest),
            Err(StatusCode::FORBIDDEN)
        );
    }

    #[test]
    fn bootstrap_rejects_auth_duplicates_and_non_get_methods() {
        let policy = policy();
        let mut duplicate_auth = headers(&policy);
        duplicate_auth.remove(AUTHORIZATION);
        duplicate_auth.append(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", policy.token()))
                .expect("test bearer header"),
        );
        duplicate_auth.append(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", policy.token()))
                .expect("test bearer header"),
        );
        assert_eq!(
            policy.authorize(&Method::GET, &uri("/api/bootstrap"), &duplicate_auth),
            Err(StatusCode::UNAUTHORIZED)
        );

        let mut singleton_auth = headers(&policy);
        singleton_auth.remove(AUTHORIZATION);
        singleton_auth.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", policy.token()))
                .expect("test bearer header"),
        );
        assert_eq!(
            policy.authorize(&Method::GET, &uri("/api/bootstrap"), &singleton_auth),
            Err(StatusCode::UNAUTHORIZED)
        );
        assert_eq!(
            policy.authorize(&Method::HEAD, &uri("/api/bootstrap"), &singleton_auth),
            Err(StatusCode::FORBIDDEN)
        );
    }

    #[test]
    fn root_is_public_but_host_origin_and_method_remain_bound() {
        let policy = policy();
        let mut root = HeaderMap::new();
        root.insert(HOST, HeaderValue::from_static("127.0.0.1:43217"));
        assert_eq!(policy.authorize(&Method::GET, &uri("/"), &root), Ok(()));

        root.insert(ORIGIN, HeaderValue::from_static("http://evil.test"));
        assert_eq!(
            policy.authorize(&Method::GET, &uri("/"), &root),
            Err(StatusCode::FORBIDDEN)
        );
        root.insert(ORIGIN, HeaderValue::from_static("http://127.0.0.1:43217"));
        assert_eq!(
            policy.authorize(&Method::POST, &uri("/"), &root),
            Err(StatusCode::FORBIDDEN)
        );
    }

    #[test]
    fn authenticated_reads_and_mutations_have_distinct_origin_rules() {
        let policy = policy();
        let mut read = headers(&policy);
        read.remove(ORIGIN);
        assert_eq!(
            policy.authorize(&Method::GET, &uri("/api/session"), &read),
            Ok(())
        );

        let mut mutation = read.clone();
        assert_eq!(
            policy.authorize(&Method::POST, &uri("/api/session"), &mutation),
            Err(StatusCode::FORBIDDEN)
        );
        mutation.insert(ORIGIN, HeaderValue::from_static("http://evil.test"));
        assert_eq!(
            policy.authorize(&Method::POST, &uri("/api/session"), &mutation),
            Err(StatusCode::FORBIDDEN)
        );
        mutation.insert(ORIGIN, HeaderValue::from_static("http://127.0.0.1:43217"));
        assert_eq!(
            policy.authorize(&Method::POST, &uri("/api/session"), &mutation),
            Ok(())
        );

        let mut bad_token = mutation;
        bad_token.insert(AUTHORIZATION, HeaderValue::from_static("Bearer wrong"));
        assert_eq!(
            policy.authorize(&Method::POST, &uri("/api/session"), &bad_token),
            Err(StatusCode::UNAUTHORIZED)
        );
    }

    #[test]
    fn forwarding_absolute_query_and_cors_requests_fail_closed() {
        let policy = policy();
        let valid = headers(&policy);
        let mut forwarded = valid.clone();
        forwarded.insert("x-forwarded-host", HeaderValue::from_static("evil.test"));
        assert_eq!(
            policy.authorize(&Method::GET, &uri("/api/session"), &forwarded),
            Err(StatusCode::FORBIDDEN)
        );
        assert_eq!(
            policy.authorize(
                &Method::GET,
                &"http://127.0.0.1:43217/api/session".parse().unwrap(),
                &valid
            ),
            Err(StatusCode::FORBIDDEN)
        );
        assert_eq!(
            policy.authorize(&Method::GET, &uri("/api/session?token=secret"), &valid),
            Err(StatusCode::FORBIDDEN)
        );
        assert_eq!(
            policy.authorize(&Method::OPTIONS, &uri("/api/session"), &valid),
            Err(StatusCode::FORBIDDEN)
        );
    }

    #[test]
    fn duplicate_host_origin_and_fetch_metadata_fail_closed() {
        let policy = policy();

        let mut duplicate_host = headers(&policy);
        duplicate_host.append(HOST, HeaderValue::from_static("127.0.0.1:43217"));
        assert_eq!(
            policy.authorize(&Method::GET, &uri("/api/session"), &duplicate_host),
            Err(StatusCode::FORBIDDEN)
        );

        let mut duplicate_origin = headers(&policy);
        duplicate_origin.insert(ORIGIN, HeaderValue::from_static("http://127.0.0.1:43217"));
        duplicate_origin.append(ORIGIN, HeaderValue::from_static("http://127.0.0.1:43217"));
        assert_eq!(
            policy.authorize(&Method::GET, &uri("/api/session"), &duplicate_origin),
            Err(StatusCode::FORBIDDEN)
        );

        let mut duplicate_site = headers(&policy);
        duplicate_site.append("sec-fetch-site", HeaderValue::from_static("same-origin"));
        assert_eq!(
            policy.authorize(&Method::GET, &uri("/api/session"), &duplicate_site),
            Err(StatusCode::FORBIDDEN)
        );
    }

    #[test]
    fn wrong_host_and_origin_are_policy_errors_and_non_api_404s_keep_host_check() {
        let policy = policy();
        let mut wrong_host = headers(&policy);
        wrong_host.insert(HOST, HeaderValue::from_static("localhost:43217"));
        assert_eq!(
            policy.authorize(&Method::GET, &uri("/api/session"), &wrong_host),
            Err(StatusCode::FORBIDDEN)
        );

        let mut wrong_origin = headers(&policy);
        wrong_origin.insert(ORIGIN, HeaderValue::from_static("null"));
        assert_eq!(
            policy.authorize(&Method::GET, &uri("/api/session"), &wrong_origin),
            Err(StatusCode::FORBIDDEN)
        );

        let unknown = HeaderMap::from_iter([(HOST, HeaderValue::from_static("127.0.0.1:43217"))]);
        assert_eq!(
            policy.authorize(&Method::GET, &uri("/not-a-route"), &unknown),
            Ok(())
        );
    }
}
