mod common;

use common::*;
use escrow::DealStatus;
use solana_keypair::Keypair;
use solana_signer::Signer;

#[test]
fn funded_deal_cancel_returns_tokens_and_closes_accounts() {
    let mut svm = new_svm();

    let sender = Keypair::new();
    let receiver = Keypair::new();
    let mint = Keypair::new();

    svm.airdrop(
        &sender.pubkey(),
        2_000_000_000,
    )
    .expect("sender airdrop must succeed");

    create_mint(
        &mut svm,
        &sender,
        &mint,
    );

    let sender_token_account =
        create_token_account(
            &mut svm,
            &sender,
            sender.pubkey(),
            mint.pubkey(),
        );

    mint_tokens(
        &mut svm,
        &sender,
        mint.pubkey(),
        sender_token_account,
        MINT_AMOUNT,
    );

    let (
        initialize_instruction,
        escrow_state_address,
        vault,
    ) = initialize_ix(
        sender.pubkey(),
        mint.pubkey(),
        receiver.pubkey(),
        DEAL_ID,
        DEAL_AMOUNT,
    );

    send_success(
        &mut svm,
        initialize_instruction,
        &sender,
        &[],
    );

    assert_eq!(
        escrow_state(
            &svm,
            escrow_state_address
        ).status,
        DealStatus::Created
    );

    let deposit_instruction = deposit_ix(
        sender.pubkey(),
        escrow_state_address,
        mint.pubkey(),
        sender_token_account,
        vault,
    );

    send_success(
        &mut svm,
        deposit_instruction,
        &sender,
        &[],
    );

    assert_eq!(
        escrow_state(
            &svm,
            escrow_state_address
        ).status,
        DealStatus::Funded
    );

    assert_eq!(
        token_balance(
            &svm,
            sender_token_account
        ),
        MINT_AMOUNT - DEAL_AMOUNT
    );

    assert_eq!(
        token_balance(&svm, vault),
        DEAL_AMOUNT
    );

    let cancel_instruction = cancel_ix(
        sender.pubkey(),
        escrow_state_address,
        mint.pubkey(),
        sender_token_account,
        vault,
    );

    send_success(
        &mut svm,
        cancel_instruction,
        &sender,
        &[],
    );

    assert_eq!(
        token_balance(
            &svm,
            sender_token_account
        ),
        MINT_AMOUNT
    );

    assert!(
        svm.get_account(&vault).is_none(),
        "vault must be closed after cancel"
    );

    assert!(
        svm.get_account(
            &escrow_state_address
        ).is_none(),
        "escrow state must be closed after cancel"
    );
}

#[test]
fn created_deal_can_be_cancelled_without_deposit() {
    let mut svm = new_svm();

    let sender = Keypair::new();
    let receiver = Keypair::new();
    let mint = Keypair::new();

    svm.airdrop(
        &sender.pubkey(),
        2_000_000_000,
    )
    .expect("sender airdrop must succeed");

    create_mint(
        &mut svm,
        &sender,
        &mint,
    );

    let sender_token_account =
        create_token_account(
            &mut svm,
            &sender,
            sender.pubkey(),
            mint.pubkey(),
        );

    let (
        initialize_instruction,
        escrow_state_address,
        vault,
    ) = initialize_ix(
        sender.pubkey(),
        mint.pubkey(),
        receiver.pubkey(),
        DEAL_ID,
        DEAL_AMOUNT,
    );

    send_success(
        &mut svm,
        initialize_instruction,
        &sender,
        &[],
    );

    assert_eq!(
        token_balance(&svm, vault),
        0
    );

    let cancel_instruction = cancel_ix(
        sender.pubkey(),
        escrow_state_address,
        mint.pubkey(),
        sender_token_account,
        vault,
    );

    send_success(
        &mut svm,
        cancel_instruction,
        &sender,
        &[],
    );

    assert!(
        svm.get_account(&vault).is_none()
    );

    assert!(
        svm.get_account(
            &escrow_state_address
        ).is_none()
    );
}