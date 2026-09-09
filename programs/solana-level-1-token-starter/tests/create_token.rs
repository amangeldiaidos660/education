use anchor_lang::{InstructionData, ToAccountMetas};
use litesvm::LiteSVM;
use solana_keypair::Keypair;
use solana_message::{AccountMeta, Instruction, Message};
use solana_signer::Signer;
use solana_transaction::Transaction;
use std::{fs, path::PathBuf};

use anchor_lang::AccountDeserialize;
use anchor_spl::token_interface::Mint;
use anchor_lang::solana_program::program_option::COption;

const DECIMALS: u8 = 6;

fn program_bytes() -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/deploy/solana_level_1_token_starter.so");
    fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "Build the program with `anchor build` before running tests. Could not read {}: {error}",
            path.display()
        )
    })
}

#[test]
fn creates_token_2022_mint() {
    let program_id = solana_level_1_token_starter::ID;
    let token_program = anchor_spl::token_2022::ID;
    let mut svm = LiteSVM::new();
    svm.add_program(program_id, &program_bytes())
        .expect("program must load");

    let payer = Keypair::new();
    let authority = Keypair::new();
    let mint = Keypair::new();
    svm.airdrop(&payer.pubkey(), 1_000_000_000)
        .expect("airdrop must succeed");

    let accounts = solana_level_1_token_starter::accounts::CreateToken {
        payer: payer.pubkey(),
        authority: authority.pubkey(),
        mint: mint.pubkey(),
        token_program,
        system_program: anchor_lang::system_program::ID,
    };
    let instruction = Instruction {
        program_id,
        accounts: accounts
            .to_account_metas(None)
            .into_iter()
            .map(|meta| AccountMeta {
                pubkey: meta.pubkey,
                is_signer: meta.is_signer,
                is_writable: meta.is_writable,
            })
            .collect(),
        data: solana_level_1_token_starter::instruction::CreateToken { decimals: DECIMALS }.data(),
    };
    let blockhash = svm.latest_blockhash();
    let message = Message::new(&[instruction], Some(&payer.pubkey()));
    let transaction = Transaction::new(&[&payer, &authority, &mint], message, blockhash);

    svm.send_transaction(transaction)
        .expect("create_token must succeed");

    let mint_account = svm.get_account(&mint.pubkey()).expect("mint must exist");
    assert_eq!(mint_account.owner, token_program);
    assert!(!mint_account.data.is_empty());

    let mint_state = Mint::try_deserialize(&mut mint_account.data.as_slice())
        .expect("mint data must deserialize");

    assert_eq!(mint_state.decimals, DECIMALS);
    assert_eq!(mint_state.supply, 0);

    assert_eq!(
        mint_state.mint_authority,
        COption::Some(authority.pubkey())
    );
}

// Student work for task/01-tests:
// 1. Assert decimals, mint authority and supply, not only account existence.
// 2. Cover create_token_account, mint_tokens and transfer_tokens end-to-end.
// 3. Add failures for zero amount, wrong authority, wrong mint and same source/destination.
