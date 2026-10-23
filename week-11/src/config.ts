import { readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { createHash } from "node:crypto";
import assert from "node:assert/strict";
import {
  ClmmConfigLayout,
  DEV_API_URLS,
  DEVNET_PROGRAM_ID,
  getPdaAmmConfigId,
  Raydium,
  TxVersion,
} from "@raydium-io/raydium-sdk-v2";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";

export const WALLET = new PublicKey("HMg9Wz3DLAwU8GAYh91WhRMWQzdhpAsBpUYkZcf9tMtX");
export const CLMM_PROGRAM = new PublicKey("DRayAUgENGQBKVaX8owNhgzkEDyoHTGVEGHVJT1E9pfH");
export const DEVNET_GENESIS = "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG";
export const DECIMALS = 6;
export const TOKEN_SUPPLY = BigInt("1000000000000");
export const INITIAL_PRICE = "1";
export const LOWER_PRICE = "0.9";
export const UPPER_PRICE = "1.1";
export const POSITION_AMOUNT_A = "100000000";
export const POSITION_MAX_B = "200000000";
export const SLIPPAGE_BPS = 100;
export const TX_VERSION = TxVersion.V0;
export const COMPUTE_BUDGET = { units: 600000, microLamports: 10000 };

export function connection() {
  return new Connection(process.env.SOLANA_RPC_URL ?? "https://api.devnet.solana.com", {
    commitment: "confirmed",
    confirmTransactionInitialTimeout: 90000,
    fetch: async (input, init) => {
      for (let attempt = 0; ; attempt++) {
        try {
          return await fetch(input, init);
        } catch (error) {
          if (attempt >= 2) throw error;
          await new Promise((done) => setTimeout(done, 500 * 2 ** attempt));
        }
      }
    },
  });
}

export async function assertDevnet(rpc: Connection) {
  assert.equal(await rpc.getGenesisHash(), DEVNET_GENESIS, "Only Solana devnet is allowed");
  assert(DEVNET_PROGRAM_ID.CLMM_PROGRAM_ID.equals(CLMM_PROGRAM), "SDK devnet program mismatch");
  const program = await rpc.getAccountInfo(CLMM_PROGRAM);
  assert(program?.executable, "Raydium CLMM is not available in devnet");
}

export function loadWallet() {
  const file = process.env.SOLANA_WALLET ?? join(homedir(), ".config", "solana", "id.json");
  const wallet = Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(file, "utf8"))));
  assert(wallet.publicKey.equals(WALLET), "The configured wallet differs from our course wallet");
  return wallet;
}

export async function loadAmmConfig(rpc: Connection) {
  const id = getPdaAmmConfigId(CLMM_PROGRAM, 0).publicKey;
  const info = await rpc.getAccountInfo(id);
  assert(info, "Devnet AMM config is missing");
  assert(info.owner.equals(CLMM_PROGRAM), "Invalid devnet AMM config owner");
  const discriminator = createHash("sha256").update("account:AmmConfig").digest().subarray(0, 8);
  assert(info.data.subarray(0, 8).equals(discriminator), "Invalid AMM config discriminator");
  assert.equal(info.data.length, ClmmConfigLayout.span, "Unexpected AMM config layout");
  const config = ClmmConfigLayout.decode(info.data);
  assert.equal(config.index, 0);
  assert.equal(config.tradeFeeRate, 2500, "The expected 0.25% fee tier changed");
  assert.equal(config.tickSpacing, 60, "The expected tick spacing changed");
  return {
    id,
    index: config.index,
    protocolFeeRate: config.protocolFeeRate,
    tradeFeeRate: config.tradeFeeRate,
    tickSpacing: config.tickSpacing,
    fundFeeRate: config.fundFeeRate,
    fundOwner: config.fundOwner.toBase58(),
    description: "Week 11 devnet pool",
  };
}

export async function loadSdk(rpc: Connection, owner: Keypair | PublicKey) {
  return Raydium.load({
    connection: rpc,
    owner,
    cluster: "devnet",
    disableLoadToken: true,
    disableFeatureCheck: true,
    blockhashCommitment: "confirmed",
    urlConfigs: {
      ...DEV_API_URLS,
      BASE_HOST: "https://api-v3-devnet.raydium.io",
      OWNER_BASE_HOST: "https://owner-v1-devnet.raydium.io",
      SWAP_HOST: "https://transaction-v1-devnet.raydium.io",
    },
  });
}
