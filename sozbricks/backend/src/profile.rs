use axum::{
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

use crate::{
    auth::{AuthError, AuthUser},
    database::Database,
    error::ApiError,
};

pub struct GameError(pub ApiError);

impl GameError {
    pub fn new(status: StatusCode, code: &'static str, message: &'static str) -> Self {
        Self(ApiError {
            status,
            code,
            message,
        })
    }
    pub fn invalid(message: &'static str) -> Self {
        Self::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_game_data",
            message,
        )
    }
    pub fn conflict(message: &'static str) -> Self {
        Self::new(StatusCode::CONFLICT, "conflict", message)
    }
    pub fn missing() -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", "Resource not found")
    }
    pub fn unavailable() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "database_unavailable",
            "Database is not ready",
        )
    }
}

impl IntoResponse for GameError {
    fn into_response(self) -> Response {
        let status = self.0.status;
        let mut response = self.0.into_response();
        if status == StatusCode::UNAUTHORIZED {
            response
                .headers_mut()
                .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        }
        response
    }
}

impl From<AuthError> for GameError {
    fn from(error: AuthError) -> Self {
        match error {
            AuthError::Unauthorized => Self::new(
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "A valid Google-linked Supabase session is required",
            ),
            AuthError::NotConfigured => Self::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "auth_not_configured",
                "Authentication is not configured",
            ),
            AuthError::Unavailable => Self::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "auth_unavailable",
                "Authentication service is temporarily unavailable",
            ),
        }
    }
}

impl From<sqlx::Error> for GameError {
    fn from(error: sqlx::Error) -> Self {
        match error
            .as_database_error()
            .and_then(|error| error.code())
            .as_deref()
        {
            Some("23505") => Self::conflict("Request conflicts with an existing record"),
            _ => Self::unavailable(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilePatch {
    pub display_name: Option<String>,
    pub hero: Option<String>,
}

pub async fn begin(
    database: &Database,
    user: &AuthUser,
) -> Result<Transaction<'static, Postgres>, GameError> {
    let mut tx = database
        .runtime_pool()
        .ok_or_else(GameError::unavailable)?
        .begin()
        .await?;
    sqlx::query("SELECT set_config('app.user_id', $1, true), set_config('statement_timeout', '5000', true), set_config('lock_timeout', '3000', true)")
        .bind(user.id.to_string()).execute(&mut *tx).await?;
    let inserted = sqlx::query("INSERT INTO sozbricks.profiles(user_id,display_name) VALUES($1,$2) ON CONFLICT(user_id) DO NOTHING")
        .bind(user.id).bind(&user.display_name).execute(&mut *tx).await;
    if let Err(error) = inserted {
        if error
            .as_database_error()
            .and_then(|error| error.code())
            .as_deref()
            == Some("23503")
        {
            return Err(GameError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "auth_database_mismatch",
                "Verified user is missing from this database; check that Auth and DATABASE_URL use the same Supabase project",
            ));
        }
        return Err(error.into());
    }
    Ok(tx)
}

pub async fn ensure(database: &Database, user: &AuthUser) -> Result<(), GameError> {
    begin(database, user).await?.commit().await?;
    Ok(())
}

pub async fn lock(tx: &mut Transaction<'_, Postgres>, user: Uuid) -> Result<(), GameError> {
    sqlx::query("SELECT user_id FROM sozbricks.profiles WHERE user_id=$1 FOR UPDATE")
        .bind(user)
        .fetch_one(&mut **tx)
        .await?;
    Ok(())
}

pub async fn get(tx: &mut Transaction<'_, Postgres>, user: &AuthUser) -> Result<Value, GameError> {
    let row = sqlx::query("SELECT user_id,display_name,hero,created_at::text,updated_at::text FROM sozbricks.profiles WHERE user_id=$1")
        .bind(user.id).fetch_one(&mut **tx).await?;
    Ok(
        json!({ "user_id": row.get::<Uuid,_>("user_id"), "email": user.email,
        "display_name": row.get::<String,_>("display_name"), "hero": row.get::<String,_>("hero"),
        "created_at": row.get::<String,_>("created_at"), "updated_at": row.get::<String,_>("updated_at") }),
    )
}

pub async fn patch(
    tx: &mut Transaction<'_, Postgres>,
    user: Uuid,
    patch: ProfilePatch,
) -> Result<(), GameError> {
    if patch.display_name.is_none() && patch.hero.is_none() {
        return Err(GameError::invalid("Provide display_name or hero"));
    }
    let name = patch.display_name.map(|name| name.trim().to_owned());
    if name.as_ref().is_some_and(|name| {
        !(1..=60).contains(&name.chars().count()) || name.chars().any(char::is_control)
    }) || patch
        .hero
        .as_ref()
        .is_some_and(|hero| !matches!(hero.as_str(), "alikhan" | "aiaru"))
    {
        return Err(GameError::invalid(
            "Name must contain 1..60 printable characters; hero must be alikhan or aiaru",
        ));
    }
    sqlx::query("UPDATE sozbricks.profiles SET display_name=coalesce($2,display_name),hero=coalesce($3,hero),updated_at=now() WHERE user_id=$1")
        .bind(user).bind(name).bind(patch.hero).execute(&mut **tx).await?;
    Ok(())
}
