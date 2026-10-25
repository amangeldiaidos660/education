use std::{collections::HashMap, time::Duration};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{HeaderMap, HeaderValue, Request, StatusCode, header},
    routing::get,
};
use serde_json::{Value, json};
use sozbricks_api::{app, config::{AuthConfig, Config}};
use tower::ServiceExt;

const USER_ID: &str = "17bcb386-6840-4d10-8715-a45f73b92e6d";

fn user() -> Value {
    json!({
        "id": USER_ID, "role": "authenticated", "email": "player@example.com",
        "is_anonymous": false, "app_metadata": {"providers": ["google"]},
        "user_metadata": {"full_name": "  Қазақ ойыншысы  ", "admin": true},
        "refresh_token": "must-not-be-returned"
    })
}

struct Fixture {
    router: Router,
    worker: tokio::task::JoinHandle<()>,
}

impl Drop for Fixture {
    fn drop(&mut self) { self.worker.abort(); }
}

async fn fixture(status: StatusCode, body: String, delay: Duration, timeout: Duration) -> Fixture {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let upstream = Router::new().route("/auth/v1/user", get(move |headers: HeaderMap| {
        let body = body.clone();
        async move {
            assert_eq!(headers[header::AUTHORIZATION], "Bearer test-token");
            assert_eq!(headers["apikey"], "sb_publishable_test");
            tokio::time::sleep(delay).await;
            (status, body)
        }
    }));
    let worker = tokio::spawn(async move { axum::serve(listener, upstream).await.unwrap(); });
    let mut config = Config::from_values(&HashMap::new()).unwrap();
    config.auth = Some(AuthConfig {
        user_url: format!("http://{address}/auth/v1/user").parse().unwrap(),
        publishable_key: HeaderValue::from_static("sb_publishable_test"),
        timeout,
    });
    Fixture { router: app(config), worker }
}

async fn request(router: Router, token: Option<&str>) -> (StatusCode, HeaderMap, Value) {
    let mut request = Request::get("/v1/auth/me");
    if let Some(token) = token { request = request.header(header::AUTHORIZATION, token); }
    let response = router.oneshot(request.body(Body::empty()).unwrap()).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
    (status, headers, body)
}

#[tokio::test]
async fn bearer_is_required_before_auth_configuration_is_used() {
    let router = app(Config::from_values(&HashMap::new()).unwrap());
    for token in [None, Some("Basic abc"), Some("Bearer "), Some("Bearer a b"), Some("Bearer x,y")] {
        let (status, headers, body) = request(router.clone(), token).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(headers[header::WWW_AUTHENTICATE], "Bearer");
        assert_eq!(headers[header::CACHE_CONTROL], "no-store");
        assert!(headers.contains_key("x-request-id"));
        assert_eq!(body["error"]["code"], "unauthorized");
    }
    let (status, _, body) = request(router.clone(), Some("Bearer test-token")).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "auth_not_configured");
    let response = router.oneshot(Request::get("/v1/auth/me")
        .header(header::AUTHORIZATION, "Bearer first")
        .header(header::AUTHORIZATION, "Bearer second")
        .body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn verified_identity_uses_auth_server_and_only_returns_public_fields() {
    let fixture = fixture(StatusCode::OK, user().to_string(), Duration::ZERO, Duration::from_secs(2)).await;
    let (status, _, body) = request(fixture.router.clone(), Some("bearer test-token")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({"id": USER_ID, "email": "player@example.com", "display_name": "Қазақ ойыншысы", "provider": "google"}));
}

#[tokio::test]
async fn anonymous_non_google_and_privileged_identities_are_rejected() {
    let mut variants = Vec::new();
    let mut anonymous = user(); anonymous["is_anonymous"] = json!(true); variants.push(anonymous);
    let mut password = user(); password["app_metadata"]["providers"] = json!(["email"]); variants.push(password);
    let mut privileged = user(); privileged["role"] = json!("service_role"); variants.push(privileged);
    let mut missing_id = user(); missing_id["id"] = json!("00000000-0000-0000-0000-000000000000"); variants.push(missing_id);
    for body in variants {
        let fixture = fixture(StatusCode::OK, body.to_string(), Duration::ZERO, Duration::from_secs(2)).await;
        let (status, _, body) = request(fixture.router.clone(), Some("Bearer test-token")).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["error"]["code"], "unauthorized");
    }
}

#[tokio::test]
async fn rejected_tokens_and_upstream_failures_have_distinct_errors() {
    for upstream in [StatusCode::UNAUTHORIZED, StatusCode::FORBIDDEN, StatusCode::TOO_MANY_REQUESTS, StatusCode::INTERNAL_SERVER_ERROR, StatusCode::FOUND] {
        let fixture = fixture(upstream, "upstream-secret".into(), Duration::ZERO, Duration::from_secs(2)).await;
        let (status, _, body) = request(fixture.router.clone(), Some("Bearer test-token")).await;
        let rejected = matches!(upstream, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN);
        assert_eq!(status, if rejected { StatusCode::UNAUTHORIZED } else { StatusCode::SERVICE_UNAVAILABLE });
        assert_eq!(body["error"]["code"], if rejected { "unauthorized" } else { "auth_unavailable" });
        assert!(!body.to_string().contains("upstream-secret"));
        assert!(!body.to_string().contains("test-token"));
    }
}

#[tokio::test]
async fn invalid_oversized_and_slow_auth_responses_fail_closed() {
    for (body, delay) in [
        ("not JSON".to_owned(), Duration::ZERO),
        (" ".repeat(65537), Duration::ZERO),
        (user().to_string(), Duration::from_secs(1)),
    ] {
        let fixture = fixture(StatusCode::OK, body, delay, Duration::from_millis(100)).await;
        let (status, _, body) = request(fixture.router.clone(), Some("Bearer test-token")).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["error"]["code"], "auth_unavailable");
    }
}

#[test]
fn auth_configuration_requires_https_and_publishable_keys_without_leaking_values() {
    let valid = HashMap::from([
        ("SUPABASE_URL".to_owned(), "https://example.supabase.co".to_owned()),
        ("SUPABASE_PUBLISHABLE_KEY".to_owned(), "sb_publishable_test".to_owned()),
    ]);
    let config = Config::from_values(&valid).unwrap();
    assert_eq!(config.auth.unwrap().user_url.as_str(), "https://example.supabase.co/auth/v1/user");
    for (key, value) in [
        ("SUPABASE_URL", "http://example.supabase.co"),
        ("SUPABASE_URL", "https://user:password@example.supabase.co"),
        ("SUPABASE_URL", "https://example.supabase.co/path"),
        ("SUPABASE_URL", "https://example.supabase.co?secret=x"),
        ("SUPABASE_URL", "https://example.supabase.co#fragment"),
        ("SUPABASE_PUBLISHABLE_KEY", "sb_secret_never-log-this"),
        ("SUPABASE_PUBLISHABLE_KEY", "sb_publishable_"),
        ("SUPABASE_PUBLISHABLE_KEY", "sb_publishable_bad key"),
        ("AUTH_TIMEOUT_SECONDS", "0"),
        ("AUTH_TIMEOUT_SECONDS", "11"),
    ] {
        let mut values = valid.clone(); values.insert(key.into(), value.into());
        let error = Config::from_values(&values).err().expect("invalid auth configuration").to_string();
        assert!(error.contains(key));
        assert!(!error.contains(value));
    }
    let mut values = valid.clone(); values.remove("SUPABASE_URL");
    assert!(Config::from_values(&values).is_err());
    let mut values = valid; values.remove("SUPABASE_PUBLISHABLE_KEY");
    assert!(Config::from_values(&values).unwrap().auth.is_none());
}
