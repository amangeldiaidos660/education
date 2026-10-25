use std::collections::HashMap;

use axum::{body::{Body, to_bytes}, http::{Request, StatusCode, header}, response::Response};
use serde_json::{Value, json};
use sozbricks_api::{app, config::Config};
use tower::ServiceExt;
use uuid::Uuid;

fn config(values: &[(&str, &str)]) -> Config {
    Config::from_values(&values.iter().map(|(key, value)| (key.to_string(), value.to_string())).collect()).unwrap()
}

async fn body(response: Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap()
}

#[tokio::test]
async fn live_and_ready_distinguish_scaffold_readiness() {
    let router = app(config(&[]));
    let live = router.clone().oneshot(Request::get("/health/live").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(live.status(), StatusCode::OK);
    assert_eq!(body(live).await, json!({"status": "ok"}));
    let ready = router.oneshot(Request::get("/health/ready").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(ready.status(), StatusCode::OK);
    assert_eq!(body(ready).await, json!({"status": "ready", "stage": "scaffold"}));
}

#[tokio::test]
async fn version_exposes_only_public_build_metadata() {
    let response = app(config(&[("APP_REVISION", "test-commit")])).oneshot(Request::get("/version").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(body(response).await, json!({"name": "sozbricks-api", "version": env!("CARGO_PKG_VERSION"), "revision": "test-commit"}));
}

#[tokio::test]
async fn errors_are_json_and_have_request_ids() {
    for (method, path, status, code) in [
        ("GET", "/missing", StatusCode::NOT_FOUND, "not_found"),
        ("POST", "/version", StatusCode::METHOD_NOT_ALLOWED, "method_not_allowed"),
    ] {
        let response = app(config(&[])).oneshot(Request::builder().method(method).uri(path).body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), status);
        Uuid::parse_str(response.headers()["x-request-id"].to_str().unwrap()).unwrap();
        let error = body(response).await;
        assert_eq!(error["error"]["code"], code);
        assert!(error["error"]["message"].is_string());
    }
}

#[tokio::test]
async fn request_ids_are_generated_by_the_server() {
    let router = app(config(&[]));
    let request = || Request::get("/version").header("x-request-id", "untrusted").body(Body::empty()).unwrap();
    let first = router.clone().oneshot(request()).await.unwrap();
    let second = router.oneshot(request()).await.unwrap();
    let first = first.headers()["x-request-id"].to_str().unwrap();
    let second = second.headers()["x-request-id"].to_str().unwrap();
    assert_ne!(first, second);
    assert_ne!(first, "untrusted");
    Uuid::parse_str(first).unwrap();
}

#[tokio::test]
async fn head_is_supported_without_a_response_body() {
    let response = app(config(&[])).oneshot(Request::builder().method("HEAD").uri("/health/live").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(to_bytes(response.into_body(), 65536).await.unwrap().is_empty());
}

#[tokio::test]
async fn oversized_bodies_are_rejected_with_and_without_content_length() {
    for declared in [false, true] {
        let mut request = Request::get("/version");
        if declared { request = request.header(header::CONTENT_LENGTH, "9"); }
        let response = app(config(&[("MAX_REQUEST_BODY_BYTES", "8")])).oneshot(request.body(Body::from("123456789")).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(body(response).await["error"]["code"], "payload_too_large");
    }
}

#[tokio::test]
async fn cors_allows_only_configured_origins() {
    let router = app(config(&[("CORS_ALLOWED_ORIGINS", "https://game.example")]));
    for (origin, allowed) in [("https://game.example", true), ("https://other.example", false)] {
        let response = router.clone().oneshot(Request::get("/version").header(header::ORIGIN, origin).body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.headers().contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN), allowed);
        if allowed { assert_eq!(response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN], origin); }
        assert!(!response.headers().contains_key(header::ACCESS_CONTROL_ALLOW_CREDENTIALS));
    }
}

#[tokio::test]
async fn cors_preflight_allows_get_with_bearer_auth() {
    let response = app(config(&[("CORS_ALLOWED_ORIGINS", "https://game.example")])).oneshot(Request::builder().method("OPTIONS").uri("/version")
        .header(header::ORIGIN, "https://game.example")
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
        .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "authorization")
        .body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN], "https://game.example");
    assert!(response.headers()[header::ACCESS_CONTROL_ALLOW_METHODS].to_str().unwrap().contains("GET"));
    assert!(response.headers()[header::ACCESS_CONTROL_ALLOW_HEADERS].to_str().unwrap().contains("authorization"));
}

#[test]
fn configuration_defaults_are_valid_and_closed_to_cross_origin_requests() {
    let config = config(&[]);
    assert_eq!(config.address.to_string(), "0.0.0.0:8080");
    assert!(config.cors_origins.is_empty());
    assert_eq!(config.max_request_body_bytes, 65536);
    assert_eq!(config.request_timeout.as_secs(), 30);
}

#[test]
fn render_revision_is_used_unless_explicitly_overridden() {
    assert_eq!(config(&[("RENDER_GIT_COMMIT", "render-commit")]).revision, "render-commit");
    assert_eq!(config(&[("RENDER_GIT_COMMIT", "render-commit"), ("APP_REVISION", "explicit")]).revision, "explicit");
}

#[test]
fn malformed_configuration_is_rejected_without_echoing_values() {
    for (key, value) in [
        ("PORT", "0"), ("PORT", "65536"), ("PORT", "not-a-port"),
        ("HOST", "example.com"), ("REQUEST_TIMEOUT_SECONDS", "0"),
        ("REQUEST_TIMEOUT_SECONDS", "121"), ("MAX_REQUEST_BODY_BYTES", "0"),
        ("MAX_REQUEST_BODY_BYTES", "1048577"), ("APP_REVISION", "secret/token"),
        ("CORS_ALLOWED_ORIGINS", "*"), ("CORS_ALLOWED_ORIGINS", "null"),
        ("CORS_ALLOWED_ORIGINS", "https://game.example/path"),
        ("CORS_ALLOWED_ORIGINS", "https://user:secret@game.example"),
    ] {
        let values = HashMap::from([(key.to_string(), value.to_string())]);
        let error = Config::from_values(&values).err().expect("invalid value must fail").to_string();
        assert!(error.contains(key));
        assert!(!error.contains(value), "configuration error leaked a value");
    }
}
