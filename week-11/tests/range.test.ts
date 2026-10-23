import assert from "node:assert/strict";
import test from "node:test";
import { PublicKey } from "@solana/web3.js";
import { TickUtil } from "@raydium-io/raydium-sdk-v2";
import Decimal from "decimal.js";
import { canonicalMints, explorer, maxWithSlippage, positionRange } from "../src/utils";

test("mints are sorted by their public key bytes", () => {
  const first = new PublicKey(new Uint8Array(32).fill(1));
  const second = new PublicKey(new Uint8Array(32).fill(2));
  assert.deepEqual(canonicalMints(second, first).map((key) => key.toBase58()), [first, second].map((key) => key.toBase58()));
});

test("the same mint cannot be used twice", () => {
  const mint = new PublicKey(new Uint8Array(32).fill(1));
  assert.throws(() => canonicalMints(mint, mint));
});

test("the range surrounds the initial price", () => {
  const range = positionRange("0.9", "1.1", 6, 6, 60);
  assert(range.tickLower < 0 && range.tickUpper > 0);
  assert(new Decimal(range.lowerPrice).lt(1) && new Decimal(range.upperPrice).gt(1));
});

test("range boundaries are valid tick spacing multiples", () => {
  const range = positionRange("0.9", "1.1", 6, 6, 60);
  assert(range.tickLower % 60 === 0);
  assert(range.tickUpper % 60 === 0);
});

test("tick rounding covers the requested interval", () => {
  const range = positionRange("0.9", "1.1", 6, 6, 60);
  assert(new Decimal(range.lowerPrice).lte("0.9"));
  assert(new Decimal(range.upperPrice).gte("1.1"));
});

test("negative ticks round down, not toward zero", () => {
  const range = positionRange("0.91", "0.95", 6, 6, 60);
  assert(range.tickLower < range.tickUpper && range.tickUpper < 0);
  assert(new Decimal(range.lowerPrice).lte("0.91"));
});

test("different mint decimals preserve the requested price units", () => {
  const range = positionRange("0.9", "1.1", 6, 9, 10);
  assert(TickUtil.tickToPrice(range.tickLower, 6, 9).lte("0.9"));
  assert(TickUtil.tickToPrice(range.tickUpper, 6, 9).gte("1.1"));
});

test("a reversed or empty range is rejected", () => {
  assert.throws(() => positionRange("1.1", "0.9", 6, 6, 60));
  assert.throws(() => positionRange("1", "1", 6, 6, 60));
});

test("zero, negative and non-finite prices are rejected", () => {
  for (const price of ["0", "-1", "NaN", "Infinity"]) {
    assert.throws(() => positionRange(price, "1.1", 6, 6, 60));
  }
});

test("invalid tick spacing is rejected", () => {
  for (const spacing of [0, -1, 1.5]) {
    assert.throws(() => positionRange("0.9", "1.1", 6, 6, spacing));
  }
});

test("one percent slippage raises the maximum by one percent", () => {
  assert.equal(maxWithSlippage(BigInt(10000), 100), BigInt(10100));
});

test("slippage rounds upward in base units", () => {
  assert.equal(maxWithSlippage(BigInt(1), 100), BigInt(2));
  assert.equal(maxWithSlippage(BigInt(101), 100), BigInt(103));
});

test("invalid slippage or negative amounts are rejected", () => {
  assert.throws(() => maxWithSlippage(BigInt(-1), 100));
  assert.throws(() => maxWithSlippage(BigInt(1), -1));
  assert.throws(() => maxWithSlippage(BigInt(1), 10001));
});

test("Explorer links always select devnet", () => {
  assert.equal(explorer("example"), "https://explorer.solana.com/address/example?cluster=devnet");
  assert.equal(explorer("example", true), "https://explorer.solana.com/tx/example?cluster=devnet");
});
