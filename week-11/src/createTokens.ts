import assert from "node:assert/strict";
import {
  createAssociatedTokenAccountIdempotentInstruction,
  createInitializeMint2Instruction,
  createMintToCheckedInstruction,
  getAssociatedTokenAddressSync,
  MINT_SIZE,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { ComputeBudgetProgram, PublicKey, SystemProgram, Transaction } from "@solana/web3.js";
import { DECIMALS, loadWallet, TOKEN_SUPPLY, WALLET } from "./config";
import {
  canonicalMints, explorer, localKey, readState, resume, run, saveState, submit, TokenRecord, verifyToken,
} from "./utils";

run(async (rpc) => {
  const wallet = loadWallet();
  const state = readState();
  let keys: ReturnType<typeof localKey>[] = [];
  if (!state.tokenA || !state.tokenB) {
    assert(!state.tokenA && !state.tokenB, "Token records are incomplete; restore the original state file");
    assert(!state.pool && !state.position, "Existing pool requires its original token addresses");
    keys = [localKey("token-1"), localKey("token-2")];
    const mints = canonicalMints(keys[0].publicKey, keys[1].publicKey);
    const record = (mint: PublicKey): TokenRecord => ({
      mint: mint.toBase58(),
      tokenAccount: getAssociatedTokenAddressSync(mint, WALLET).toBase58(),
      decimals: DECIMALS,
      supply: TOKEN_SUPPLY.toString(),
    });
    state.tokenA = record(mints[0]);
    state.tokenB = record(mints[1]);
    saveState(state);
  }
  for (const [label, token] of [["token-a", state.tokenA], ["token-b", state.tokenB]] as const) {
    assert(token);
    const resumed = await resume(rpc, label);
    if (resumed) token.signature = resumed;
    const mint = new PublicKey(token.mint);
    if (!await rpc.getAccountInfo(mint)) {
      if (keys.length === 0) keys = [localKey("token-1"), localKey("token-2")];
      const signer = keys.find((key) => key.publicKey.equals(mint));
      assert(signer, "Original mint key is missing; do not replace the saved token addresses");
      const ata = getAssociatedTokenAddressSync(mint, WALLET);
      assert.equal(token.tokenAccount, ata.toBase58());
      const transaction = new Transaction().add(
        ComputeBudgetProgram.setComputeUnitLimit({ units: 200000 }),
        ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 10000 }),
        SystemProgram.createAccount({
          fromPubkey: WALLET,
          newAccountPubkey: mint,
          lamports: await rpc.getMinimumBalanceForRentExemption(MINT_SIZE),
          space: MINT_SIZE,
          programId: TOKEN_PROGRAM_ID,
        }),
        createInitializeMint2Instruction(mint, DECIMALS, WALLET, null),
        createAssociatedTokenAccountIdempotentInstruction(WALLET, ata, WALLET, mint),
        createMintToCheckedInstruction(mint, ata, WALLET, TOKEN_SUPPLY, DECIMALS),
      );
      token.signature = await submit(rpc, label, transaction, [signer], wallet, (signature) => {
        token.signature = signature;
        saveState(state);
      });
    }
    const { account } = await verifyToken(rpc, token);
    saveState(state);
    console.log(label, explorer(token.mint));
    console.log("Wallet token balance:", account.amount.toString(), "base units");
    if (token.signature) console.log("Creation:", explorer(token.signature, true));
  }
});
