import assert from "node:assert/strict";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import {
  getPdaPersonalPositionAddress, getPdaProtocolPositionAddress, getPdaTickArrayAddress,
  LiquidityMathUtil, PersonalPositionLayout, PoolInfoLayout, TickArrayUtil, TickUtil,
} from "@raydium-io/raydium-sdk-v2";
import {
  ASSOCIATED_TOKEN_PROGRAM_ID, getAccount, getAssociatedTokenAddressSync, getMint,
  TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { ComputeBudgetProgram, Connection, Keypair, PublicKey, SystemProgram, SYSVAR_RENT_PUBKEY, Transaction, VersionedTransaction } from "@solana/web3.js";
import BN from "bn.js";
import Decimal from "decimal.js";
import {
  CLMM, file, MEMO, mintA, mintB, pool, positionControl, preflight, price, program,
  ROOT, save, send, source, strategy, swapInstruction, vault, vaultA, vaultB, wallet, WALLET,
} from "./common";

export const budget = () => ComputeBudgetProgram.setComputeUnitLimit({ units: 1000000 });
export function loadState(): any { return JSON.parse(readFileSync(file, "utf8")); }
export function localKey(label: string) {
  assert(/^[a-z0-9-]+$/.test(label));
  const directory = resolve(ROOT, ".keys");
  mkdirSync(directory, { recursive: true });
  const path = resolve(directory, label + "-keypair.json");
  if (existsSync(path)) return Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(path, "utf8"))));
  const key = Keypair.generate();
  writeFileSync(path, JSON.stringify([...key.secretKey]), { flag: "wx", mode: 0o600 });
  return key;
}

export function rangeAround(current: string, percent = "0.05", spacing = 60) {
  const center = new Decimal(current), delta = new Decimal(percent);
  assert(center.isPositive() && delta.gt(0) && delta.lt(1));
  const lower = center.mul(new Decimal(1).sub(delta)), upper = center.mul(new Decimal(1).add(delta));
  const tickLower = TickUtil.toTickIndex(TickUtil.priceToTick(lower, 6, 6), spacing);
  let tickUpper = TickUtil.toTickIndex(TickUtil.priceToTick(upper, 6, 6), spacing);
  if (TickUtil.tickToPrice(tickUpper, 6, 6).lt(upper)) tickUpper += spacing;
  return { tickLower, tickUpper, lowerPrice: TickUtil.tickToPrice(tickLower, 6, 6).toFixed(),
    upperPrice: TickUtil.tickToPrice(tickUpper, 6, 6).toFixed() };
}

export function positionAddresses(nft: PublicKey, lower: number, upper: number) {
  const lowerStart = TickArrayUtil.getTickArrayStartIndex(lower, 60);
  const upperStart = TickArrayUtil.getTickArrayStartIndex(upper, 60);
  return { nftMint: nft.toBase58(), nftAccount: getAssociatedTokenAddressSync(nft, vault, true, TOKEN_2022_PROGRAM_ID).toBase58(),
    personalPosition: getPdaPersonalPositionAddress(CLMM, nft).publicKey.toBase58(),
    protocolPosition: getPdaProtocolPositionAddress(CLMM, pool, lower, upper).publicKey.toBase58(),
    tickArrayLower: getPdaTickArrayAddress(CLMM, pool, lowerStart).publicKey.toBase58(),
    tickArrayUpper: getPdaTickArrayAddress(CLMM, pool, upperStart).publicKey.toBase58(), lowerStart, upperStart };
}

export function positionAccounts(record: any, admin = WALLET) {
  return { vaultState: vault, strategyState: strategy, positionControl, vaultTokenAccount: vaultA,
    secondaryVault: vaultB, tokenMint: mintA, tokenB: mintB, poolState: pool,
    poolVaultA: new PublicKey(source.pool.vaultA), poolVaultB: new PublicKey(source.pool.vaultB),
    nftMint: new PublicKey(record.nftMint), nftAccount: new PublicKey(record.nftAccount),
    personalPosition: new PublicKey(record.personalPosition), protocolPosition: new PublicKey(record.protocolPosition),
    tickArrayLower: new PublicKey(record.tickArrayLower), tickArrayUpper: new PublicKey(record.tickArrayUpper),
    clmmProgram: CLMM, admin, tokenProgram: TOKEN_PROGRAM_ID, tokenProgram2022: TOKEN_2022_PROGRAM_ID,
    systemProgram: SystemProgram.programId };
}

export function extra() { return [{ pubkey: new PublicKey(source.pool.bitmapExtension), isSigner: false, isWritable: true }]; }
export function manage(rpc: Connection, record: any, admin = WALLET, overrides = {}) {
  return { ...positionAccounts(record, admin), memoProgram: MEMO, ...overrides };
}

export async function openInstruction(rpc: Connection, record: any, admin = WALLET, overrides = {}) {
  return program(rpc).methods.openPosition(record.tickLower, record.tickUpper, record.lowerStart, record.upperStart,
    new BN(record.liquidity), new BN(record.maximumA), new BN(record.maximumB))
    .accountsStrict({ ...positionAccounts(record, admin), associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      rent: SYSVAR_RENT_PUBKEY, ...overrides }).remainingAccounts(extra()).instruction();
}

export async function balances(rpc: Connection) {
  const [a, b] = await Promise.all([getAccount(rpc, vaultA), getAccount(rpc, vaultB)]);
  return { a: a.amount, b: b.amount, delegateA: a.delegate?.toBase58() ?? null, delegateB: b.delegate?.toBase58() ?? null };
}

export async function planOpen(rpc: Connection, label: string) {
  const decoded = await preflight(rpc);
  const current = await price(rpc);
  const range = rangeAround(current);
  const balance = await balances(rpc);
  const a = new BN((balance.a / BigInt(2)).toString()), b = new BN((balance.b / BigInt(2)).toString());
  const cap = new BN(5_000_000);
  const maximumA = BN.min(a, cap), maximumB = BN.min(b, cap);
  assert(maximumA.gtn(1000) && maximumB.gtn(1000), "Vault needs both tokens before opening a position");
  const liquidity = LiquidityMathUtil.getLiquidityFromAmounts(decoded.sqrtPriceX64,
    TickUtil.getSqrtPriceAtTick(range.tickLower), TickUtil.getSqrtPriceAtTick(range.tickUpper), maximumA, maximumB).muln(99).divn(100);
  assert(liquidity.gt(new BN(0)));
  const key = localKey("position-" + label);
  return { ...positionAddresses(key.publicKey, range.tickLower, range.tickUpper), ...range,
    label, liquidity: liquidity.toString(), maximumA: maximumA.toString(), maximumB: maximumB.toString(), priceAtOpen: current };
}

export function recordCycle(record: any) {
  const state = loadState();
  save({ vaultPosition: record, positionCycles: { ...(state.positionCycles ?? {}), [record.label]: record } });
}

export async function verifyPosition(rpc: Connection, record: any) {
  const app = program(rpc);
  const control = await app.account.positionControl.fetch(positionControl);
  assert(control.active && control.vault.equals(vault));
  assert.equal(control.nftMint.toBase58(), record.nftMint);
  assert.equal(control.personalPosition.toBase58(), record.personalPosition);
  const info = await rpc.getAccountInfo(new PublicKey(record.personalPosition));
  assert(info?.owner.equals(CLMM));
  const position = PersonalPositionLayout.decode(info!.data);
  assert(position.poolId.equals(pool) && position.nftMint.equals(new PublicKey(record.nftMint)));
  assert.equal(position.tickLower, record.tickLower); assert.equal(position.tickUpper, record.tickUpper);
  const nft = await getAccount(rpc, new PublicKey(record.nftAccount), "confirmed", TOKEN_2022_PROGRAM_ID);
  const mint = await getMint(rpc, new PublicKey(record.nftMint), "confirmed", TOKEN_2022_PROGRAM_ID);
  assert(nft.owner.equals(vault) && nft.amount === BigInt(1) && mint.supply === BigInt(1));
  assert.equal(mint.decimals, 0);
  const balance = await balances(rpc);
  assert.equal(balance.delegateA, null); assert.equal(balance.delegateB, null);
  return position;
}

export async function open(rpc: Connection, record: any) {
  const control = await rpc.getAccountInfo(positionControl);
  if (control && (await program(rpc).account.positionControl.fetch(positionControl)).active) {
    await verifyPosition(rpc, record); return record;
  }
  recordCycle(record);
  const before = await balances(rpc);
  const signature = await send(rpc, "open-" + record.label, [budget(), await openInstruction(rpc, record)],
    [wallet(), localKey("position-" + record.label)]);
  const position = await verifyPosition(rpc, record), after = await balances(rpc);
  assert.equal(position.liquidity.toString(), record.liquidity);
  const spentA = before.a - after.a, spentB = before.b - after.b;
  assert(spentA <= BigInt(record.maximumA) && spentB <= BigInt(record.maximumB));
  Object.assign(record, { openSignature: signature, spentA: spentA.toString(), spentB: spentB.toString(), active: true });
  recordCycle(record); return record;
}

export async function collect(rpc: Connection, record: any, stage: string) {
  const before = await balances(rpc), position = await verifyPosition(rpc, record);
  const instruction = await program(rpc).methods.collectFees().accountsStrict(manage(rpc, record)).remainingAccounts(extra()).instruction();
  const signature = await send(rpc, "fees-" + stage, [budget(), instruction], [wallet()]);
  const after = await balances(rpc), updated = await verifyPosition(rpc, record);
  assert(position.liquidity.eq(updated.liquidity));
  const feeA = after.a - before.a, feeB = after.b - before.b;
  Object.assign(record, { collectSignature: signature, collectedA: feeA.toString(), collectedB: feeB.toString() });
  recordCycle(record); return { feeA, feeB, signature };
}

export async function closeQuote(rpc: Connection, record: any) {
  const position = await verifyPosition(rpc, record), current = await preflight(rpc);
  const amounts = LiquidityMathUtil.getAmountsForLiquidity(current.sqrtPriceX64,
    TickUtil.getSqrtPriceAtTick(record.tickLower), TickUtil.getSqrtPriceAtTick(record.tickUpper), position.liquidity, false);
  return { minimumA: amounts.amountA.muln(99).divn(100), minimumB: amounts.amountB.muln(99).divn(100) };
}

export async function close(rpc: Connection, record: any) {
  const before = await balances(rpc), limits = await closeQuote(rpc, record);
  const instruction = await program(rpc).methods.closePosition(limits.minimumA, limits.minimumB)
    .accountsStrict(manage(rpc, record)).remainingAccounts(extra()).instruction();
  const signature = await send(rpc, "close-" + record.label, [budget(), instruction], [wallet()]);
  const after = await balances(rpc);
  assert(!(await program(rpc).account.positionControl.fetch(positionControl)).active);
  assert.equal(await rpc.getAccountInfo(new PublicKey(record.personalPosition)), null);
  const nft = await rpc.getAccountInfo(new PublicKey(record.nftAccount));
  assert(nft === null || getAccountAmount(nft.data) === BigInt(0));
  Object.assign(record, { closeSignature: signature, returnedA: (after.a - before.a).toString(), returnedB: (after.b - before.b).toString(),
    minimumA: limits.minimumA.toString(), minimumB: limits.minimumB.toString(), active: false });
  recordCycle(record); return record;
}

function getAccountAmount(data: Buffer) { return data.readBigUInt64LE(64); }

export async function simulateReject(rpc: Connection, instruction: any, pattern: RegExp) {
  const { blockhash } = await rpc.getLatestBlockhash("confirmed");
  const tx = new VersionedTransaction(new Transaction({ feePayer: WALLET, recentBlockhash: blockhash }).add(budget(), instruction).compileMessage());
  const result = await rpc.simulateTransaction(tx, { sigVerify: false, commitment: "confirmed" });
  assert(result.value.err); assert.match((result.value.logs ?? []).join("\n"), pattern);
}
