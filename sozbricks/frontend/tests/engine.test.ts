import assert from "node:assert/strict";
import test from "node:test";
import { createGame, H, launch, movePaddle, PADDLE_Y, step, togglePause, W } from "../src/game/engine";
import { alphabet, letters, levels, sameWord } from "../src/game/words";
import { initialSave, recordResult } from "../src/model";

test("Kazakh alphabet, Unicode normalization and codepoint lengths", () => {
  for (const letter of Array.from("ӘҒҚҢӨҰҮҺІ")) assert(alphabet.includes(letter));
  assert.deepEqual(letters("  кітап  "), ["К", "І", "Т", "А", "П"]);
  assert.equal(letters("ТӘУЕЛСІЗДІК").length, 11);
  assert(sameWord("й", "и\u0306"));
  assert(!sameWord("КИТАП", "КІТАП"));
  assert.equal(new Set(levels.map(l => l.id)).size, 24);
});

test("ready, launch, pause and resume preserve physics state", () => {
  const g = createGame(levels[0]);
  movePaddle(g, -100); assert(g.paddle > 0); assert.equal(g.balls[0].x, g.paddle);
  movePaddle(g, W + 100); assert(g.paddle < W);
  launch(g); assert.equal(g.phase, "playing");
  togglePause(g); const before = JSON.stringify(g); step(g, 1); assert.equal(JSON.stringify(g), before);
  togglePause(g); assert.equal(g.phase, "playing");
  step(g, 1 / 120); assert(g.elapsed > 0);
});

test("paddle reflection and destroying a brick emit a target letter", () => {
  const g = createGame(levels[2]); launch(g);
  Object.assign(g.balls[0], { x: g.paddle, y: PADDLE_Y - 7, vx: 0, vy: 200 });
  step(g, 1 / 120); assert(g.balls[0].vy < 0);
  const b = g.bricks[0];
  Object.assign(g.balls[0], { x: b.x + 20, y: b.y + 29, vx: 0, vy: -200 });
  step(g, 1 / 120);
  assert.equal(b.hp, 0); assert.equal(g.drops[0].letter, "К"); assert(g.score > 0);
});

test("a missed target can be dropped again", () => {
  const g = createGame(levels[2]); launch(g);
  g.drops = [{ id: 900, x: 10, y: H + 19, letter: "К", needed: true }];
  step(g, 0.02); assert.equal(g.drops.length, 0);
  const b = g.bricks[0];
  Object.assign(g.balls[0], { x: b.x + 20, y: b.y + 29, vx: 0, vy: -200 });
  step(g, 1 / 120); assert(g.drops.some(d => d.letter === "К"));
});

function catchWord(word: string) {
  const g = createGame(levels[0]); launch(g);
  for (const letter of letters(word)) {
    g.drops = [{ id: g.nextId++, x: g.paddle, y: PADDLE_Y - 13.1, letter, needed: true }];
    step(g, 1 / 120);
  }
  return g;
}

test("only the exact target wins; a full incorrect rack loses", () => {
  assert.equal(catchWord("АТ").phase, "won");
  const g = catchWord("ӘТ"); assert.equal(g.phase, "lost"); assert.match(g.reason, /неверно/);
});

test("losing the final ball ends the game; there are no reserve lives", () => {
  const g = createGame(levels[0]); launch(g);
  g.balls[0].y = H + 30; step(g, 1 / 120);
  assert.equal(g.phase, "lost"); assert.equal(g.balls.length, 0);
});

test("empty brick wave is replenished so a missed letter cannot deadlock the level", () => {
  const g = createGame(levels[0]); launch(g);
  for (const b of g.bricks) b.hp = 0;
  step(g, 1 / 120); assert.equal(g.wave, 1); assert(g.bricks.some(b => b.hp > 0));
});

test("fixed-step physics remains finite over an extended autonomous run", () => {
  const g = createGame(levels[23]); launch(g);
  for (let i = 0; i < 18000 && g.phase === "playing"; i++) {
    movePaddle(g, g.balls[0]?.x ?? W / 2); step(g, 1 / 120);
    for (const b of g.balls) assert(Number.isFinite(b.x + b.y + b.vx + b.vy));
    assert(g.balls.length <= 3);
  }
});

test("rewards unlock levels once, duplicates are idempotent and losses preserve balance", () => {
  const won = { level: levels[0], score: 1000, elapsed: 30.7, phase: "won" as const };
  const first = recordResult(initialSave, won, "first");
  assert.deepEqual(first.completed, [1]); assert.equal(first.demoLamports, 100_000_000);
  assert.equal(recordResult(first, won, "first"), first);
  const repeat = recordResult(first, won, "second");
  assert.equal(repeat.demoLamports, first.demoLamports); assert.equal(repeat.history.length, 2);
  const lost = recordResult(repeat, { ...won, level: levels[1], phase: "lost" }, "third");
  assert.equal(lost.demoLamports, first.demoLamports); assert.deepEqual(lost.completed, [1]);
  assert.equal(initialSave.demoLamports, 0); assert.equal(initialSave.history.length, 0);
  assert.equal(recordResult(JSON.parse(JSON.stringify(first)), won, "restored").demoLamports, first.demoLamports);
});
