import * as anchor from "@coral-xyz/anchor";
import * as web3 from "@solana/web3.js";
import type { OnchainProfile } from "../target/types/onchain_profile";

// Configure the client to use the local cluster
anchor.setProvider(anchor.AnchorProvider.env());

const program = anchor.workspace.OnchainProfile as anchor.Program<OnchainProfile>;

console.log("Wallet:", program.provider.publicKey.toString());

const balance = await program.provider.connection.getBalance(program.provider.publicKey);
console.log(`Balance: ${balance / web3.LAMPORTS_PER_SOL} SOL`);

const [profilePda, bump] = web3.PublicKey.findProgramAddressSync(
  [Buffer.from("profile"), program.provider.publicKey.toBuffer()],
  program.programId
);

console.log("Profile PDA:", profilePda.toString());
console.log("PDA bump:", bump);

// Проверяем, существует ли PDA
const existingAccount = await program.provider.connection.getAccountInfo(profilePda);

if (!existingAccount) {
  console.log("Profile does not exist. Creating...");

  const createTx = await program.methods
    .createProfile("Aidos", "Solana developer profile")
    .accounts({
      profile: profilePda,
      signer: program.provider.publicKey,
      systemProgram: web3.SystemProgram.programId,
    })
    .rpc();

  console.log("Create profile tx:", createTx);

  await program.provider.connection.confirmTransaction(createTx, "finalized");
} else {
  console.log("Profile already exists.");
}

// Читаем текущий профиль
let profileAccount = await program.account.profile.fetch(profilePda);

console.log("Current profile:");
console.log("Owner:", profileAccount.owner.toString());
console.log("Username:", profileAccount.username);
console.log("Bio:", profileAccount.bio);
console.log("Bump:", profileAccount.bump);

// Обновляем профиль
const updateTx = await program.methods
  .updateProfile("Aidos Updated", "Updated on-chain Solana profile")
  .accounts({
    profile: profilePda,
    signer: program.provider.publicKey,
  })
  .rpc();

console.log("Update profile tx:", updateTx);

await program.provider.connection.confirmTransaction(updateTx, "finalized");

// Читаем обновленный профиль
profileAccount = await program.account.profile.fetch(profilePda);

console.log("Profile after update:");
console.log("Owner:", profileAccount.owner.toString());
console.log("Username:", profileAccount.username);
console.log("Bio:", profileAccount.bio);
console.log("Bump:", profileAccount.bump);
