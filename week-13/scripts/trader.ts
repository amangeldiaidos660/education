import assert from "node:assert/strict";
import {
  createAssociatedTokenAccountIdempotentInstruction, createTransferCheckedInstruction,
  getAccount, getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import { ClmmInstrument } from "@raydium-io/raydium-sdk-v2";
import { Connection, PublicKey, SystemProgram } from "@solana/web3.js";
import BN from "bn.js";
import { CLMM, mintA, mintB, pool, quote, save, send, source, wallet, WALLET } from "./common";
import { budget, localKey } from "./positions";

export function trader() { return localKey("trader"); }
export async function prepareTrader(rpc: Connection) {
  const user = trader();
  const a = getAssociatedTokenAddressSync(mintA, user.publicKey), b = getAssociatedTokenAddressSync(mintB, user.publicKey);
  if (!await rpc.getAccountInfo(a)) {
    const instructions = [
      budget(), SystemProgram.transfer({ fromPubkey: WALLET, toPubkey: user.publicKey, lamports: 20_000_000 }),
      createAssociatedTokenAccountIdempotentInstruction(WALLET, a, user.publicKey, mintA),
      createAssociatedTokenAccountIdempotentInstruction(WALLET, b, user.publicKey, mintB),
      createTransferCheckedInstruction(new PublicKey(source.tokenA.tokenAccount), mintA, a, WALLET, 200_000_000, 6),
      createTransferCheckedInstruction(new PublicKey(source.tokenB.tokenAccount), mintB, b, WALLET, 200_000_000, 6),
    ];
    const signature = await send(rpc, "prepare-trader", instructions, [wallet()]);
    save({ trader: { wallet: user.publicKey.toBase58(), tokenA: a.toBase58(), tokenB: b.toBase58(), setupSignature: signature } });
  }
  return { user, a, b };
}

export async function traderSwap(rpc: Connection, amount: BN, aToB: boolean, label: string) {
  const { user, a, b } = await prepareTrader(rpc);
  const before = await getAccount(rpc, aToB ? b : a);
  const q = await quote(rpc, amount, aToB);
  const ix = ClmmInstrument.swapV2Instruction(CLMM, user.publicKey, pool,
    new PublicKey(source.pool.configId), aToB ? a : b, aToB ? b : a,
    new PublicKey(aToB ? source.pool.vaultA : source.pool.vaultB),
    new PublicKey(aToB ? source.pool.vaultB : source.pool.vaultA),
    aToB ? mintA : mintB, aToB ? mintB : mintA, q.remaining,
    new PublicKey(source.pool.observationId), amount, q.minimum, new BN(0), true,
    new PublicKey(source.pool.bitmapExtension));
  const signature = await send(rpc, "trader-" + label, [budget(), ix], [wallet(), user]);
  const after = await getAccount(rpc, aToB ? b : a);
  assert(after.amount - before.amount >= BigInt(q.minimum.toString()));
  return { signature, aToB, amountIn: amount.toString(), amountOut: (after.amount - before.amount).toString(), trader: user.publicKey.toBase58() };
}
