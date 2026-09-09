use anchor_lang::{
    prelude::Pubkey,
    AccountDeserialize,
    InstructionData,
    ToAccountMetas,
};
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

pub const DECIMALS: u8 = 6;
pub const MINT_AMOUNT: u64 = 1_000_000;
pub const DEAL_AMOUNT: u64 = 250_000;
pub const DEAL_ID: u64 = 42;

pub fn program_bytes(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../target/deploy/{name}.so"));

    fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "Build programs with `anchor build --ignore-keys` first. Could not read {}: {error}",
            path.display()
        )
    })
}

pub fn new_svm() -> LiteSVM {
    let mut svm = LiteSVM::new();

    svm.add_program(escrow::ID, &program_bytes("escrow"))
        .expect("escrow program must load");

    svm.add_program(
        solana_level_1_token_starter::ID,
        &program_bytes("solana_level_1_token_starter"),
    )
    .expect("token starter program must load");

    svm
}

pub fn to_metas<T: ToAccountMetas>(accounts: T) -> Vec<AccountMeta> {
    accounts
        .to_account_metas(None)
        .into_iter()
        .map(|meta| AccountMeta {
            pubkey: meta.pubkey,
            is_signer: meta.is_signer,
            is_writable: meta.is_writable,
        })
        .collect()
}

pub fn send_success(
    svm: &mut LiteSVM,
    instruction: Instruction,
    payer: &Keypair,
    signers: &[&Keypair],
) {
    let blockhash = svm.latest_blockhash();
    let message = Message::new(&[instruction], Some(&payer.pubkey()));

    let mut all_signers = vec![payer];
    all_signers.extend_from_slice(signers);

    let transaction = Transaction::new(
        &all_signers,
        message,
        blockhash,
    );

    svm.send_transaction(transaction)
        .expect("transaction must succeed");
}

pub fn send_failure(
    svm: &mut LiteSVM,
    instruction: Instruction,
    payer: &Keypair,
    signers: &[&Keypair],
) -> String {
    let blockhash = svm.latest_blockhash();
    let message = Message::new(&[instruction], Some(&payer.pubkey()));

    let mut all_signers = vec![payer];
    all_signers.extend_from_slice(signers);

    let transaction = Transaction::new(
        &all_signers,
        message,
        blockhash,
    );

    let error = svm
        .send_transaction(transaction)
        .expect_err("transaction must fail");

    format!("{error:?}")
}

pub fn create_mint(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Keypair,
) {
    let token_program = anchor_spl::token_2022::ID;

    let accounts =
        solana_level_1_token_starter::accounts::CreateToken {
            payer: payer.pubkey(),
            authority: payer.pubkey(),
            mint: mint.pubkey(),
            token_program,
            system_program: anchor_lang::system_program::ID,
        };

    let instruction = Instruction {
        program_id: solana_level_1_token_starter::ID,
        accounts: to_metas(accounts),
        data: solana_level_1_token_starter::instruction::CreateToken {
            decimals: DECIMALS,
        }
        .data(),
    };

    send_success(
        svm,
        instruction,
        payer,
        &[mint],
    );
}

pub fn create_token_account(
    svm: &mut LiteSVM,
    payer: &Keypair,
    owner: Pubkey,
    mint: Pubkey,
) -> Pubkey {
    let token_program = anchor_spl::token_2022::ID;

    let token_account =
        get_associated_token_address_with_program_id(
            &owner,
            &mint,
            &token_program,
        );

    let accounts =
        solana_level_1_token_starter::accounts::CreateTokenAccount {
            payer: payer.pubkey(),
            owner,
            mint,
            token_account,
            token_program,
            associated_token_program:
                anchor_spl::associated_token::ID,
            system_program: anchor_lang::system_program::ID,
        };

    let instruction = Instruction {
        program_id: solana_level_1_token_starter::ID,
        accounts: to_metas(accounts),
        data:
            solana_level_1_token_starter::instruction::CreateTokenAccount {}
                .data(),
    };

    send_success(
        svm,
        instruction,
        payer,
        &[],
    );

    token_account
}

pub fn mint_tokens(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: Pubkey,
    destination: Pubkey,
    amount: u64,
) {
    let token_program = anchor_spl::token_2022::ID;

    let accounts =
        solana_level_1_token_starter::accounts::MintTokens {
            authority: payer.pubkey(),
            mint,
            destination,
            token_program,
        };

    let instruction = Instruction {
        program_id: solana_level_1_token_starter::ID,
        accounts: to_metas(accounts),
        data:
            solana_level_1_token_starter::instruction::MintTokens {
                amount,
            }
            .data(),
    };

    send_success(
        svm,
        instruction,
        payer,
        &[],
    );
}

pub fn escrow_pda(
    sender: Pubkey,
    deal_id: u64,
) -> Pubkey {
    Pubkey::find_program_address(
        &[
            b"escrow",
            sender.as_ref(),
            &deal_id.to_le_bytes(),
        ],
        &escrow::ID,
    )
    .0
}

pub fn vault_pda(
    escrow_state: Pubkey,
) -> Pubkey {
    Pubkey::find_program_address(
        &[
            b"vault",
            escrow_state.as_ref(),
        ],
        &escrow::ID,
    )
    .0
}

pub fn initialize_ix(
    sender: Pubkey,
    mint: Pubkey,
    receiver: Pubkey,
    deal_id: u64,
    amount: u64,
) -> (Instruction, Pubkey, Pubkey) {
    let token_program = anchor_spl::token_2022::ID;

    let escrow_state = escrow_pda(sender, deal_id);
    let vault = vault_pda(escrow_state);

    let accounts = escrow::accounts::Initialize {
        sender,
        escrow_state,
        mint,
        vault,
        token_program,
        system_program: anchor_lang::system_program::ID,
    };

    let instruction = Instruction {
        program_id: escrow::ID,
        accounts: to_metas(accounts),
        data: escrow::instruction::Initialize {
            deal_id,
            receiver,
            amount,
        }
        .data(),
    };

    (instruction, escrow_state, vault)
}

pub fn deposit_ix(
    sender: Pubkey,
    escrow_state: Pubkey,
    mint: Pubkey,
    sender_token_account: Pubkey,
    vault: Pubkey,
) -> Instruction {
    let accounts = escrow::accounts::Deposit {
        sender,
        escrow_state,
        mint,
        sender_token_account,
        vault,
        token_program: anchor_spl::token_2022::ID,
    };

    Instruction {
        program_id: escrow::ID,
        accounts: to_metas(accounts),
        data: escrow::instruction::Deposit {}.data(),
    }
}

pub fn release_ix(
    sender: Pubkey,
    receiver: Pubkey,
    escrow_state: Pubkey,
    mint: Pubkey,
    vault: Pubkey,
    receiver_token_account: Pubkey,
) -> Instruction {
    let accounts = escrow::accounts::Release {
        sender,
        escrow_state,
        receiver,
        mint,
        vault,
        receiver_token_account,
        token_program: anchor_spl::token_2022::ID,
    };

    Instruction {
        program_id: escrow::ID,
        accounts: to_metas(accounts),
        data: escrow::instruction::Release {}.data(),
    }
}

pub fn cancel_ix(
    sender: Pubkey,
    escrow_state: Pubkey,
    mint: Pubkey,
    sender_token_account: Pubkey,
    vault: Pubkey,
) -> Instruction {
    let accounts = escrow::accounts::Cancel {
        sender,
        escrow_state,
        mint,
        sender_token_account,
        vault,
        token_program: anchor_spl::token_2022::ID,
    };

    Instruction {
        program_id: escrow::ID,
        accounts: to_metas(accounts),
        data: escrow::instruction::Cancel {}.data(),
    }
}

pub fn token_balance(
    svm: &LiteSVM,
    token_account: Pubkey,
) -> u64 {
    let account = svm
        .get_account(&token_account)
        .expect("token account must exist");

    let token_account =
        TokenAccount::try_deserialize(
            &mut account.data.as_slice(),
        )
        .expect("token account must deserialize");

    token_account.amount
}

pub fn escrow_state(
    svm: &LiteSVM,
    address: Pubkey,
) -> escrow::EscrowState {
    let account = svm
        .get_account(&address)
        .expect("escrow state must exist");

    escrow::EscrowState::try_deserialize(
        &mut account.data.as_slice(),
    )
    .expect("escrow state must deserialize")
}