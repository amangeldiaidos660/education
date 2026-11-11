use std::{str::FromStr, sync::Arc, time::Duration};

use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use solana_hash::Hash;
use solana_instruction::{AccountMeta, Instruction};
use solana_message::Message;
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use solana_transaction::Transaction;
use sqlx::{PgPool, Row, postgres::PgRow};
use uuid::Uuid;

use crate::{config::Config, database::Database, treasury::Treasury};

pub struct Chain {
    pub program: Pubkey,
    pub treasury: Arc<Treasury>,
    client: reqwest::Client,
    rpc_url: String,
}

pub fn discriminator(name: &str) -> [u8; 8] {
    Sha256::digest(name.as_bytes())[..8].try_into().unwrap()
}

impl Chain {
    pub fn new(program: Pubkey, treasury: Arc<Treasury>) -> Self {
        Self {
            program,
            treasury,
            rpc_url: "https://api.devnet.solana.com".into(),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(8))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("RPC client"),
        }
    }

    pub async fn rpc(&self, method: &str, params: Value) -> Result<Value, &'static str> {
        let response = self
            .client
            .post(&self.rpc_url)
            .header("Content-Type", "application/json")
            .body(json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}).to_string())
            .send()
            .await
            .map_err(|_| "rpc_unavailable")?
            .error_for_status()
            .map_err(|_| "rpc_unavailable")?
            .bytes()
            .await
            .map_err(|_| "rpc_unavailable")?;
        let value: Value = serde_json::from_slice(&response).map_err(|_| "rpc_invalid")?;
        if !value["error"].is_null() || value.get("result").is_none() {
            return Err("rpc_rejected");
        }
        Ok(value["result"].clone())
    }

    pub fn authority(&self) -> Pubkey {
        Pubkey::from_str(self.treasury.address()).expect("validated treasury")
    }
    pub fn pda(&self, seeds: &[&[u8]]) -> Pubkey {
        Pubkey::find_program_address(seeds, &self.program).0
    }

    pub async fn account(
        &self,
        address: Pubkey,
        min_slot: u64,
    ) -> Result<Option<Vec<u8>>, &'static str> {
        let value = self.rpc("getAccountInfo", json!([address.to_string(),{"encoding":"base64","commitment":"finalized","minContextSlot":min_slot}])).await?;
        if value["value"].is_null() {
            return Ok(None);
        }
        if value["value"]["owner"] != self.program.to_string()
            || value["value"]["executable"] != false
        {
            return Err("account_mismatch");
        }
        let data = STANDARD
            .decode(
                value["value"]["data"][0]
                    .as_str()
                    .ok_or("account_invalid")?,
            )
            .map_err(|_| "account_invalid")?;
        Ok(Some(data))
    }

    pub async fn check(&self, allow_paused: bool) -> Result<(), &'static str> {
        if self.rpc("getGenesisHash", json!([])).await?
            != "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG"
        {
            return Err("wrong_cluster");
        }
        let program = self
            .rpc(
                "getAccountInfo",
                json!([self.program.to_string(),{"encoding":"base64","commitment":"finalized"}]),
            )
            .await?;
        if program["value"]["executable"] != true {
            return Err("program_not_deployed");
        }
        let bytes = self
            .account(self.pda(&[b"config"]), 0)
            .await?
            .ok_or("program_not_initialized")?;
        if bytes.len() != 49
            || bytes[..8] != discriminator("account:Settings")
            || bytes[8..40] != self.authority().to_bytes()
            || u64::from_le_bytes(bytes[41..49].try_into().unwrap()) != 500_000_000
        {
            return Err("program_config_mismatch");
        }
        if bytes[40] != 0 && !allow_paused {
            return Err("program_paused");
        }
        Ok(())
    }

    pub fn instruction(&self, name: &str, args: &[u8], accounts: Vec<AccountMeta>) -> Instruction {
        Instruction {
            program_id: self.program,
            accounts,
            data: [discriminator(&format!("global:{name}")).as_slice(), args].concat(),
        }
    }

    pub fn payout_instruction(&self, reward: Uuid, recipient: Pubkey, amount: u64) -> Instruction {
        self.instruction(
            "payout",
            &[reward.as_bytes().as_slice(), &amount.to_le_bytes()].concat(),
            vec![
                AccountMeta::new(self.authority(), true),
                AccountMeta::new_readonly(self.pda(&[b"config"]), false),
                AccountMeta::new(self.pda(&[b"reserve"]), false),
                AccountMeta::new(recipient, false),
                AccountMeta::new(self.pda(&[b"receipt", reward.as_bytes()]), false),
                AccountMeta::new_readonly(Pubkey::default(), false),
            ],
        )
    }

    pub async fn signed(
        &self,
        instruction: Instruction,
    ) -> Result<(String, String, i64), &'static str> {
        let value = self
            .rpc("getLatestBlockhash", json!([{"commitment":"finalized"}]))
            .await?;
        let hash = Hash::from_str(
            value["value"]["blockhash"]
                .as_str()
                .ok_or("blockhash_invalid")?,
        )
        .map_err(|_| "blockhash_invalid")?;
        let height = value["value"]["lastValidBlockHeight"]
            .as_i64()
            .ok_or("blockhash_invalid")?;
        let message = Message::new_with_blockhash(&[instruction], Some(&self.authority()), &hash);
        let mut tx = Transaction::new_unsigned(message);
        tx.signatures[0] = Signature::from(self.treasury.sign(&tx.message_data()));
        tx.verify().map_err(|_| "signature_invalid")?;
        let encoded = STANDARD.encode(bincode::serialize(&tx).map_err(|_| "transaction_invalid")?);
        Ok((tx.signatures[0].to_string(), encoded, height))
    }

    pub async fn send(&self, encoded: &str, expected: &str) -> Result<(), &'static str> {
        let signature = self.rpc("sendTransaction",json!([encoded,{"encoding":"base64","skipPreflight":false,"preflightCommitment":"finalized","maxRetries":0}])).await?;
        if signature.as_str() != Some(expected) {
            return Err("signature_mismatch");
        }
        Ok(())
    }

    pub async fn receipt(
        &self,
        reward: Uuid,
        recipient: Pubkey,
        amount: u64,
    ) -> Result<bool, &'static str> {
        let slot = self
            .rpc("getSlot", json!([{"commitment":"finalized"}]))
            .await?
            .as_u64()
            .ok_or("slot_invalid")?;
        let Some(bytes) = self
            .account(self.pda(&[b"receipt", reward.as_bytes()]), slot)
            .await?
        else {
            return Ok(false);
        };
        if !valid_receipt(&bytes, reward, recipient, amount) {
            return Err("receipt_mismatch");
        }
        Ok(true)
    }
}

fn valid_receipt(bytes: &[u8], reward: Uuid, recipient: Pubkey, amount: u64) -> bool {
    bytes.len() == 72
        && bytes[..8] == discriminator("account:Receipt")
        && bytes[8..24] == *reward.as_bytes()
        && bytes[24..56] == recipient.to_bytes()
        && bytes[56..64] == amount.to_le_bytes()
}

pub async fn run(config: Config, database: Database) {
    if !config.payments_enabled {
        return;
    }
    let chain = Chain::new(
        config.program_address.expect("configured program"),
        config.treasury.clone().expect("configured treasury"),
    );
    loop {
        if let Some(pool) = database.worker_pool()
            && let Err(code) = tick(&chain, pool).await
        {
            tracing::warn!(code, "payout worker will retry");
        }
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}

async fn tick(chain: &Chain, pool: &PgPool) -> Result<(), &'static str> {
    let active: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sozbricks.reward_campaign WHERE id AND activated_at IS NOT NULL AND program_address=$1)")
        .bind(chain.program.to_string()).fetch_one(pool).await.map_err(|_| "queue_unavailable")?;
    if !active {
        return Ok(());
    }
    chain.check(false).await?;
    let lease = Uuid::new_v4();
    let row = sqlx::query("WITH candidate AS (SELECT p.reward_id FROM sozbricks.reward_payouts p WHERE p.status NOT IN ('paid','blocked') AND p.next_attempt_at<=now() AND (p.lease_until IS NULL OR p.lease_until<now()) AND (p.recipient IS NOT NULL OR EXISTS(SELECT 1 FROM sozbricks.wallet_bindings w WHERE w.user_id=p.user_id)) ORDER BY p.next_attempt_at,p.reward_id FOR UPDATE SKIP LOCKED LIMIT 1) UPDATE sozbricks.reward_payouts p SET lease_id=$1,lease_until=now()+interval '2 minutes',recipient=coalesce(p.recipient,(SELECT address FROM sozbricks.wallet_bindings WHERE user_id=p.user_id)),status=CASE WHEN p.status='awaiting_wallet' THEN 'queued' ELSE p.status END FROM candidate c,sozbricks.reward_ledger r WHERE p.reward_id=c.reward_id AND r.id=p.reward_id AND r.payable RETURNING p.*,r.amount_lamports")
        .bind(lease).fetch_optional(pool).await.map_err(|_| "queue_unavailable")?;
    let Some(row) = row else {
        return Ok(());
    };
    let reward: Uuid = row.get("reward_id");
    if let Err(code) = process(chain, pool, &row, lease).await {
        let blocked = matches!(
            code,
            "receipt_mismatch" | "account_mismatch" | "recipient_invalid"
        );
        sqlx::query("UPDATE sozbricks.reward_payouts SET status=$3,error_code=$4,lease_until=NULL,lease_id=NULL,next_attempt_at=now()+interval '30 seconds',updated_at=now() WHERE reward_id=$1 AND lease_id=$2")
            .bind(reward).bind(lease).bind(if blocked { "blocked" } else { "retry" }).bind(code).execute(pool).await.map_err(|_| "queue_unavailable")?;
        return Err(code);
    }
    Ok(())
}

async fn mark_paid(
    chain: &Chain,
    pool: &PgPool,
    reward: Uuid,
    lease: Uuid,
) -> Result<(), &'static str> {
    let receipt = chain.pda(&[b"receipt", reward.as_bytes()]).to_string();
    let signatures = chain
        .rpc(
            "getSignaturesForAddress",
            json!([receipt,{"commitment":"finalized","limit":100}]),
        )
        .await?;
    let known: Vec<String> =
        sqlx::query_scalar("SELECT signature FROM sozbricks.payout_attempts WHERE reward_id=$1")
            .bind(reward)
            .fetch_all(pool)
            .await
            .map_err(|_| "queue_unavailable")?;
    let signature = signatures
        .as_array()
        .ok_or("rpc_invalid")?
        .iter()
        .filter(|v| v["err"].is_null())
        .filter_map(|v| v["signature"].as_str())
        .find(|s| known.iter().any(|v| v == s))
        .ok_or("receipt_signature_pending")?;
    sqlx::query("UPDATE sozbricks.reward_payouts SET status='paid',signature=$3,signed_transaction=(SELECT signed_transaction FROM sozbricks.payout_attempts WHERE signature=$3),last_valid_height=(SELECT last_valid_height FROM sozbricks.payout_attempts WHERE signature=$3),receipt_address=$4,paid_at=now(),error_code=NULL,lease_id=NULL,lease_until=NULL,updated_at=now() WHERE reward_id=$1 AND lease_id=$2")
        .bind(reward).bind(lease).bind(signature).bind(receipt).execute(pool).await.map_err(|_| "queue_unavailable")?;
    Ok(())
}

async fn process(
    chain: &Chain,
    pool: &PgPool,
    row: &PgRow,
    lease: Uuid,
) -> Result<(), &'static str> {
    let reward: Uuid = row.get("reward_id");
    let recipient =
        Pubkey::from_str(&row.get::<String, _>("recipient")).map_err(|_| "recipient_invalid")?;
    let amount =
        u64::try_from(row.get::<i64, _>("amount_lamports")).map_err(|_| "recipient_invalid")?;
    if chain.receipt(reward, recipient, amount).await? {
        return mark_paid(chain, pool, reward, lease).await;
    }
    let mut signature: Option<String> = row.get("signature");
    let mut encoded: Option<String> = row.get("signed_transaction");
    if let Some(height) = row.get::<Option<i64>, _>("last_valid_height") {
        let current = chain
            .rpc("getBlockHeight", json!([{"commitment":"finalized"}]))
            .await?
            .as_i64()
            .ok_or("height_invalid")?;
        if current > height {
            if chain.receipt(reward, recipient, amount).await? {
                return mark_paid(chain, pool, reward, lease).await;
            }
            signature = None;
            encoded = None;
        }
    }
    if signature.is_none() {
        let (sig, wire, height) = chain
            .signed(chain.payout_instruction(reward, recipient, amount))
            .await?;
        let mut tx = pool.begin().await.map_err(|_| "queue_unavailable")?;
        let updated = sqlx::query("UPDATE sozbricks.reward_payouts SET signature=$3,signed_transaction=$4,last_valid_height=$5,receipt_address=$6,status='submitted',updated_at=now() WHERE reward_id=$1 AND lease_id=$2 AND lease_until>now()")
            .bind(reward).bind(lease).bind(&sig).bind(&wire).bind(height).bind(chain.pda(&[b"receipt",reward.as_bytes()]).to_string()).execute(&mut *tx).await.map_err(|_| "queue_unavailable")?;
        if updated.rows_affected() != 1 {
            return Err("lease_expired");
        }
        sqlx::query("INSERT INTO sozbricks.payout_attempts(signature,reward_id,signed_transaction,last_valid_height) VALUES($1,$2,$3,$4)")
            .bind(&sig).bind(reward).bind(&wire).bind(height).execute(&mut *tx).await.map_err(|_| "queue_unavailable")?;
        tx.commit().await.map_err(|_| "queue_unavailable")?;
        signature = Some(sig);
        encoded = Some(wire);
    }
    chain
        .send(
            encoded.as_deref().ok_or("transaction_invalid")?,
            signature.as_deref().ok_or("transaction_invalid")?,
        )
        .await?;
    sqlx::query("UPDATE sozbricks.reward_payouts SET status='submitted',error_code=NULL,lease_id=NULL,lease_until=NULL,next_attempt_at=now()+interval '15 seconds',updated_at=now() WHERE reward_id=$1 AND lease_id=$2")
        .bind(reward).bind(lease).execute(pool).await.map_err(|_| "queue_unavailable")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Json, Router, routing::post};
    use ring::signature::{Ed25519KeyPair, KeyPair};

    #[tokio::test]
    async fn serialized_transaction_can_be_recovered_and_resubmitted_unchanged() {
        let key = Ed25519KeyPair::from_seed_unchecked(&[7; 32]).unwrap();
        let bytes =
            serde_json::to_vec(&[[7; 32].as_slice(), key.public_key().as_ref()].concat()).unwrap();
        let address = bs58::encode(key.public_key().as_ref()).into_string();
        let treasury = Arc::new(Treasury::from_bytes(&bytes, &address).unwrap());
        let mut chain = Chain::new(Pubkey::new_unique(), treasury);
        let router=Router::new().route("/",post(|Json(value):Json<Value>| async move {
            let result=match value["method"].as_str().unwrap() {
                "getLatestBlockhash"=>json!({"value":{"blockhash":Hash::new_from_array([3;32]).to_string(),"lastValidBlockHeight":150}}),
                "sendTransaction"=>{
                    let tx:Transaction=bincode::deserialize(&STANDARD.decode(value["params"][0].as_str().unwrap()).unwrap()).unwrap();
                    tx.verify().unwrap();
                    assert_eq!(tx.message.instructions.len(),1);
                    json!(tx.signatures[0].to_string())
                },
                _=>panic!("unexpected method"),
            };
            Json(json!({"jsonrpc":"2.0","id":1,"result":result}))
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        chain.rpc_url = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let (signature, wire, height) = chain
            .signed(chain.payout_instruction(Uuid::new_v4(), Pubkey::new_unique(), 100_000_000))
            .await
            .unwrap();
        assert_eq!(height, 150);
        let saved = serde_json::to_string(&(signature, wire)).unwrap();
        let (signature, wire): (String, String) = serde_json::from_str(&saved).unwrap();
        chain.send(&wire, &signature).await.unwrap();
        chain.send(&wire, &signature).await.unwrap();
        assert_eq!(
            chain.send(&wire, "wrong-signature").await,
            Err("signature_mismatch")
        );
        server.abort();
    }
    #[test]
    fn receipt_binds_reward_recipient_and_amount() {
        let id = Uuid::new_v4();
        let recipient = Pubkey::new_unique();
        let mut bytes = [
            discriminator("account:Receipt").as_slice(),
            id.as_bytes(),
            recipient.as_ref(),
            &7_u64.to_le_bytes(),
            &1_u64.to_le_bytes(),
        ]
        .concat();
        assert!(valid_receipt(&bytes, id, recipient, 7));
        assert!(!valid_receipt(&bytes, id, recipient, 8));
        assert!(!valid_receipt(&bytes, Uuid::new_v4(), recipient, 7));
        assert!(!valid_receipt(&bytes, id, Pubkey::new_unique(), 7));
        bytes[0] ^= 1;
        assert!(!valid_receipt(&bytes, id, recipient, 7));
    }
}
