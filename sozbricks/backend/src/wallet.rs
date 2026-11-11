use std::{str::FromStr, sync::Arc};

use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    http::HeaderMap,
};
use ring::signature::{self, UnparsedPublicKey};
use serde::Deserialize;
use serde_json::{Value, json};
use solana_pubkey::Pubkey;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    AppState,
    game_api::{body, context},
    profile::{self, GameError},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChallengeRequest {
    address: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct VerifyRequest {
    challenge_id: Uuid,
    signature: String,
}

pub fn verify_signature(address: &str, message: &str, encoded: &str) -> Result<(), GameError> {
    let public =
        Pubkey::from_str(address).map_err(|_| GameError::invalid("Invalid wallet address"))?;
    let bytes = bs58::decode(encoded)
        .into_vec()
        .map_err(|_| GameError::invalid("Invalid signature"))?;
    if bytes.len() != 64 || !public.is_on_curve() {
        return Err(GameError::invalid("Invalid wallet signature"));
    }
    UnparsedPublicKey::new(&signature::ED25519, public.as_ref())
        .verify(message.as_bytes(), &bytes)
        .map_err(|_| GameError::invalid("Signature does not match the wallet and challenge"))
}

pub(crate) async fn get(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Value>, GameError> {
    let (user, mut tx) = context(&state, &headers).await?;
    let address: Option<String> =
        sqlx::query_scalar("SELECT address FROM sozbricks.wallet_bindings WHERE user_id=$1")
            .bind(user.id)
            .fetch_optional(&mut *tx)
            .await?;
    tx.commit().await?;
    Ok(Json(json!({"address":address,"cluster":"devnet"})))
}

pub(crate) async fn challenge(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    input: Result<Json<ChallengeRequest>, JsonRejection>,
) -> Result<Json<Value>, GameError> {
    let request = body(input)?;
    let address = Pubkey::from_str(&request.address)
        .map_err(|_| GameError::invalid("Invalid wallet address"))?;
    if !address.is_on_curve()
        || state
            .config
            .treasury
            .as_ref()
            .is_some_and(|t| t.address() == request.address)
    {
        return Err(GameError::invalid(
            "Use a personal wallet, not the project treasury",
        ));
    }
    let (user, mut tx) = context(&state, &headers).await?;
    profile::lock(&mut tx, user.id).await?;
    let recent: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sozbricks.wallet_challenges WHERE user_id=$1 AND created_at>now()-interval '5 seconds')").bind(user.id).fetch_one(&mut *tx).await?;
    if recent {
        return Err(GameError::conflict(
            "Wait five seconds before requesting another challenge",
        ));
    }
    let id = Uuid::new_v4();
    let expires: String = sqlx::query_scalar("SELECT (now()+interval '5 minutes')::text")
        .fetch_one(&mut *tx)
        .await?;
    let message = format!(
        "Söz Bricks — link your personal wallet\nNetwork: Solana Devnet (test SOL only)\nAccount: {}\nWallet: {}\nNonce: {}\nExpires: {}\nThis signature links this wallet to your game account. It does not authorize a transfer.",
        user.id, address, id, expires
    );
    sqlx::query("INSERT INTO sozbricks.wallet_challenges(user_id,id,address,message,expires_at) VALUES($1,$2,$3,$4,$5::text::timestamptz) ON CONFLICT(user_id) DO UPDATE SET id=excluded.id,address=excluded.address,message=excluded.message,expires_at=excluded.expires_at,created_at=now(),consumed=false")
        .bind(user.id).bind(id).bind(address.to_string()).bind(&message).bind(&expires).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(
        json!({"challenge_id":id,"message":message,"expires_at":expires}),
    ))
}

pub(crate) async fn verify(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    input: Result<Json<VerifyRequest>, JsonRejection>,
) -> Result<Json<Value>, GameError> {
    let request = body(input)?;
    if request.signature.len() > 100 {
        return Err(GameError::invalid("Invalid signature length"));
    }
    let (user, mut tx) = context(&state, &headers).await?;
    profile::lock(&mut tx, user.id).await?;
    let row = sqlx::query("SELECT address,message FROM sozbricks.wallet_challenges WHERE user_id=$1 AND id=$2 AND NOT consumed AND expires_at>now() FOR UPDATE")
        .bind(user.id).bind(request.challenge_id).fetch_optional(&mut *tx).await?.ok_or_else(|| GameError::conflict("Challenge expired or was already used; request a new one"))?;
    let address: String = row.get("address");
    verify_signature(
        &address,
        &row.get::<String, _>("message"),
        &request.signature,
    )?;
    sqlx::query("INSERT INTO sozbricks.wallet_bindings(user_id,address) VALUES($1,$2) ON CONFLICT(user_id) DO UPDATE SET address=excluded.address,verified_at=now()")
        .bind(user.id).bind(&address).execute(&mut *tx).await?;
    sqlx::query("UPDATE sozbricks.wallet_challenges SET consumed=true WHERE user_id=$1")
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({"address":address,"cluster":"devnet"})))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::signature::{Ed25519KeyPair, KeyPair};
    #[test]
    fn signature_is_bound_to_message_and_wallet() {
        let key = Ed25519KeyPair::from_seed_unchecked(&[9; 32]).unwrap();
        let address = bs58::encode(key.public_key().as_ref()).into_string();
        let proof = bs58::encode(key.sign(b"account:one:nonce:one").as_ref()).into_string();
        assert!(verify_signature(&address, "account:one:nonce:one", &proof).is_ok());
        assert!(verify_signature(&address, "account:two:nonce:one", &proof).is_err());
        assert!(verify_signature(&address, "account:one:nonce:two", &proof).is_err());
        assert!(
            verify_signature(
                "11111111111111111111111111111111",
                "account:one:nonce:one",
                &proof
            )
            .is_err()
        );
    }
}
