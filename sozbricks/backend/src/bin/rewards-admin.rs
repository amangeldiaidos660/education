use serde_json::json;
use solana_instruction::AccountMeta;
use solana_pubkey::Pubkey;
use sozbricks_api::{config::Config, database::Database, payouts::Chain};
use std::{error::Error, str::FromStr};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("status");
    let config = Config::from_env()?;
    let chain = Chain::new(
        config
            .program_address
            .ok_or("Set SOLANA_REWARDS_PROGRAM_ID")?,
        config.treasury.clone().ok_or("Configure treasury")?,
    );
    if chain.rpc("getGenesisHash", json!([])).await?
        != "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG"
    {
        return Err("Wrong network".into());
    }
    println!(
        "Devnet program: {}\nReserve PDA: {}\nTreasury: {}",
        chain.program,
        chain.pda(&[b"reserve"]),
        chain.authority()
    );
    if command == "status" {
        chain.check(true).await?;
        let value = chain
            .rpc(
                "getBalance",
                json!([chain.pda(&[b"reserve"]).to_string(),{"commitment":"finalized"}]),
            )
            .await?;
        println!("Reserve lamports: {}", value["value"]);
        return Ok(());
    }
    if !args.iter().any(|arg| arg == "--execute") {
        return Err(
            "Review the command, then add --execute to change Devnet or activate rewards".into(),
        );
    }
    if command == "activate" {
        chain.check(false).await?;
        let balance = chain
            .rpc(
                "getBalance",
                json!([chain.pda(&[b"reserve"]).to_string(),{"commitment":"finalized"}]),
            )
            .await?;
        if balance["value"].as_u64().unwrap_or(0) < 501_000_000 {
            return Err("Fund the reserve before activation".into());
        }
        let db = Database::new(config.database.as_ref());
        db.prepare().await?;
        let row = sqlx::query("UPDATE sozbricks.reward_campaign SET activated_at=coalesce(activated_at,now()),program_address=$1 WHERE id AND (program_address IS NULL OR program_address=$1)")
            .bind(chain.program.to_string()).execute(db.pool().ok_or("Configure database")?).await?;
        if row.rows_affected() != 1 {
            return Err("Campaign is bound to another program; do not reset it".into());
        }
        println!(
            "Campaign activated; existing activation time preserved. Historical rewards stay excluded."
        );
        db.close().await;
        return Ok(());
    }
    let authority = AccountMeta::new(chain.authority(), true);
    let settings = chain.pda(&[b"config"]);
    let system = AccountMeta::new_readonly(Pubkey::default(), false);
    let instruction = match command {
        "initialize" => {
            let program=chain.rpc("getAccountInfo",json!([chain.program.to_string(),{"encoding":"base64","commitment":"finalized"}])).await?;
            if program["value"]["executable"] != true {
                return Err("Deploy the program first".into());
            }
            chain.instruction(
                "initialize",
                &[],
                vec![authority, AccountMeta::new(settings, false), system],
            )
        }
        "fund" => {
            chain.check(true).await?;
            let amount = u64::from_str(
                args.get(1)
                    .ok_or("Provide integer lamports, e.g. 5000000000")?,
            )?;
            if amount == 0 {
                return Err("Amount must be positive".into());
            }
            chain.instruction(
                "fund",
                &amount.to_le_bytes(),
                vec![
                    authority,
                    AccountMeta::new_readonly(settings, false),
                    AccountMeta::new(chain.pda(&[b"reserve"]), false),
                    system,
                ],
            )
        }
        "pause" | "resume" => {
            chain.check(true).await?;
            chain.instruction(
                "set_paused",
                &[u8::from(command == "pause")],
                vec![authority, AccountMeta::new(settings, false)],
            )
        }
        _ => {
            return Err(
                "Commands: status, initialize, fund <lamports>, pause, resume, activate".into(),
            );
        }
    };
    let (signature, wire, _) = chain.signed(instruction).await?;
    println!("Transaction: https://explorer.solana.com/tx/{signature}?cluster=devnet");
    chain.send(&wire, &signature).await?;
    println!(
        "Submitted. Confirm finalization before the next action. Do not repeat fund after a timeout without checking this signature."
    );
    Ok(())
}
