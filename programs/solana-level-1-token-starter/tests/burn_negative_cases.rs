use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_lang::prelude::Pubkey;
use anchor_spl::associated_token::get_associated_token_address_with_program_id;
use litesvm::LiteSVM;
use solana_keypair::Keypair;
use solana_message::{AccountMeta, Instruction, Message};
use solana_signer::Signer;
use solana_transaction::Transaction;
use std::{fs, path::PathBuf};

const DECIMALS: u8 = 6;
const MINT_AMOUNT: u64 = 1_000_000;
const BURN_AMOUNT: u64 = 250_000;

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

fn send_success(
    svm: &mut LiteSVM,
    instruction: Instruction,
    payer: &Keypair,
    signers: &[&Keypair],
) {
    let blockhash = svm.latest_blockhash();
    let message = Message::new(&[instruction], Some(&payer.pubkey()));

    let mut all_signers = vec![payer];
    all_signers.extend_from_slice(signers);

    let transaction = Transaction::new(&all_signers, message, blockhash);

    svm.send_transaction(transaction)
        .expect("transaction must succeed");
}

fn send_failure(
    svm: &mut LiteSVM,
    instruction: Instruction,
    payer: &Keypair,
    signers: &[&Keypair],
) {
    let blockhash = svm.latest_blockhash();
    let message = Message::new(&[instruction], Some(&payer.pubkey()));

    let mut all_signers = vec![payer];
    all_signers.extend_from_slice(signers);

    let transaction = Transaction::new(&all_signers, message, blockhash);

    assert!(
        svm.send_transaction(transaction).is_err(),
        "transaction was expected to fail"
    );
}

fn setup() -> (LiteSVM, Keypair) {
    let mut svm = LiteSVM::new();

    svm.add_program(
        solana_level_1_token_starter::ID,
        &program_bytes(),
    )
    .expect("program must load");

    let payer = Keypair::new();

    svm.airdrop(&payer.pubkey(), 2_000_000_000)
        .expect("airdrop must succeed");

    (svm, payer)
}

fn create_mint(
    svm: &mut LiteSVM,
    payer: &Keypair,
    authority: &Keypair,
    mint: &Keypair,
) {
    let token_program = anchor_spl::token_2022::ID;

    let accounts =
        solana_level_1_token_starter::accounts::CreateToken {
            payer: payer.pubkey(),
            authority: authority.pubkey(),
            mint: mint.pubkey(),
            token_program,
            system_program: anchor_lang::system_program::ID,
        };

    let instruction = Instruction {
        program_id: solana_level_1_token_starter::ID,
        accounts: accounts
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

    send_success(
        svm,
        instruction,
        payer,
        &[authority, mint],
    );
}

fn create_token_account(
    svm: &mut LiteSVM,
    payer: &Keypair,
    owner: &Keypair,
    mint: &Keypair,
) -> Pubkey {
    let token_program = anchor_spl::token_2022::ID;

    let token_account = get_associated_token_address_with_program_id(
        &owner.pubkey(),
        &mint.pubkey(),
        &token_program,
    );

    let accounts =
        solana_level_1_token_starter::accounts::CreateTokenAccount {
            payer: payer.pubkey(),
            owner: owner.pubkey(),
            mint: mint.pubkey(),
            token_account,
            token_program,
            associated_token_program: anchor_spl::associated_token::ID,
            system_program: anchor_lang::system_program::ID,
        };

    let instruction = Instruction {
        program_id: solana_level_1_token_starter::ID,
        accounts: accounts
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

    send_success(svm, instruction, payer, &[]);

    token_account
}

fn mint_tokens(
    svm: &mut LiteSVM,
    payer: &Keypair,
    authority: &Keypair,
    mint: &Keypair,
    destination: Pubkey,
    amount: u64,
) {
    let accounts =
        solana_level_1_token_starter::accounts::MintTokens {
            authority: authority.pubkey(),
            mint: mint.pubkey(),
            destination,
            token_program: anchor_spl::token_2022::ID,
        };

    let instruction = Instruction {
        program_id: solana_level_1_token_starter::ID,
        accounts: accounts
            .to_account_metas(None)
            .into_iter()
            .map(|meta| AccountMeta {
                pubkey: meta.pubkey,
                is_signer: meta.is_signer,
                is_writable: meta.is_writable,
            })
            .collect(),
        data: solana_level_1_token_starter::instruction::MintTokens {
            amount,
        }
        .data(),
    };

    send_success(
        svm,
        instruction,
        payer,
        &[authority],
    );
}

// ---------------------------------------------------------
// WRONG AUTHORITY
// ---------------------------------------------------------

#[test]
fn rejects_burn_with_wrong_authority() {
    let (mut svm, payer) = setup();

    let authority = Keypair::new();
    let wrong_authority = Keypair::new();
    let mint = Keypair::new();

    create_mint(
        &mut svm,
        &payer,
        &authority,
        &mint,
    );

    let token_account = create_token_account(
        &mut svm,
        &payer,
        &authority,
        &mint,
    );

    mint_tokens(
        &mut svm,
        &payer,
        &authority,
        &mint,
        token_account,
        MINT_AMOUNT,
    );

    let accounts =
        solana_level_1_token_starter::accounts::BurnTokens {
            authority: wrong_authority.pubkey(),
            mint: mint.pubkey(),
            token_account,
            token_program: anchor_spl::token_2022::ID,
        };

    let instruction = Instruction {
        program_id: solana_level_1_token_starter::ID,
        accounts: accounts
            .to_account_metas(None)
            .into_iter()
            .map(|meta| AccountMeta {
                pubkey: meta.pubkey,
                is_signer: meta.is_signer,
                is_writable: meta.is_writable,
            })
            .collect(),
        data: solana_level_1_token_starter::instruction::BurnTokens {
            amount: BURN_AMOUNT,
        }
        .data(),
    };

    send_failure(
        &mut svm,
        instruction,
        &payer,
        &[&wrong_authority],
    );
}

// ---------------------------------------------------------
// WRONG MINT
// ---------------------------------------------------------

#[test]
fn rejects_burn_with_wrong_mint() {
    let (mut svm, payer) = setup();

    let authority = Keypair::new();
    let mint_a = Keypair::new();
    let mint_b = Keypair::new();

    create_mint(
        &mut svm,
        &payer,
        &authority,
        &mint_a,
    );

    create_mint(
        &mut svm,
        &payer,
        &authority,
        &mint_b,
    );

    // Token account belongs to mint_a.
    let token_account = create_token_account(
        &mut svm,
        &payer,
        &authority,
        &mint_a,
    );

    mint_tokens(
        &mut svm,
        &payer,
        &authority,
        &mint_a,
        token_account,
        MINT_AMOUNT,
    );

    // burn_tokens receives mint_b while token_account belongs to mint_a.
    let accounts =
        solana_level_1_token_starter::accounts::BurnTokens {
            authority: authority.pubkey(),
            mint: mint_b.pubkey(),
            token_account,
            token_program: anchor_spl::token_2022::ID,
        };

    let instruction = Instruction {
        program_id: solana_level_1_token_starter::ID,
        accounts: accounts
            .to_account_metas(None)
            .into_iter()
            .map(|meta| AccountMeta {
                pubkey: meta.pubkey,
                is_signer: meta.is_signer,
                is_writable: meta.is_writable,
            })
            .collect(),
        data: solana_level_1_token_starter::instruction::BurnTokens {
            amount: BURN_AMOUNT,
        }
        .data(),
    };

    send_failure(
        &mut svm,
        instruction,
        &payer,
        &[&authority],
    );
}

// ---------------------------------------------------------
// INSUFFICIENT BALANCE
// ---------------------------------------------------------

#[test]
fn rejects_burn_with_insufficient_balance() {
    let (mut svm, payer) = setup();

    let authority = Keypair::new();
    let mint = Keypair::new();

    create_mint(
        &mut svm,
        &payer,
        &authority,
        &mint,
    );

    let token_account = create_token_account(
        &mut svm,
        &payer,
        &authority,
        &mint,
    );

    // Account receives only 100_000 tokens.
    let available_amount: u64 = 100_000;

    mint_tokens(
        &mut svm,
        &payer,
        &authority,
        &mint,
        token_account,
        available_amount,
    );

    // Try to burn 250_000 while only 100_000 is available.
    let accounts =
        solana_level_1_token_starter::accounts::BurnTokens {
            authority: authority.pubkey(),
            mint: mint.pubkey(),
            token_account,
            token_program: anchor_spl::token_2022::ID,
        };

    let instruction = Instruction {
        program_id: solana_level_1_token_starter::ID,
        accounts: accounts
            .to_account_metas(None)
            .into_iter()
            .map(|meta| AccountMeta {
                pubkey: meta.pubkey,
                is_signer: meta.is_signer,
                is_writable: meta.is_writable,
            })
            .collect(),
        data: solana_level_1_token_starter::instruction::BurnTokens {
            amount: BURN_AMOUNT,
        }
        .data(),
    };

    send_failure(
        &mut svm,
        instruction,
        &payer,
        &[&authority],
    );
}