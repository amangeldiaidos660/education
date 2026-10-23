import assert from "node:assert/strict";
import { getPdaPoolId } from "@raydium-io/raydium-sdk-v2";
import { PublicKey } from "@solana/web3.js";
import Decimal from "decimal.js";
import {
  CLMM_PROGRAM, COMPUTE_BUDGET, INITIAL_PRICE, loadAmmConfig, loadSdk, loadWallet, TX_VERSION,
} from "./config";
import {
  apiToken, canonicalMints, explorer, readState, resume, run, saveState, submit, verifyPool, verifyToken,
} from "./utils";

run(async (rpc) => {
  const state = readState();
  assert(state.tokenA && state.tokenB, "Run create:tokens first");
  await verifyToken(rpc, state.tokenA);
  await verifyToken(rpc, state.tokenB);
  const [mintA] = canonicalMints(new PublicKey(state.tokenA.mint), new PublicKey(state.tokenB.mint));
  assert.equal(mintA.toBase58(), state.tokenA.mint, "Saved mint order must be A then B");
  const config = await loadAmmConfig(rpc);
  const poolId = getPdaPoolId(
    CLMM_PROGRAM, config.id, new PublicKey(state.tokenA.mint), new PublicKey(state.tokenB.mint)
  ).publicKey;
  if (state.pool) assert.equal(state.pool.id, poolId.toBase58(), "Pool address changed");
  const resumed = await resume(rpc, "pool");
  if (await rpc.getAccountInfo(poolId)) {
    assert(state.pool, "Existing pool has no saved record; refusing to replace state");
    if (resumed) state.pool.signature = resumed;
    await verifyPool(rpc, state);
    saveState(state);
    console.log("Pool already exists:", explorer(poolId.toBase58()));
    return;
  }
  const wallet = loadWallet();
  const sdk = await loadSdk(rpc, wallet);
  const built = await sdk.clmm.createPool({
    programId: CLMM_PROGRAM,
    mint1: apiToken(state.tokenA, "TOKEN-A"),
    mint2: apiToken(state.tokenB, "TOKEN-B"),
    ammConfig: config,
    initialPrice: new Decimal(INITIAL_PRICE),
    txVersion: TX_VERSION,
    computeBudgetConfig: COMPUTE_BUDGET,
  });
  const addresses = built.extInfo.address;
  assert.equal(addresses.id, poolId.toBase58());
  state.pool = {
    id: addresses.id,
    configId: config.id.toBase58(),
    tradeFeeRate: config.tradeFeeRate,
    tickSpacing: config.tickSpacing,
    vaultA: addresses.vault.A,
    vaultB: addresses.vault.B,
    observationId: addresses.observationId,
    bitmapExtension: addresses.exBitmapAccount,
    initialPrice: INITIAL_PRICE,
  };
  saveState(state);
  state.pool.signature = await submit(rpc, "pool", built.transaction, built.signers, wallet, (signature) => {
    state.pool!.signature = signature;
    saveState(state);
  });
  await verifyPool(rpc, state);
  saveState(state);
  console.log("Pool:", explorer(state.pool.id));
  console.log("Creation:", explorer(state.pool.signature, true));
  console.log("Initial price:", INITIAL_PRICE, "B per A; fee: 0.25%");
});
