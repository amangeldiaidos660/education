use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

use crate::profile::{self, GameError};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GuestImport { pub import_id: Uuid, pub save: GuestSave }

#[derive(Deserialize, Serialize)]
pub struct GuestSave {
    pub version: u8,
    pub hero: Option<String>,
    pub completed: Vec<i16>,
    pub best: BTreeMap<String, i64>,
    pub history: Vec<GuestResult>,
}

#[derive(Deserialize, Serialize)]
pub struct GuestResult {
    pub id: String, pub level: i16, pub word: String, pub score: i64,
    pub won: bool, pub seconds: u32, pub at: i64,
}

pub fn hash(value: &impl Serialize) -> Vec<u8> {
    Sha256::digest(serde_json::to_vec(value).expect("validated serializable request")).to_vec()
}

pub async fn import(tx: &mut Transaction<'_, Postgres>, user: Uuid, request: GuestImport) -> Result<Value, GameError> {
    let save = &request.save;
    let level_ok = |id: i16| (1..=24).contains(&id);
    if request.import_id.is_nil() || save.version != 1 || save.completed.len() > 24 || save.best.len() > 24 || save.history.len() > 100
        || !save.completed.iter().all(|id| level_ok(*id))
        || save.best.iter().any(|(key, score)| key.parse::<i16>().ok().is_none_or(|id| !level_ok(id) || id.to_string() != *key) || !(0..=100_000_000).contains(score))
        || save.hero.as_ref().is_some_and(|hero| !matches!(hero.as_str(), "alikhan" | "aiaru"))
        || save.history.iter().any(|entry| !level_ok(entry.level) || entry.id.is_empty() || entry.id.len() > 160
            || entry.word.chars().count() > 32 || !(0..=100_000_000).contains(&entry.score) || entry.seconds > 86400 || entry.at < 0)
    { return Err(GameError::invalid("Invalid guest save; at most 24 levels and 100 history entries are accepted")); }
    profile::lock(tx, user).await?;
    let digest = hash(&request);
    let existing = sqlx::query("SELECT import_id,payload_hash FROM sozbricks.guest_imports WHERE user_id=$1").bind(user).fetch_optional(&mut **tx).await?;
    if let Some(row) = existing {
        if row.get::<Uuid,_>("import_id") != request.import_id || row.get::<Vec<u8>,_>("payload_hash") != digest {
            return Err(GameError::conflict("Guest progress has already been imported for this account"));
        }
        return Ok(json!({"imported": true, "repeated": true, "awarded_lamports": 0}));
    }
    sqlx::query("INSERT INTO sozbricks.guest_imports(user_id,import_id,payload_hash,payload) VALUES($1,$2,$3,$4::jsonb)")
        .bind(user).bind(request.import_id).bind(digest).bind(serde_json::to_string(save).unwrap()).execute(&mut **tx).await?;
    let mut ids: BTreeSet<i16> = save.completed.iter().copied().collect();
    ids.extend(save.best.keys().map(|key| key.parse::<i16>().unwrap()));
    ids.extend(save.history.iter().map(|entry| entry.level));
    for id in ids {
        let best = save.best.get(&id.to_string()).copied().unwrap_or(0)
            .max(save.history.iter().filter(|entry| entry.level == id).map(|entry| entry.score).max().unwrap_or(0));
        let completed = save.completed.contains(&id) || save.history.iter().any(|entry| entry.level == id && entry.won);
        sqlx::query("INSERT INTO sozbricks.guest_progress(user_id,level_id,completed,best_score) VALUES($1,$2,$3,$4)")
            .bind(user).bind(id).bind(completed).bind(best).execute(&mut **tx).await?;
    }
    if let Some(hero) = &save.hero {
        sqlx::query("UPDATE sozbricks.profiles SET hero=$2,updated_at=now() WHERE user_id=$1").bind(user).bind(hero).execute(&mut **tx).await?;
    }
    Ok(json!({"imported": true, "repeated": false, "awarded_lamports": 0}))
}

pub async fn levels(tx: &mut Transaction<'_, Postgres>, user: Uuid) -> Result<Value, GameError> {
    let rows = sqlx::query("SELECT l.id,l.word,l.translation,l.letter_count,l.reward_lamports,l.speed,l.rules_version,coalesce(p.best_score,0) AS verified_best,coalesce(g.best_score,0) AS imported_best,p.completed_at IS NOT NULL AS recorded_completed,coalesce(g.completed,false) AS imported_completed FROM sozbricks.levels l LEFT JOIN sozbricks.level_progress p ON p.level_id=l.id AND p.user_id=$1 LEFT JOIN sozbricks.guest_progress g ON g.level_id=l.id AND g.user_id=$1 WHERE l.active ORDER BY l.id")
        .bind(user).fetch_all(&mut **tx).await?;
    let completed: BTreeSet<i16> = rows.iter().filter(|row| row.get::<bool,_>("recorded_completed") || row.get::<bool,_>("imported_completed")).map(|row| row.get("id")).collect();
    let items: Vec<_> = rows.iter().map(|row| {
        let id: i16 = row.get("id");
        json!({"id":id, "word":row.get::<String,_>("word"), "translation":row.get::<String,_>("translation"),
            "letter_count":row.get::<i16,_>("letter_count"),"reward_lamports":row.get::<i64,_>("reward_lamports"),
            "speed":row.get::<i32,_>("speed"), "rules_version":row.get::<i32,_>("rules_version"),
            "unlocked":id == 1 || completed.contains(&(id-1)), "completed":completed.contains(&id),
            "recorded_completed":row.get::<bool,_>("recorded_completed"), "imported_completed":row.get::<bool,_>("imported_completed"),
            "best_score":row.get::<i64,_>("verified_best").max(row.get::<i64,_>("imported_best")),
            "recorded_best_score":row.get::<i64,_>("verified_best")})
    }).collect();
    Ok(json!({"items":items}))
}

pub async fn summary(tx: &mut Transaction<'_, Postgres>, user: Uuid) -> Result<Value, GameError> {
    let levels = levels(tx, user).await?;
    let items = levels["items"].as_array().unwrap();
    let completed: Vec<_> = items.iter().filter(|level| level["completed"] == true).map(|level| level["id"].clone()).collect();
    let verified: Vec<_> = items.iter().filter(|level| level["recorded_completed"] == true).map(|level| level["id"].clone()).collect();
    let imported: Vec<_> = items.iter().filter(|level| level["imported_completed"] == true).map(|level| level["id"].clone()).collect();
    let best: BTreeMap<_,_> = items.iter().map(|level| (level["id"].to_string(),level["best_score"].clone())).collect();
    let balance: i64 = sqlx::query_scalar("SELECT coalesce(sum(amount_lamports),0)::bigint FROM sozbricks.reward_ledger WHERE user_id=$1").bind(user).fetch_one(&mut **tx).await?;
    Ok(json!({"completed":completed,"recorded_completed":verified,"imported_completed":imported,"best":best,
        "best_score":items.iter().filter_map(|level|level["best_score"].as_i64()).max().unwrap_or(0),
        "next_level_id":items.iter().find(|level|level["completed"] != true && level["unlocked"] == true).map(|level|level["id"].clone()),
        "earned_lamports":balance,"balance_kind":"database_ledger_not_wallet"}))
}

pub async fn history(tx: &mut Transaction<'_, Postgres>, user: Uuid, limit: i64, offset: i64) -> Result<Value, GameError> {
    let rows = sqlx::query("SELECT r.session_id,r.level_id,r.won,r.score,r.collected_word,r.duration_ticks,r.recorded_at::text,l.word FROM sozbricks.game_results r JOIN sozbricks.levels l ON l.id=r.level_id WHERE r.user_id=$1 ORDER BY r.recorded_at DESC,r.session_id DESC LIMIT $2 OFFSET $3")
        .bind(user).bind(limit).bind(offset).fetch_all(&mut **tx).await?;
    let items: Vec<_> = rows.iter().map(|row|json!({"session_id":row.get::<Uuid,_>("session_id"),"level_id":row.get::<i16,_>("level_id"),"won":row.get::<bool,_>("won"),"score":row.get::<i64,_>("score"),"word":row.get::<String,_>("word"),"collected_word":row.get::<String,_>("collected_word"),"duration_seconds":row.get::<i32,_>("duration_ticks")/120,"recorded_at":row.get::<String,_>("recorded_at"),"source":"frontend_reported"})).collect();
    let legacy: Option<String> = sqlx::query_scalar("SELECT payload::text FROM sozbricks.guest_imports WHERE user_id=$1").bind(user).fetch_optional(&mut **tx).await?;
    let imported = legacy.and_then(|text|serde_json::from_str::<Value>(&text).ok()).map(|value|value["history"].clone()).unwrap_or(json!([]));
    Ok(json!({"items":items,"limit":limit,"offset":offset,"imported_history":imported,"imported_history_verified":false}))
}

pub async fn rewards(tx: &mut Transaction<'_, Postgres>, user: Uuid, limit: i64, offset: i64) -> Result<Value, GameError> {
    let rows = sqlx::query("SELECT id,level_id,session_id,amount_lamports,created_at::text FROM sozbricks.reward_ledger WHERE user_id=$1 ORDER BY created_at DESC,id DESC LIMIT $2 OFFSET $3")
        .bind(user).bind(limit).bind(offset).fetch_all(&mut **tx).await?;
    let items: Vec<_> = rows.iter().map(|row|json!({"id":row.get::<Uuid,_>("id"),"level_id":row.get::<i16,_>("level_id"),"session_id":row.get::<Uuid,_>("session_id"),"amount_lamports":row.get::<i64,_>("amount_lamports"),"created_at":row.get::<String,_>("created_at"),"status":"recorded_not_paid"})).collect();
    let total: i64 = sqlx::query_scalar("SELECT coalesce(sum(amount_lamports),0)::bigint FROM sozbricks.reward_ledger WHERE user_id=$1").bind(user).fetch_one(&mut **tx).await?;
    Ok(json!({"items":items,"limit":limit,"offset":offset,"total_lamports":total,"solana_payments_enabled":false}))
}
