import assert from "node:assert/strict";
import test from "node:test";
import { connectWallet } from "../src/wallet.web";

test("wallet proof signs the server challenge and rejects an address change", async () => {
  const previous = Object.getOwnPropertyDescriptor(globalThis, "window");
  let address = "wallet-one";
  let switchAddress = false;
  const provider = {
    publicKey: { toString: () => address },
    connect: async () => {},
    signMessage: async (bytes: Uint8Array) => {
      assert.equal(new TextDecoder().decode(bytes), "server nonce for account one");
      if (switchAddress) address = "wallet-two";
      return { signature: new Uint8Array(64).fill(1) };
    },
  };
  Object.defineProperty(globalThis, "window", { configurable: true, value: { phantom: { solana: provider } } });
  try {
    const challenge = async (value: string) => {
      assert.equal(value, "wallet-one");
      return { challenge_id: "nonce-one", message: "server nonce for account one" };
    };
    const proof = await connectWallet("phantom", challenge);
    assert.equal(proof.challengeId, "nonce-one");
    assert.equal(proof.address, "wallet-one");
    switchAddress = true;
    await assert.rejects(connectWallet("phantom", challenge), /Адрес изменился/);
    await assert.rejects(connectWallet("solflare", challenge), /Solflare/);
  } finally {
    if (previous) Object.defineProperty(globalThis, "window", previous);
    else Reflect.deleteProperty(globalThis, "window");
  }
});
