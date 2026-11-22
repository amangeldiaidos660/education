import assert from "node:assert/strict";
import test from "node:test";
import { devnetMessage } from "../src/devnet-status";

test("Devnet notice distinguishes guests, loading, recovery and enabled payouts", () => {
  const rewards = { total_lamports: 100000000, items: [], solana_payments_enabled: true, wallet_address: "wallet" };
  assert.match(devnetMessage(false), /Гостевой режим/);
  assert.match(devnetMessage(true), /Загружаем/);
  assert.match(devnetMessage(true, { ...rewards, wallet_address: null }), /ожидать привязки/);
  assert.match(devnetMessage(true, { ...rewards, solana_payments_enabled: false }), /пока не включены/);
  assert.match(devnetMessage(true, { ...rewards, wallet_reconfirmation_required: true }), /новой подписью/);
  assert.match(devnetMessage(true, rewards), /отправляются автоматически/);
  assert.doesNotMatch(devnetMessage(true, rewards), /не подключён|не отправляются/);
});
