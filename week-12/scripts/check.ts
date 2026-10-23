import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { getAccount } from "@solana/spl-token";
import { confirmedTransaction, connection, explorer, file, preflight, price, PROGRAM, program, ROOT, strategy, vault, vaultA, vaultB, WALLET } from "./common";

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
    console.log("Pool checks passed. Week-12 vault strategy is not initialized yet.");
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
  console.log("Deposits/withdrawals:", b.amount === BigInt(0) ? "enabled" : "blocked until B is converted to A");
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
  if (existsSync(resolve(ROOT, "target/deploy/simple_vault.so"))) console.log("Local program binary exists");
  console.log("Week-12 checks passed.");
}

main().catch((error) => { console.error(error instanceof Error ? error.message : error); process.exitCode = 1; });
