import AsyncStorage from "@react-native-async-storage/async-storage";
import React, { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { AppState, Platform } from "react-native";
import { randomUUID } from "expo-crypto";
import { Game } from "./game/engine";
import { levels as localLevels } from "./game/words";
import { initialSave, recordResult, Save } from "./model";
import { accessToken, useAuth } from "./auth";
import { accountSave, createApi, errorText } from "./api";
import { AccountSync, SyncView } from "./sync";

const key = "sozbricks.local.v1";
type State = { save: Save; guest: Save; loaded: boolean; storageError: string; account: boolean;
  sync: AccountSync | null; syncView: SyncView | null; levels: typeof localLevels;
  update: (patch: Partial<Save>) => void; finish: (game: Game, id: string) => Promise<number> };
const Context = createContext<State | null>(null);

function validSave(value: unknown): value is Save {
  if (!value || typeof value !== "object") return false;
  const s = value as Save;
  return s.version === 1 && typeof s.dark === "boolean" && typeof s.sound === "boolean"
    && typeof s.vibration === "boolean" && ["alikhan", "aiaru"].includes(s.hero)
    && Array.isArray(s.completed) && s.completed.every(n => Number.isInteger(n) && n >= 1 && n <= 24)
    && s.best !== null && typeof s.best === "object" && Object.values(s.best).every(n => Number.isFinite(n) && n >= 0)
    && Array.isArray(s.history) && s.history.every(r => typeof r.id === "string" && typeof r.word === "string"
      && typeof r.won === "boolean" && Number.isFinite(r.score) && Number.isFinite(r.at))
    && Number.isSafeInteger(s.demoLamports) && s.demoLamports >= 0;
}

export function StoreProvider({ children }: { children: React.ReactNode }) {
  const auth = useAuth();
  const owner = auth.session?.user.id;
  const [guest, setGuest] = useState<Save>(initialSave);
  const guestRef = useRef(guest);
  const [loaded, setLoaded] = useState(false);
  const [storageError, setStorageError] = useState("");
  const writes = useRef(Promise.resolve());
  const sync = useMemo(() => owner ? new AccountSync(owner, createApi(refresh => accessToken(owner, refresh)), AsyncStorage, randomUUID) : null, [owner]);
  const subscribe = useCallback((notify: () => void) => sync ? sync.subscribe(notify) : () => {}, [sync]);
  const snapshot = useCallback(() => sync?.snapshot() ?? null, [sync]);
  const syncView = useSyncExternalStore(subscribe, snapshot, snapshot);
  useEffect(() => {
    if (!sync) return;
    const refresh = () => {
      const view = sync.snapshot();
      if (!view.busy && (!view.disk.attempt || view.disk.attempt.action || !view.disk.data)) void sync.refresh().catch(() => {});
    };
    refresh();
    const interval = setInterval(refresh, 30000);
    const sub = AppState.addEventListener("change", state => { if (state === "active") refresh(); });
    if (Platform.OS === "web") window.addEventListener("online", refresh);
    return () => { clearInterval(interval); sub.remove(); if (Platform.OS === "web") window.removeEventListener("online", refresh); };
  }, [sync]);
  useEffect(() => {
    let alive = true;
    AsyncStorage.getItem(key).then(raw => {
      if (!alive || !raw) return;
      const parsed: unknown = JSON.parse(raw);
      if (validSave(parsed)) { guestRef.current = parsed; setGuest(parsed); }
      else setStorageError("Сохранение не удалось прочитать. Начат новый локальный профиль.");
    }).catch(() => { if (alive) setStorageError("Хранилище недоступно. Прогресс сохраняется только в этой сессии."); })
      .finally(() => { if (alive) setLoaded(true); });
    return () => { alive = false; };
  }, []);
  const writeGuest = (next: Save) => {
    guestRef.current = next; setGuest(next);
    const task = writes.current.then(() => AsyncStorage.setItem(key, JSON.stringify(next)));
    writes.current = task.catch(() => { setStorageError("Не удалось сохранить прогресс на устройстве."); });
    return task;
  };
  const data = syncView?.disk.data;
  const save = owner ? data ? accountSave(guest, data) : { ...initialSave, dark: guest.dark, sound: guest.sound, vibration: guest.vibration } : guest;
  const levels = useMemo(() => owner && data ? data.levels.map(level => ({ id: level.id, word: level.word,
    translation: level.translation, reward: level.reward_lamports / 1e9, speed: level.speed })) : localLevels, [owner, data]);
  const update = (patch: Partial<Save>) => {
    if (sync && patch.hero) void sync.patchProfile({ hero: patch.hero }).catch(() => {});
    const local = sync ? Object.fromEntries(Object.entries(patch).filter(([key]) => ["dark", "sound", "vibration"].includes(key))) : patch;
    if (Object.keys(local).length) void writeGuest({ ...guestRef.current, ...local }).catch(() => {});
  };
  const finish = async (game: Game, id: string) => {
    if (sync) {
      const result = await sync.finish(id, { won: game.phase === "won", score: game.score,
        collected_word: game.collected.join("").normalize("NFC"), duration_seconds: Math.floor(game.elapsed) });
      void sync.refresh().catch(() => {});
      return result.awarded_lamports;
    }
    const previous = guestRef.current;
    const next = recordResult(previous, game, id);
    try { await writeGuest(next); } catch (error) { throw new Error(`Результат остался в памяти: ${errorText(error)}`); }
    return next.demoLamports - previous.demoLamports;
  };
  return <Context.Provider value={{ save, guest, loaded: loaded && auth.loaded, storageError, update, finish,
    sync, syncView, levels, account: Boolean(owner) }}>{children}</Context.Provider>;
}

export function useStore() {
  const value = useContext(Context);
  if (!value) throw new Error("StoreProvider is missing");
  return value;
}
