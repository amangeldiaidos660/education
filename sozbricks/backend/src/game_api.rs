use axum::{
    Json,
    extract::{
        Path, Query, State,
        rejection::{JsonRejection, QueryRejection},
    },
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction, postgres::PgRow};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    AppState,
    auth::AuthUser,
    profile::{self, GameError, ProfilePatch},
    progress::{self, GuestImport},
};

async fn context(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(AuthUser, Transaction<'static, Postgres>), GameError> {
    let user = state.auth.authenticate(headers).await?;
    let tx = profile::begin(&state.database, &user).await?;
    Ok((user, tx))
}

fn body<T>(value: Result<Json<T>, JsonRejection>) -> Result<T, GameError> {
    value.map(|Json(value)| value).map_err(|error| {
        GameError::new(
            error.status(),
            "invalid_body",
            "Expected JSON matching the documented request schema",
        )
    })
}

fn identifier(value: &str) -> Result<Uuid, GameError> {
    Uuid::parse_str(value)
        .ok()
        .filter(|value| !value.is_nil())
        .ok_or_else(|| GameError::invalid("A nonzero UUID is required"))
}

pub(crate) async fn get_profile(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Value>, GameError> {
    let (user, mut tx) = context(&state, &headers).await?;
    let result = json!({"profile":profile::get(&mut tx,&user).await?,"progress":progress::summary(&mut tx,user.id).await?});
    tx.commit().await?;
    Ok(Json(result))
}

pub(crate) async fn patch_profile(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    input: Result<Json<ProfilePatch>, JsonRejection>,
) -> Result<Json<Value>, GameError> {
    let patch = body(input)?;
    let (user, mut tx) = context(&state, &headers).await?;
    profile::patch(&mut tx, user.id, patch).await?;
    let result = json!({"profile":profile::get(&mut tx,&user).await?,"progress":progress::summary(&mut tx,user.id).await?});
    tx.commit().await?;
    Ok(Json(result))
}

pub(crate) async fn import_progress(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    input: Result<Json<GuestImport>, JsonRejection>,
) -> Result<Json<Value>, GameError> {
    let input = body(input)?;
    let (user, mut tx) = context(&state, &headers).await?;
    let mut result = progress::import(&mut tx, user.id, input).await?;
    result["progress"] = progress::summary(&mut tx, user.id).await?;
    tx.commit().await?;
    Ok(Json(result))
}

pub(crate) async fn get_levels(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Value>, GameError> {
    let (user, mut tx) = context(&state, &headers).await?;
    let result = progress::levels(&mut tx, user.id).await?;
    tx.commit().await?;
    Ok(Json(result))
}

pub(crate) async fn get_progress(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Value>, GameError> {
    let (user, mut tx) = context(&state, &headers).await?;
    let result = progress::summary(&mut tx, user.id).await?;
    tx.commit().await?;
    Ok(Json(result))
}

#[derive(Deserialize, Default)]
pub struct Page {
    limit: Option<i64>,
    offset: Option<i64>,
}

fn page(input: Result<Query<Page>, QueryRejection>) -> Result<(i64, i64), GameError> {
    let Query(value) = input.map_err(|_| GameError::invalid("Invalid pagination parameters"))?;
    let (limit, offset) = (value.limit.unwrap_or(20), value.offset.unwrap_or(0));
    if !(1..=100).contains(&limit) || !(0..=10000).contains(&offset) {
        return Err(GameError::invalid(
            "limit must be 1..100; offset must be 0..10000",
        ));
    }
    Ok((limit, offset))
}

pub(crate) async fn get_history(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    query: Result<Query<Page>, QueryRejection>,
) -> Result<Json<Value>, GameError> {
    let (limit, offset) = page(query)?;
    let (user, mut tx) = context(&state, &headers).await?;
    let result = progress::history(&mut tx, user.id, limit, offset).await?;
    tx.commit().await?;
    Ok(Json(result))
}

pub(crate) async fn get_rewards(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    query: Result<Query<Page>, QueryRejection>,
) -> Result<Json<Value>, GameError> {
    let (limit, offset) = page(query)?;
    let (user, mut tx) = context(&state, &headers).await?;
    let result = progress::rewards(&mut tx, user.id, limit, offset).await?;
    tx.commit().await?;
    Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartSession {
    pub level_id: i16,
    pub request_id: Uuid,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FinishSession {
    pub won: bool,
    pub score: i64,
    pub collected_word: String,
    pub duration_seconds: u32,
}

fn session_json(row: &PgRow) -> Result<Value, GameError> {
    let word: String = row
        .try_get("word_snapshot")
        .map_err(|_| GameError::conflict("Legacy session is not supported; start a new attempt"))?;
    let speed: i32 = row
        .try_get("speed_snapshot")
        .map_err(|_| GameError::conflict("Legacy session is not supported; start a new attempt"))?;
    let reward: i64 = row
        .try_get("reward_snapshot")
        .map_err(|_| GameError::conflict("Legacy session is not supported; start a new attempt"))?;
    Ok(
        json!({"id":row.get::<Uuid,_>("id"),"request_id":row.get::<Option<Uuid>,_>("request_id"),
        "level_id":row.get::<i16,_>("level_id"),"seed":row.get::<i64,_>("seed"),"rules_version":row.get::<i32,_>("rules_version"),
        "status":row.get::<String,_>("status"),"started_at":row.get::<String,_>("started_at"),"expires_at":row.get::<String,_>("expires_at"),
        "word":word,"speed":speed,"reward_lamports":reward,
        "result_source":"frontend_reported"}),
    )
}

async fn session(
    tx: &mut Transaction<'_, Postgres>,
    user: Uuid,
    id: Uuid,
    lock: bool,
) -> Result<PgRow, GameError> {
    let sql = if lock {
        "SELECT id,request_id,level_id,seed,rules_version,status,started_at::text,expires_at::text,word_snapshot,speed_snapshot,reward_snapshot,expires_at<=now() AS expired FROM sozbricks.game_sessions WHERE id=$1 AND user_id=$2 FOR UPDATE"
    } else {
        "SELECT id,request_id,level_id,seed,rules_version,status,started_at::text,expires_at::text,word_snapshot,speed_snapshot,reward_snapshot,expires_at<=now() AS expired FROM sozbricks.game_sessions WHERE id=$1 AND user_id=$2"
    };
    sqlx::query(sql)
        .bind(id)
        .bind(user)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(GameError::missing)
}

pub(crate) async fn start_session(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    input: Result<Json<StartSession>, JsonRejection>,
) -> Result<Json<Value>, GameError> {
    let input = body(input)?;
    if input.request_id.is_nil() || !(1..=24).contains(&input.level_id) {
        return Err(GameError::invalid(
            "A valid level_id and nonzero request_id UUID are required",
        ));
    }
    let (user, mut tx) = context(&state, &headers).await?;
    profile::lock(&mut tx, user.id).await?;
    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM sozbricks.game_sessions WHERE user_id=$1 AND request_id=$2",
    )
    .bind(user.id)
    .bind(input.request_id)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(id) = existing {
        let row = session(&mut tx, user.id, id, false).await?;
        if row.get::<i16, _>("level_id") != input.level_id {
            return Err(GameError::conflict(
                "request_id was already used for a different level",
            ));
        }
        let result = session_json(&row)?;
        tx.commit().await?;
        return Ok(Json(result));
    }
    let level = sqlx::query("SELECT word,speed,reward_lamports,rules_version FROM sozbricks.levels WHERE id=$1 AND active AND (id=1 OR EXISTS(SELECT 1 FROM sozbricks.level_progress WHERE user_id=$2 AND level_id=$1-1 AND completed_at IS NOT NULL) OR EXISTS(SELECT 1 FROM sozbricks.guest_progress WHERE user_id=$2 AND level_id=$1-1 AND completed))")
        .bind(input.level_id).bind(user.id).fetch_optional(&mut *tx).await?.ok_or_else(||GameError::new(StatusCode::FORBIDDEN,"level_locked","Complete the preceding level first"))?;
    sqlx::query("UPDATE sozbricks.game_sessions SET status=CASE WHEN expires_at<=now() THEN 'expired' ELSE 'abandoned' END,finished_at=now() WHERE user_id=$1 AND status='started'").bind(user.id).execute(&mut *tx).await?;
    let seed = i64::from(input.level_id) * 12345 + 17;
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO sozbricks.game_sessions(id,user_id,level_id,rules_version,seed,expires_at,request_id,word_snapshot,speed_snapshot,reward_snapshot) VALUES($1,$2,$3,$4,$5,now()+interval '2 hours',$6,$7,$8,$9)")
        .bind(id).bind(user.id).bind(input.level_id).bind(level.get::<i32,_>("rules_version")).bind(seed).bind(input.request_id)
        .bind(level.get::<String,_>("word")).bind(level.get::<i32,_>("speed")).bind(level.get::<i64,_>("reward_lamports"))
        .execute(&mut *tx).await?;
    let result = session_json(&session(&mut tx, user.id, id, false).await?)?;
    tx.commit().await?;
    Ok(Json(result))
}

pub(crate) async fn get_session(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, GameError> {
    let id = identifier(&id)?;
    let (user, mut tx) = context(&state, &headers).await?;
    sqlx::query("UPDATE sozbricks.game_sessions SET status='expired',finished_at=now() WHERE id=$1 AND user_id=$2 AND status='started' AND expires_at<=now()")
        .bind(id).bind(user.id).execute(&mut *tx).await?;
    let result = session_json(&session(&mut tx, user.id, id, false).await?)?;
    tx.commit().await?;
    Ok(Json(result))
}

pub(crate) async fn abandon_session(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, GameError> {
    let id = identifier(&id)?;
    let (user, mut tx) = context(&state, &headers).await?;
    let row = session(&mut tx, user.id, id, true).await?;
    if row.get::<String, _>("status") == "completed" {
        return Err(GameError::conflict("Completed session cannot be abandoned"));
    }
    sqlx::query("UPDATE sozbricks.game_sessions SET status=CASE WHEN expires_at<=now() THEN 'expired' ELSE 'abandoned' END,finished_at=now() WHERE id=$1 AND user_id=$2 AND status='started'")
        .bind(id).bind(user.id).execute(&mut *tx).await?;
    let result = session_json(&session(&mut tx, user.id, id, false).await?)?;
    tx.commit().await?;
    Ok(Json(result))
}

async fn finished(
    tx: &mut Transaction<'_, Postgres>,
    user: Uuid,
    id: Uuid,
    digest: &[u8],
    repeated: bool,
) -> Result<Value, GameError> {
    let row = sqlx::query("SELECT r.level_id,r.won,r.score,r.collected_word,r.duration_ticks,r.result_hash,coalesce(l.amount_lamports,0) AS awarded_lamports FROM sozbricks.game_results r LEFT JOIN sozbricks.reward_ledger l ON l.session_id=r.session_id AND l.user_id=r.user_id WHERE r.session_id=$1 AND r.user_id=$2")
        .bind(id).bind(user).fetch_optional(&mut **tx).await?.ok_or_else(GameError::missing)?;
    if row.get::<Option<Vec<u8>>, _>("result_hash").as_deref() != Some(digest) {
        return Err(GameError::conflict(
            "Session was already completed with a different result",
        ));
    }
    Ok(
        json!({"session_id":id,"level_id":row.get::<i16,_>("level_id"),"won":row.get::<bool,_>("won"),"score":row.get::<i64,_>("score"),
        "collected_word":row.get::<String,_>("collected_word"),"duration_seconds":row.get::<i32,_>("duration_ticks")/120,"source":"frontend_reported",
        "awarded_lamports":row.get::<i64,_>("awarded_lamports"),"repeated":repeated,"progress":progress::summary(tx,user).await?}),
    )
}

pub(crate) async fn finish_session(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    input: Result<Json<FinishSession>, JsonRejection>,
) -> Result<Json<Value>, GameError> {
    let id = identifier(&id)?;
    let result = body(input)?;
    if !(0..=100_000_000).contains(&result.score)
        || result.collected_word.chars().count() > 32
        || result.duration_seconds > 7200
    {
        return Err(GameError::invalid(
            "score must be 0..100000000, collected_word at most 32 characters, duration_seconds 0..7200",
        ));
    }
    let digest = progress::hash(&result);
    let (user, mut tx) = context(&state, &headers).await?;
    profile::lock(&mut tx, user.id).await?;
    let row = session(&mut tx, user.id, id, true).await?;
    if row.get::<String, _>("status") == "completed" {
        let result = finished(&mut tx, user.id, id, &digest, true).await?;
        tx.commit().await?;
        return Ok(Json(result));
    }
    check_active(&row)?;
    let level = row.get::<i16, _>("level_id");
    let amount: i64 = row
        .try_get("reward_snapshot")
        .map_err(|_| GameError::conflict("Legacy session is not supported; start a new attempt"))?;
    let won = result.won;
    sqlx::query("INSERT INTO sozbricks.game_results(session_id,user_id,level_id,won,collected_word,score,duration_ticks,result_hash) VALUES($1,$2,$3,$4,$5,$6,$7,$8)")
        .bind(id).bind(user.id).bind(level).bind(won).bind(result.collected_word).bind(result.score).bind((result.duration_seconds * 120) as i32).bind(&digest).execute(&mut *tx).await?;
    sqlx::query("UPDATE sozbricks.game_sessions SET status='completed',finished_at=now() WHERE id=$1 AND user_id=$2").bind(id).bind(user.id).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO sozbricks.level_progress(user_id,level_id,best_score,completed_at,first_win_session_id) VALUES($1,$2,$3,CASE WHEN $4 THEN now() ELSE NULL END,CASE WHEN $4 THEN $5 ELSE NULL END) ON CONFLICT(user_id,level_id) DO UPDATE SET best_score=greatest(sozbricks.level_progress.best_score,excluded.best_score),completed_at=coalesce(sozbricks.level_progress.completed_at,excluded.completed_at),first_win_session_id=coalesce(sozbricks.level_progress.first_win_session_id,excluded.first_win_session_id),updated_at=now()")
        .bind(user.id).bind(level).bind(result.score).bind(won).bind(id).execute(&mut *tx).await?;
    if won && amount > 0 {
        sqlx::query("INSERT INTO sozbricks.reward_ledger(user_id,level_id,session_id,amount_lamports) VALUES($1,$2,$3,$4) ON CONFLICT(user_id,level_id) DO NOTHING")
            .bind(user.id).bind(level).bind(id).bind(amount).execute(&mut *tx).await?;
    }
    let result = finished(&mut tx, user.id, id, &digest, false).await?;
    tx.commit().await?;
    Ok(Json(result))
}

fn check_active(row: &PgRow) -> Result<(), GameError> {
    if row.get::<bool, _>("expired") {
        return Err(GameError::new(
            StatusCode::GONE,
            "session_expired",
            "Session has expired; start a new attempt",
        ));
    }
    if row.get::<String, _>("status") != "started" {
        return Err(GameError::conflict("Session is no longer active"));
    }
    Ok(())
}
