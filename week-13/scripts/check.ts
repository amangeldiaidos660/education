import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { getAccount } from "@solana/spl-token";
import { confirmedTransaction, connection, explorer, file, positionControl, preflight, price, PROGRAM, program, ROOT, strategy, vault, vaultA, vaultB, WALLET } from "./common";
import { verifyPosition } from "./positions";

async function main() {
  const rpc = connection();
  await preflight(rpc);
  console.log("Network: devnet");
  console.log("Wallet:", WALLET.toBase58());
  console.log("Balance:", (await rpc.getBalance(WALLET)) / 1e9, "SOL");
  const deployed = await rpc.getAccountInfo(PROGRAM);
  assert(deployed?.executable, "Existing vault program is missing");
  console.log("Program:", explorer(PROGRAM));
  console.log("Pool price:", await price(rpc), "B per A");
  if (!await rpc.getAccountInfo(strategy)) {
    console.log("Pool checks passed. Vault strategy is not initialized yet.");
    return;
  }
  const app = program(rpc);
  const configured = await app.account.strategyState.fetch(strategy);
  const state = await app.account.vaultState.fetch(vault);
  const a = await getAccount(rpc, vaultA), b = await getAccount(rpc, vaultB);
  assert(configured.vault.equals(vault) && configured.secondaryVault.equals(vaultB));
  assert(state.admin.equals(WALLET) && a.owner.equals(vault) && b.owner.equals(vault));
  assert.equal(state.totalAssets.toString(), a.amount.toString());
  console.log("Vault A:", a.amount.toString(), "base units");
  console.log("Vault B:", b.amount.toString(), "base units");
  const control = await rpc.getAccountInfo(positionControl)
    ? await app.account.positionControl.fetch(positionControl) : undefined;
  console.log("Position active:", Boolean(control?.active));
  console.log("Deposits/withdrawals:", !control?.active && b.amount === BigInt(0) ? "enabled" : "blocked");
  const saved = JSON.parse(readFileSync(file, "utf8"));
  for (const name of ["successfulSwap", "rejectedSwap", "reverseSwap"]) {
    const evidence = saved[name];
    if (!evidence) continue;
    const transaction = await confirmedTransaction(rpc, evidence.signature);
    assert(transaction?.meta, "Transaction unavailable: " + evidence.signature);
    if (name === "rejectedSwap") {
      assert(transaction.meta.err);
      assert((transaction.meta.logMessages ?? []).some((log: string) => /TooLittleOutputReceived/.test(log)));
    } else assert.equal(transaction.meta.err, null);
    console.log(name + ":", explorer(evidence.signature, true));
  }
  for (const [label, record] of Object.entries(saved.positionCycles ?? {}) as [string, any][]) {
    console.log("Cycle:", label, record.lowerPrice, record.upperPrice, "B per A");
    if (record.active) await verifyPosition(rpc, record);
    else if (record.closeSignature) {
      if (await rpc.getAccountInfo(new (await import("@solana/web3.js")).PublicKey(record.personalPosition))) {
        throw new Error("Closed position still exists: " + record.personalPosition);
      }
    }
    for (const key of ["openSignature", "collectSignature", "closeSignature"]) {
      if (!record[key]) continue;
      const tx = await confirmedTransaction(rpc, record[key]);
      assert.equal(tx.meta!.err, null);
      console.log(key + ":", explorer(record[key], true));
    }
    console.log("Collected fees A/B (base units):", record.collectedA ?? "0", record.collectedB ?? "0");
    console.log("Returned A/B (base units):", record.returnedA ?? "0", record.returnedB ?? "0");
  }
  if (saved.rebalance) console.log("Rebalance:", JSON.stringify(saved.rebalance));
  if (existsSync(resolve(ROOT, "target/deploy/simple_vault.so"))) console.log("Local program binary exists");
  console.log("Week-13 checks passed.");
}

main().catch((error) => { console.error(error instanceof Error ? error.message : error); process.exitCode = 1; });
