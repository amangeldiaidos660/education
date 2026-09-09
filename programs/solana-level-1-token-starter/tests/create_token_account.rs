use anchor_lang::{AccountDeserialize, InstructionData, ToAccountMetas};
use anchor_spl::{
    associated_token::get_associated_token_address_with_program_id,
    token_interface::TokenAccount,
};
use litesvm::LiteSVM;
use solana_keypair::Keypair;
use solana_message::{AccountMeta, Instruction, Message};
use solana_signer::Signer;
use solana_transaction::Transaction;
use std::{fs, path::PathBuf};

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
fn creates_token_2022_account() {
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

    // First create the Token-2022 mint that the token account will use.
    let create_token_accounts =
        solana_level_1_token_starter::accounts::CreateToken {
            payer: payer.pubkey(),
            authority: authority.pubkey(),
            mint: mint.pubkey(),
            token_program,
            system_program: anchor_lang::system_program::ID,
        };

    let create_token_instruction = Instruction {
        program_id,
        accounts: create_token_accounts
            .to_account_metas(None)
            .into_iter()
            .map(|meta| AccountMeta {
                pubkey: meta.pubkey,
                is_signer: meta.is_signer,
                is_writable: meta.is_writable,
            })
            .collect(),
        data: solana_level_1_token_starter::instruction::CreateToken {
            decimals: DECIMALS,
        }
        .data(),
    };

    let blockhash = svm.latest_blockhash();
    let message = Message::new(
        &[create_token_instruction],
        Some(&payer.pubkey()),
    );
    let transaction =
        Transaction::new(&[&payer, &authority, &mint], message, blockhash);

    svm.send_transaction(transaction)
        .expect("create_token must succeed");

    // The ATA address is deterministic for owner + mint + token program.
    let token_account = get_associated_token_address_with_program_id(
        &authority.pubkey(),
        &mint.pubkey(),
        &token_program,
    );

    let create_token_account_accounts =
        solana_level_1_token_starter::accounts::CreateTokenAccount {
            payer: payer.pubkey(),
            owner: authority.pubkey(),
            mint: mint.pubkey(),
            token_account,
            token_program,
            associated_token_program: anchor_spl::associated_token::ID,
            system_program: anchor_lang::system_program::ID,
        };

    let create_token_account_instruction = Instruction {
        program_id,
        accounts: create_token_account_accounts
            .to_account_metas(None)
            .into_iter()
            .map(|meta| AccountMeta {
                pubkey: meta.pubkey,
                is_signer: meta.is_signer,
                is_writable: meta.is_writable,
            })
            .collect(),
        data: solana_level_1_token_starter::instruction::CreateTokenAccount {}.data(),
    };

    let blockhash = svm.latest_blockhash();
    let message = Message::new(
        &[create_token_account_instruction],
        Some(&payer.pubkey()),
    );
    let transaction = Transaction::new(&[&payer], message, blockhash);

    svm.send_transaction(transaction)
        .expect("create_token_account must succeed");

    let token_account_data = svm
        .get_account(&token_account)
        .expect("token account must exist");

    // The account itself must be owned by the Token-2022 program.
    assert_eq!(token_account_data.owner, token_program);

    let token_state =
        TokenAccount::try_deserialize(&mut token_account_data.data.as_slice())
            .expect("token account data must deserialize");

    // The token account must belong to the requested wallet.
    assert_eq!(token_state.owner, authority.pubkey());

    // The token account must hold tokens from the requested mint.
    assert_eq!(token_state.mint, mint.pubkey());
}