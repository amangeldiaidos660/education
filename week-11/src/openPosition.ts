import assert from "node:assert/strict";
import { getPdaPersonalPositionAddress, LiquidityMathUtil, TickUtil } from "@raydium-io/raydium-sdk-v2";
import { PublicKey } from "@solana/web3.js";
import BN from "bn.js";
import {
  CLMM_PROGRAM, COMPUTE_BUDGET, LOWER_PRICE, loadSdk, loadWallet,
  POSITION_AMOUNT_A, POSITION_MAX_B, SLIPPAGE_BPS, TX_VERSION, UPPER_PRICE,
} from "./config";
import {
  explorer, localKey, maxWithSlippage, positionRange, readState, resume, run,
  saveState, submit, verifyPool, verifyPosition, verifyToken,
} from "./utils";

run(async (rpc) => {
  const state = readState();
  assert(state.tokenA && state.tokenB && state.pool, "Create tokens and pool first");
  await verifyPool(rpc, state);
  const resumed = await resume(rpc, "position");
  if (state.position && await rpc.getAccountInfo(new PublicKey(state.position.personalPosition))) {
    const decoded = await verifyPosition(rpc, state);
    if (resumed) state.position.signature = resumed;
    state.position.liquidity = decoded.liquidity.toString();
    saveState(state);
    console.log("Position already exists:", explorer(state.position.personalPosition));
    return;
  }
  const wallet = loadWallet();
  const sdk = await loadSdk(rpc, wallet);
  const { poolInfo, rpcData } = await sdk.clmm.getSimplePoolInfo(state.pool.id);
  const poolKeys = { vault: { A: rpcData.vaultA, B: rpcData.vaultB } };
  assert.equal(poolInfo.programId, CLMM_PROGRAM.toBase58());
  assert.equal(poolInfo.mintA.address, state.tokenA.mint);
  assert.equal(poolInfo.mintB.address, state.tokenB.mint);
  const range = positionRange(
    LOWER_PRICE, UPPER_PRICE, poolInfo.mintA.decimals, poolInfo.mintB.decimals, poolInfo.config.tickSpacing
  );
  const price = TickUtil.sqrtPriceX64ToPrice(
    rpcData.sqrtPriceX64, poolInfo.mintA.decimals, poolInfo.mintB.decimals
  );
  assert(price.gt(range.lowerPrice) && price.lt(range.upperPrice),
    "Current price is outside the chosen range");
  const current = rpcData.sqrtPriceX64;
  const lower = TickUtil.getSqrtPriceAtTick(range.tickLower);
  const upper = TickUtil.getSqrtPriceAtTick(range.tickUpper);
  const quote = LiquidityMathUtil.getLiquidityAndAmountsFromAmount({
    sqrtPriceCurrentX64: current,
    sqrtPriceLowerX64: lower,
    sqrtPriceUpperX64: upper,
    amountInfo: { type: "amountA", amount: new BN(POSITION_AMOUNT_A) },
  });
  const amounts = LiquidityMathUtil.getAmountsForLiquidity(current, lower, upper, quote.liquidity, true);
  const maxB = maxWithSlippage(BigInt(amounts.amountB.toString()), SLIPPAGE_BPS);
  assert(maxB > BigInt(0) && maxB <= BigInt(POSITION_MAX_B), "Token B quote exceeds the configured cap");
  const balanceA = await verifyToken(rpc, state.tokenA);
  const balanceB = await verifyToken(rpc, state.tokenB);
  assert(balanceA.account.amount >= BigInt(POSITION_AMOUNT_A), "Insufficient token A balance");
  assert(balanceB.account.amount >= maxB, "Insufficient token B balance");
  const nft = localKey("position-nft");
  const personal = getPdaPersonalPositionAddress(CLMM_PROGRAM, nft.publicKey).publicKey;
  if (state.position) {
    assert.equal(state.position.nftMint, nft.publicKey.toBase58(), "Original position NFT key is missing");
    assert.equal(state.position.personalPosition, personal.toBase58());
  }
  const built = await sdk.clmm.openPositionFromBase({
    poolInfo,
    poolKeys,
    ownerInfo: { useSOLBalance: false },
    tickLower: range.tickLower,
    tickUpper: range.tickUpper,
    base: "MintA",
    baseAmount: new BN(POSITION_AMOUNT_A),
    otherAmountMax: new BN(maxB.toString()),
    liquidity: new BN(0),
    nft2022: true,
    withMetadata: "no-create",
    getEphemeralSigners: async (count: number) => {
      assert.equal(count, 1);
      return [nft.publicKey.toBase58()];
    },
    txVersion: TX_VERSION,
    computeBudgetConfig: COMPUTE_BUDGET,
  });
  const addresses = built.extInfo;
  assert(addresses.personalPosition.equals(personal));
  state.position = {
    nftMint: addresses.nftMint.toBase58(),
    nftAccount: addresses.positionNftAccount.toBase58(),
    personalPosition: addresses.personalPosition.toBase58(),
    protocolPosition: addresses.protocolPosition.toBase58(),
    tickArrayLower: addresses.tickArrayLower.toBase58(),
    tickArrayUpper: addresses.tickArrayUpper.toBase58(),
    ...range,
    baseAmountA: POSITION_AMOUNT_A,
    maxAmountB: maxB.toString(),
  };
  saveState(state);
  state.position.signature = await submit(
    rpc, "position", built.transaction, [...built.signers, nft], wallet, (signature) => {
      state.position!.signature = signature;
      saveState(state);
    }
  );
  const decoded = await verifyPosition(rpc, state);
  state.position.liquidity = decoded.liquidity.toString();
  saveState(state);
  console.log("Position:", explorer(state.position.personalPosition));
  console.log("Position NFT:", explorer(state.position.nftMint));
  console.log("Opening:", explorer(state.position.signature, true));
  console.log("Range:", range.lowerPrice, "to", range.upperPrice, "B per A");
});
