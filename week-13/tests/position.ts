import assert from "node:assert/strict";
import { getAccount, TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { Keypair, PublicKey } from "@solana/web3.js";
import BN from "bn.js";
import Decimal from "decimal.js";
import {
  card, connection, mintA, positionControl, preflight, price, program, quote, save, send,
  shareMint, source, strategy, swapInstruction, userA, userShares, vault, vaultA, vaultB, wallet, WALLET,
} from "../scripts/common";
import {
  balances, budget, close, closeQuote, collect, loadState, manage, open, openInstruction, planOpen,
  recordCycle, simulateReject, verifyPosition,
} from "../scripts/positions";
import { prepareTrader, trader, traderSwap } from "../scripts/trader";

describe("week-13: vault CLMM position and rebalance", function () {
  const rpc = connection();
  const runId = Date.now().toString();
  let app: any, first: any, rebalance: any, replacement: any;
  const userAccounts = () => ({ positionControl, strategyState: strategy, secondaryVault: vaultB,
    vaultState: vault, userCard: card, vaultTokenAccount: vaultA, shareMint, tokenMint: mintA,
    userTokenAccount: userA, userShareAccount: userShares, user: WALLET, tokenProgram: TOKEN_PROGRAM_ID });

  before(async () => {
    await preflight(rpc); app = program(rpc);
    const info = await rpc.getAccountInfo(positionControl);
    if (info) assert(!(await app.account.positionControl.fetch(positionControl)).active, "Close the recorded active position before rerunning");
    const balance = await balances(rpc);
    if (balance.b < BigInt(10_000_000)) {
      const q = await quote(rpc, new BN(30_000_000));
      const signature = await send(rpc, "position-funds-" + runId, [budget(), await swapInstruction(rpc, q)], [wallet()]);
      save({ positionFunding: { signature, amountA: q.amount.toString(), minimumB: q.minimum.toString() } });
    }
    await prepareTrader(rpc);
    first = await planOpen(rpc, "cycle-" + runId);
  });

  it("range surrounds the current price and respects tick spacing", () => {
    assert(first.tickLower % 60 === 0 && first.tickUpper % 60 === 0);
    assert(new Decimal(first.lowerPrice).lt(first.priceAtOpen));
    assert(new Decimal(first.upperPrice).gt(first.priceAtOpen));
  });

  it("non-admin cannot open a vault position", async () => {
    await simulateReject(rpc, await openInstruction(rpc, first, trader().publicKey), /ConstraintHasOne/);
  });

  it("zero liquidity is rejected", async () => {
    await simulateReject(rpc, await openInstruction(rpc, { ...first, liquidity: "0" }), /InvalidPositionAmount/);
  });

  it("reversed price range is rejected", async () => {
    await simulateReject(rpc, await openInstruction(rpc, { ...first, tickLower: first.tickUpper, tickUpper: first.tickLower }), /InvalidRange/);
  });

  it("insufficient token caps reject the entire opening", async () => {
    await simulateReject(rpc, await openInstruction(rpc, { ...first, maximumA: "1", maximumB: "1" }), /PriceSlippageCheck/);
  });

  it("opens using vault funds, caps both amounts and assigns its NFT to the PDA", async () => {
    first = await open(rpc, first);
    assert((await verifyPosition(rpc, first)).liquidity.gt(new BN(0)));
    assert(BigInt(first.spentA) > BigInt(0) && BigInt(first.spentB) > BigInt(0));
  });

  it("another active position cannot be opened", async () => {
    const other = await planOpen(rpc, "duplicate-" + runId);
    await simulateReject(rpc, await openInstruction(rpc, other), /PositionActive/);
  });

  it("withdrawals and deposits are blocked while the position is open", async () => {
    await simulateReject(rpc, await app.methods.withdraw(new BN(1_000_000)).accountsStrict(userAccounts()).instruction(), /PositionActive/);
    await simulateReject(rpc, await app.methods.deposit(new BN(1_000_000)).accountsStrict(userAccounts()).instruction(), /PositionActive/);
  });

  it("non-admin cannot collect fees or close the position", async () => {
    const foreign = Keypair.generate().publicKey;
    await simulateReject(rpc, await app.methods.collectFees().accountsStrict(manage(rpc, first, foreign)).remainingAccounts([{ pubkey: new PublicKey(source.pool.bitmapExtension), isSigner: false, isWritable: true }]).instruction(), /ConstraintHasOne/);
    await simulateReject(rpc, await app.methods.closePosition(new BN(0), new BN(0)).accountsStrict(manage(rpc, first, foreign)).instruction(), /ConstraintHasOne/);
  });

  it("another NFT account cannot replace the vault NFT", async () => {
    await simulateReject(rpc, await app.methods.collectFees().accountsStrict(manage(rpc, first, WALLET, {
      nftAccount: new PublicKey(source.position.nftAccount),
    })).instruction(), /ConstraintTokenMint|ConstraintTokenOwner/);
  });

  it("swaps from a separate test wallet generate fees", async () => {
    const swaps = [];
    for (let i = 0; i < 3; i++) swaps.push(await traderSwap(rpc, new BN(1_000_000), i !== 1, runId + "-fees-" + i));
    assert(swaps.every((swap) => swap.trader !== WALLET.toBase58()));
    first.traderSwaps = swaps; recordCycle(first);
  });

  it("collects nonzero fees while preserving liquidity and NFT ownership", async () => {
    const fees = await collect(rpc, first, runId);
    assert(fees.feeA + fees.feeB > BigInt(0), "Real trading fees must be collected");
  });

  it("unrealistic minimums reject closing without changing position balances", async () => {
    const before = await balances(rpc), position = await verifyPosition(rpc, first);
    const ix = await app.methods.closePosition(new BN("18446744073709551615"), new BN("18446744073709551615"))
      .accountsStrict(manage(rpc, first)).instruction();
    await simulateReject(rpc, ix, /PriceSlippageCheck/);
    assert.deepEqual(await balances(rpc), before);
    assert((await verifyPosition(rpc, first)).liquidity.eq(position.liquidity));
  });

  it("closes the complete position and returns both tokens to the vault", async () => {
    first = await close(rpc, first);
    assert(BigInt(first.returnedA) > BigInt(0) && BigInt(first.returnedB) > BigInt(0));
  });

  it("opens a position for the rebalance scenario", async () => {
    rebalance = await open(rpc, await planOpen(rpc, "rebalance-old-" + runId));
  });

  it("moves the pool price outside the position range using another wallet", async () => {
    const swaps = [];
    const downward = new Decimal(rebalance.lowerPrice).gt(source.position.lowerPrice);
    const inside = (value: string) => downward
      ? new Decimal(value).gte(rebalance.lowerPrice) : new Decimal(value).lt(rebalance.upperPrice);
    let current = await price(rpc);
    for (let i = 0; i < 16 && inside(current); i++) {
      swaps.push(await traderSwap(rpc, new BN(10_000_000), downward, runId + "-move-" + i));
      current = await price(rpc);
    }
    assert(!inside(current), "The price must exit the range");
    rebalance.priceOutside = current; rebalance.priceMovingSwaps = swaps; recordCycle(rebalance);
  });

  it("closes the position outside its range", async () => {
    rebalance = await close(rpc, rebalance);
  });

  it("opens a replacement around the new current price", async () => {
    replacement = await open(rpc, await planOpen(rpc, "rebalance-new-" + runId));
    assert(replacement.tickLower !== rebalance.tickLower || replacement.tickUpper !== rebalance.tickUpper);
    const current = new Decimal(await price(rpc));
    assert(current.gte(replacement.lowerPrice) && current.lt(replacement.upperPrice));
    save({ rebalance: { oldPosition: rebalance.personalPosition, newPosition: replacement.personalPosition,
      priceOutside: rebalance.priceOutside, oldRange: [rebalance.lowerPrice, rebalance.upperPrice],
      newRange: [replacement.lowerPrice, replacement.upperPrice] } });
  });

  it("closes the replacement and converts remaining B back into A", async () => {
    replacement = await close(rpc, replacement);
    const balance = await balances(rpc);
    if (balance.b > BigInt(0)) {
      const q = await quote(rpc, new BN(balance.b.toString()), false);
      const signature = await send(rpc, "position-exit-" + runId, [budget(), await swapInstruction(rpc, q)], [wallet()]);
      save({ positionExit: { signature, amountB: q.amount.toString(), minimumA: q.minimum.toString() } });
    }
    assert.equal((await balances(rpc)).b, BigInt(0));
  });

  it("restores withdrawals after all positions are closed and B is converted", async () => {
    const ix = await app.methods.withdraw(new BN(10000)).accountsStrict(userAccounts()).instruction();
    const signature = await send(rpc, "position-withdraw-" + runId, [budget(), ix], [wallet()]);
    save({ positionFinalWithdrawal: signature });
    const state = loadState(); assert(state.positionCycles[first.label].collectSignature && state.rebalance);
  });
});
