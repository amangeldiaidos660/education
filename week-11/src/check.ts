import assert from "node:assert/strict";
import { TickUtil } from "@raydium-io/raydium-sdk-v2";
import { LAMPORTS_PER_SOL, PublicKey } from "@solana/web3.js";
import {
  assertDevnet, CLMM_PROGRAM, connection, loadAmmConfig, WALLET,
} from "./config";
import { explorer, readState, verifyPool, verifyPosition, verifyToken } from "./utils";

async function main() {
  const rpc = connection();
  await assertDevnet(rpc);
  const config = await loadAmmConfig(rpc);
  console.log("Network: devnet");
  console.log("Wallet:", WALLET.toBase58());
  console.log("Balance:", (await rpc.getBalance(WALLET)) / LAMPORTS_PER_SOL, "SOL");
  console.log("Raydium CLMM:", explorer(CLMM_PROGRAM.toBase58()));
  console.log("AMM config:", explorer(config.id.toBase58()));
  console.log("Fee:", config.tradeFeeRate / 10000, "%; tick spacing:", config.tickSpacing);
  const state = readState();
  if (!state.tokenA && !state.tokenB) {
    console.log("Preflight checks passed. Tokens, pool and position have not been created.");
    return;
  }
  assert(state.tokenA && state.tokenB, "Incomplete token records");
  for (const [label, token] of [["A", state.tokenA], ["B", state.tokenB]] as const) {
    const { account } = await verifyToken(rpc, token);
    console.log("Token " + label + ":", explorer(token.mint));
    console.log("Wallet balance " + label + ":", account.amount.toString(), "base units");
  }
  if (!state.pool) {
    console.log("Token checks passed. Pool has not been created.");
    return;
  }
  const { pool, vaultA, vaultB } = await verifyPool(rpc, state);
  const price = TickUtil.sqrtPriceX64ToPrice(pool.sqrtPriceX64, pool.mintDecimalsA, pool.mintDecimalsB);
  console.log("Pool:", explorer(state.pool.id));
  console.log("Vault A:", explorer(state.pool.vaultA));
  console.log("Vault B:", explorer(state.pool.vaultB));
  console.log("Observation:", explorer(state.pool.observationId));
  console.log("Current price:", price.toFixed(), "B per A");
  console.log("Pool token balances:", vaultA.amount.toString(), vaultB.amount.toString(), "base units");
  if (!state.position) {
    console.log("Pool checks passed. Position has not been opened.");
    return;
  }
  const position = await verifyPosition(rpc, state);
  console.log("Position:", explorer(state.position.personalPosition));
  console.log("Position NFT:", explorer(state.position.nftMint));
  console.log("NFT token account:", explorer(state.position.nftAccount));
  console.log("Tick arrays:", explorer(state.position.tickArrayLower), explorer(state.position.tickArrayUpper));
  console.log("Ticks:", position.tickLower, position.tickUpper);
  console.log("Actual range:", state.position.lowerPrice, state.position.upperPrice, "B per A");
  console.log("Position liquidity:", position.liquidity.toString());
  console.log("In range:", pool.tickCurrent >= position.tickLower && pool.tickCurrent < position.tickUpper);
  for (const address of [state.pool.bitmapExtension, state.position.protocolPosition]) {
    const account = await rpc.getAccountInfo(new PublicKey(address));
    if (account) assert(account.owner.equals(CLMM_PROGRAM));
    console.log("Related PDA:", explorer(address), "exists:", Boolean(account));
  }
  const signatures = [state.tokenA.signature, state.tokenB.signature, state.pool.signature, state.position.signature];
  assert(signatures.every((signature) => typeof signature === "string"), "Some transaction signatures are missing");
  const statuses = await rpc.getSignatureStatuses(signatures as string[], { searchTransactionHistory: true });
  statuses.value.forEach((status, index) => {
    assert(status && !status.err, "Transaction is missing or failed: " + signatures[index]);
    assert(status.confirmationStatus === "confirmed" || status.confirmationStatus === "finalized");
    console.log("Transaction:", explorer(signatures[index]!, true));
  });
  console.log("Week 11 on-chain checks passed.");
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
