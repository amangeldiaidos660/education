# Solana Level 1 — Burn Tokens

Второе задание первого уровня курса Superteam KZ.

В рамках задания в токен-программу на Anchor добавлена инструкция `burn_tokens` для сжигания Token-2022 токенов и интеграционные тесты с использованием Rust и LiteSVM.

## Submission

Repository: `amangeldiaidos660/education`

Branch: `task/02-burn`

Submission link:

`https://github.com/amangeldiaidos660/education/tree/task/02-burn`

## Stack

* Anchor CLI / crates: `1.1.2`

* Solana CLI: `3.1.10`

* Rust: `1.89.0`

* LiteSVM: `0.10.0`

* Token standard: Token-2022

* Token interface: `anchor_spl::token_interface`

## Implementation

Added instruction:

`burn_tokens`

The instruction burns tokens from a Token-2022 token account using:

`anchor_spl::token_interface::burn_checked`

The mint decimals are read directly from the validated mint account and passed to `burn_checked`.

The instruction rejects a burn when `amount == 0`.

## Account Constraints

`authority`

* Must be a transaction signer.
* Must be the authority of the token account.

`mint`

* Must be a valid mint account.
* Must belong to the provided token program.
* Mint decimals are used by `burn_checked`.

`token_account`

* Must be a valid token account.
* Must belong to the provided mint.
* Must be controlled by `authority`.
* Must belong to the provided token program.

`token_program`

* Validated through `Interface<TokenInterface>`.

Critical accounts are not accepted as unvalidated `UncheckedAccount`.

## Tests

Tests for the burn instruction are located in:

```text
programs/solana-level-1-token-starter/tests/
```

Added test files:

```text
burn_tokens.rs
burn_negative_cases.rs
burn_failed_state.rs
```

The tests verify:

* Successful burn decreases the token-account balance by the burned amount.
* Successful burn decreases the mint total supply by the same amount.
* `amount == 0` is rejected with `AmountMustBePositive`.
* Burn with the wrong authority is rejected.
* Burn with the wrong mint is rejected.
* Burn with insufficient balance is rejected.
* Token-account balance and mint supply remain unchanged after a failed burn transaction.

## Build

From the repository root:

```bash
anchor build --ignore-keys
```

`--ignore-keys` is used because the program keypair is intentionally not stored in the repository.

A successful build completes without errors and produces the program artifact used by LiteSVM tests.

## Run tests

After building:

```bash
cargo test --workspace --locked
```

Burn-specific tests can also be run separately:

```bash
cargo test --test burn_tokens --locked
cargo test --test burn_negative_cases --locked
cargo test --test burn_failed_state --locked
```

Expected burn test results:

```text
burn_tokens:
2 passed

burn_negative_cases:
3 passed

burn_failed_state:
1 passed

Failed: 0
```

## Reproducibility

A clean checkout of branch `task/02-burn` should pass:

```bash
anchor build --ignore-keys
cargo test --workspace --locked
```

No program keypair, seed phrase, private key, or other secret is required to run the test suite.
