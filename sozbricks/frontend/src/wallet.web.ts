import bs58 from "bs58";
import type { RequestChallenge, WalletChoice, WalletProof } from "./wallet-types";

type Provider = {
  publicKey?: { toString(): string };
  connect(): Promise<unknown>;
  signMessage(message: Uint8Array, display?: string): Promise<Uint8Array | { signature: Uint8Array }>;
};

export async function connectWallet(choice: WalletChoice, challenge: RequestChallenge): Promise<WalletProof> {
  const browser = window as unknown as { phantom?: { solana?: Provider }; solflare?: Provider };
  const provider = choice === "phantom" ? browser.phantom?.solana : browser.solflare;
  if (!provider) throw new Error(`Откройте игру в браузере с установленным ${choice === "phantom" ? "Phantom" : "Solflare"}.`);
  await provider.connect();
  const address = provider.publicKey?.toString();
  if (!address) throw new Error("Кошелёк не передал адрес.");
  const request = await challenge(address);
  const signed = await provider.signMessage(new TextEncoder().encode(request.message), "utf8");
  if (provider.publicKey?.toString() !== address) throw new Error("Адрес изменился во время подписи. Подключите кошелёк снова.");
  const signature = signed instanceof Uint8Array ? signed : signed.signature;
  return { address, signature: bs58.encode(signature), challengeId: request.challenge_id };
}
