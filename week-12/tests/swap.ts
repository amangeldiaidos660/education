import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { getAccount, TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { ComputeBudgetProgram, Keypair, PublicKey, SystemProgram, Transaction, VersionedTransaction } from "@solana/web3.js";
import BN from "bn.js";
import {
  card, confirmedTransaction, connection, file, mintA, mintB, pool, preflight, price, program, quote, save, send,
  shareMint, source, strategy, swapAccounts, swapInstruction, userA, userShares,
  vault, vaultA, vaultB, wallet, WALLET,
} from "../scripts/common";

describe("week-12: vault swap through Raydium CPI", function () {
  const rpc = connection();
  const budget = ComputeBudgetProgram.setComputeUnitLimit({ units: 600000 });
  const unit = new BN(1_000_000);
  let app: any;
  let q: Awaited<ReturnType<typeof quote>>;
  const runId = Date.now().toString();

  async function snapshot() {
    const accounts = await rpc.getMultipleAccountsInfo([
      vault, vaultA, vaultB, shareMint, card, userA, userShares,
      pool, new PublicKey(source.pool.vaultA), new PublicKey(source.pool.vaultB),
      new PublicKey(source.pool.observationId),
    ]);
    assert(accounts.every(Boolean));
    return accounts.map((account) => account!.data.toString("base64"));
  }

  async function reject(ix: any, pattern: RegExp) {
    const before = await snapshot();
    const { blockhash } = await rpc.getLatestBlockhash("confirmed");
    const tx = new VersionedTransaction(new Transaction({ feePayer: WALLET, recentBlockhash: blockhash }).add(budget, ix).compileMessage());
    const result = await rpc.simulateTransaction(tx, { sigVerify: false, commitment: "confirmed" });
    assert(result.value.err, "Instruction must be rejected");
    assert.match((result.value.logs ?? []).join("\n"), pattern);
    assert.deepEqual(await snapshot(), before);
  }

  before(async () => {
    await preflight(rpc);
    app = program(rpc);
    assert(await rpc.getAccountInfo(strategy), "Run npm run setup:devnet after deployment");
    const configured = await app.account.strategyState.fetch(strategy);
    assert(configured.pool.equals(pool) && configured.tokenB.equals(mintB));
    assert.equal((await getAccount(rpc, vaultB)).amount, BigInt(0), "Return token B to A before rerunning the suite");
    assert((await getAccount(rpc, vaultA)).amount >= BigInt(1_000_000));
    q = await quote(rpc, unit);
  });

  it("both vault token accounts are controlled by the vault PDA", async () => {
    const a = await getAccount(rpc, vaultA), b = await getAccount(rpc, vaultB);
    assert(a.owner.equals(vault) && b.owner.equals(vault));
    assert(a.mint.equals(mintA) && b.mint.equals(mintB));
  });

  it("quote uses a positive minimum one percent below expected output", () => {
    assert(q.expected.gt(new BN(0)) && q.minimum.gt(new BN(0)));
    assert(q.minimum.eq(q.expected.muln(99).divn(100)));
    assert(q.remaining.length > 0);
  });

  it("a non-administrator cannot swap", async () => {
    await reject(await swapInstruction(rpc, q, q.minimum, Keypair.generate().publicKey), /ConstraintHasOne/);
  });

  it("zero input is rejected", async () => {
    await reject(await swapInstruction(rpc, { ...q, amount: new BN(0) }), /ZeroSwap/);
  });

  it("zero minimum output is rejected", async () => {
    await reject(await swapInstruction(rpc, q, new BN(0)), /ZeroMinimumOut/);
  });

  it("an excessive input is rejected", async () => {
    const balance = await getAccount(rpc, vaultA);
    await reject(await swapInstruction(rpc, { ...q, amount: new BN(balance.amount.toString()).addn(1) }), /InsufficientTokens/);
  });

  it("another destination account cannot replace the second vault", async () => {
    await reject(await swapInstruction(rpc, q, q.minimum, WALLET, { secondaryVault: userA }), /ConstraintHasOne|ConstraintSeeds|ConstraintTokenMint/);
  });

  it("another program cannot replace Raydium CLMM", async () => {
    await reject(await swapInstruction(rpc, q, q.minimum, WALLET, { clmmProgram: SystemProgram.programId }), /InvalidProgramId/);
  });

  it("another pool cannot replace the configured pool", async () => {
    await reject(await swapInstruction(rpc, q, q.minimum, WALLET, { poolState: new PublicKey(source.pool.observationId) }), /InvalidPool|AccountDiscriminatorMismatch/);
  });

  it("an existing strategy cannot be initialized again", async () => {
    const ix = await app.methods.initializeStrategy().accountsStrict({
      vaultState: vault, strategyState: strategy, secondaryVault: vaultB,
      tokenMint: mintA, tokenB: mintB, poolState: pool,
      clmmProgram: swapAccounts().clmmProgram, admin: WALLET,
      tokenProgram: TOKEN_PROGRAM_ID, systemProgram: SystemProgram.programId,
    }).instruction();
    await reject(ix, /already in use|already been initialized|custom program error: 0x0/);
  });

  it("A to B swaps vault funds and changes the pool price", async () => {
    q = await quote(rpc, unit);
    const aBefore = await getAccount(rpc, vaultA), bBefore = await getAccount(rpc, vaultB);
    const sharesBefore = await rpc.getAccountInfo(shareMint);
    const ix = await swapInstruction(rpc, q);
    const signature = await send(rpc, "swap-success-" + runId, [budget, ix], [wallet()]);
    const aAfter = await getAccount(rpc, vaultA), bAfter = await getAccount(rpc, vaultB);
    assert.equal(aBefore.amount - aAfter.amount, BigInt(unit.toString()));
    const received = bAfter.amount - bBefore.amount;
    assert(received >= BigInt(q.minimum.toString()));
    const priceAfter = await price(rpc);
    assert(Number(priceAfter) < Number(q.priceBefore));
    assert.equal((await app.account.vaultState.fetch(vault)).totalAssets.toString(), aAfter.amount.toString());
    assert.deepEqual((await rpc.getAccountInfo(shareMint))!.data, sharesBefore!.data);
    const transaction = await confirmedTransaction(rpc, signature);
    assert(transaction?.meta && transaction.meta.err === null);
    assert((transaction.meta.innerInstructions ?? []).some((group) => group.instructions.length > 0));
    assert((transaction.meta.logMessages ?? []).some((log) => /Instruction: SwapV2/.test(log)));
    save({ successfulSwap: { signature, amountIn: unit.toString(), amountOut: received.toString(),
      expectedOut: q.expected.toString(), minimumOut: q.minimum.toString(), priceBefore: q.priceBefore, priceAfter } });
  });

  it("a slippage rejection is recorded on-chain and rolls back all account changes", async () => {
    const next = await quote(rpc, unit);
    const minimum = next.expected.muln(2);
    const before = await snapshot();
    const ix = await swapInstruction(rpc, next, minimum);
    const signature = await send(rpc, "swap-rejected-" + runId, [budget, ix], [wallet()], /TooLittleOutputReceived/);
    assert.deepEqual(await snapshot(), before);
    const transaction = await confirmedTransaction(rpc, signature);
    assert(transaction?.meta?.err);
    assert.match((transaction.meta.logMessages ?? []).join("\n"), /TooLittleOutputReceived/);
    save({ rejectedSwap: { signature, amountIn: unit.toString(), minimumOut: minimum.toString(),
      expectedOut: next.expected.toString(), reason: "TooLittleOutputReceived" } });
  });

  const userAccounts = () => ({ strategyState: strategy, secondaryVault: vaultB, vaultState: vault,
    userCard: card, vaultTokenAccount: vaultA, shareMint, tokenMint: mintA,
    userTokenAccount: userA, userShareAccount: userShares, user: WALLET, tokenProgram: TOKEN_PROGRAM_ID });

  it("deposits are blocked while the vault holds token B", async () => {
    await reject(await app.methods.deposit(unit).accountsStrict(userAccounts()).instruction(), /UnconvertedAssets/);
  });

  it("withdrawals are blocked while the vault holds token B", async () => {
    await reject(await app.methods.withdraw(unit).accountsStrict(userAccounts()).instruction(), /UnconvertedAssets/);
  });

  it("B to A converts the entire secondary balance back to the base token", async () => {
    const balance = await getAccount(rpc, vaultB);
    assert(balance.amount > BigInt(0));
    const reverse = await quote(rpc, new BN(balance.amount.toString()), false);
    const aBefore = (await getAccount(rpc, vaultA)).amount;
    const signature = await send(rpc, "swap-reverse-" + runId, [budget, await swapInstruction(rpc, reverse)], [wallet()]);
    assert.equal((await getAccount(rpc, vaultB)).amount, BigInt(0));
    const received = (await getAccount(rpc, vaultA)).amount - aBefore;
    assert(received >= BigInt(reverse.minimum.toString()));
    save({ reverseSwap: { signature, amountIn: reverse.amount.toString(), amountOut: received.toString(),
      minimumOut: reverse.minimum.toString(), priceBefore: reverse.priceBefore, priceAfter: await price(rpc) } });
  });

  it("deposits and withdrawals resume after B is converted back to A", async () => {
    const deposit = await app.methods.deposit(unit).accountsStrict(userAccounts()).instruction();
    await send(rpc, "deposit-resumed-" + runId, [budget, deposit], [wallet()]);
    const withdraw = await app.methods.withdraw(unit).accountsStrict(userAccounts()).instruction();
    await send(rpc, "withdraw-resumed-" + runId, [budget, withdraw], [wallet()]);
    const saved = JSON.parse(readFileSync(file, "utf8"));
    assert(saved.successfulSwap && saved.rejectedSwap && saved.reverseSwap);
  });
});
