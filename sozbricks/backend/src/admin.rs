use std::sync::Arc;

use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::{
    AppState,
    payouts::Chain,
    profile::{self, GameError},
};

async fn authorize(state: &AppState, headers: &HeaderMap) -> Result<(), GameError> {
    let mut user = state.auth.authenticate(headers).await?;
    if state.config.admin_player_ids.is_empty() {
        return Err(GameError::new(
            StatusCode::FORBIDDEN,
            "admin_required",
            "Administrator access is required",
        ));
    }
    profile::begin(&state.database, &mut user)
        .await?
        .commit()
        .await?;
    if !state.config.admin_player_ids.contains(&user.id) {
        return Err(GameError::new(
            StatusCode::FORBIDDEN,
            "admin_required",
            "Administrator access is required",
        ));
    }
    Ok(())
}

async fn read(state: &AppState) -> Result<Transaction<'static, Postgres>, GameError> {
    let mut tx = state
        .database
        .pool()
        .ok_or_else(GameError::unavailable)?
        .begin()
        .await?;
    sqlx::query("SET TRANSACTION READ ONLY")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='5000'")
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}

#[derive(Deserialize)]
pub(crate) struct Page {
    limit: Option<i64>,
    offset: Option<i64>,
    player_id: Option<Uuid>,
}

impl Page {
    fn bounds(&self) -> Result<(i64, i64), GameError> {
        let (limit, offset) = (self.limit.unwrap_or(25), self.offset.unwrap_or(0));
        if !(1..=100).contains(&limit) || !(0..=1_000_000).contains(&offset) {
            return Err(GameError::invalid("Invalid pagination"));
        }
        Ok((limit, offset))
    }
}

pub(crate) async fn overview(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Value>, GameError> {
    authorize(&state, &headers).await?;
    let mut tx = read(&state).await?;
    let summary: Value=sqlx::query_scalar("SELECT jsonb_build_object('accounts',(SELECT count(*) FROM sozbricks.profiles),'archived_accounts',(SELECT count(*) FROM sozbricks.profiles WHERE auth_user_id IS NULL),'wallets',(SELECT count(*) FROM sozbricks.wallet_bindings),'wallets_to_reconfirm',(SELECT count(*) FROM sozbricks.wallet_bindings WHERE reconfirmation_required),'earned_lamports',(SELECT coalesce(sum(amount_lamports),0) FROM sozbricks.reward_ledger WHERE payable),'paid_lamports',(SELECT coalesce(sum(r.amount_lamports),0) FROM sozbricks.reward_ledger r JOIN sozbricks.reward_payouts p ON p.reward_id=r.id WHERE p.status='paid'),'pending_lamports',(SELECT coalesce(sum(r.amount_lamports),0) FROM sozbricks.reward_ledger r JOIN sozbricks.reward_payouts p ON p.reward_id=r.id WHERE p.status<>'paid'))").fetch_one(&mut *tx).await?;
    let statuses: Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('status',status,'count',count(*)) FROM sozbricks.reward_payouts GROUP BY status ORDER BY status").fetch_all(&mut *tx).await?;
    let campaign: Option<Value>=sqlx::query_scalar("SELECT jsonb_build_object('activated_at',activated_at,'program_address',program_address) FROM sozbricks.reward_campaign WHERE id").fetch_optional(&mut *tx).await?;
    tx.commit().await?;
    let mut chain_status = json!({"status":"not_configured"});
    if let (Some(program), Some(treasury)) =
        (state.config.program_address, state.config.treasury.clone())
    {
        let chain = Chain::new(program, treasury);
        let reserve = chain.pda(&[b"reserve"]);
        chain_status = json!({"status":"unavailable","program_address":program.to_string(),"treasury_address":chain.authority().to_string(),"reserve_address":reserve.to_string()});
        match chain.check(true).await {
            Ok(()) => match chain.rpc("getMultipleAccounts",json!([[chain.authority().to_string(),reserve.to_string()],{"encoding":"base64","commitment":"finalized","dataSlice":{"offset":0,"length":0}}])).await {
                Ok(value) => {
                    chain_status["status"]=json!("ready");
                    chain_status["treasury_lamports"]=value["value"][0]["lamports"].clone();
                    chain_status["reserve_lamports"]=value["value"][1]["lamports"].clone();
                    chain_status["slot"]=value["context"]["slot"].clone();
                }
                Err(code) => chain_status["error_code"]=json!(code),
            },
            Err(code) => chain_status["error_code"]=json!(code),
        }
    }
    Ok(Json(
        json!({"summary":summary,"statuses":statuses,"campaign":campaign,"chain":chain_status,"payments_enabled":state.config.payments_enabled,"cluster":"devnet"}),
    ))
}

pub(crate) async fn accounts(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(page): Query<Page>,
) -> Result<Json<Value>, GameError> {
    authorize(&state, &headers).await?;
    let (limit, offset) = page.bounds()?;
    let mut tx = read(&state).await?;
    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM sozbricks.profiles")
        .fetch_one(&mut *tx)
        .await?;
    let items:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('player_id',p.user_id,'display_name',p.display_name,'archived',p.auth_user_id IS NULL,'recovery_ready',p.google_subject IS NOT NULL,'wallet_address',w.address,'wallet_reconfirmation_required',coalesce(w.reconfirmation_required,false),'completed_levels',(SELECT count(*) FROM sozbricks.level_progress l WHERE l.user_id=p.user_id AND l.completed_at IS NOT NULL),'imported_levels',(SELECT count(*) FROM sozbricks.guest_progress g WHERE g.user_id=p.user_id AND g.completed),'paid_lamports',(SELECT coalesce(sum(r.amount_lamports),0) FROM sozbricks.reward_ledger r JOIN sozbricks.reward_payouts q ON q.reward_id=r.id WHERE r.user_id=p.user_id AND q.status='paid'),'pending_lamports',(SELECT coalesce(sum(r.amount_lamports),0) FROM sozbricks.reward_ledger r JOIN sozbricks.reward_payouts q ON q.reward_id=r.id WHERE r.user_id=p.user_id AND q.status<>'paid')) FROM sozbricks.profiles p LEFT JOIN sozbricks.wallet_bindings w ON w.user_id=p.user_id ORDER BY p.created_at DESC,p.user_id LIMIT $1 OFFSET $2")
        .bind(limit).bind(offset).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(
        json!({"items":items,"total":total,"limit":limit,"offset":offset}),
    ))
}

pub(crate) async fn rewards(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(page): Query<Page>,
) -> Result<Json<Value>, GameError> {
    authorize(&state, &headers).await?;
    let (limit, offset) = page.bounds()?;
    let mut tx = read(&state).await?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM sozbricks.reward_ledger WHERE ($1::uuid IS NULL OR user_id=$1)",
    )
    .bind(page.player_id)
    .fetch_one(&mut *tx)
    .await?;
    let items:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('reward_id',r.id,'player_id',r.user_id,'display_name',a.display_name,'level_id',r.level_id,'amount_lamports',r.amount_lamports,'created_at',r.created_at,'status',coalesce(p.status,'historical'),'recipient',p.recipient,'signature',p.signature,'receipt_address',p.receipt_address,'error_code',p.error_code,'next_attempt_at',p.next_attempt_at,'updated_at',p.updated_at,'paid_at',p.paid_at) FROM sozbricks.reward_ledger r JOIN sozbricks.profiles a ON a.user_id=r.user_id LEFT JOIN sozbricks.reward_payouts p ON p.reward_id=r.id WHERE ($1::uuid IS NULL OR r.user_id=$1) ORDER BY r.created_at DESC,r.id DESC LIMIT $2 OFFSET $3")
        .bind(page.player_id).bind(limit).bind(offset).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(
        json!({"items":items,"total":total,"limit":limit,"offset":offset}),
    ))
}
