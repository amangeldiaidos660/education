# Solana Level 1 — Escrow

Minimal DeFi escrow program for Token-2022 built with Anchor.

The program allows a sender to create a deal, deposit Token-2022 tokens into a per-deal vault, and later either release the tokens to the receiver or cancel the deal and refund the sender.

## Stack

- Anchor 1.1.2
- Solana CLI 3.1.10
- Rust 1.89.0
- LiteSVM 0.10.0
- Token-2022
- `anchor_spl::token_interface`

No new TypeScript code is required for this implementation. If TypeScript is added later, it must use `@solana/kit` and not legacy `@solana/web3.js`.

## Architecture

Each escrow deal has its own state PDA.

Escrow state PDA seeds:

```text
[b"escrow", sender, deal_id]
```

Each deal also has its own Token-2022 vault PDA.

Vault seeds:

```text
[b"vault", escrow_state]
```

The escrow state PDA is the authority of its vault.

This isolates every deal and prevents different deals from sharing the same vault.

## Escrow State

`EscrowState` stores:

- `sender`
- `receiver`
- `mint`
- `amount`
- `deal_id`
- `bump`
- `status`

Supported statuses:

```text
Created
Funded
Released
Cancelled
```

## State Machine

Normal release flow:

```text
initialize
    |
    v
Created
    |
  deposit
    |
    v
Funded
    |
 release
    |
    v
Released
```

Cancellation is allowed for unfinished deals:

```text
Created ---- cancel ----> Cancelled

Funded  ---- cancel ----> Cancelled
```

After `release` or `cancel`, the vault and escrow state accounts are closed and their rent is returned to the sender.

Because the state account is closed during finalization, the final `Released` or `Cancelled` status exists only during the successful final transaction before account closure.

## Instructions

### initialize

Creates:

- escrow state PDA
- per-deal Token-2022 vault

Validates:

- sender is a signer
- amount is greater than zero
- sender and receiver are different
- escrow PDA is derived from sender and `deal_id`
- mint belongs to the provided Token-2022 program
- token program is explicitly constrained to Token-2022

Initial state:

```text
Created
```

### deposit

Transfers exactly the deal amount from the sender token account into the vault using:

```text
token_interface::transfer_checked
```

Validates:

- sender signer and authority
- escrow PDA seeds and bump
- stored sender
- stored mint
- sender token account mint and authority
- vault PDA
- vault mint and authority
- Token-2022 program
- current state is `Created`
- amount stored in the deal is non-zero

Transition:

```text
Created -> Funded
```

### release

Only the sender can release a funded deal.

Tokens are transferred from the vault to the receiver's Token-2022 associated token account using `transfer_checked`.

Validates:

- sender authorization
- escrow PDA seeds and bump
- stored mint
- stored receiver
- receiver Token-2022 ATA
- vault PDA, mint and authority
- Token-2022 program
- current state is `Funded`

The complete vault balance is transferred before closure. This prevents unsolicited Token-2022 transfers into the vault from permanently blocking finalization.

Transition:

```text
Funded -> Released
```

The empty vault and escrow state are then closed and rent is returned to the sender.

### cancel

Only the sender can cancel an unfinished deal.

Allowed states:

```text
Created
Funded
```

If the deal is already funded, the complete vault balance is returned to the sender using `transfer_checked`.

Transferring the complete vault balance ensures that unsolicited deposits cannot prevent the vault from being closed.

Transitions:

```text
Created -> Cancelled
Funded  -> Cancelled
```

The empty vault and escrow state are closed and rent is returned to the sender.

## Token Safety

All token transfers use:

```text
anchor_spl::token_interface
token_interface::transfer_checked
```

Mint decimals are read from the mint account and passed to `transfer_checked`.

The token program account is explicitly constrained to the Token-2022 program.

Token accounts are constrained by their expected:

- mint
- authority
- token program

The vault is controlled by the unique escrow state PDA.

## Threat Model

The program protects against:

- zero-value deals
- sender equal to receiver
- unauthorized signer
- substituted mint
- substituted receiver
- non-ATA receiver token account
- incorrect token-account authority
- incorrect token program
- incorrect PDA seeds or bump
- duplicate active `deal_id`
- insufficient sender balance
- repeated deposit
- release before funding
- repeated finalization
- shared vaults between deals
- unsolicited tokens permanently locking a vault

Failed Solana transactions are atomic, so rejected operations do not leave partial escrow or token-account state changes.

## Tests

LiteSVM integration tests cover two positive end-to-end flows:

- funded deal release
- funded deal cancellation

An additional positive test verifies cancellation before deposit.

Negative tests cover:

- zero amount
- duplicate active `deal_id`
- insufficient balance
- substituted mint
- wrong signer
- substituted receiver
- repeated finalization

Tests also verify that state remains unchanged after rejected transactions.

Current escrow test result:

```text
11 passed
0 failed
```

## Build

From the repository root:

```bash
$env:CARGO_BUILD_JOBS="1"
anchor build --ignore-keys
```

## Run Escrow Tests

```bash
cargo test -p escrow --tests -- --nocapture
```

## Run Full Workspace Tests

```bash
cargo test --workspace --locked -j 1
```

## Submission

Repository:

```text
https://github.com/amangeldiaidos660/education
```

Branch:

```text
task/03-escrow
```

Public branch URL:

```text
https://github.com/amangeldiaidos660/education/tree/task/03-escrow
```

After the final commit, include the exact commit SHA in the submission.

Example:

```text
Repository: https://github.com/amangeldiaidos660/education
Branch: task/03-escrow
Commit SHA: <exact final commit SHA>
```

## Security

No private keys, wallet secrets, seed phrases, API keys, or other credentials must be committed to the repository.

Critical token and escrow accounts are validated on-chain rather than relying only on client-side checks.
