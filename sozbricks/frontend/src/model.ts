import type { Game } from "./game/engine";

export type Result = { id: string; level: number; word: string; score: number; won: boolean; seconds: number; at: number };
export type Save = { version: 1; dark: boolean; sound: boolean; vibration: boolean; hero: "alikhan" | "aiaru";
  completed: number[]; best: Record<string, number>; history: Result[]; demoLamports: number };
export const initialSave: Save = { version: 1, dark: true, sound: true, vibration: true, hero: "alikhan",
  completed: [], best: {}, history: [], demoLamports: 0 };

export function recordResult(save: Save, game: Pick<Game, "level" | "score" | "elapsed" | "phase">, id: string): Save {
  if (save.history.some(r => r.id === id)) return save;
  const won = game.phase === "won";
  const firstWin = won && !save.completed.includes(game.level.id);
  return { ...save,
    completed: firstWin ? [...save.completed, game.level.id] : save.completed,
    best: { ...save.best, [game.level.id]: Math.max(save.best[game.level.id] ?? 0, game.score) },
    demoLamports: save.demoLamports + (firstWin ? Math.round(game.level.reward * 1e9) : 0),
    history: [{ id, level: game.level.id, word: game.level.word, score: game.score, won,
      seconds: Math.floor(game.elapsed), at: Date.now() }, ...save.history].slice(0, 100) };
}
