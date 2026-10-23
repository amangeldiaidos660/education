import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import {
  createMint,
  getAccount,
  getMint,
  getOrCreateAssociatedTokenAccount,
  mintTo,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import {
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
  Transaction,
} from "@solana/web3.js";
import { assert } from "chai";
import { SimpleVault } from "../target/types/simple_vault";

describe("week-10: deposit regression", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.SimpleVault as Program<SimpleVault>;
  const admin = (provider.wallet as anchor.Wallet).payer;
  const userB = Keypair.generate();
  const unit = 1_000_000;
  const rpcHost = new URL(provider.connection.rpcEndpoint).hostname;
  const isDevnet = rpcHost === "api.devnet.solana.com";

  if (!isDevnet && !["localhost", "127.0.0.1", "::1", "[::1]"].includes(rpcHost)) {
    throw new Error("Only localnet and devnet are allowed");
  }
  if (
    program.programId.equals(SystemProgram.programId) ||
    program.programId.toBase58() === "BSBHGpeTSSp3jEHdhaWUnBPVTgBok63je37evskejiWh"
  ) {
    throw new Error("Use the existing vault program ID configured for week-10");
  }

  let tokenMint: PublicKey;
  let vaultState: PublicKey;
  let vaultTokenAccount: PublicKey;
  let shareMint: PublicKey;
  let userCardA: PublicKey;
  let userCardB: PublicKey;
  let tokensA: PublicKey;
  let tokensB: PublicKey;
  let sharesA: PublicKey;
  let sharesB: PublicKey;

  const cardPda = (user: PublicKey) =>
    PublicKey.findProgramAddressSync(
      [Buffer.from("user_card"), vaultState.toBuffer(), user.toBuffer()],
      program.programId
    )[0];

  const accountsFor = (user: Keypair) => ({
    vaultState,
    userCard: user.publicKey.equals(admin.publicKey) ? userCardA : userCardB,
    vaultTokenAccount,
    shareMint,
    tokenMint,
    userTokenAccount: user.publicKey.equals(admin.publicKey) ? tokensA : tokensB,
    userShareAccount: user.publicKey.equals(admin.publicKey) ? sharesA : sharesB,
    user: user.publicKey,
    tokenProgram: TOKEN_PROGRAM_ID,
  });

  const deposit = (
    user: Keypair,
    amount: number,
    overrides: Record<string, PublicKey> = {}
  ) =>
    program.methods
      .deposit(new anchor.BN(amount))
      .accountsStrict({ ...accountsFor(user), ...overrides })
      .signers(user.publicKey.equals(admin.publicKey) ? [] : [user])
      .rpc();

  async function snapshot() {
    const addresses = [
      vaultTokenAccount, tokensA, tokensB, sharesA, sharesB,
      shareMint, vaultState, userCardA, userCardB,
    ];
    const accounts = await provider.connection.getMultipleAccountsInfo(addresses);
    return accounts.map((account) => {
      assert.exists(account, "snapshot account must exist");
      return account!.data.toString("base64");
    });
  }

  async function expectFailure(
    action: () => Promise<string>,
    expectedCode: string
  ) {
    const before = await snapshot();
    let caught: any;
    try {
      await action();
    } catch (error) {
      caught = error;
    }
    assert.exists(caught, "transaction must fail");
    assert.equal(caught.error?.errorCode?.code, expectedCode);
    assert.deepEqual(await snapshot(), before, "failed deposit must not change balances or accounting");
  }

  async function expectDeposit(user: Keypair, amount: number, expectedShares: number) {
    const accounts = accountsFor(user);
    const before = {
      tokens: (await getAccount(provider.connection, accounts.userTokenAccount)).amount,
      shares: (await getAccount(provider.connection, accounts.userShareAccount)).amount,
      vault: (await getAccount(provider.connection, vaultTokenAccount)).amount,
      supply: (await getMint(provider.connection, shareMint)).supply,
      deposited: (
        await program.account.userCard.fetch(accounts.userCard)
      ).depositedAmount,
    };
    const signature = await deposit(user, amount);
    assert.equal(
      (await getAccount(provider.connection, accounts.userTokenAccount)).amount.toString(),
      (before.tokens - BigInt(amount)).toString()
    );
    assert.equal(
      (await getAccount(provider.connection, vaultTokenAccount)).amount.toString(),
      (before.vault + BigInt(amount)).toString()
    );
    assert.equal(
      (await getAccount(provider.connection, accounts.userShareAccount)).amount.toString(),
      (before.shares + BigInt(expectedShares)).toString()
    );
    assert.equal(
      (await getMint(provider.connection, shareMint)).supply.toString(),
      (before.supply + BigInt(expectedShares)).toString()
    );
    const vault = await program.account.vaultState.fetch(vaultState);
    const card = await program.account.userCard.fetch(accounts.userCard);
    assert.equal(vault.totalAssets.toString(), (before.vault + BigInt(amount)).toString());
    assert.equal(
      card.depositedAmount.toString(),
      before.deposited.add(new anchor.BN(amount)).toString()
    );
    console.log(
      isDevnet
        ? "Deposit: https://explorer.solana.com/tx/" + signature + "?cluster=devnet"
        : "Deposit signature: " + signature
    );
  }

  before(async () => {
    await provider.sendAndConfirm(
      new Transaction().add(
        SystemProgram.transfer({
          fromPubkey: admin.publicKey,
          toPubkey: userB.publicKey,
          lamports: 0.03 * LAMPORTS_PER_SOL,
        })
      ),
      [admin]
    );

    tokenMint = await createMint(provider.connection, admin, admin.publicKey, null, 6);
    [vaultState] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), tokenMint.toBuffer()],
      program.programId
    );
    [vaultTokenAccount] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault_tokens"), tokenMint.toBuffer()],
      program.programId
    );
    [shareMint] = PublicKey.findProgramAddressSync(
      [Buffer.from("share_mint"), tokenMint.toBuffer()],
      program.programId
    );

    await program.methods
      .initializeVault()
      .accounts({
        vaultState,
        vaultTokenAccount,
        shareMint,
        tokenMint,
        admin: admin.publicKey,
      } as any)
      .rpc();

    userCardA = cardPda(admin.publicKey);
    userCardB = cardPda(userB.publicKey);
    for (const user of [admin, userB]) {
      await program.methods
        .createUserCard()
        .accounts({
          vaultState,
          userCard: cardPda(user.publicKey),
          user: user.publicKey,
        } as any)
        .signers(user.publicKey.equals(admin.publicKey) ? [] : [user])
        .rpc();
    }

    tokensA = (
      await getOrCreateAssociatedTokenAccount(
        provider.connection, admin, tokenMint, admin.publicKey
      )
    ).address;
    tokensB = (
      await getOrCreateAssociatedTokenAccount(
        provider.connection, admin, tokenMint, userB.publicKey
      )
    ).address;
    sharesA = (
      await getOrCreateAssociatedTokenAccount(
        provider.connection, admin, shareMint, admin.publicKey
      )
    ).address;
    sharesB = (
      await getOrCreateAssociatedTokenAccount(
        provider.connection, admin, shareMint, userB.publicKey
      )
    ).address;

    for (const destination of [tokensA, tokensB]) {
      await mintTo(
        provider.connection, admin, tokenMint, destination, admin, 10_000 * unit
      );
    }
    console.log("Program:", program.programId.toBase58());
    console.log("Vault:", vaultState.toBase58());
    console.log("Token mint:", tokenMint.toBase58());
    console.log("Share mint:", shareMint.toBase58());
    console.log("Token storage:", vaultTokenAccount.toBase58());
  });

  it("vault controls storage and share mint", async () => {
    const storage = await getAccount(provider.connection, vaultTokenAccount);
    const mint = await getMint(provider.connection, shareMint);
    assert.equal(storage.owner.toBase58(), vaultState.toBase58());
    assert.equal(storage.mint.toBase58(), tokenMint.toBase58());
    assert.equal(mint.mintAuthority?.toBase58(), vaultState.toBase58());
    assert.equal(mint.decimals, 6);
    assert.equal(mint.supply.toString(), "0");
    const vault = await program.account.vaultState.fetch(vaultState);
    assert.equal(vault.totalAssets.toString(), "0");
    for (const address of [userCardA, userCardB]) {
      const card = await program.account.userCard.fetch(address);
      assert.equal(card.depositedAmount.toString(), "0");
    }
  });

  it("first deposit: 1000 tokens produce 1000 shares", async () => {
    await expectDeposit(admin, 1000 * unit, 1000 * unit);
  });

  it("next depositor: 500 tokens produce 500 shares", async () => {
    await expectDeposit(userB, 500 * unit, 500 * unit);
  });

  it("after simulated yield: 110 tokens produce 100 shares", async () => {
    await mintTo(
      provider.connection, admin, tokenMint, vaultTokenAccount, admin, 150 * unit
    );
    await expectDeposit(userB, 110 * unit, 100 * unit);
  });

  it("repeat deposit: 55 tokens produce 50 shares", async () => {
    await expectDeposit(admin, 55 * unit, 50 * unit);
  });

  it("zero deposit is rejected", async () => {
    await expectFailure(() => deposit(admin, 0), "ZeroDeposit");
  });

  it("deposit rounded to zero shares is rejected", async () => {
    await expectFailure(() => deposit(admin, 1), "ZeroShares");
  });

  it("insufficient tokens are rejected", async () => {
    const balance = (await getAccount(provider.connection, tokensA)).amount;
    await expectFailure(
      () => deposit(admin, Number(balance) + 1),
      "InsufficientTokens"
    );
  });

  it("another user's source account is rejected", async () => {
    await expectFailure(
      () => deposit(admin, unit, { userTokenAccount: tokensB }),
      "ConstraintTokenOwner"
    );
  });

  it("another user's share account is rejected", async () => {
    await expectFailure(
      () => deposit(admin, unit, { userShareAccount: sharesB }),
      "ConstraintTokenOwner"
    );
  });

  it("a source account with the wrong mint is rejected", async () => {
    await expectFailure(
      () => deposit(admin, unit, { userTokenAccount: sharesA }),
      "ConstraintTokenMint"
    );
  });

  it("another user's card is rejected", async () => {
    await expectFailure(
      () => deposit(admin, unit, { userCard: userCardB }),
      "ConstraintSeeds"
    );
  });

  it("vault includes yield while cards count only personal deposits", async () => {
    const vault = await program.account.vaultState.fetch(vaultState);
    const cardA = await program.account.userCard.fetch(userCardA);
    const cardB = await program.account.userCard.fetch(userCardB);
    const storage = await getAccount(provider.connection, vaultTokenAccount);
    assert.equal(vault.totalAssets.toString(), String(1815 * unit));
    assert.equal(vault.totalAssets.toString(), storage.amount.toString());
    assert.equal(cardA.depositedAmount.toString(), String(1055 * unit));
    assert.equal(cardB.depositedAmount.toString(), String(610 * unit));
  });
});
