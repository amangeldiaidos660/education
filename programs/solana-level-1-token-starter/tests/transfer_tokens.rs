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
const TRANSFER_AMOUNT: u64 = 250_000;

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
fn transfers_tokens_and_keeps_supply_unchanged() {
    let program_id = solana_level_1_token_starter::ID;
    let token_program = anchor_spl::token_2022::ID;

    let mut svm = LiteSVM::new();

    svm.add_program(program_id, &program_bytes())
        .expect("program must load");

    let payer = Keypair::new();
    let authority = Keypair::new();
    let recipient = Keypair::new();
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

    send_transaction(
        &mut svm,
        create_token_instruction,
        &payer,
        &[&authority, &mint],
    );

    // 2. Derive source and destination Token-2022 ATAs.
    let source = get_associated_token_address_with_program_id(
        &authority.pubkey(),
        &mint.pubkey(),
        &token_program,
    );

    let destination = get_associated_token_address_with_program_id(
        &recipient.pubkey(),
        &mint.pubkey(),
        &token_program,
    );

    // 3. Create source token account.
    let source_accounts =
        solana_level_1_token_starter::accounts::CreateTokenAccount {
            payer: payer.pubkey(),
            owner: authority.pubkey(),
            mint: mint.pubkey(),
            token_account: source,
            token_program,
            associated_token_program: anchor_spl::associated_token::ID,
            system_program: anchor_lang::system_program::ID,
        };

    let create_source_instruction = Instruction {
        program_id,
        accounts: source_accounts
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
        create_source_instruction,
        &payer,
        &[],
    );

    // 4. Create destination token account.
    let destination_accounts =
        solana_level_1_token_starter::accounts::CreateTokenAccount {
            payer: payer.pubkey(),
            owner: recipient.pubkey(),
            mint: mint.pubkey(),
            token_account: destination,
            token_program,
            associated_token_program: anchor_spl::associated_token::ID,
            system_program: anchor_lang::system_program::ID,
        };

    let create_destination_instruction = Instruction {
        program_id,
        accounts: destination_accounts
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
        create_destination_instruction,
        &payer,
        &[],
    );

    // 5. Mint tokens into source.
    let mint_tokens_accounts =
        solana_level_1_token_starter::accounts::MintTokens {
            authority: authority.pubkey(),
            mint: mint.pubkey(),
            destination: source,
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

    // 6. Read state before transfer.
    let source_before_account = svm
        .get_account(&source)
        .expect("source must exist");

    let source_before =
        TokenAccount::try_deserialize(&mut source_before_account.data.as_slice())
            .expect("source must deserialize");

    let destination_before_account = svm
        .get_account(&destination)
        .expect("destination must exist");

    let destination_before =
        TokenAccount::try_deserialize(&mut destination_before_account.data.as_slice())
            .expect("destination must deserialize");

    let mint_before_account = svm
        .get_account(&mint.pubkey())
        .expect("mint must exist");

    let mint_before =
        Mint::try_deserialize(&mut mint_before_account.data.as_slice())
            .expect("mint must deserialize");

    assert_eq!(source_before.amount, MINT_AMOUNT);
    assert_eq!(destination_before.amount, 0);
    assert_eq!(mint_before.supply, MINT_AMOUNT);

    // 7. Transfer tokens from source to destination.
    let transfer_accounts =
        solana_level_1_token_starter::accounts::TransferTokens {
            authority: authority.pubkey(),
            mint: mint.pubkey(),
            source,
            destination,
            token_program,
        };

    let transfer_instruction = Instruction {
        program_id,
        accounts: transfer_accounts
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

    send_transaction(
        &mut svm,
        transfer_instruction,
        &payer,
        &[&authority],
    );

    // 8. Read state after transfer.
    let source_after_account = svm
        .get_account(&source)
        .expect("source must exist");

    let source_after =
        TokenAccount::try_deserialize(&mut source_after_account.data.as_slice())
            .expect("source must deserialize");

    let destination_after_account = svm
        .get_account(&destination)
        .expect("destination must exist");

    let destination_after =
        TokenAccount::try_deserialize(&mut destination_after_account.data.as_slice())
            .expect("destination must deserialize");

    let mint_after_account = svm
        .get_account(&mint.pubkey())
        .expect("mint must exist");

    let mint_after =
        Mint::try_deserialize(&mut mint_after_account.data.as_slice())
            .expect("mint must deserialize");

    // Source loses exactly TRANSFER_AMOUNT.
    assert_eq!(
        source_after.amount,
        source_before.amount - TRANSFER_AMOUNT
    );

    // Destination receives exactly TRANSFER_AMOUNT.
    assert_eq!(
        destination_after.amount,
        destination_before.amount + TRANSFER_AMOUNT
    );

    // A transfer must not create or destroy tokens.
    assert_eq!(mint_after.supply, mint_before.supply);
    assert_eq!(mint_after.supply, MINT_AMOUNT);
}