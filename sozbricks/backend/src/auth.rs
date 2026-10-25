use std::time::Duration;

use axum::{
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{config::AuthConfig, error::ApiError};

#[derive(Clone)]
pub struct AuthClient {
    config: Option<AuthConfig>,
    client: Option<reqwest::Client>,
}

#[derive(Serialize)]
pub struct AuthUser {
    pub id: Uuid,
    pub email: Option<String>,
    pub display_name: String,
    pub provider: &'static str,
}

#[derive(Deserialize)]
struct SupabaseUser {
    id: Uuid,
    role: String,
    email: Option<String>,
    #[serde(default)]
    is_anonymous: bool,
    #[serde(default)]
    app_metadata: AppMetadata,
    #[serde(default)]
    user_metadata: UserMetadata,
}

#[derive(Default, Deserialize)]
struct AppMetadata {
    #[serde(default)]
    providers: Vec<String>,
}

#[derive(Default, Deserialize)]
struct UserMetadata {
    full_name: Option<String>,
    name: Option<String>,
}

pub enum AuthError {
    Unauthorized,
    Unavailable,
    NotConfigured,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "A valid Google-linked Supabase session is required",
            ),
            Self::Unavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "auth_unavailable",
                "Authentication service is temporarily unavailable",
            ),
            Self::NotConfigured => (
                StatusCode::SERVICE_UNAVAILABLE,
                "auth_not_configured",
                "Authentication is not configured",
            ),
        };
        let mut response = ApiError {
            status,
            code,
            message,
        }
        .into_response();
        if status == StatusCode::UNAUTHORIZED {
            response
                .headers_mut()
                .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        }
        response
    }
}

impl AuthClient {
    pub fn new(config: Option<&AuthConfig>) -> Self {
        let client = config.and_then(|config| {
            reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(3))
                .timeout(config.timeout)
                .redirect(reqwest::redirect::Policy::none())
                .no_proxy()
                .build()
                .map_err(|_| tracing::error!("could not initialize authentication HTTP client"))
                .ok()
        });
        Self {
            config: config.cloned(),
            client,
        }
    }

    pub async fn authenticate(&self, headers: &HeaderMap) -> Result<AuthUser, AuthError> {
        let token = bearer_token(headers)?;
        let config = self.config.as_ref().ok_or(AuthError::NotConfigured)?;
        let client = self.client.as_ref().ok_or(AuthError::Unavailable)?;
        let mut response = client
            .get(config.user_url.clone())
            .header("apikey", config.publishable_key.clone())
            .bearer_auth(token)
            .send()
            .await
            .map_err(|_| AuthError::Unavailable)?;
        match response.status() {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                return Err(AuthError::Unauthorized);
            }
            StatusCode::OK => {}
            _ => return Err(AuthError::Unavailable),
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| AuthError::Unavailable)? {
            if body.len() + chunk.len() > 65536 {
                return Err(AuthError::Unavailable);
            }
            body.extend_from_slice(&chunk);
        }
        let user: SupabaseUser =
            serde_json::from_slice(&body).map_err(|_| AuthError::Unavailable)?;
        if user.id.is_nil()
            || user.role != "authenticated"
            || user.is_anonymous
            || !user
                .app_metadata
                .providers
                .iter()
                .any(|provider| provider == "google")
        {
            return Err(AuthError::Unauthorized);
        }
        let display_name = user
            .user_metadata
            .full_name
            .or(user.user_metadata.name)
            .filter(|name| !name.trim().is_empty())
            .map_or_else(
                || "Игрок".to_owned(),
                |name| name.trim().chars().take(60).collect(),
            );
        Ok(AuthUser {
            id: user.id,
            email: user.email,
            display_name,
            provider: "google",
        })
    }
}

fn bearer_token(headers: &HeaderMap) -> Result<&str, AuthError> {
    if headers.get_all(header::AUTHORIZATION).iter().count() != 1 {
        return Err(AuthError::Unauthorized);
    }
    let value = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or(AuthError::Unauthorized)?;
    let (scheme, token) = value.split_once(' ').ok_or(AuthError::Unauthorized)?;
    if !scheme.eq_ignore_ascii_case("Bearer")
        || token.is_empty()
        || token.len() > 8192
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-._~+/=".contains(&byte))
    {
        return Err(AuthError::Unauthorized);
    }
    Ok(token)
}
