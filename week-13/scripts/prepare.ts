import assert from "node:assert/strict";
import { ComputeBudgetProgram, SystemProgram, SYSVAR_RENT_PUBKEY } from "@solana/web3.js";
import { createAssociatedTokenAccountIdempotentInstruction, getAccount, getMint, TOKEN_PROGRAM_ID } from "@solana/spl-token";
import BN from "bn.js";
import {
  card, positionControl, CLMM, connection, explorer, mintA, mintB, pool, preflight, program, save, send,
  shareMint, strategy, userA, userShares, vault, vaultA, vaultB, wallet, WALLET,
} from "./common";

async function main() {
  const rpc = connection();
  await preflight(rpc);
  const signer = wallet();
  const app = program(rpc);
  const budget = ComputeBudgetProgram.setComputeUnitLimit({ units: 400000 });
  if (!await rpc.getAccountInfo(vault)) {
    const ix = await app.methods.initializeVault().accountsStrict({
      vaultState: vault, vaultTokenAccount: vaultA, shareMint, tokenMint: mintA,
      admin: WALLET, tokenProgram: TOKEN_PROGRAM_ID, systemProgram: SystemProgram.programId,
      rent: SYSVAR_RENT_PUBKEY,
    }).instruction();
    console.log("Initialize vault:", explorer(await send(rpc, "initialize-vault", [budget, ix], [signer]), true));
  }
  const state = await app.account.vaultState.fetch(vault);
  assert(state.admin.equals(WALLET) && state.tokenMint.equals(mintA));
  assert(state.vaultTokenAccount.equals(vaultA) && state.shareMint.equals(shareMint));
  if (!await rpc.getAccountInfo(strategy)) {
    const ix = await app.methods.initializeStrategy().accountsStrict({
      vaultState: vault, positionControl, strategyState: strategy, secondaryVault: vaultB, tokenMint: mintA,
      tokenB: mintB, poolState: pool, clmmProgram: CLMM, admin: WALLET,
      tokenProgram: TOKEN_PROGRAM_ID, systemProgram: SystemProgram.programId,
    }).instruction();
    console.log("Initialize strategy:", explorer(await send(rpc, "initialize-strategy", [budget, ix], [signer]), true));
  }
  const configured = await app.account.strategyState.fetch(strategy);
  assert(configured.pool.equals(pool) && configured.tokenB.equals(mintB) && configured.secondaryVault.equals(vaultB));
  if (!await rpc.getAccountInfo(card)) {
    const ix = await app.methods.createUserCard().accountsStrict({
      vaultState: vault, userCard: card, positionControl, user: WALLET, systemProgram: SystemProgram.programId,
    }).instruction();
    console.log("User card:", explorer(await send(rpc, "create-card", [ix], [signer]), true));
  }
  const balance = await getAccount(rpc, vaultA);
  const secondary = await getAccount(rpc, vaultB);
  assert(balance.owner.equals(vault) && secondary.owner.equals(vault));
  if ((await getMint(rpc, shareMint)).supply === BigInt(0) && balance.amount === BigInt(0)) {
    const ix = await app.methods.deposit(new BN(100_000_000)).accountsStrict({
      positionControl, strategyState: strategy, secondaryVault: vaultB, vaultState: vault, userCard: card,
      vaultTokenAccount: vaultA, shareMint, tokenMint: mintA, userTokenAccount: userA,
      userShareAccount: userShares, user: WALLET, tokenProgram: TOKEN_PROGRAM_ID,
    }).instruction();
    const ata = createAssociatedTokenAccountIdempotentInstruction(WALLET, userShares, WALLET, shareMint);
    console.log("Deposit 100 A:", explorer(await send(rpc, "deposit-fixture", [budget, ata, ix], [signer]), true));
  }
  assert((await getAccount(rpc, vaultA)).amount >= BigInt(1_000_000), "Vault needs at least 1 A");
  save({ vaultProgramId: app.programId.toBase58(), vault: {
    state: vault.toBase58(), tokenAccountA: vaultA.toBase58(), tokenAccountB: vaultB.toBase58(),
    strategy: strategy.toBase58(), shareMint: shareMint.toBase58(), userCard: card.toBase58(), userShares: userShares.toBase58(),
  } });
  console.log("Vault ready:", explorer(vault));
}

main().catch((error) => { console.error(error instanceof Error ? error.message : error); process.exitCode = 1; });
