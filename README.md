# Solana Level 1 — Token Program Tests

Итоговое задание первого уровня курса Superteam KZ.

В рамках задания токен-программа на Anchor покрыта позитивными и негативными интеграционными тестами с использованием Rust и LiteSVM.

## Submission

Repository: `amangeldiaidos660/education`

Branch: `task/01-tests`

Submission link:

`https://github.com/amangeldiaidos660/education/tree/task/01-tests`

## Stack

* Anchor CLI / crates: `1.1.2`
* Solana CLI: `3.1.10`
* Rust: `1.89.0`
* LiteSVM: `0.10.0`
* Token standard: Token-2022
* Token interface: `anchor_spl::token_interface`

Legacy `@solana/web3.js` is not used in the added test code.

## Architecture

Program:

`programs/solana-level-1-token-starter`

Implemented instructions:

* `create_token` — creates a mint using the selected token program.
* `create_token_account` — creates an associated token account for an owner and mint.
* `mint_tokens` — mints tokens to a destination token account.
* `transfer_tokens` — transfers tokens between token accounts using `transfer_checked`.

The program uses `anchor_spl::token_interface` and Token-2022.

## Tests

Tests are located in:

```text
programs/solana-level-1-token-starter/tests/
```

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

Expected result:

```text
test_id ................................ ok

creates_token_2022_mint ................ ok
creates_token_2022_account ............. ok
mints_tokens_and_updates_supply ........ ok

rejects_zero_amount .................... ok
rejects_wrong_authority ................ ok
rejects_wrong_mint ..................... ok
rejects_identical_source_and_destination ok

transfers_tokens_and_keeps_supply_unchanged ... ok
```

Expected test summary:

```text
Program unit tests: 1 passed
create_token:       1 passed
create_token_account: 1 passed
mint_tokens:        1 passed
negative_cases:     4 passed
transfer_tokens:    1 passed

Failed: 0
```

## Reproducibility

A clean checkout of branch `task/01-tests` should pass:

```bash
anchor build --ignore-keys
cargo test --workspace --locked
```

No program keypair, seed phrase, private key, or other secret is required to run the test suite.
