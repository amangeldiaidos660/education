use anchor_lang::{AccountDeserialize, InstructionData, ToAccountMetas};
use anchor_lang::prelude::Pubkey;
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
const MINT_AMOUNT: u64 = 100_000;
const BURN_AMOUNT: u64 = 250_000;

fn program_bytes() -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/deploy/solana_level_1_token_starter.so");

    fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "Build the program before running tests. Could not read {}: {error}",
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

fn create_mint(
    svm: &mut LiteSVM,
    payer: &Keypair,
    authority: &Keypair,
    mint: &Keypair,
) {
    let accounts =
        solana_level_1_token_starter::accounts::CreateToken {
            payer: payer.pubkey(),
            authority: authority.pubkey(),
            mint: mint.pubkey(),
            token_program: anchor_spl::token_2022::ID,
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
    let token_account = get_associated_token_address_with_program_id(
        &owner.pubkey(),
        &mint.pubkey(),
        &anchor_spl::token_2022::ID,
    );

    let accounts =
        solana_level_1_token_starter::accounts::CreateTokenAccount {
            payer: payer.pubkey(),
            owner: owner.pubkey(),
            mint: mint.pubkey(),
            token_account,
            token_program: anchor_spl::token_2022::ID,
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

fn read_state(
    svm: &LiteSVM,
    token_account: Pubkey,
    mint: Pubkey,
) -> (u64, u64) {
    let token_account_data = svm
        .get_account(&token_account)
        .expect("token account must exist");

    let token_state =
        TokenAccount::try_deserialize(
            &mut token_account_data.data.as_slice(),
        )
        .expect("token account must deserialize");

    let mint_account = svm
        .get_account(&mint)
        .expect("mint must exist");

    let mint_state =
        Mint::try_deserialize(
            &mut mint_account.data.as_slice(),
        )
        .expect("mint must deserialize");

    (token_state.amount, mint_state.supply)
}

#[test]
fn failed_burn_keeps_account_state_unchanged() {
    let mut svm = LiteSVM::new();

    svm.add_program(
        solana_level_1_token_starter::ID,
        &program_bytes(),
    )
    .expect("program must load");

    let payer = Keypair::new();
    let authority = Keypair::new();
    let mint = Keypair::new();

    svm.airdrop(&payer.pubkey(), 1_000_000_000)
        .expect("airdrop must succeed");

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

    let state_before = read_state(
        &svm,
        token_account,
        mint.pubkey(),
    );

    assert_eq!(state_before.0, MINT_AMOUNT);
    assert_eq!(state_before.1, MINT_AMOUNT);

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

    let blockhash = svm.latest_blockhash();
    let message = Message::new(
        &[instruction],
        Some(&payer.pubkey()),
    );

    let transaction = Transaction::new(
        &[&payer, &authority],
        message,
        blockhash,
    );

    assert!(
        svm.send_transaction(transaction).is_err(),
        "burn must fail because balance is insufficient"
    );

    let state_after = read_state(
        &svm,
        token_account,
        mint.pubkey(),
    );

    assert_eq!(
        state_after.0,
        state_before.0,
        "token-account balance changed after failed burn"
    );

    assert_eq!(
        state_after.1,
        state_before.1,
        "mint supply changed after failed burn"
    );
}