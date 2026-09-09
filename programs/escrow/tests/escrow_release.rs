mod common;

use common::*;
use escrow::DealStatus;
use solana_keypair::Keypair;
use solana_signer::Signer;

#[test]
fn release_end_to_end() {
    let mut svm = new_svm();

    let sender = Keypair::new();
    let receiver = Keypair::new();
    let mint = Keypair::new();

    svm.airdrop(
        &sender.pubkey(),
        2_000_000_000,
    )
    .expect("sender airdrop must succeed");

    svm.airdrop(
        &receiver.pubkey(),
        1_000_000,
    )
    .expect("receiver airdrop must succeed");

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

    let receiver_token_account =
        create_token_account(
            &mut svm,
            &sender,
            receiver.pubkey(),
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

    let state = escrow_state(
        &svm,
        escrow_state_address,
    );

    assert_eq!(
        state.status,
        DealStatus::Created
    );

    assert_eq!(
        state.sender,
        sender.pubkey()
    );

    assert_eq!(
        state.receiver,
        receiver.pubkey()
    );

    assert_eq!(
        state.mint,
        mint.pubkey()
    );

    assert_eq!(
        state.amount,
        DEAL_AMOUNT
    );

    assert_eq!(
        state.deal_id,
        DEAL_ID
    );

    assert_eq!(
        token_balance(&svm, vault),
        0
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

    let funded_state = escrow_state(
        &svm,
        escrow_state_address,
    );

    assert_eq!(
        funded_state.status,
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

    assert_eq!(
        token_balance(
            &svm,
            receiver_token_account
        ),
        0
    );

    let release_instruction = release_ix(
        sender.pubkey(),
        receiver.pubkey(),
        escrow_state_address,
        mint.pubkey(),
        vault,
        receiver_token_account,
    );

    send_success(
        &mut svm,
        release_instruction,
        &sender,
        &[],
    );

    assert_eq!(
        token_balance(
            &svm,
            receiver_token_account
        ),
        DEAL_AMOUNT
    );

    assert_eq!(
        token_balance(
            &svm,
            sender_token_account
        ),
        MINT_AMOUNT - DEAL_AMOUNT
    );

    assert!(
        svm.get_account(&vault).is_none(),
        "vault must be closed after release"
    );

    assert!(
        svm.get_account(
            &escrow_state_address
        ).is_none(),
        "escrow state must be closed after release"
    );
}