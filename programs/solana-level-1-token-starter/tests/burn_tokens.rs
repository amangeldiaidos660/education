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

fn send_transaction(
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

#[test]
fn burns_tokens_and_decreases_supply() {
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

    // 1. Create mint.
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

    send_transaction(
        &mut svm,
        create_token_instruction,
        &payer,
        &[&authority, &mint],
    );

    // 2. Create authority token account.
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

    send_transaction(
        &mut svm,
        create_token_account_instruction,
        &payer,
        &[],
    );

    // 3. Mint tokens first.
    let mint_tokens_accounts =
        solana_level_1_token_starter::accounts::MintTokens {
            authority: authority.pubkey(),
            mint: mint.pubkey(),
            destination: token_account,
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

    send_transaction(
        &mut svm,
        mint_tokens_instruction,
        &payer,
        &[&authority],
    );

    // 4. Read state before burn.
    let token_account_before_account = svm
        .get_account(&token_account)
        .expect("token account must exist");

    let token_account_before =
        TokenAccount::try_deserialize(
            &mut token_account_before_account.data.as_slice(),
        )
        .expect("token account must deserialize");

    let mint_before_account = svm
        .get_account(&mint.pubkey())
        .expect("mint must exist");

    let mint_before =
        Mint::try_deserialize(&mut mint_before_account.data.as_slice())
            .expect("mint must deserialize");

    assert_eq!(token_account_before.amount, MINT_AMOUNT);
    assert_eq!(mint_before.supply, MINT_AMOUNT);

    // 5. Burn tokens.
    let burn_accounts =
        solana_level_1_token_starter::accounts::BurnTokens {
            authority: authority.pubkey(),
            mint: mint.pubkey(),
            token_account,
            token_program,
        };

    let burn_instruction = Instruction {
        program_id,
        accounts: burn_accounts
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

    send_transaction(
        &mut svm,
        burn_instruction,
        &payer,
        &[&authority],
    );

    // 6. Read state after burn.
    let token_account_after_account = svm
        .get_account(&token_account)
        .expect("token account must exist");

    let token_account_after =
        TokenAccount::try_deserialize(
            &mut token_account_after_account.data.as_slice(),
        )
        .expect("token account must deserialize");

    let mint_after_account = svm
        .get_account(&mint.pubkey())
        .expect("mint must exist");

    let mint_after =
        Mint::try_deserialize(&mut mint_after_account.data.as_slice())
            .expect("mint must deserialize");

    assert_eq!(
        token_account_after.amount,
        token_account_before.amount - BURN_AMOUNT
    );

    assert_eq!(
        mint_after.supply,
        mint_before.supply - BURN_AMOUNT
    );

    assert_eq!(
        token_account_before.amount - token_account_after.amount,
        mint_before.supply - mint_after.supply
    );
}

#[test]
fn rejects_zero_burn_amount_with_expected_error() {
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

    // Create mint.
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

    send_transaction(
        &mut svm,
        create_token_instruction,
        &payer,
        &[&authority, &mint],
    );

    // Create token account.
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

    send_transaction(
        &mut svm,
        create_token_account_instruction,
        &payer,
        &[],
    );

    // Try to burn zero tokens.
    let burn_accounts =
        solana_level_1_token_starter::accounts::BurnTokens {
            authority: authority.pubkey(),
            mint: mint.pubkey(),
            token_account,
            token_program,
        };

    let burn_instruction = Instruction {
        program_id,
        accounts: burn_accounts
            .to_account_metas(None)
            .into_iter()
            .map(|meta| AccountMeta {
                pubkey: meta.pubkey,
                is_signer: meta.is_signer,
                is_writable: meta.is_writable,
            })
            .collect(),
        data: solana_level_1_token_starter::instruction::BurnTokens {
            amount: 0,
        }
        .data(),
    };

    let blockhash = svm.latest_blockhash();

    let message = Message::new(
        &[burn_instruction],
        Some(&payer.pubkey()),
    );

    let transaction = Transaction::new(
        &[&payer, &authority],
        message,
        blockhash,
    );

    let error = svm
        .send_transaction(transaction)
        .expect_err("burning zero tokens must fail");

    let error_text = format!("{error:?}");

    assert!(
        error_text.contains("Custom(6000)"),
        "expected AmountMustBePositive error (6000), got: {error_text}"
    );
}