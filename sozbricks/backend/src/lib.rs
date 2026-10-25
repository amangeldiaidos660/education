pub mod auth;
pub mod config;
pub mod database;
pub mod docs;
pub mod error;

use std::{error::Error as _, sync::Arc, time::Instant};

use axum::{
    Json, Router,
    body::{Body, to_bytes},
    extract::{Request, State},
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Serialize;
use tower_http::cors::CorsLayer;
use uuid::Uuid;

use auth::{AuthClient, AuthError, AuthUser};
use config::Config;
use database::{Database, DatabaseError};
use error::ApiError;

pub fn app(config: Config) -> Router {
    let database = Database::new(config.database.as_ref());
    app_with_database(config, database)
}

#[derive(Clone)]
struct AppState {
    auth: AuthClient,
    config: Config,
    database: Database,
}

pub fn app_with_database(config: Config, database: Database) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(config.cors_origins.clone())
        .allow_methods([Method::GET, Method::HEAD])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
        .expose_headers([HeaderName::from_static("x-request-id")]);
    let auth = AuthClient::new(config.auth.as_ref());
    let state = Arc::new(AppState {
        config,
        database,
        auth,
    });
    Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .route("/version", get(version))
        .route("/v1/auth/me", get(auth_me))
        .route("/docs", get(docs::swagger_ui))
        .route("/openapi.json", get(docs::openapi))
        .fallback(not_found)
        .method_not_allowed_fallback(method_not_allowed)
        .with_state(state.clone())
        .layer(cors)
        .layer(middleware::from_fn_with_state(
            Arc::new(state.config.clone()),
            request_context,
        ))
}

async fn auth_me(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<AuthUser>, AuthError> {
    state.auth.authenticate(&headers).await.map(Json)
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
}

async fn live() -> Json<Health> {
    Json(Health { status: "ok" })
}

#[derive(Serialize)]
struct Readiness {
    status: &'static str,
    stage: &'static str,
    database: &'static str,
    schema: &'static str,
}

async fn ready(State(state): State<Arc<AppState>>) -> (StatusCode, Json<Readiness>) {
    let (status, database, schema) = match state.database.readiness().await {
        Ok(()) => (StatusCode::OK, "ok", "ok"),
        Err(DatabaseError::NotConfigured) => {
            (StatusCode::SERVICE_UNAVAILABLE, "not_configured", "pending")
        }
        Err(DatabaseError::Schema | DatabaseError::Migration) => {
            (StatusCode::SERVICE_UNAVAILABLE, "ok", "pending")
        }
        Err(DatabaseError::Unavailable) => {
            (StatusCode::SERVICE_UNAVAILABLE, "unavailable", "unknown")
        }
    };
    (
        status,
        Json(Readiness {
            status: if status == StatusCode::OK {
                "ready"
            } else {
                "not_ready"
            },
            stage: "database",
            database,
            schema,
        }),
    )
}

#[derive(Serialize)]
struct Version {
    name: &'static str,
    version: &'static str,
    revision: String,
}

async fn version(State(state): State<Arc<AppState>>) -> Json<Version> {
    Json(Version {
        name: env!("CARGO_PKG_NAME"),
        version: env!("CARGO_PKG_VERSION"),
        revision: state.config.revision.clone(),
    })
}

async fn not_found() -> ApiError {
    ApiError {
        status: StatusCode::NOT_FOUND,
        code: "not_found",
        message: "Route not found",
    }
}

async fn method_not_allowed() -> ApiError {
    ApiError {
        status: StatusCode::METHOD_NOT_ALLOWED,
        code: "method_not_allowed",
        message: "Method not allowed",
    }
}

async fn request_context(
    State(config): State<Arc<Config>>,
    request: Request,
    next: Next,
) -> Response {
    let request_id = Uuid::new_v4().to_string();
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    let started = Instant::now();
    let process = async {
        let (parts, body) = request.into_parts();
        if let Some(length) = parts
            .headers
            .get(header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            && length > config.max_request_body_bytes as u64
        {
            return payload_too_large().into_response();
        }
        match to_bytes(body, config.max_request_body_bytes).await {
            Ok(bytes) => {
                next.run(Request::from_parts(parts, Body::from(bytes)))
                    .await
            }
            Err(error)
                if error
                    .source()
                    .is_some_and(|source| source.is::<http_body_util::LengthLimitError>()) =>
            {
                payload_too_large().into_response()
            }
            Err(_) => ApiError {
                status: StatusCode::BAD_REQUEST,
                code: "invalid_body",
                message: "Could not read request body",
            }
            .into_response(),
        }
    };
    let mut response = match tokio::time::timeout(config.request_timeout, process).await {
        Ok(response) => response,
        Err(_) => ApiError {
            status: StatusCode::REQUEST_TIMEOUT,
            code: "request_timeout",
            message: "Request timed out",
        }
        .into_response(),
    };
    response.headers_mut().insert(
        "x-request-id",
        HeaderValue::from_str(&request_id).expect("UUID is a valid header"),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    tracing::info!(%request_id, %method, %path, status = response.status().as_u16(), elapsed_ms = started.elapsed().as_millis() as u64, "request completed");
    response
}

fn payload_too_large() -> ApiError {
    ApiError {
        status: StatusCode::PAYLOAD_TOO_LARGE,
        code: "payload_too_large",
        message: "Request body exceeds the configured limit",
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, time::Duration};

    use super::*;
    use tower::ServiceExt;

    #[tokio::test]
    async fn timeout_returns_json_and_request_id() {
        let mut config = Config::from_values(&HashMap::new()).unwrap();
        config.request_timeout = Duration::from_millis(1);
        let router = Router::new()
            .route(
                "/slow",
                get(|| async {
                    tokio::time::sleep(Duration::from_secs(60)).await;
                    "finished"
                }),
            )
            .layer(middleware::from_fn_with_state(
                Arc::new(config),
                request_context,
            ));
        let response = router
            .oneshot(Request::get("/slow").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
        assert!(response.headers().contains_key("x-request-id"));
        let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
        let error: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(error["error"]["code"], "request_timeout");
    }
}
