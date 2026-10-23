import BN from "bn.js";
import { connection, quote } from "./common";

async function main() {
  const amount = process.argv[2] ?? "1000000";
  if (!/^\d+$/.test(amount)) throw new Error("Pass an integer amount in base units");
  const result = await quote(connection(), new BN(amount), process.argv[3] !== "b-to-a");
  console.log("Direction:", result.aToB ? "A to B" : "B to A");
  console.log("Input:", result.amount.toString(), "base units");
  console.log("Expected output:", result.expected.toString(), "base units");
  console.log("Minimum output (1% tolerance):", result.minimum.toString(), "base units");
  console.log("Pool price:", result.priceBefore, "B per A");
  console.log("Tick accounts:", result.remaining.map((key) => key.toBase58()).join(", "));
}

main().catch((error) => { console.error(error instanceof Error ? error.stack : error); process.exitCode = 1; });
