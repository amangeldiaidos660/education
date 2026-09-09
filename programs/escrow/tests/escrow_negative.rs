mod common;

use common::*;
use escrow::DealStatus;
use solana_keypair::Keypair;
use solana_signer::Signer;

#[test]
fn rejects_zero_amount_and_rolls_back() {
    let mut svm = new_svm();

    let sender = Keypair::new();
    let receiver = Keypair::new();
    let mint = Keypair::new();

    svm.airdrop(
        &sender.pubkey(),
        2_000_000_000,
    )
    .expect("airdrop must succeed");

    create_mint(
        &mut svm,
        &sender,
        &mint,
    );

    let (
        instruction,
        escrow_state_address,
        vault,
    ) = initialize_ix(
        sender.pubkey(),
        mint.pubkey(),
        receiver.pubkey(),
        DEAL_ID,
        0,
    );

    let error = send_failure(
        &mut svm,
        instruction,
        &sender,
        &[],
    );

    assert!(
        error.contains("Custom(6000)"),
        "expected AmountMustBePositive: {error}"
    );

    assert!(
        svm.get_account(
            &escrow_state_address
        ).is_none(),
        "failed initialize must not leave state"
    );

    assert!(
        svm.get_account(&vault).is_none(),
        "failed initialize must not leave vault"
    );
}

#[test]
fn rejects_duplicate_deal_id_and_keeps_original_state() {
    let mut svm = new_svm();

    let sender = Keypair::new();
    let receiver = Keypair::new();
    let mint = Keypair::new();

    svm.airdrop(
        &sender.pubkey(),
        2_000_000_000,
    )
    .expect("airdrop must succeed");

    create_mint(
        &mut svm,
        &sender,
        &mint,
    );

    let (
        first_instruction,
        escrow_state_address,
        _vault,
    ) = initialize_ix(
        sender.pubkey(),
        mint.pubkey(),
        receiver.pubkey(),
        DEAL_ID,
        DEAL_AMOUNT,
    );

    send_success(
        &mut svm,
        first_instruction,
        &sender,
        &[],
    );

    let before = escrow_state(
        &svm,
        escrow_state_address,
    );

    let (
        duplicate_instruction,
        _,
        _,
    ) = initialize_ix(
        sender.pubkey(),
        mint.pubkey(),
        receiver.pubkey(),
        DEAL_ID,
        DEAL_AMOUNT + 1,
    );

    let _error = send_failure(
        &mut svm,
        duplicate_instruction,
        &sender,
        &[],
    );

    let after = escrow_state(
        &svm,
        escrow_state_address,
    );

    assert_eq!(
        before.sender,
        after.sender
    );

    assert_eq!(
        before.receiver,
        after.receiver
    );

    assert_eq!(
        before.mint,
        after.mint
    );

    assert_eq!(
        before.amount,
        after.amount
    );

    assert_eq!(
        before.deal_id,
        after.deal_id
    );

    assert_eq!(
        before.status,
        after.status
    );
}

#[test]
fn rejects_insufficient_balance_and_keeps_created_state() {
    let mut svm = new_svm();

    let sender = Keypair::new();
    let receiver = Keypair::new();
    let mint = Keypair::new();

    svm.airdrop(
        &sender.pubkey(),
        2_000_000_000,
    )
    .expect("airdrop must succeed");

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
        100,
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
        1_000,
    );

    send_success(
        &mut svm,
        initialize_instruction,
        &sender,
        &[],
    );

    let before_balance =
        token_balance(
            &svm,
            sender_token_account
        );

    let deposit_instruction = deposit_ix(
        sender.pubkey(),
        escrow_state_address,
        mint.pubkey(),
        sender_token_account,
        vault,
    );

    let _error = send_failure(
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
        DealStatus::Created
    );

    assert_eq!(
        token_balance(
            &svm,
            sender_token_account
        ),
        before_balance
    );

    assert_eq!(
        token_balance(&svm, vault),
        0
    );
}

#[test]
fn rejects_substituted_mint_and_keeps_state_unchanged() {
    let mut svm = new_svm();

    let sender = Keypair::new();
    let receiver = Keypair::new();

    let real_mint = Keypair::new();
    let fake_mint = Keypair::new();

    svm.airdrop(
        &sender.pubkey(),
        3_000_000_000,
    )
    .expect("airdrop must succeed");

    create_mint(
        &mut svm,
        &sender,
        &real_mint,
    );

    create_mint(
        &mut svm,
        &sender,
        &fake_mint,
    );

    let fake_sender_token_account =
        create_token_account(
            &mut svm,
            &sender,
            sender.pubkey(),
            fake_mint.pubkey(),
        );

    mint_tokens(
        &mut svm,
        &sender,
        fake_mint.pubkey(),
        fake_sender_token_account,
        MINT_AMOUNT,
    );

    let (
        initialize_instruction,
        escrow_state_address,
        vault,
    ) = initialize_ix(
        sender.pubkey(),
        real_mint.pubkey(),
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

    let before = escrow_state(
        &svm,
        escrow_state_address,
    );

    let deposit_instruction = deposit_ix(
        sender.pubkey(),
        escrow_state_address,
        fake_mint.pubkey(),
        fake_sender_token_account,
        vault,
    );

    let _error = send_failure(
        &mut svm,
        deposit_instruction,
        &sender,
        &[],
    );

    let after = escrow_state(
        &svm,
        escrow_state_address,
    );

    assert_eq!(
        before.status,
        DealStatus::Created
    );

    assert_eq!(
        after.status,
        DealStatus::Created
    );

    assert_eq!(
        before.mint,
        after.mint
    );

    assert_eq!(
        token_balance(&svm, vault),
        0
    );
}

#[test]
fn rejects_wrong_signer_and_keeps_funded_state() {
    let mut svm = new_svm();

    let sender = Keypair::new();
    let attacker = Keypair::new();
    let receiver = Keypair::new();
    let mint = Keypair::new();

    svm.airdrop(
        &sender.pubkey(),
        2_000_000_000,
    )
    .expect("sender airdrop must succeed");

    svm.airdrop(
        &attacker.pubkey(),
        1_000_000_000,
    )
    .expect("attacker airdrop must succeed");

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

    send_success(
        &mut svm,
        deposit_ix(
            sender.pubkey(),
            escrow_state_address,
            mint.pubkey(),
            sender_token_account,
            vault,
        ),
        &sender,
        &[],
    );

    let vault_before =
        token_balance(&svm, vault);

    let fake_sender_token =
        create_token_account(
            &mut svm,
            &attacker,
            attacker.pubkey(),
            mint.pubkey(),
        );

    let bad_cancel = cancel_ix(
        attacker.pubkey(),
        escrow_state_address,
        mint.pubkey(),
        fake_sender_token,
        vault,
    );

    let _error = send_failure(
        &mut svm,
        bad_cancel,
        &attacker,
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
        token_balance(&svm, vault),
        vault_before
    );
}

#[test]
fn rejects_substituted_receiver_and_keeps_funded_state() {
    let mut svm = new_svm();

    let sender = Keypair::new();
    let receiver = Keypair::new();
    let fake_receiver = Keypair::new();
    let mint = Keypair::new();

    svm.airdrop(
        &sender.pubkey(),
        3_000_000_000,
    )
    .expect("sender airdrop must succeed");

    svm.airdrop(
        &receiver.pubkey(),
        1_000_000,
    )
    .expect("receiver airdrop must succeed");

    svm.airdrop(
        &fake_receiver.pubkey(),
        1_000_000,
    )
    .expect("fake receiver airdrop must succeed");

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

    let fake_receiver_token =
        create_token_account(
            &mut svm,
            &sender,
            fake_receiver.pubkey(),
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

    send_success(
        &mut svm,
        deposit_ix(
            sender.pubkey(),
            escrow_state_address,
            mint.pubkey(),
            sender_token_account,
            vault,
        ),
        &sender,
        &[],
    );

    let vault_before =
        token_balance(&svm, vault);

    let bad_release = release_ix(
        sender.pubkey(),
        fake_receiver.pubkey(),
        escrow_state_address,
        mint.pubkey(),
        vault,
        fake_receiver_token,
    );

    let _error = send_failure(
        &mut svm,
        bad_release,
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
        token_balance(&svm, vault),
        vault_before
    );

    assert_eq!(
        token_balance(
            &svm,
            fake_receiver_token
        ),
        0
    );
}

#[test]
fn rejects_repeated_finalization() {
    let mut svm = new_svm();

    let sender = Keypair::new();
    let receiver = Keypair::new();
    let mint = Keypair::new();

    svm.airdrop(
        &sender.pubkey(),
        3_000_000_000,
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

    send_success(
        &mut svm,
        deposit_ix(
            sender.pubkey(),
            escrow_state_address,
            mint.pubkey(),
            sender_token_account,
            vault,
        ),
        &sender,
        &[],
    );

    let first_release = release_ix(
        sender.pubkey(),
        receiver.pubkey(),
        escrow_state_address,
        mint.pubkey(),
        vault,
        receiver_token_account,
    );

    send_success(
        &mut svm,
        first_release,
        &sender,
        &[],
    );

    let receiver_balance_after_first =
        token_balance(
            &svm,
            receiver_token_account
        );

    assert_eq!(
        receiver_balance_after_first,
        DEAL_AMOUNT
    );

    let second_release = release_ix(
        sender.pubkey(),
        receiver.pubkey(),
        escrow_state_address,
        mint.pubkey(),
        vault,
        receiver_token_account,
    );

    let _error = send_failure(
        &mut svm,
        second_release,
        &sender,
        &[],
    );

    assert_eq!(
        token_balance(
            &svm,
            receiver_token_account
        ),
        receiver_balance_after_first
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