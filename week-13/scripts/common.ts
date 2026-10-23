import assert from "node:assert/strict";
import { existsSync, mkdirSync, readFileSync, renameSync, unlinkSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { homedir } from "node:os";
import * as anchor from "@coral-xyz/anchor";
import { ComputeBudgetProgram, Connection, Keypair, PublicKey, Signer, Transaction, TransactionInstruction, VersionedTransaction } from "@solana/web3.js";
import { getAccount, getMint, getAssociatedTokenAddressSync, TOKEN_PROGRAM_ID, TOKEN_2022_PROGRAM_ID } from "@solana/spl-token";
import { PoolInfoLayout, PoolUtils, Raydium, TickUtil, TickArrayBitmapExtensionLayout } from "@raydium-io/raydium-sdk-v2";
import BN from "bn.js";
import bs58 from "bs58";

export const ROOT = resolve(__dirname, "..");
export const PROGRAM = new PublicKey("HnAz6jnZQ2iwBUWutPEs8HTWJMHey6DUG1dCR5DvHk9f");
export const WALLET = new PublicKey("HMg9Wz3DLAwU8GAYh91WhRMWQzdhpAsBpUYkZcf9tMtX");
export const CLMM = new PublicKey("DRayAUgENGQBKVaX8owNhgzkEDyoHTGVEGHVJT1E9pfH");
export const MEMO = new PublicKey("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr");
export const file = resolve(ROOT, "addresses.devnet.json");
export const source: any = JSON.parse(readFileSync(file, "utf8"));
assert.equal(source.wallet, WALLET.toBase58());
assert.equal(source.clmmProgramId, CLMM.toBase58());
export const mintA = new PublicKey(source.tokenA.mint);
export const mintB = new PublicKey(source.tokenB.mint);
export const pool = new PublicKey(source.pool.id);
export function pda(seed: string, ...keys: PublicKey[]) {
  return PublicKey.findProgramAddressSync([Buffer.from(seed), ...keys.map((key) => key.toBuffer())], PROGRAM)[0];
}
export const vault = pda("vault", mintA);
export const vaultA = pda("vault_tokens", mintA);
export const shareMint = pda("share_mint", mintA);
export const strategy = pda("strategy", vault);
export const vaultB = pda("strategy_tokens", vault);
export const positionControl = pda("position_control", vault);
export const card = pda("user_card", vault, WALLET);
export const userA = getAssociatedTokenAddressSync(mintA, WALLET);
export const userShares = getAssociatedTokenAddressSync(shareMint, WALLET);

export function connection() {
  const rpc = new Connection(process.env.ANCHOR_PROVIDER_URL ?? "https://api.devnet.solana.com", {
    commitment: "confirmed",
    confirmTransactionInitialTimeout: 90000,
    fetch: async (input, init) => {
      for (let attempt = 0; ; attempt++) {
        try {
          const response = await fetch(input, init);
          if (response.status === 429 && attempt < 5) {
            await response.text();
            await new Promise((done) => setTimeout(done, Math.min(3000 * 2 ** attempt, 15000)));
            continue;
          }
          return response;
        } catch (error) {
          if (attempt >= 2) throw error;
          await new Promise((done) => setTimeout(done, 500 * 2 ** attempt));
        }
      }
    },
  });
  rpc.confirmTransaction = (async (strategy: string | anchor.web3.TransactionConfirmationStrategy, commitment?: anchor.web3.Commitment) => {
    const signature = typeof strategy === "string" ? strategy : strategy.signature;
    const target = commitment ?? "confirmed";
    const deadline = Date.now() + 120000;
    for (let attempt = 0; Date.now() < deadline; attempt++) {
      const status = (await rpc.getSignatureStatuses([signature], { searchTransactionHistory: true })).value[0];
      const confirmed = target === "finalized" ? status?.confirmationStatus === "finalized"
        : status?.confirmationStatus === "confirmed" || status?.confirmationStatus === "finalized";
      if (status && confirmed) return { context: { slot: status.slot }, value: { err: status.err } };
      if (!status && typeof strategy !== "string" && "lastValidBlockHeight" in strategy && attempt % 4 === 0) {
        if (await rpc.getBlockHeight("confirmed") > strategy.lastValidBlockHeight) {
          throw new Error("Blockhash expired; check transaction before retrying: " + signature);
        }
      }
      await new Promise((done) => setTimeout(done, 1000));
    }
    throw new Error("Confirmation is delayed; check transaction before retrying: " + signature);
  }) as Connection["confirmTransaction"];
  return rpc;
}

export function wallet() {
  const path = process.env.ANCHOR_WALLET ?? resolve(homedir(), ".config/solana/id.json");
  const signer = Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(path, "utf8"))));
  assert(signer.publicKey.equals(WALLET), "Use the existing course wallet");
  return signer;
}

export function program(rpc: Connection) {
  const idl = JSON.parse(readFileSync(resolve(ROOT, "target/idl/simple_vault.json"), "utf8"));
  assert.equal(idl.address, PROGRAM.toBase58());
  assert(idl.instructions.some((instruction: any) => instruction.name.replace(/_/g, "") === "initializestrategy" || instruction.name === "initializeStrategy"), "Build week-13 first");
  const readOnlyWallet = {
    publicKey: WALLET,
    signTransaction: async <T extends Transaction | VersionedTransaction>(_tx: T): Promise<T> => { throw new Error("Use send() to sign explicitly"); },
    signAllTransactions: async <T extends Transaction | VersionedTransaction>(_tx: T[]): Promise<T[]> => { throw new Error("Use send() to sign explicitly"); },
  };
  return new anchor.Program(idl, new anchor.AnchorProvider(rpc, readOnlyWallet, { commitment: "confirmed" })) as any;
}

export function save(values: Record<string, unknown>) {
  const state = JSON.parse(readFileSync(file, "utf8"));
  Object.assign(state, values);
  writeFileSync(file + ".tmp", JSON.stringify(state, null, 2) + "\n", "utf8");
  renameSync(file + ".tmp", file);
}

export function explorer(id: string | PublicKey, tx = false) {
  return `https://explorer.solana.com/${tx ? "tx" : "address"}/${id}?cluster=devnet`;
}

export async function preflight(rpc: Connection) {
  assert.equal(await rpc.getGenesisHash(), "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG", "Only devnet is allowed");
  const account = await rpc.getAccountInfo(pool);
  assert(account, "Pool account missing");
  assert(account.owner.equals(CLMM), "Wrong CLMM pool owner");
  const decoded = PoolInfoLayout.decode(account.data);
  assert(decoded.mintA.equals(mintA) && decoded.mintB.equals(mintB));
  assert.equal(decoded.vaultA.toBase58(), source.pool.vaultA);
  assert.equal(decoded.vaultB.toBase58(), source.pool.vaultB);
  assert.equal(decoded.configId.toBase58(), source.pool.configId);
  assert.equal(decoded.observationId.toBase58(), source.pool.observationId);
  assert(decoded.liquidity.gt(new BN(0)), "Pool must have active liquidity");
  const [a, b, pa, pb, clmm] = await Promise.all([
    getMint(rpc, mintA), getMint(rpc, mintB), getAccount(rpc, decoded.vaultA),
    getAccount(rpc, decoded.vaultB), rpc.getAccountInfo(CLMM),
  ]);
  assert(a.decimals === 6 && b.decimals === 6);
  assert(pa.owner.equals(pool) && pb.owner.equals(pool));
  assert(pa.mint.equals(mintA) && pb.mint.equals(mintB));
  assert(clmm?.executable);
  return decoded;
}

export async function quote(rpc: Connection, amount: BN, aToB = true) {
  assert(amount.gt(new BN(0)), "Amount must be positive");
  await preflight(rpc);
  const sdk = await Raydium.load({ connection: rpc, owner: WALLET, cluster: "devnet", disableLoadToken: true, disableFeatureCheck: true });
  const info = await sdk.clmm.getPoolInfoFromRpc(pool.toBase58());
  const bitmap = await rpc.getAccountInfo(new PublicKey(source.pool.bitmapExtension));
  assert(bitmap, "Bitmap extension missing");
  assert(bitmap.owner.equals(CLMM));
  const slot = await rpc.getSlot("confirmed");
  const timestamp = await rpc.getBlockTime(slot);
  assert(timestamp !== null, "Pool quote needs chain time");
  const result = PoolUtils.computeAmountOut({
    poolInfo: info.computePoolInfo,
    tickarrayBitmapExtension: TickArrayBitmapExtensionLayout.decode(bitmap.data),
    tickArrayCache: info.tickData[pool.toBase58()],
    baseMint: aToB ? mintA : mintB,
    epochInfo: await rpc.getEpochInfo(), amountIn: amount,
    slippage: 0, catchLiquidityInsufficient: false, blockTimestamp: timestamp,
  });
  assert(result.allTrade, "Insufficient pool liquidity for the full amount");
  const expected = result.amountOut.amount;
  const minimum = expected.muln(99).divn(100);
  assert(minimum.gt(new BN(0)), "Output is too small");
  return { amount, expected, minimum, remaining: result.remainingAccounts,
    priceBefore: info.poolInfo.price.toString(), aToB };
}

export function swapAccounts(admin = WALLET) {
  return {
    vaultState: vault, strategyState: strategy, vaultTokenAccount: vaultA, secondaryVault: vaultB,
    tokenMint: mintA, tokenB: mintB, poolState: pool, ammConfig: new PublicKey(source.pool.configId),
    poolVaultA: new PublicKey(source.pool.vaultA), poolVaultB: new PublicKey(source.pool.vaultB),
    observationState: new PublicKey(source.pool.observationId), clmmProgram: CLMM, admin,
    tokenProgram: TOKEN_PROGRAM_ID, tokenProgram2022: TOKEN_2022_PROGRAM_ID, memoProgram: MEMO,
  };
}

export async function swapInstruction(rpc: Connection, q: Awaited<ReturnType<typeof quote>>, minimum = q.minimum, admin = WALLET, overrides = {}) {
  const remaining = [new PublicKey(source.pool.bitmapExtension), ...q.remaining];
  const unique = [...new Map(remaining.map((key) => [key.toBase58(), key])).values()];
  return program(rpc).methods.swap(q.amount, minimum, q.aToB)
    .accountsStrict({ ...swapAccounts(admin), ...overrides })
    .remainingAccounts(unique.map((pubkey) => ({ pubkey, isSigner: false, isWritable: true }))).instruction() as Promise<TransactionInstruction>;
}

export async function price(rpc: Connection) {
  const info = await preflight(rpc);
  return TickUtil.sqrtPriceX64ToPrice(info.sqrtPriceX64, info.mintDecimalsA, info.mintDecimalsB).toFixed();
}

export async function confirmedTransaction(rpc: Connection, signature: string) {
  for (let attempt = 0; attempt < 8; attempt++) {
    const result = await rpc.getTransaction(signature, { commitment: "confirmed", maxSupportedTransactionVersion: 0 });
    if (result?.meta) return result;
    await new Promise((done) => setTimeout(done, 1000));
  }
  throw new Error("Transaction indexing is delayed; check signature: " + signature);
}

export async function send(rpc: Connection, stage: string, instructions: TransactionInstruction[], signers: Signer[], expectedFailure?: RegExp) {
  assert(/^[a-z0-9-]+$/.test(stage));
  const directory = resolve(ROOT, ".local");
  mkdirSync(directory, { recursive: true });
  const pendingFile = resolve(directory, stage + ".json");
  let pending: any = existsSync(pendingFile) ? JSON.parse(readFileSync(pendingFile, "utf8")) : undefined;
  if (pending) {
    const status = (await rpc.getSignatureStatuses([pending.signature], { searchTransactionHistory: true })).value[0];
    if (status?.confirmationStatus === "confirmed" || status?.confirmationStatus === "finalized") {
      assert.equal(Boolean(status.err), Boolean(expectedFailure), "Previous transaction has unexpected status");
      return pending.signature as string;
    }
    if (await rpc.getBlockHeight("confirmed") > pending.lastValidBlockHeight) {
      assert(!status, "Transaction still processed; retry later");
      unlinkSync(pendingFile);
      pending = undefined;
    }
  }
  if (!pending) {
    const latest = await rpc.getLatestBlockhash("confirmed");
    const legacy = new Transaction({ feePayer: WALLET, recentBlockhash: latest.blockhash })
      .add(ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 10000 }), ...instructions);
    const tx = new VersionedTransaction(legacy.compileMessage());
    tx.sign([...new Map(signers.map((key) => [key.publicKey.toBase58(), key])).values()]);
    assert(tx.serialize().length <= 1232, "Transaction too large");
    const simulation = await rpc.simulateTransaction(tx, { sigVerify: true, commitment: "confirmed" });
    const logs = (simulation.value.logs ?? []).join("\n");
    if (expectedFailure) {
      assert(simulation.value.err && expectedFailure.test(logs), "Expected rejection was not reproduced: " + logs);
    } else {
      assert.equal(simulation.value.err, null, logs);
    }
    pending = { ...latest, signature: bs58.encode(tx.signatures[0]), raw: Buffer.from(tx.serialize()).toString("base64"), expectedFailure: Boolean(expectedFailure) };
    writeFileSync(pendingFile, JSON.stringify(pending), { mode: 0o600 });
  }
  assert.equal(pending.expectedFailure, Boolean(expectedFailure));
  await rpc.sendRawTransaction(Buffer.from(pending.raw, "base64"), {
    skipPreflight: Boolean(expectedFailure), preflightCommitment: "confirmed", maxRetries: 5,
  });
  const result = await rpc.confirmTransaction({ signature: pending.signature, blockhash: pending.blockhash, lastValidBlockHeight: pending.lastValidBlockHeight }, "confirmed");
  assert.equal(Boolean(result.value.err), Boolean(expectedFailure), "Unexpected transaction result: " + JSON.stringify(result.value.err));
  return pending.signature as string;
}
