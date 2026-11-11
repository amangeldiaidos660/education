export type WalletChoice = "phantom" | "solflare";
export type WalletProof = { address: string; signature: string; challengeId: string };
export type Challenge = { challenge_id: string; message: string };
export type RequestChallenge = (address: string) => Promise<Challenge>;
