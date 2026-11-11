import assert from "node:assert/strict";
import test from "node:test";
import { AccountSync } from "../src/sync";
import { Api, ApiError, accountSave, createApi, Finished, Session } from "../src/api";
import { initialSave } from "../src/model";

function memory() {
  const items = new Map<string, string>();
  return { items, getItem: async (key: string) => items.get(key) ?? null,
    setItem: async (key: string, value: string) => { items.set(key, value); } };
}
const session: Session = { id: "session-1", request_id: "request-1", level_id: 1, seed: 12362,
  word: "АТ", speed: 210, reward_lamports: 100000000, status: "started", expires_at: "2099-01-01T00:00:00Z", rules_version: 1 };
const outcome = { won: true, score: 780, collected_word: "АТ", duration_seconds: 35 };
const progress = { completed: [1], best: { "1": 780 }, earned_lamports: 100000000 };
const receipt: Finished = { ...outcome, session_id: session.id, awarded_lamports: 100000000, progress };
function server(handler?: (path: string, method: string, body: unknown) => unknown): Api {
  return (async (path: string, method = "GET", body?: unknown) => {
    if (handler) {
      const result = handler(path, method, body);
      if (result !== undefined) return result;
    }
    if (path === "/v1/game-sessions") return session;
    if (path.endsWith("/finish")) return receipt;
    if (path.endsWith("/abandon")) return { ...session, status: "abandoned" };
    if (path === "/v1/profile") return { profile: { user_id: "alpha", email: "test@example.com", display_name: "Айдос", hero: "aiaru" }, progress };
    if (path === "/v1/levels") return { items: [] };
    if (path.startsWith("/v1/history")) return { items: [], imported_history: [] };
    if (path.startsWith("/v1/rewards")) return { items: [], total_lamports: 100000000, solana_payments_enabled: false };
    if (path === "/v1/progress/import") return { imported: true, awarded_lamports: 0, progress };
    throw new Error(`Unexpected request ${path}`);
  }) as Api;
}

test("start retry keeps the persisted request ID when its response is lost", async () => {
  const storage = memory();
  const bodies: unknown[] = [];
  const api = server((path, _method, body) => {
    if (path !== "/v1/game-sessions") return;
    assert(storage.items.get("sozbricks.account.v1.alpha")?.includes("request-1"));
    bodies.push(body);
    if (bodies.length === 1) throw new Error("response lost");
  });
  const first = new AccountSync("alpha", api, storage, () => "import-1");
  await assert.rejects(first.start(1, "request-1"));
  const restored = new AccountSync("alpha", api, storage, () => "import-2");
  assert.equal((await restored.start(1, "request-1")).id, session.id);
  assert.deepEqual(bodies, [{ level_id: 1, request_id: "request-1" }, { level_id: 1, request_id: "request-1" }]);
});

test("finish survives restart and retries the identical result without a new session", async () => {
  const storage = memory();
  let calls = 0, starts = 0;
  const bodies: unknown[] = [];
  const api = server((path, _method, body) => {
    if (path === "/v1/game-sessions") starts++;
    if (!path.endsWith("/finish")) return;
    assert(storage.items.get("sozbricks.account.v1.alpha")?.includes('"action":"finish"'));
    bodies.push(body);
    if (++calls === 1) throw new Error("response lost after database commit");
  });
  const sync = new AccountSync("alpha", api, storage, () => "import-1");
  await sync.start(1, "request-1");
  await assert.rejects(sync.finish(session.id, outcome));
  const restored = new AccountSync("alpha", api, storage, () => "import-2");
  await restored.refresh();
  assert.equal(starts, 1);
  assert.deepEqual(bodies, [outcome, outcome]);
  assert.equal(restored.snapshot().disk.attempt, undefined);
  assert.equal(restored.snapshot().disk.receipt?.awarded_lamports, 100000000);
  await restored.finish(session.id, outcome);
  assert.equal(calls, 2);
});

test("unfinished upload blocks new attempts and cannot be abandoned or overwritten", async () => {
  const storage = memory();
  let starts = 0, abandons = 0;
  const api = server(path => {
    if (path === "/v1/game-sessions") starts++;
    if (path.endsWith("/abandon")) abandons++;
    if (path.endsWith("/finish")) throw new Error("offline");
  });
  const sync = new AccountSync("alpha", api, storage, () => "import-1");
  await sync.start(1, "request-1");
  await assert.rejects(sync.finish(session.id, outcome));
  await sync.abandon(session.id);
  await assert.rejects(sync.finish(session.id, { ...outcome, score: 999 }));
  await assert.rejects(sync.start(2, "request-2"));
  assert.equal(starts, 1); assert.equal(abandons, 0);
  assert.deepEqual(sync.snapshot().disk.attempt?.outcome, outcome);
});

test("rejected results stop retrying and are archived only on explicit dismissal", async () => {
  const storage = memory();
  let finishes = 0;
  const api = server(path => {
    if (path.endsWith("/finish")) { finishes++; throw new ApiError(410, "session_expired", "expired"); }
  });
  const sync = new AccountSync("alpha", api, storage, () => "import-1");
  await sync.start(1, "request-1");
  await assert.rejects(sync.finish(session.id, outcome));
  await sync.refresh();
  assert.equal(finishes, 1);
  await assert.rejects(sync.start(2, "request-2"));
  await sync.dismissRejected();
  assert.equal(sync.snapshot().disk.attempt, undefined);
  assert(storage.items.has("sozbricks.account.v1.alpha.rejected.request-1"));
});

test("storage failure prevents sending an unpersisted request", async () => {
  let calls = 0;
  const sync = new AccountSync("alpha", server(() => { calls++; }), {
    getItem: async () => null, setItem: async () => { throw new Error("disk full"); },
  }, () => "import-1");
  await assert.rejects(sync.start(1, "request-1"));
  assert.equal(calls, 0);
});

test("another account never submits the first account's pending result", async () => {
  const storage = memory();
  const first = new AccountSync("alpha", server(path => { if (path.endsWith("/finish")) throw new Error("offline"); }), storage, () => "import-1");
  await first.start(1, "request-1");
  await assert.rejects(first.finish(session.id, outcome));
  const requests: string[] = [];
  const second = new AccountSync("beta", server(path => { requests.push(path); }), storage, () => "import-2");
  await second.refresh();
  assert(!requests.some(path => path.endsWith("/finish")));
  assert.equal(second.snapshot().disk.attempt, undefined);
  assert(storage.items.get("sozbricks.account.v1.alpha")?.includes('"action":"finish"'));
});

test("guest import persists its snapshot, excludes demo balance, and marks done only after success", async () => {
  const storage = memory();
  let count = 0;
  const bodies: unknown[] = [];
  const api = server((path, _method, body) => {
    if (path !== "/v1/progress/import") return;
    assert.equal(storage.items.get("sozbricks.guest-import-owner.v1"), "alpha");
    bodies.push(body);
    if (++count === 1) throw new Error("offline");
  });
  const sync = new AccountSync("alpha", api, storage, () => "import-1");
  const guest = { ...initialSave, completed: [1], demoLamports: 900000000 };
  await assert.rejects(sync.importGuest(guest));
  assert(!sync.snapshot().disk.imported?.done);
  guest.completed.push(2);
  const restored = new AccountSync("alpha", api, storage, () => "import-2");
  await restored.refresh();
  assert.deepEqual(bodies[0], bodies[1]);
  assert(!JSON.stringify(bodies[0]).includes("demoLamports"));
  assert.deepEqual(restored.snapshot().disk.imported?.save.completed, [1]);
  assert.equal(restored.snapshot().disk.imported?.done, true);
  const second = new AccountSync("beta", api, storage, () => "import-3");
  await assert.rejects(second.importGuest(guest), /другим аккаунтом/);
  assert.equal(count, 2);
});

test("account view replaces guest balance and progress while preserving device preferences", async () => {
  const sync = new AccountSync("alpha", server(), memory(), () => "import-1");
  await sync.refresh();
  const save = accountSave({ ...initialSave, completed: [20], demoLamports: 999999999, dark: false }, sync.snapshot().disk.data!);
  assert.deepEqual(save.completed, [1]);
  assert.equal(save.demoLamports, 100000000);
  assert.equal(save.dark, false);
  assert.equal(save.hero, "aiaru");
});

test("API refreshes a 401 once and reuses the same body", async () => {
  const tokens: boolean[] = [];
  const payloads: unknown[] = [];
  let count = 0;
  const api = createApi(async refresh => { tokens.push(refresh); return refresh ? "new" : "old"; }, (async (_url, init) => {
    payloads.push(init?.body);
    assert.equal((init?.headers as Record<string, string>).Authorization, count === 0 ? "Bearer old" : "Bearer new");
    return new Response(JSON.stringify(count++ ? receipt : { error: { code: "unauthorized" } }), { status: count === 1 ? 401 : 200 });
  }) as typeof fetch);
  await api("/v1/game-sessions/session-1/finish", "POST", outcome);
  assert.deepEqual(tokens, [false, true]);
  assert.deepEqual(payloads, [JSON.stringify(outcome), JSON.stringify(outcome)]);
});

test("account ownership failure before a request never reaches transport", async () => {
  let calls = 0;
  const api = createApi(async () => { throw new ApiError(401, "account_changed", "changed"); }, (async () => { calls++; return new Response("{}"); }) as typeof fetch);
  await assert.rejects(api("/v1/profile"));
  assert.equal(calls, 0);
});
