use anchor_lang::{AccountDeserialize, InstructionData, ToAccountMetas};
use anchor_spl::{
    associated_token::get_associated_token_address_with_program_id,
    token_interface::{Mint, TokenAccount},
};
use litesvm::LiteSVM;
use solana_keypair::Keypair;
use solana_message::{AccountMeta, Instruction, Message};
use solana_signer::Signer;
use solana_transaction::Transaction;
use std::{fs, path::PathBuf};

const DECIMALS: u8 = 6;
const MINT_AMOUNT: u64 = 1_000_000;

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
fn mints_tokens_and_updates_supply() {
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

    // 1. Create Token-2022 mint.
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

    // 2. Create associated token account for authority.
    let destination = get_associated_token_address_with_program_id(
        &authority.pubkey(),
        &mint.pubkey(),
        &token_program,
    );

    let create_token_account_accounts =
        solana_level_1_token_starter::accounts::CreateTokenAccount {
            payer: payer.pubkey(),
            owner: authority.pubkey(),
            mint: mint.pubkey(),
            token_account: destination,
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

    let transaction =
        Transaction::new(&[&payer], message, blockhash);

    svm.send_transaction(transaction)
        .expect("create_token_account must succeed");

    // Check initial balances before minting.
    let destination_account = svm
        .get_account(&destination)
        .expect("destination must exist");

    let destination_state =
        TokenAccount::try_deserialize(&mut destination_account.data.as_slice())
            .expect("destination data must deserialize");

    assert_eq!(destination_state.amount, 0);

    let mint_account = svm
        .get_account(&mint.pubkey())
        .expect("mint must exist");

    let mint_state =
        Mint::try_deserialize(&mut mint_account.data.as_slice())
            .expect("mint data must deserialize");

    assert_eq!(mint_state.supply, 0);

    // 3. Mint tokens.
    let mint_tokens_accounts =
        solana_level_1_token_starter::accounts::MintTokens {
            authority: authority.pubkey(),
            mint: mint.pubkey(),
            destination,
            token_program,
        };

    let mint_tokens_instruction = Instruction {
        program_id,
        accounts: mint_tokens_accounts
            .to_account_metas(None)
            .into_iter()
            .map(|meta| AccountMeta {
                pubkey: meta.pubkey,
                is_signer: meta.is_signer,
                is_writable: meta.is_writable,
            })
            .collect(),
        data: solana_level_1_token_starter::instruction::MintTokens {
            amount: MINT_AMOUNT,
        }
        .data(),
    };

    let blockhash = svm.latest_blockhash();

    let message = Message::new(
        &[mint_tokens_instruction],
        Some(&payer.pubkey()),
    );

    let transaction =
        Transaction::new(&[&payer, &authority], message, blockhash);

    svm.send_transaction(transaction)
        .expect("mint_tokens must succeed");

    // 4. Check recipient balance after minting.
    let destination_account = svm
        .get_account(&destination)
        .expect("destination must exist");

    let destination_state =
        TokenAccount::try_deserialize(&mut destination_account.data.as_slice())
            .expect("destination data must deserialize");

    assert_eq!(destination_state.amount, MINT_AMOUNT);

    // 5. Check total mint supply after minting.
    let mint_account = svm
        .get_account(&mint.pubkey())
        .expect("mint must exist");

    let mint_state =
        Mint::try_deserialize(&mut mint_account.data.as_slice())
            .expect("mint data must deserialize");

    assert_eq!(mint_state.supply, MINT_AMOUNT);
}