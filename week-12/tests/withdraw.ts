import * as anchor from "@coral-xyz/anchor";
import {
  createMint,
  createTransferCheckedInstruction,
  freezeAccount,
  getOrCreateAssociatedTokenAccount,
  mintTo,
  thawAccount,
  transferChecked,
  unpackAccount,
  unpackMint,
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
import { pda } from "../scripts/common";

describe("week-12: withdrawal regression", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program: any = anchor.workspace.SimpleVault;
  const admin = (provider.wallet as anchor.Wallet).payer;
  const userB = Keypair.generate();
  const userC = Keypair.generate();
  const users = [admin, userB, userC];
  const unit = 1_000_000;
  const rpcHost = new URL(provider.connection.rpcEndpoint).hostname;
  const isDevnet = rpcHost === "api.devnet.solana.com";

  if (!isDevnet && !["localhost", "127.0.0.1", "::1", "[::1]"].includes(rpcHost)) {
    throw new Error("Only localnet and devnet are allowed");
  }
  if (program.programId.toBase58() !== "HnAz6jnZQ2iwBUWutPEs8HTWJMHey6DUG1dCR5DvHk9f") {
    throw new Error("Use the existing vault program ID configured for week-12");
  }
  if (!program.idl.instructions.some((instruction: any) => instruction.name === "withdraw")) {
    throw new Error("Build week-12 to generate an IDL containing withdraw");
  }

  let tokenMint: PublicKey;
  let vaultState: PublicKey;
  let vaultTokenAccount: PublicKey;
  let shareMint: PublicKey;
  const cards: PublicKey[] = [];
  const tokens: PublicKey[] = [];
  const shares: PublicKey[] = [];

  const accountsFor = (user: Keypair) => {
    const index = users.findIndex((candidate) => candidate.publicKey.equals(user.publicKey));
    if (index < 0) {
      throw new Error("Unknown test user");
    }
    return {
      strategyState: pda("strategy", vaultState),
      secondaryVault: pda("strategy_tokens", vaultState),
      vaultState,
      userCard: cards[index],
      vaultTokenAccount,
      shareMint,
      tokenMint,
      userTokenAccount: tokens[index],
      userShareAccount: shares[index],
      user: user.publicKey,
      tokenProgram: TOKEN_PROGRAM_ID,
    };
  };

  const withdrawal = (
    user: Keypair,
    amount: number,
    overrides: Record<string, PublicKey> = {}
  ) =>
    program.methods
      .withdraw(new anchor.BN(amount))
      .accountsStrict({ ...accountsFor(user), ...overrides });

  const withdraw = (
    user: Keypair,
    amount: number,
    overrides: Record<string, PublicKey> = {}
  ) =>
    withdrawal(user, amount, overrides)
      .signers(user.publicKey.equals(admin.publicKey) ? [] : [user])
      .rpc();

  async function readState() {
    const addresses = [vaultTokenAccount, shareMint, ...tokens, ...shares, vaultState, ...cards];
    const infos = await provider.connection.getMultipleAccountsInfo(addresses);
    assert.isTrue(infos.every((info) => info !== null), "all fixture accounts must exist");
    const decode = (name: string, index: number) => {
      const definition = program.idl.accounts!.find(
        (account: any) => account.name.replace(/_/g, "").toLowerCase() === name
      );
      assert.exists(definition, "IDL account must exist");
      return program.coder.accounts.decode(definition!.name, infos[index]!.data);
    };
    const vault = decode("vaultstate", 8);
    return {
      assets: unpackAccount(vaultTokenAccount, infos[0]).amount,
      supply: unpackMint(shareMint, infos[1]).supply,
      tokens: tokens.map((address, index) => unpackAccount(address, infos[2 + index]).amount),
      shares: shares.map((address, index) => unpackAccount(address, infos[5 + index]).amount),
      totalAssets: vault.totalAssets.toString() as string,
      deposits: cards.map((_, index) => decode("usercard", 9 + index).depositedAmount.toString() as string),
      encoded: infos.map((info) => info!.data.toString("base64")),
    };
  }

  async function expectFailure(action: () => Promise<unknown>, expected: string | RegExp) {
    const before = await readState();
    let caught: any;
    try {
      await action();
    } catch (error) {
      caught = error;
    }
    assert.exists(caught, "withdrawal must fail");
    if (typeof expected === "string") {
      assert.equal(caught.error?.errorCode?.code, expected);
    } else {
      assert.match(String(caught) + "\n" + (caught.logs ?? []).join("\n"), expected);
    }
    assert.deepEqual((await readState()).encoded, before.encoded, "failed withdrawal must not change accounts");
  }

  async function receipt(signature: string) {
    for (let attempt = 0; attempt < 5; attempt++) {
      const result = await provider.connection.getParsedTransaction(signature, {
        commitment: "confirmed",
        maxSupportedTransactionVersion: 0,
      });
      if (result) {
        return result;
      }
      await new Promise((resolve) => setTimeout(resolve, 1000));
    }
    throw new Error("Confirmed transaction is not available: " + signature);
  }

  async function expectWithdrawal(user: Keypair, amount: number, expectedTokens: number) {
    const index = users.findIndex((candidate) => candidate.publicKey.equals(user.publicKey));
    const before = await readState();
    const signature = await withdraw(user, amount);
    const after = await readState();
    assert.equal(after.assets.toString(), (before.assets - BigInt(expectedTokens)).toString());
    assert.equal(after.supply.toString(), (before.supply - BigInt(amount)).toString());
    for (let current = 0; current < users.length; current++) {
      assert.equal(
        after.tokens[current].toString(),
        (before.tokens[current] + (current === index ? BigInt(expectedTokens) : BigInt(0))).toString()
      );
      assert.equal(
        after.shares[current].toString(),
        (before.shares[current] - (current === index ? BigInt(amount) : BigInt(0))).toString()
      );
    }
    assert.equal(after.totalAssets, after.assets.toString());
    assert.deepEqual(after.deposits, before.deposits, "withdrawal must preserve deposit history");
    console.log(
      isDevnet
        ? "Withdrawal: https://explorer.solana.com/tx/" + signature + "?cluster=devnet"
        : "Withdrawal signature: " + signature
    );
    return signature;
  }

  before(async () => {
    await provider.sendAndConfirm(
      new Transaction().add(
        ...[userB, userC].map((user) =>
          SystemProgram.transfer({
            fromPubkey: admin.publicKey,
            toPubkey: user.publicKey,
            lamports: 0.02 * LAMPORTS_PER_SOL,
          })
        )
      ),
      [admin]
    );
    tokenMint = await createMint(provider.connection, admin, admin.publicKey, admin.publicKey, 6);
    [vaultState] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), tokenMint.toBuffer()], program.programId
    );
    [vaultTokenAccount] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault_tokens"), tokenMint.toBuffer()], program.programId
    );
    [shareMint] = PublicKey.findProgramAddressSync(
      [Buffer.from("share_mint"), tokenMint.toBuffer()], program.programId
    );
    await program.methods.initializeVault().accounts({
      vaultState,
      vaultTokenAccount,
      shareMint,
      tokenMint,
      admin: admin.publicKey,
    } as any).rpc();

    for (const user of users) {
      const [card] = PublicKey.findProgramAddressSync(
        [Buffer.from("user_card"), vaultState.toBuffer(), user.publicKey.toBuffer()],
        program.programId
      );
      cards.push(card);
      await program.methods.createUserCard().accounts({
        vaultState, userCard: card, user: user.publicKey,
      } as any).signers(user.publicKey.equals(admin.publicKey) ? [] : [user]).rpc();
      tokens.push((await getOrCreateAssociatedTokenAccount(
        provider.connection, admin, tokenMint, user.publicKey
      )).address);
      shares.push((await getOrCreateAssociatedTokenAccount(
        provider.connection, admin, shareMint, user.publicKey
      )).address);
    }
    for (let index = 0; index < 2; index++) {
      await mintTo(provider.connection, admin, tokenMint, tokens[index], admin, 10_000 * unit);
      await program.methods.deposit(new anchor.BN((index === 0 ? 1000 : 500) * unit))
        .accountsStrict(accountsFor(users[index]))
        .signers(index === 0 ? [] : [users[index]])
        .rpc();
    }
    await mintTo(provider.connection, admin, tokenMint, vaultTokenAccount, admin, 150 * unit);
    console.log("Withdrawal vault:", vaultState.toBase58());
    console.log("Withdrawal token mint:", tokenMint.toBase58());
    console.log("Withdrawal share mint:", shareMint.toBase58());
  });

  it("zero withdrawal is rejected", async () => {
    await expectFailure(() => withdraw(admin, 0), "ZeroWithdrawal");
  });

  it("withdrawing more shares than owned is rejected", async () => {
    await expectFailure(() => withdraw(admin, 1000 * unit + 1), "InsufficientShares");
  });

  it("admin cannot withdraw another user's shares", async () => {
    await expectFailure(
      () => withdraw(admin, unit, { userShareAccount: shares[1] }), "ConstraintTokenOwner"
    );
  });

  it("another user's receiving account is rejected", async () => {
    await expectFailure(
      () => withdraw(admin, unit, { userTokenAccount: tokens[1] }), "ConstraintTokenOwner"
    );
  });

  it("a share account with the wrong mint is rejected", async () => {
    await expectFailure(
      () => withdraw(admin, unit, { userShareAccount: tokens[0] }), "ConstraintTokenMint"
    );
  });

  it("a receiving account with the wrong mint is rejected", async () => {
    await expectFailure(
      () => withdraw(admin, unit, { userTokenAccount: shares[0] }), "ConstraintTokenMint"
    );
  });

  it("another user's card is rejected", async () => {
    await expectFailure(
      () => withdraw(admin, unit, { userCard: cards[1] }), "ConstraintSeeds"
    );
  });

  it("a substituted vault storage account is rejected", async () => {
    await expectFailure(
      () => withdraw(admin, unit, { vaultTokenAccount: tokens[0] }), "ConstraintHasOne"
    );
  });

  it("admin signature alone cannot transfer tokens out of vault", async () => {
    await expectFailure(
      () => provider.sendAndConfirm(new Transaction().add(
        createTransferCheckedInstruction(vaultTokenAccount, tokenMint, tokens[0], admin.publicKey, 1, 6)
      ), [admin]),
      /owner does not match|0x4\b/i
    );
  });

  it("failed transfer rolls back the burn and accounting", async () => {
    await freezeAccount(provider.connection, admin, tokens[0], tokenMint, admin);
    try {
      const before = await readState();
      const transaction = await withdrawal(admin, 10 * unit).transaction();
      const latest = await provider.connection.getLatestBlockhash("confirmed");
      transaction.feePayer = admin.publicKey;
      transaction.recentBlockhash = latest.blockhash;
      transaction.sign(admin);
      const signature = await provider.connection.sendRawTransaction(transaction.serialize(), {
        skipPreflight: true,
      });
      const confirmation = await provider.connection.confirmTransaction({
        signature, ...latest,
      }, "confirmed");
      assert.deepEqual(confirmation.value.err, { InstructionError: [0, { Custom: 17 }] });
      const result = await receipt(signature);
      const inner = (result.meta?.innerInstructions ?? []).flatMap((group) => group.instructions);
      const operations = inner.filter((instruction) =>
        instruction.programId.equals(TOKEN_PROGRAM_ID) && "parsed" in instruction
      ).map((instruction) => (instruction as any).parsed.type);
      assert.deepEqual(operations, ["burn", "transferChecked"]);
      assert.include(result.meta?.logMessages ?? [], "Program " + TOKEN_PROGRAM_ID.toBase58() + " success");
      assert.deepEqual((await readState()).encoded, before.encoded);
    } finally {
      await thawAccount(provider.connection, admin, tokens[0], tokenMint, admin);
    }
  });

  it("partial withdrawal returns yield and shows both CPI instructions", async () => {
    const signature = await expectWithdrawal(admin, 100 * unit, 110 * unit);
    const result = await receipt(signature);
    assert.isNull(result.meta!.err);
    assert.equal(result.transaction.message.instructions[0].programId.toBase58(), program.programId.toBase58());
    const inner = (result.meta?.innerInstructions ?? []).flatMap((group) => group.instructions);
    const operations = inner.filter((instruction) =>
      instruction.programId.equals(TOKEN_PROGRAM_ID) && "parsed" in instruction
    ).map((instruction) => (instruction as any).parsed);
    assert.deepEqual(operations.map((operation) => operation.type), ["burn", "transferChecked"]);
    assert.equal(operations[0].info.authority, admin.publicKey.toBase58());
    assert.equal(operations[0].info.mint, shareMint.toBase58());
    assert.equal(operations[0].info.amount, String(100 * unit));
    assert.equal(operations[1].info.authority, vaultState.toBase58());
    assert.equal(operations[1].info.tokenAmount.amount, String(110 * unit));
  });

  it("a recipient of transferred shares can withdraw without a personal deposit", async () => {
    await transferChecked(
      provider.connection, admin, shares[0], shareMint, shares[2], admin, 100 * unit, 6
    );
    assert.equal((await readState()).deposits[2], "0");
    await expectWithdrawal(userC, 100 * unit, 110 * unit);
  });

  it("first depositor can withdraw all remaining shares", async () => {
    await expectWithdrawal(admin, 800 * unit, 880 * unit);
  });

  it("last depositor receives all remaining assets", async () => {
    await expectWithdrawal(userB, 500 * unit, 550 * unit);
    const state = await readState();
    assert.equal(state.assets.toString(), "0");
    assert.equal(state.supply.toString(), "0");
    assert.equal(state.totalAssets, "0");
  });

  it("withdrawal from an empty vault is rejected without division by zero", async () => {
    await expectFailure(() => withdraw(admin, 1), "InsufficientShares");
  });

  it("deposit and withdrawal work again after everyone exits", async () => {
    await program.methods.deposit(new anchor.BN(10 * unit)).accountsStrict(accountsFor(admin)).rpc();
    const state = await readState();
    assert.equal(state.shares[0].toString(), String(10 * unit));
    assert.equal(state.totalAssets, String(10 * unit));
    await expectWithdrawal(admin, 10 * unit, 10 * unit);
  });

  it("fractional withdrawal rounds down in base token units", async () => {
    await program.methods.deposit(new anchor.BN(2)).accountsStrict(accountsFor(admin)).rpc();
    await mintTo(provider.connection, admin, tokenMint, vaultTokenAccount, admin, 1);
    await expectWithdrawal(admin, 1, 1);
  });

  it("the last share receives the remaining rounding balance", async () => {
    await expectWithdrawal(admin, 1, 2);
    const state = await readState();
    assert.equal(state.assets.toString(), "0");
    assert.equal(state.supply.toString(), "0");
  });
});
