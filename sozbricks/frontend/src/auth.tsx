import "react-native-url-polyfill/auto";
import AsyncStorage from "@react-native-async-storage/async-storage";
import { createClient, Session } from "@supabase/supabase-js";
import Constants from "expo-constants";
import * as ExpoCrypto from "expo-crypto";
import * as WebBrowser from "expo-web-browser";
import React, { createContext, useContext, useEffect, useRef, useState } from "react";
import { AppState, Platform } from "react-native";
import { ApiError, errorText } from "./api";

const url = process.env.EXPO_PUBLIC_SUPABASE_URL;
const key = process.env.EXPO_PUBLIC_SUPABASE_PUBLISHABLE_KEY;
export const authConfigured = Boolean(url && key);
export const nativeGo = Platform.OS !== "web" && Constants.appOwnership === "expo";
if (Platform.OS !== "web") {
  const crypto = globalThis.crypto ?? ({} as typeof globalThis.crypto);
  if (!crypto.getRandomValues) crypto.getRandomValues = ExpoCrypto.getRandomValues as typeof crypto.getRandomValues;
  if (!crypto.subtle) Object.defineProperty(crypto, "subtle", { value: {
    digest: (algorithm: string | { name: string }, data: BufferSource) => {
      if ((typeof algorithm === "string" ? algorithm : algorithm.name) !== "SHA-256") throw new Error("Unsupported auth digest");
      return ExpoCrypto.digest(ExpoCrypto.CryptoDigestAlgorithm.SHA256, data);
    },
  } });
  if (!globalThis.crypto) Object.defineProperty(globalThis, "crypto", { value: crypto, configurable: true });
}
const storage = {
  getItem: (key: string) => Platform.OS === "web" && typeof window === "undefined" ? Promise.resolve(null) : AsyncStorage.getItem(key),
  setItem: (key: string, value: string) => Platform.OS === "web" && typeof window === "undefined" ? Promise.resolve() : AsyncStorage.setItem(key, value),
  removeItem: (key: string) => Platform.OS === "web" && typeof window === "undefined" ? Promise.resolve() : AsyncStorage.removeItem(key),
};
const client = url && key ? createClient(url, key, { auth: { storage, persistSession: true,
  autoRefreshToken: true, detectSessionInUrl: false, flowType: "pkce" } }) : null;
let exchange: { code: string; promise: Promise<void> } | undefined;

export async function accessToken(owner: string, refresh = false): Promise<string> {
  if (!client) throw new Error("Не настроено подключение к Supabase.");
  const current = await client.auth.getSession();
  if (current.error || current.data.session?.user.id !== owner) throw new ApiError(401, "account_changed", "Войдите в аккаунт этой попытки.");
  const result = refresh ? await client.auth.refreshSession() : current;
  if (result.error || result.data.session?.user.id !== owner) throw new ApiError(401, "unauthorized", "Войдите через Google снова.");
  return result.data.session.access_token;
}

type Auth = { session: Session | null; loaded: boolean; busy: boolean; error: string;
  signIn: () => Promise<void>; signOut: () => Promise<void>; complete: (code: string, flowId?: string) => Promise<void> };
const Context = createContext<Auth | null>(null);

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [session, setSession] = useState<Session | null>(null);
  const [loaded, setLoaded] = useState(!client);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const locked = useRef(false);
  useEffect(() => {
    if (!client) return;
    let alive = true, eventReceived = false;
    const { data } = client.auth.onAuthStateChange((_event, value) => {
      eventReceived = true;
      if (alive) { setSession(value); setLoaded(true); }
    });
    void client.auth.getSession().then(({ data, error }) => {
      if (!alive) return;
      if (!eventReceived) setSession(data.session);
      if (error) setError("Не удалось восстановить вход. Войдите через Google снова.");
      setLoaded(true);
    }).catch(() => { if (alive) { setError("Не удалось восстановить вход."); setLoaded(true); } });
    const sub = AppState.addEventListener("change", state => {
      if (state === "active") client.auth.startAutoRefresh();
      else client.auth.stopAutoRefresh();
    });
    return () => { alive = false; data.subscription.unsubscribe(); sub.remove(); };
  }, []);
  const complete = async (code: string, flowId?: string) => {
    if (!client || !code) throw new Error("Не получен код входа Google.");
    if (!exchange || exchange.code !== code) {
      exchange = { code, promise: client.auth.exchangeCodeForSession(code, flowId ? { flowId } : undefined).then(({ error }) => {
        if (error) throw new Error("Не удалось завершить вход. Вернитесь в кабинет и войдите снова.");
      }) };
    }
    await exchange.promise;
  };
  const signIn = async () => {
    if (locked.current) return;
    locked.current = true; setBusy(true); setError("");
    try {
      if (!client) throw new Error("Добавьте публичные настройки Supabase в .env.local и перезапустите Expo.");
      if (nativeGo) throw new Error("В Expo Go доступна гостевая игра. Для Google-входа используйте веб или отдельную сборку приложения.");
      if (Platform.OS === "web" && !window.isSecureContext) throw new Error("Для входа откройте игру через localhost на ноутбуке или HTTPS.");
      const redirectTo = Platform.OS === "web" ? `${window.location.origin}/auth/callback` : "sozbricks://auth/callback";
      const { data, error } = await client.auth.signInWithOAuth({ provider: "google",
        options: { redirectTo, skipBrowserRedirect: true, queryParams: { prompt: "select_account" } } });
      if (error || !data.url) throw new Error("Google-вход недоступен. Проверьте настройки провайдера в Supabase.");
      if (Platform.OS === "web") window.location.assign(data.url);
      else {
        const result = await WebBrowser.openAuthSessionAsync(data.url, redirectTo);
        if (result.type === "success") {
          const parsed = new URL(result.url);
          if (parsed.searchParams.has("error")) throw new Error("Вход отменён или отклонён провайдером.");
          await complete(parsed.searchParams.get("code") ?? "", parsed.searchParams.get("sb_flow_id") ?? undefined);
        }
      }
    } catch (error) { setError(errorText(error)); }
    finally { locked.current = false; setBusy(false); }
  };
  const signOut = async () => {
    setBusy(true); setError("");
    try {
      const result = await client?.auth.signOut({ scope: "local" });
      if (result?.error) throw result.error;
      setSession(null);
    } catch { setError("Не удалось выйти. Повторите попытку."); }
    finally { setBusy(false); }
  };
  return <Context.Provider value={{ session, loaded, busy, error, signIn, signOut, complete }}>{children}</Context.Provider>;
}

export function useAuth() {
  const auth = useContext(Context);
  if (!auth) throw new Error("AuthProvider is missing");
  return auth;
}
