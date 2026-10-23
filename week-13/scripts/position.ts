import assert from "node:assert/strict";
import BN from "bn.js";
import { connection, explorer, quote, send, swapInstruction, wallet } from "./common";
import { balances, budget, close, collect, loadState, open, planOpen } from "./positions";

async function main() {
  const rpc = connection();
  const action = process.argv[2];
  if (action === "fund") {
    const balance = await balances(rpc);
    if (balance.b >= BigInt(10_000_000)) return console.log("Vault already has both tokens");
    const q = await quote(rpc, new BN(30_000_000), true);
    const signature = await send(rpc, "prepare-position-tokens", [budget(), await swapInstruction(rpc, q)], [wallet()]);
    console.log("Prepared token B:", explorer(signature, true)); return;
  }
  const label = process.argv[3] ?? "manual-" + Date.now();
  if (action === "open") {
    const record = await open(rpc, await planOpen(rpc, label));
    console.log("Position:", explorer(record.personalPosition)); return;
  }
  const record = loadState().vaultPosition;
  assert(record?.active, "No active position recorded");
  if (action === "collect") {
    const result = await collect(rpc, record, label);
    console.log("Fees:", result.feeA.toString(), result.feeB.toString()); return;
  }
  if (action === "close") { await close(rpc, record); console.log("Position closed"); return; }
  throw new Error("Use fund, open, collect or close");
}

main().catch((error) => { console.error(error instanceof Error ? error.stack : error); process.exitCode = 1; });
