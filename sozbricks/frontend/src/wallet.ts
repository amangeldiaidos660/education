import { Platform } from "react-native";
import bs58 from "bs58";
import { fromByteArray, toByteArray } from "base64-js";
import { nativeGo } from "./auth";
import type { RequestChallenge, WalletChoice, WalletProof } from "./wallet-types";

export async function connectWallet(_choice: WalletChoice, challenge: RequestChallenge): Promise<WalletProof> {
  if (Platform.OS !== "android" || nativeGo) throw new Error("Подключение кошелька доступно в отдельном Android APK и веб-версии.");
  const { transact } = await import("@solana-mobile/mobile-wallet-adapter-protocol");
  return transact(async wallet => {
    const authorization = await wallet.authorize({ chain: "solana:devnet", identity: { name: "Söz Bricks", uri: "https://sozbricks-api-tests.onrender.com" } });
    const account = authorization.accounts[0];
    if (!account) throw new Error("Кошелёк не передал адрес.");
    const encodedAddress = "publicKey" in account ? fromByteArray(Uint8Array.from(account.publicKey)) : account.address;
    const address = bs58.encode(toByteArray(encodedAddress));
    const request = await challenge(address);
    const message = new TextEncoder().encode(request.message);
    const result = await wallet.signMessages({ addresses: [encodedAddress], payloads: [fromByteArray(message)] });
    const signed = toByteArray(result.signed_payloads[0]);
    if (signed.length !== message.length + 64 || !message.every((byte, index) => signed[index] === byte)) {
      throw new Error("Кошелёк вернул подпись другого сообщения.");
    }
    return { address, signature: bs58.encode(signed.slice(message.length)), challengeId: request.challenge_id };
  });
}
