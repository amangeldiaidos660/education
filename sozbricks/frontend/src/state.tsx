import AsyncStorage from "@react-native-async-storage/async-storage";
import React, { createContext, useContext, useEffect, useRef, useState } from "react";
import { Game } from "./game/engine";
import { initialSave, recordResult, Save } from "./model";
const key = "sozbricks.local.v1";

type State = { save: Save; loaded: boolean; storageError: string; update: (patch: Partial<Save>) => void;
  finish: (game: Game, id: string) => void };
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
  const [save, setSave] = useState<Save>(initialSave);
  const [loaded, setLoaded] = useState(false);
  const [storageError, setStorageError] = useState("");
  const writes = useRef(Promise.resolve());
  useEffect(() => {
    let alive = true;
    AsyncStorage.getItem(key).then(raw => {
      if (!alive || !raw) return;
      const parsed: unknown = JSON.parse(raw);
      if (validSave(parsed)) setSave(parsed);
      else setStorageError("Сохранение не удалось прочитать. Начат новый локальный профиль.");
    }).catch(() => { if (alive) setStorageError("Хранилище недоступно. Прогресс сохраняется только в этой сессии."); })
      .finally(() => { if (alive) setLoaded(true); });
    return () => { alive = false; };
  }, []);
  useEffect(() => {
    if (!loaded) return;
    writes.current = writes.current.then(() => AsyncStorage.setItem(key, JSON.stringify(save)))
      .catch(() => setStorageError("Не удалось сохранить прогресс на устройстве."));
  }, [save, loaded]);
  return <Context.Provider value={{ save, loaded, storageError,
    update: patch => setSave(s => ({ ...s, ...patch })),
    finish: (game, id) => setSave(s => recordResult(s, game, id)) }}>{children}</Context.Provider>;
}

export function useStore() {
  const value = useContext(Context);
  if (!value) throw new Error("StoreProvider is missing");
  return value;
}
