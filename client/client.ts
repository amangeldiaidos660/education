import * as anchor from "@coral-xyz/anchor";
import * as web3 from "@solana/web3.js";
import type { AnchorCounter } from "../target/types/anchor_counter";

// Configure the client to use the local cluster
anchor.setProvider(anchor.AnchorProvider.env());

const program = anchor.workspace.AnchorCounter as anchor.Program<AnchorCounter>;

console.log("Wallet:", program.provider.publicKey.toString());

const balance = await program.provider.connection.getBalance(program.provider.publicKey);
console.log(`Balance: ${balance / web3.LAMPORTS_PER_SOL} SOL`);

const counter = web3.Keypair.generate();

console.log("Counter account:", counter.publicKey.toString());

// 1. Initialize
const initTx = await program.methods
  .initialize()
  .accounts({
    counter: counter.publicKey,
    signer: program.provider.publicKey,
    systemProgram: web3.SystemProgram.programId,
  })
  .signers([counter])
  .rpc();

console.log("Initialize tx:", initTx);

await program.provider.connection.confirmTransaction(initTx, "finalized");

let counterAccount = await program.account.counter.fetch(counter.publicKey);

console.log("Counter after initialize:", counterAccount.count.toString());

// 2. Increment
const incrementTx = await program.methods
  .increment()
  .accounts({
    counter: counter.publicKey,
  })
  .rpc();

console.log("Increment tx:", incrementTx);

await program.provider.connection.confirmTransaction(incrementTx, "finalized");

counterAccount = await program.account.counter.fetch(counter.publicKey);

console.log("Counter after increment:", counterAccount.count.toString());

// 3. Decrement
const decrementTx = await program.methods
  .decrement()
  .accounts({
    counter: counter.publicKey,
  })
  .rpc();

console.log("Decrement tx:", decrementTx);

await program.provider.connection.confirmTransaction(decrementTx, "finalized");

counterAccount = await program.account.counter.fetch(counter.publicKey);

console.log("Counter after decrement:", counterAccount.count.toString());
