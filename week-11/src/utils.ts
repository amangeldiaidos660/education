import assert from "node:assert/strict";
import { existsSync, mkdirSync, readFileSync, renameSync, unlinkSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { createHash } from "node:crypto";
import {
  ApiV3Token,
  getPdaPersonalPositionAddress,
  getPdaPoolId,
  getPdaTickArrayAddress,
  PersonalPositionLayout,
  PoolInfoLayout,
  TickArrayLayout,
  TickArrayUtil,
  TickUtil,
} from "@raydium-io/raydium-sdk-v2";
import { getAccount, getAssociatedTokenAddressSync, getMint, TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { Connection, Keypair, PublicKey, Signer, Transaction, VersionedTransaction } from "@solana/web3.js";
import Decimal from "decimal.js";
import BN from "bn.js";
import bs58 from "bs58";
import { assertDevnet, CLMM_PROGRAM, DECIMALS, TOKEN_SUPPLY, WALLET } from "./config";

export interface TokenRecord {
  mint: string;
  tokenAccount: string;
  decimals: number;
  supply: string;
  signature?: string;
}

export interface PoolRecord {
  id: string;
  configId: string;
  tradeFeeRate: number;
  tickSpacing: number;
  vaultA: string;
  vaultB: string;
  observationId: string;
  bitmapExtension: string;
  initialPrice: string;
  signature?: string;
}

export interface PositionRecord {
  nftMint: string;
  nftAccount: string;
  personalPosition: string;
  protocolPosition: string;
  tickArrayLower: string;
  tickArrayUpper: string;
  tickLower: number;
  tickUpper: number;
  lowerPrice: string;
  upperPrice: string;
  baseAmountA: string;
  maxAmountB: string;
  liquidity?: string;
  signature?: string;
}

export interface State {
  schemaVersion: 1;
  network: "devnet";
  wallet: string;
  clmmProgramId: string;
  tokenA?: TokenRecord;
  tokenB?: TokenRecord;
  pool?: PoolRecord;
  position?: PositionRecord;
}

interface Pending {
  signature: string;
  blockhash: string;
  lastValidBlockHeight: number;
  raw: string;
}

export const ROOT = resolve(__dirname, "..").endsWith("dist")
  ? resolve(__dirname, "..", "..")
  : resolve(__dirname, "..");
const stateFile = resolve(ROOT, "addresses.devnet.json");
const localDir = resolve(ROOT, ".local");

export function writeJson(file: string, value: unknown, privateFile = false) {
  mkdirSync(dirname(file), { recursive: true });
  const temporary = file + ".tmp";
  writeFileSync(temporary, JSON.stringify(value, null, 2) + "\n", { mode: privateFile ? 0o600 : 0o644 });
  renameSync(temporary, file);
}

export function readState(): State {
  const state: State = existsSync(stateFile)
    ? JSON.parse(readFileSync(stateFile, "utf8"))
    : { schemaVersion: 1, network: "devnet", wallet: WALLET.toBase58(), clmmProgramId: CLMM_PROGRAM.toBase58() };
  assert.equal(state.schemaVersion, 1);
  assert.equal(state.network, "devnet");
  assert.equal(state.wallet, WALLET.toBase58());
  assert.equal(state.clmmProgramId, CLMM_PROGRAM.toBase58());
  return state;
}

export function saveState(state: State) {
  writeJson(stateFile, state);
}

export function localKey(name: string) {
  assert(/^[a-z0-9-]+$/.test(name));
  const file = resolve(ROOT, ".keys", name + "-keypair.json");
  if (!existsSync(file)) {
    const generated = Keypair.generate();
    mkdirSync(dirname(file), { recursive: true });
    writeFileSync(file, JSON.stringify([...generated.secretKey]) + "\n", { flag: "wx", mode: 0o600 });
    return generated;
  }
  return Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(file, "utf8"))));
}

export function canonicalMints(first: PublicKey, second: PublicKey) {
  assert(!first.equals(second), "Pool requires different mints");
  return Buffer.compare(first.toBuffer(), second.toBuffer()) < 0 ? [first, second] : [second, first];
}

export function positionRange(lower: string, upper: string, decimalsA: number, decimalsB: number, spacing: number) {
  const low = new Decimal(lower);
  const high = new Decimal(upper);
  assert(low.isFinite() && high.isFinite() && low.gt(0) && high.gt(low), "Invalid price range");
  assert(Number.isInteger(spacing) && spacing > 0, "Invalid tick spacing");
  let tickLower = TickUtil.toTickIndex(TickUtil.priceToTick(low, decimalsA, decimalsB), spacing);
  let tickUpper = TickUtil.toTickIndex(TickUtil.priceToTick(high, decimalsA, decimalsB), spacing);
  if (TickUtil.tickToPrice(tickLower, decimalsA, decimalsB).gt(low)) tickLower -= spacing;
  if (TickUtil.tickToPrice(tickUpper, decimalsA, decimalsB).lt(high)) tickUpper += spacing;
  assert(tickLower >= -443636 && tickUpper <= 443636 && tickLower < tickUpper, "Tick range is out of bounds");
  return {
    tickLower,
    tickUpper,
    lowerPrice: TickUtil.tickToPrice(tickLower, decimalsA, decimalsB).toFixed(),
    upperPrice: TickUtil.tickToPrice(tickUpper, decimalsA, decimalsB).toFixed(),
  };
}

export function maxWithSlippage(amount: bigint, bps: number) {
  assert(amount >= BigInt(0) && Number.isInteger(bps) && bps >= 0 && bps <= 10000);
  return (amount * BigInt(10000 + bps) + BigInt(9999)) / BigInt(10000);
}

export function explorer(address: string, transaction = false) {
  return "https://explorer.solana.com/" + (transaction ? "tx/" : "address/") + address + "?cluster=devnet";
}

export function apiToken(token: TokenRecord, label: string): ApiV3Token {
  return {
    chainId: 103,
    address: token.mint,
    programId: TOKEN_PROGRAM_ID.toBase58(),
    logoURI: "",
    symbol: label,
    name: "Week 11 " + label,
    decimals: token.decimals,
    tags: [],
    extensions: {},
  };
}

export async function verifyToken(rpc: Connection, token: TokenRecord) {
  assert.equal(token.decimals, DECIMALS);
  assert.equal(token.supply, TOKEN_SUPPLY.toString());
  assert.equal(getAssociatedTokenAddressSync(new PublicKey(token.mint), WALLET).toBase58(), token.tokenAccount);
  const mint = await getMint(rpc, new PublicKey(token.mint));
  const account = await getAccount(rpc, new PublicKey(token.tokenAccount));
  assert.equal(mint.decimals, DECIMALS);
  assert.equal(mint.supply.toString(), TOKEN_SUPPLY.toString());
  assert(mint.mintAuthority?.equals(WALLET));
  assert.equal(mint.freezeAuthority, null);
  assert(account.owner.equals(WALLET) && account.mint.equals(new PublicKey(token.mint)));
  return { mint, account };
}

export async function verifyPool(rpc: Connection, state: State) {
  assert(state.pool && state.tokenA && state.tokenB, "Create both tokens and pool first");
  const poolKey = new PublicKey(state.pool.id);
  assert(getPdaPoolId(
    CLMM_PROGRAM, new PublicKey(state.pool.configId), new PublicKey(state.tokenA.mint), new PublicKey(state.tokenB.mint)
  ).publicKey.equals(poolKey), "Invalid pool PDA");
  const info = await rpc.getAccountInfo(poolKey);
  assert(info, "Pool account is missing");
  assert(info.owner.equals(CLMM_PROGRAM), "Invalid pool owner");
  const discriminator = createHash("sha256").update("account:PoolState").digest().subarray(0, 8);
  assert(info.data.subarray(0, 8).equals(discriminator), "Invalid pool discriminator");
  const pool = PoolInfoLayout.decode(info.data);
  assert(pool.mintA.equals(new PublicKey(state.tokenA.mint)) && pool.mintB.equals(new PublicKey(state.tokenB.mint)));
  assert(pool.configId.equals(new PublicKey(state.pool.configId)));
  assert(pool.creator.equals(WALLET), "Unexpected pool creator");
  assert.equal(pool.vaultA.toBase58(), state.pool.vaultA);
  assert.equal(pool.vaultB.toBase58(), state.pool.vaultB);
  assert.equal(pool.observationId.toBase58(), state.pool.observationId);
  assert.equal(pool.tickSpacing, state.pool.tickSpacing);
  assert(pool.sqrtPriceX64.gt(new BN(0)));
  const [vaultA, vaultB] = await Promise.all([
    getAccount(rpc, pool.vaultA), getAccount(rpc, pool.vaultB),
  ]);
  assert(vaultA.owner.equals(poolKey) && vaultB.owner.equals(poolKey));
  assert(vaultA.mint.equals(pool.mintA) && vaultB.mint.equals(pool.mintB));
  const observation = await rpc.getAccountInfo(pool.observationId);
  assert(observation?.owner.equals(CLMM_PROGRAM), "Observation account is missing");
  return { pool, vaultA, vaultB };
}

export async function verifyPosition(rpc: Connection, state: State) {
  assert(state.pool && state.position, "Open a position first");
  const position = state.position;
  assert.equal(
    getPdaPersonalPositionAddress(CLMM_PROGRAM, new PublicKey(position.nftMint)).publicKey.toBase58(),
    position.personalPosition,
    "Invalid personal position PDA"
  );
  const info = await rpc.getAccountInfo(new PublicKey(position.personalPosition));
  assert(info, "Personal position is missing");
  assert(info.owner.equals(CLMM_PROGRAM), "Invalid position owner");
  const discriminator = createHash("sha256").update("account:PersonalPositionState").digest().subarray(0, 8);
  assert(info.data.subarray(0, 8).equals(discriminator), "Invalid position discriminator");
  const decoded = PersonalPositionLayout.decode(info.data);
  assert.equal(decoded.poolId.toBase58(), state.pool.id);
  assert.equal(decoded.nftMint.toBase58(), position.nftMint);
  assert.equal(decoded.tickLower, position.tickLower);
  assert.equal(decoded.tickUpper, position.tickUpper);
  assert(!decoded.liquidity.isZero(), "Position has no liquidity");
  for (const [tick, address] of [
    [position.tickLower, position.tickArrayLower], [position.tickUpper, position.tickArrayUpper],
  ] as const) {
    const start = TickArrayUtil.getTickArrayStartIndex(tick, state.pool.tickSpacing);
    assert.equal(
      getPdaTickArrayAddress(CLMM_PROGRAM, new PublicKey(state.pool.id), start).publicKey.toBase58(), address
    );
    const array = await rpc.getAccountInfo(new PublicKey(address));
    assert(array, "Tick array is missing");
    assert(array.owner.equals(CLMM_PROGRAM), "Invalid tick array owner");
    assert.equal(TickArrayLayout.decode(array.data).poolId.toBase58(), state.pool.id);
  }
  const mint = await getMint(rpc, new PublicKey(position.nftMint), "confirmed", TOKEN_2022_PROGRAM_ID);
  const nft = await getAccount(rpc, new PublicKey(position.nftAccount), "confirmed", TOKEN_2022_PROGRAM_ID);
  assert.equal(mint.decimals, 0);
  assert.equal(mint.supply.toString(), "1");
  assert(nft.owner.equals(WALLET) && nft.mint.equals(new PublicKey(position.nftMint)) && nft.amount === BigInt(1));
  return decoded;
}

async function settle(rpc: Connection, pending: Pending) {
  let status = (await rpc.getSignatureStatuses([pending.signature], { searchTransactionHistory: true })).value[0];
  if (status?.err) throw new Error("Transaction failed: " + pending.signature + " " + JSON.stringify(status.err));
  if (status?.confirmationStatus === "confirmed" || status?.confirmationStatus === "finalized") return pending.signature;
  const height = await rpc.getBlockHeight("confirmed");
  if (height > pending.lastValidBlockHeight) {
    assert(!status, "Transaction is processed but not confirmed; wait and rerun the same command");
    return undefined;
  }
  await rpc.sendRawTransaction(Buffer.from(pending.raw, "base64"), {
    skipPreflight: false, preflightCommitment: "confirmed", maxRetries: 3,
  });
  const result = await rpc.confirmTransaction({
    signature: pending.signature,
    blockhash: pending.blockhash,
    lastValidBlockHeight: pending.lastValidBlockHeight,
  }, "confirmed");
  assert.equal(result.value.err, null, "Transaction failed: " + JSON.stringify(result.value.err));
  status = (await rpc.getSignatureStatuses([pending.signature], { searchTransactionHistory: true })).value[0];
  assert(status && !status.err, "Transaction status is unavailable");
  return pending.signature;
}

export async function resume(rpc: Connection, stage: string) {
  assert(/^[a-z0-9-]+$/.test(stage));
  const file = resolve(localDir, stage + ".json");
  if (!existsSync(file)) return undefined;
  const pending: Pending = JSON.parse(readFileSync(file, "utf8"));
  const status = (await rpc.getSignatureStatuses([pending.signature], { searchTransactionHistory: true })).value[0];
  if (status?.err) {
    unlinkSync(file);
    console.log("Previous transaction failed; rebuilding the same stage:", pending.signature);
    return undefined;
  }
  const signature = await settle(rpc, pending);
  if (!signature) unlinkSync(file);
  return signature;
}

export async function submit(
  rpc: Connection,
  stage: string,
  transaction: Transaction | VersionedTransaction,
  signers: Signer[],
  owner: Keypair,
  onSigned: (signature: string) => void
) {
  const latest = await rpc.getLatestBlockhash("confirmed");
  let tx: VersionedTransaction;
  if (transaction instanceof Transaction) {
    transaction.feePayer = owner.publicKey;
    transaction.recentBlockhash = latest.blockhash;
    tx = new VersionedTransaction(transaction.compileMessage());
  } else {
    transaction.message.recentBlockhash = latest.blockhash;
    tx = transaction;
  }
  const unique = [...new Map([owner, ...signers].map((signer) => [signer.publicKey.toBase58(), signer])).values()];
  tx.sign(unique);
  const raw = tx.serialize();
  assert(raw.length <= 1232, "Transaction exceeds Solana packet size");
  const simulated = await rpc.simulateTransaction(tx, { sigVerify: true, commitment: "confirmed" });
  if (simulated.value.err) {
    throw new Error("Simulation failed: " + JSON.stringify(simulated.value.err) + "\n" + (simulated.value.logs ?? []).join("\n"));
  }
  const signature = bs58.encode(tx.signatures[0]);
  const pending: Pending = { ...latest, signature, raw: Buffer.from(raw).toString("base64") };
  writeJson(resolve(localDir, stage + ".json"), pending, true);
  onSigned(signature);
  const confirmed = await settle(rpc, pending);
  assert(confirmed, "Blockhash expired before sending; rerun the same command");
  return confirmed;
}

export async function run(action: (rpc: Connection) => Promise<void>) {
  const { connection } = await import("./config");
  const rpc = connection();
  let locked = false;
  const lock = resolve(localDir, "run.lock");
  try {
    await assertDevnet(rpc);
    mkdirSync(localDir, { recursive: true });
    if (existsSync(lock)) {
      const pid = Number(readFileSync(lock, "utf8"));
      assert(Number.isSafeInteger(pid) && pid > 0, "Invalid lock file");
      let active = true;
      try { process.kill(pid, 0); } catch (error) {
        if ((error as NodeJS.ErrnoException).code === "ESRCH") active = false;
      }
      assert(!active, "Another week-11 command is running");
      unlinkSync(lock);
    }
    writeFileSync(lock, String(process.pid), { flag: "wx" });
    locked = true;
    await action(rpc);
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  } finally {
    if (locked) unlinkSync(lock);
  }
}
