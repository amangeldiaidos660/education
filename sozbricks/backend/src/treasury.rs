use std::{fs::File, io::Read, time::Duration};

use axum::{Json, extract::State, http::StatusCode};
use ring::signature::{self, Ed25519KeyPair, KeyPair};
use serde::Serialize;
use serde_json::{Value, json};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{AppState, error::ApiError};

const RPC_URL: &str = "https://api.devnet.solana.com";
const GENESIS_HASH: &str = "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG";

pub struct Treasury {
    key: Ed25519KeyPair,
    address: String,
    proof_message: String,
    proof_signature: String,
    client: reqwest::Client,
}

#[derive(Serialize)]
pub struct TreasuryStatus {
    status: &'static str,
    cluster: &'static str,
    address: String,
    balance_lamports: u64,
    slot: u64,
    commitment: &'static str,
    signer_verified: bool,
    proof_message: String,
    proof_signature: String,
    payments_enabled: bool,
}

impl Treasury {
    pub fn load(path: &str, expected_address: &str) -> Result<Self, &'static str> {
        let mut bytes = Zeroizing::new(Vec::new());
        File::open(path)
            .and_then(|file| file.take(4097).read_to_end(&mut bytes))
            .map_err(|_| "must point to a readable Solana keypair file")?;
        if bytes.len() > 4096 {
            return Err("keypair file must not exceed 4096 bytes");
        }
        Self::from_bytes(&bytes, expected_address)
    }

    pub(crate) fn from_bytes(bytes: &[u8], expected_address: &str) -> Result<Self, &'static str> {
        let secret: Zeroizing<Vec<u8>> = Zeroizing::new(
            serde_json::from_slice(bytes).map_err(|_| "must contain a Solana JSON keypair")?,
        );
        if secret.len() != 64 {
            return Err("keypair must contain exactly 64 bytes");
        }
        let key = Ed25519KeyPair::from_seed_and_public_key(&secret[..32], &secret[32..])
            .map_err(|_| "keypair public and private keys do not match")?;
        let address = bs58::encode(key.public_key().as_ref()).into_string();
        if address != expected_address {
            return Err("keypair does not match SOLANA_TREASURY_ADDRESS");
        }
        let proof_message = format!(
            "sozbricks:treasury-check:devnet:{address}:{}",
            Uuid::new_v4()
        );
        let proof = key.sign(proof_message.as_bytes());
        signature::UnparsedPublicKey::new(&signature::ED25519, key.public_key().as_ref())
            .verify(proof_message.as_bytes(), proof.as_ref())
            .map_err(|_| "keypair signing check failed")?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(8))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| "could not initialize Devnet RPC client")?;
        Ok(Self {
            key,
            address,
            proof_message,
            proof_signature: bs58::encode(proof.as_ref()).into_string(),
            client,
        })
    }

    pub fn address(&self) -> &str {
        &self.address
    }

    pub(crate) fn sign(&self, message: &[u8]) -> [u8; 64] {
        self.key
            .sign(message)
            .as_ref()
            .try_into()
            .expect("Ed25519 signature length")
    }

    async fn status(&self, rpc_url: &str) -> Result<TreasuryStatus, ApiError> {
        let payload = json!([
            {"jsonrpc":"2.0","id":"cluster","method":"getGenesisHash","params":[]},
            {"jsonrpc":"2.0","id":"balance","method":"getBalance","params":[self.address,{"commitment":"finalized"}]}
        ]);
        let response = self
            .client
            .post(rpc_url)
            .header("Content-Type", "application/json")
            .body(payload.to_string())
            .send()
            .await
            .map_err(|_| unavailable())?
            .error_for_status()
            .map_err(|_| unavailable())?
            .bytes()
            .await
            .map_err(|_| unavailable())?;
        let items: Vec<Value> = serde_json::from_slice(&response).map_err(|_| unavailable())?;
        if items.len() != 2 || items.iter().any(|item| !item["error"].is_null()) {
            return Err(unavailable());
        }
        let cluster = items
            .iter()
            .find(|item| item["id"] == "cluster")
            .ok_or_else(unavailable)?;
        if cluster["result"].as_str() != Some(GENESIS_HASH) {
            return Err(ApiError {
                status: StatusCode::SERVICE_UNAVAILABLE,
                code: "solana_cluster_mismatch",
                message: "RPC did not confirm the Solana Devnet genesis hash",
            });
        }
        let balance = items
            .iter()
            .find(|item| item["id"] == "balance")
            .ok_or_else(unavailable)?;
        Ok(TreasuryStatus {
            status: "ready",
            cluster: "devnet",
            address: self.address.clone(),
            balance_lamports: balance["result"]["value"]
                .as_u64()
                .ok_or_else(unavailable)?,
            slot: balance["result"]["context"]["slot"]
                .as_u64()
                .ok_or_else(unavailable)?,
            commitment: "finalized",
            signer_verified: true,
            proof_message: self.proof_message.clone(),
            proof_signature: self.proof_signature.clone(),
            payments_enabled: false,
        })
    }
}

fn unavailable() -> ApiError {
    ApiError {
        status: StatusCode::SERVICE_UNAVAILABLE,
        code: "solana_rpc_unavailable",
        message: "Solana Devnet RPC is temporarily unavailable; retry later",
    }
}

pub(crate) async fn get_status(
    State(state): State<std::sync::Arc<AppState>>,
) -> Result<Json<TreasuryStatus>, ApiError> {
    let treasury = state.config.treasury.as_ref().ok_or(ApiError {
        status: StatusCode::SERVICE_UNAVAILABLE,
        code: "treasury_not_configured",
        message: "Treasury keypair file and expected address are not configured",
    })?;
    let mut status = treasury.status(RPC_URL).await?;
    status.payments_enabled = state.config.payments_enabled;
    Ok(Json(status))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, routing::post};

    fn fixture() -> (Vec<u8>, String) {
        let seed = [7_u8; 32];
        let key = Ed25519KeyPair::from_seed_unchecked(&seed).unwrap();
        let secret = [seed.as_slice(), key.public_key().as_ref()].concat();
        (
            serde_json::to_vec(&secret).unwrap(),
            bs58::encode(key.public_key().as_ref()).into_string(),
        )
    }

    #[test]
    fn keypair_must_match_expected_address_and_produce_a_verifiable_proof() {
        let (bytes, address) = fixture();
        let treasury = Treasury::from_bytes(&bytes, &address).unwrap();
        let public = bs58::decode(&address).into_vec().unwrap();
        let proof = bs58::decode(&treasury.proof_signature).into_vec().unwrap();
        signature::UnparsedPublicKey::new(&signature::ED25519, &public)
            .verify(treasury.proof_message.as_bytes(), &proof)
            .unwrap();
        assert!(Treasury::from_bytes(&bytes, "wrong-address").is_err());
        assert!(Treasury::from_bytes(b"[]", &address).is_err());
        assert!(Treasury::from_bytes(b"private-value-not-json", &address).is_err());
        let mut corrupt: Vec<u8> = serde_json::from_slice(&bytes).unwrap();
        corrupt[63] ^= 1;
        assert!(Treasury::from_bytes(&serde_json::to_vec(&corrupt).unwrap(), &address).is_err());
    }

    #[tokio::test]
    async fn checks_cluster_and_finalized_balance_without_sending_transactions() {
        let (bytes, address) = fixture();
        let treasury = Treasury::from_bytes(&bytes, &address).unwrap();
        for (genesis, fail, valid) in [
            (GENESIS_HASH, false, true),
            ("mainnet", false, false),
            (GENESIS_HASH, true, false),
        ] {
            let expected = address.clone();
            let router = Router::new().route("/", post(move |Json(body): Json<Value>| {
                let expected = expected.clone();
                async move {
                    assert_eq!(body.as_array().unwrap().len(), 2);
                    assert_eq!(body[0]["method"], "getGenesisHash");
                    assert_eq!(body[1]["method"], "getBalance");
                    assert_eq!(body[1]["params"][0], expected);
                    assert_eq!(body[1]["params"][1]["commitment"], "finalized");
                    if fail {
                        Json(json!([{"id":"cluster","result":genesis},{"id":"balance","error":{"code":429,"message":"private-rpc-error"}}]))
                    } else {
                        Json(json!([{"id":"balance","result":{"value":5_000_000_000_u64,"context":{"slot":123}}},{"id":"cluster","result":genesis}]))
                    }
                }
            }));
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap());
            let server = tokio::spawn(async move {
                axum::serve(listener, router).await.unwrap();
            });
            let result = treasury.status(&url).await;
            server.abort();
            if valid {
                let status = result.unwrap_or_else(|_| panic!("valid Devnet response rejected"));
                assert_eq!(status.balance_lamports, 5_000_000_000);
                assert_eq!(status.slot, 123);
                assert!(status.signer_verified);
                assert!(!status.payments_enabled);
            } else {
                let error = result.err().expect("invalid RPC must fail");
                assert!(!error.message.contains("private-rpc-error"));
                assert_eq!(error.status, StatusCode::SERVICE_UNAVAILABLE);
            }
        }
    }
}
