use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_spl::associated_token::get_associated_token_address_with_program_id;
use litesvm::LiteSVM;
use solana_keypair::Keypair;
use solana_message::{AccountMeta, Instruction, Message};
use solana_signer::Signer;
use solana_transaction::Transaction;
use std::{fs, path::PathBuf};

use anchor_lang::prelude::Pubkey;

const DECIMALS: u8 = 6;
const MINT_AMOUNT: u64 = 1_000_000;
const TRANSFER_AMOUNT: u64 = 100_000;

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

fn create_mint(
    svm: &mut LiteSVM,
    payer: &Keypair,
    authority: &Keypair,
    mint: &Keypair,
) {
    let program_id = solana_level_1_token_starter::ID;
    let token_program = anchor_spl::token_2022::ID;

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
    let program_id = solana_level_1_token_starter::ID;
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
    let program_id = solana_level_1_token_starter::ID;
    let token_program = anchor_spl::token_2022::ID;

    let accounts =
        solana_level_1_token_starter::accounts::MintTokens {
            authority: authority.pubkey(),
            mint: mint.pubkey(),
            destination,
            token_program,
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

// ---------------------------------------------------------
// 1. ZERO AMOUNT
// ---------------------------------------------------------

#[test]
fn rejects_zero_amount() {
    let (mut svm, payer) = setup();

    let authority = Keypair::new();
    let mint = Keypair::new();

    create_mint(
        &mut svm,
        &payer,
        &authority,
        &mint,
    );

    let destination = create_token_account(
        &mut svm,
        &payer,
        &authority,
        &mint,
    );

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
            amount: 0,
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
// 2. WRONG AUTHORITY
// ---------------------------------------------------------

#[test]
fn rejects_wrong_authority() {
    let (mut svm, payer) = setup();

    let authority = Keypair::new();
    let wrong_authority = Keypair::new();
    let recipient = Keypair::new();
    let mint = Keypair::new();

    create_mint(
        &mut svm,
        &payer,
        &authority,
        &mint,
    );

    let source = create_token_account(
        &mut svm,
        &payer,
        &authority,
        &mint,
    );

    let destination = create_token_account(
        &mut svm,
        &payer,
        &recipient,
        &mint,
    );

    mint_tokens(
        &mut svm,
        &payer,
        &authority,
        &mint,
        source,
        MINT_AMOUNT,
    );

    let accounts =
        solana_level_1_token_starter::accounts::TransferTokens {
            authority: wrong_authority.pubkey(),
            mint: mint.pubkey(),
            source,
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
        data: solana_level_1_token_starter::instruction::TransferTokens {
            amount: TRANSFER_AMOUNT,
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
// 3. WRONG MINT
// ---------------------------------------------------------

#[test]
fn rejects_wrong_mint() {
    let (mut svm, payer) = setup();

    let authority = Keypair::new();
    let recipient = Keypair::new();

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

    let source = create_token_account(
        &mut svm,
        &payer,
        &authority,
        &mint_a,
    );

    let wrong_destination = create_token_account(
        &mut svm,
        &payer,
        &recipient,
        &mint_b,
    );

    mint_tokens(
        &mut svm,
        &payer,
        &authority,
        &mint_a,
        source,
        MINT_AMOUNT,
    );

    // mint says mint_a, but destination belongs to mint_b.
    let accounts =
        solana_level_1_token_starter::accounts::TransferTokens {
            authority: authority.pubkey(),
            mint: mint_a.pubkey(),
            source,
            destination: wrong_destination,
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
        data: solana_level_1_token_starter::instruction::TransferTokens {
            amount: TRANSFER_AMOUNT,
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
// 4. SOURCE == DESTINATION
// ---------------------------------------------------------

#[test]
fn rejects_identical_source_and_destination() {
    let (mut svm, payer) = setup();

    let authority = Keypair::new();
    let mint = Keypair::new();

    create_mint(
        &mut svm,
        &payer,
        &authority,
        &mint,
    );

    let source = create_token_account(
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
        source,
        MINT_AMOUNT,
    );

    let accounts =
        solana_level_1_token_starter::accounts::TransferTokens {
            authority: authority.pubkey(),
            mint: mint.pubkey(),
            source,
            destination: source,
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
        data: solana_level_1_token_starter::instruction::TransferTokens {
            amount: TRANSFER_AMOUNT,
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