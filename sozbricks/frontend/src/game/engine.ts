import { alphabet, letters, Level, sameWord } from "./words";

export const W = 360;
export const H = 490;
export const PADDLE_Y = 456;
export const PADDLE_W = 84;
const R = 6;
const SPEED_INTERVAL = 20;
const SPEED_INCREMENT = 0.08;
const MAX_SPEED_MULTIPLIER = 1.5;
const MAX_BALL_SPEED = 600;
export type Phase = "ready" | "playing" | "paused" | "won" | "lost";
export type Ball = { id: number; x: number; y: number; vx: number; vy: number };
export type Brick = { id: number; x: number; y: number; hp: number; gold: boolean };
export type Drop = { id: number; x: number; y: number; letter: string; needed: boolean };
export type Particle = { id: number; x: number; y: number; vx: number; vy: number; life: number; gold: boolean };
export type Game = {
  level: Level; phase: Phase; resumePhase: "ready" | "playing"; paddle: number; balls: Ball[];
  bricks: Brick[]; drops: Drop[]; particles: Particle[]; collected: string[]; score: number;
  elapsed: number; hits: number; wave: number; nextId: number; seed: number; notice: string;
  noticeUntil: number; reason: string; catches: number; bounce: boolean;
  nextNeededAt: number; nextNeededHit: number; nextDropAt: number;
};

function bricks(wave: number): Brick[] {
  return Array.from({ length: 30 }, (_, i) => ({ id: wave * 100 + i,
    x: 12 + (i % 6) * 57, y: 24 + Math.floor(i / 6) * 29,
    hp: wave > 1 && i % 7 === 0 ? 2 : 1, gold: (i + wave) % 5 === 0 }));
}

export function createGame(level: Level): Game {
  return { level, phase: "ready", resumePhase: "ready", paddle: W / 2,
    balls: [{ id: 0, x: W / 2, y: PADDLE_Y - R - 2, vx: 0, vy: 0 }],
    bricks: bricks(0), drops: [], particles: [], collected: [], score: 0, elapsed: 0,
    hits: 0, wave: 0, nextId: 1, seed: level.id * 12345 + 17,
    notice: "Запустите шарик", noticeUntil: 3, reason: "", catches: 0, bounce: false,
    nextNeededAt: 0, nextNeededHit: 1, nextDropAt: 0 };
}

export function ballSpeed(g: Game): number {
  const multiplier = Math.min(MAX_SPEED_MULTIPLIER, 1 + Math.floor(g.elapsed / SPEED_INTERVAL) * SPEED_INCREMENT);
  return Math.min(MAX_BALL_SPEED, g.level.speed * multiplier);
}

function random(g: Game) {
  g.seed = (Math.imul(g.seed, 1664525) + 1013904223) >>> 0;
  return g.seed / 4294967296;
}

export function movePaddle(g: Game, x: number) {
  g.paddle = Math.max(PADDLE_W / 2 + 2, Math.min(W - PADDLE_W / 2 - 2, x));
  if (g.phase === "ready") g.balls[0].x = g.paddle;
}

export function launch(g: Game) {
  if (g.phase !== "ready") return;
  g.phase = "playing";
  const speed = ballSpeed(g);
  g.balls[0].vx = speed * 0.34;
  g.balls[0].vy = -speed * Math.sqrt(1 - 0.34 ** 2);
  g.notice = "Ловите буквы по порядку";
  g.noticeUntil = g.elapsed + 2;
}

export function togglePause(g: Game) {
  if (g.phase === "paused") g.phase = g.resumePhase;
  else if (g.phase === "playing" || g.phase === "ready") {
    g.resumePhase = g.phase;
    g.phase = "paused";
  }
}

function burst(g: Game, x: number, y: number, gold: boolean) {
  for (let i = 0; i < 7; i++) {
    g.particles.push({ id: g.nextId++, x, y, vx: (random(g) - 0.5) * 140,
      vy: (random(g) - 0.5) * 140, life: 0.45, gold });
  }
}

function scheduleNeededLetter(g: Game) {
  g.nextNeededAt = g.elapsed + 6 + random(g) * 4;
  g.nextNeededHit = g.hits + 3 + Math.floor(random(g) * 3);
}

function destroy(g: Game, brick: Brick) {
  g.hits++;
  g.score += brick.gold ? 80 : 40;
  burst(g, brick.x + 25, brick.y + 10, brick.gold);
  const target = letters(g.level.word);
  if (!g.drops.some(d => d.needed) && g.elapsed >= g.nextNeededAt
    && g.hits >= g.nextNeededHit && g.elapsed >= g.nextDropAt) {
    g.drops.push({ id: g.nextId++, x: brick.x + 25, y: brick.y + 20,
      letter: target[g.collected.length], needed: true });
    g.nextDropAt = g.elapsed + 2.5;
  } else if (g.hits % 3 === 0 && g.elapsed >= g.nextDropAt && g.drops.length < 3) {
    const choices = alphabet.filter(letter => letter !== target[g.collected.length]);
    g.drops.push({ id: g.nextId++, x: 24 + random(g) * (W - 48), y: brick.y,
      letter: choices[Math.floor(random(g) * choices.length)], needed: false });
    g.nextDropAt = g.elapsed + 2.5;
  }
  if (g.hits % 7 === 0 && g.balls.length < 3) {
    const speed = ballSpeed(g);
    g.balls.push({ id: g.nextId++, x: g.paddle, y: PADDLE_Y - 12,
      vx: (random(g) > 0.5 ? 1 : -1) * speed * 0.65, vy: -speed * Math.sqrt(1 - 0.65 ** 2) });
    g.notice = "+1 шарик";
    g.noticeUntil = g.elapsed + 2;
  }
}

export function step(g: Game, dt: number, direction = 0) {
  g.bounce = false;
  if (g.phase !== "playing" && g.phase !== "ready") return;
  if (direction) movePaddle(g, g.paddle + direction * 370 * dt);
  if (g.phase === "ready") return;
  const previousSpeed = ballSpeed(g);
  g.elapsed += dt;
  const speed = ballSpeed(g);
  if (speed > previousSpeed) {
    const ratio = speed / previousSpeed;
    for (const ball of g.balls) { ball.vx *= ratio; ball.vy *= ratio; }
    g.notice = "Шарики ускорились";
    g.noticeUntil = g.elapsed + 2;
  }
  for (const ball of [...g.balls]) {
    const oldX = ball.x, oldY = ball.y;
    ball.x += ball.vx * dt;
    ball.y += ball.vy * dt;
    if (ball.x < R || ball.x > W - R) {
      ball.x = Math.max(R, Math.min(W - R, ball.x));
      ball.vx *= -1;
    }
    if (ball.y < R) { ball.y = R; ball.vy = Math.abs(ball.vy); }
    if (ball.vy > 0 && oldY + R <= PADDLE_Y && ball.y + R >= PADDLE_Y
      && Math.abs(ball.x - g.paddle) <= PADDLE_W / 2 + R) {
      const offset = Math.max(-0.9, Math.min(0.9, (ball.x - g.paddle) / (PADDLE_W / 2)));
      const angle = offset * 1.08;
      ball.vx = Math.sin(angle) * speed;
      ball.vy = -Math.cos(angle) * speed;
      ball.y = PADDLE_Y - R - 0.1;
      g.bounce = true;
    }
    for (const brick of g.bricks) {
      if (brick.hp <= 0 || ball.x + R < brick.x || ball.x - R > brick.x + 51
        || ball.y + R < brick.y || ball.y - R > brick.y + 22) continue;
      if (oldY + R <= brick.y || oldY - R >= brick.y + 22) {
        ball.vy *= -1;
        ball.y = oldY;
      } else { ball.vx *= -1; ball.x = oldX; }
      brick.hp--;
      if (brick.hp === 0) destroy(g, brick);
      break;
    }
  }
  for (const drop of [...g.drops]) {
    const oldY = drop.y;
    drop.y += (78 + Math.min(g.level.id, 15) * 1.5) * dt;
    if (oldY + 13 <= PADDLE_Y + 8 && drop.y + 13 >= PADDLE_Y
      && Math.abs(drop.x - g.paddle) < PADDLE_W / 2 + 11) {
      g.collected.push(drop.letter);
      g.catches++;
      g.score += 100;
      g.notice = `${drop.letter} поймана · +100`;
      g.noticeUntil = g.elapsed + 2;
      burst(g, drop.x, PADDLE_Y, true);
      g.drops = g.drops.filter(d => d.id !== drop.id);
      scheduleNeededLetter(g);
      if (g.collected.length === letters(g.level.word).length) {
        g.phase = sameWord(g.collected.join(""), g.level.word) ? "won" : "lost";
        g.reason = g.phase === "lost" ? "Слово собрано неверно" : "";
        if (g.phase === "won") g.score += 500;
        return;
      }
    }
  }
  g.drops = g.drops.filter(d => d.y < H + 20);
  g.balls = g.balls.filter(ball => ball.y < H + R);
  if (g.balls.length === 0) { g.phase = "lost"; g.reason = "Все шарики потеряны"; }
  if (g.bricks.every(brick => brick.hp === 0)) { g.wave++; g.bricks = bricks(g.wave); }
  g.particles = g.particles.filter(p => p.life > 0);
  for (const p of g.particles) { p.x += p.vx * dt; p.y += p.vy * dt; p.life -= dt; }
}
