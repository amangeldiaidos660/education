import assert from "node:assert/strict";
import { getAccount } from "@solana/spl-token";
import { ComputeBudgetProgram } from "@solana/web3.js";
import BN from "bn.js";
import { connection, explorer, price, quote, save, send, swapInstruction, vaultA, vaultB, wallet } from "./common";

async function main() {
  const rpc = connection();
  const direction = process.argv[3] ?? "a-to-b";
  assert(["a-to-b", "b-to-a"].includes(direction), "Direction must be a-to-b or b-to-a");
  const aToB = direction === "a-to-b";
  const argument = process.argv[2] ?? "1000000";
  const raw = argument === "all"
    ? (await getAccount(rpc, aToB ? vaultA : vaultB)).amount.toString()
    : argument;
  assert(/^\d+$/.test(raw), "Pass an integer amount in base units or all");
  const result = await quote(rpc, new BN(raw), aToB);
  const before = (await getAccount(rpc, aToB ? vaultB : vaultA)).amount;
  console.log("Expected output:", result.expected.toString(), "base units");
  console.log("Minimum output:", result.minimum.toString(), "base units");
  const signature = await send(rpc, "manual-swap-" + Date.now(), [
    ComputeBudgetProgram.setComputeUnitLimit({ units: 600000 }), await swapInstruction(rpc, result),
  ], [wallet()]);
  const after = (await getAccount(rpc, aToB ? vaultB : vaultA)).amount;
  const record = { signature, amountIn: raw, amountOut: (after - before).toString(),
    expectedOut: result.expected.toString(), minimumOut: result.minimum.toString(),
    priceBefore: result.priceBefore, priceAfter: await price(rpc) };
  save({ [aToB ? "successfulSwap" : "reverseSwap"]: record });
  console.log("Swap:", explorer(signature, true));
  console.log("Received:", record.amountOut, "base units");
  console.log("Price:", record.priceBefore, "->", record.priceAfter, "B per A");
}

main().catch((error) => { console.error(error instanceof Error ? error.message : error); process.exitCode = 1; });
