import type { Level } from "./game/words";
import type { Result, Save } from "./model";

export const apiUrl = (process.env.EXPO_PUBLIC_API_URL ?? "https://sozbricks-api-tests.onrender.com").replace(/\/$/, "");
export type Profile = { user_id: string; display_name: string; email: string; hero: Save["hero"] };
export type Progress = { completed: number[]; best: Record<string, number>; earned_lamports: number };
export type ServerLevel = Omit<Level, "reward"> & { reward_lamports: number; unlocked: boolean; rules_version: number };
export type Session = { id: string; request_id: string; level_id: number; seed: number; word: string; speed: number;
  reward_lamports: number; status: string; expires_at: string; rules_version: number };
export type Outcome = { won: boolean; score: number; collected_word: string; duration_seconds: number };
export type Finished = Outcome & { session_id: string; awarded_lamports: number; progress: Progress };
export type History = { items: { session_id: string; level_id: number; word: string; score: number; won: boolean;
  duration_seconds: number; recorded_at: string }[]; imported_history: Result[] };
export type Rewards = { total_lamports: number; solana_payments_enabled: boolean;
  paid_lamports?: number; pending_lamports?: number; historical_lamports?: number; wallet_address?: string | null; activated_at?: string | null;
  items: { id: string; level_id: number; amount_lamports: number; created_at: string; status: string;
    recipient?: string | null; signature?: string | null; explorer_url?: string | null; error_code?: string | null }[] };
export type AccountData = { profile: Profile; progress: Progress; levels: ServerLevel[]; history: History; rewards: Rewards };
export type Api = <T>(path: string, method?: string, body?: unknown) => Promise<T>;

export class ApiError extends Error {
  constructor(public status: number, public code: string, message: string) { super(message); }
  get terminal() { return [400, 403, 404, 409, 410, 422].includes(this.status); }
}

export function errorText(error: unknown): string {
  return error instanceof Error ? error.message : "Не удалось выполнить запрос. Повторите позже.";
}

export function createApi(token: (refresh: boolean) => Promise<string>, transport: typeof fetch = fetch): Api {
  return async <T>(path: string, method = "GET", body?: unknown): Promise<T> => {
    for (let attempt = 0; attempt < 2; attempt++) {
      const accessToken = await token(attempt === 1);
      const controller = new AbortController();
      const timer = setTimeout(() => controller.abort(), 25000);
      try {
        const response = await transport(`${apiUrl}${path}`, { method, signal: controller.signal,
          headers: { Authorization: `Bearer ${accessToken}`, ...(body === undefined ? {} : { "Content-Type": "application/json" }) },
          body: body === undefined ? undefined : JSON.stringify(body) });
        if (response.status === 401 && attempt === 0) continue;
        const value = await response.json().catch(() => null);
        if (!response.ok) {
          const messages: Record<number, string> = { 401: "Сессия истекла. Войдите через Google снова.",
            403: "Уровень пока закрыт или доступ запрещён.", 404: "Попытка не найдена для этого аккаунта.",
            409: path.startsWith("/v1/wallet") ? "Запрос подписи устарел, уже использован или адрес привязан к другому аккаунту. Повторите подключение через несколько секунд." : "Конфликт сохранения: попытка уже завершена или гостевой прогресс уже импортирован.",
            410: "Время серверной попытки истекло (2 часа).", 422: "Сервер отклонил данные результата.",
            503: "Сервер или база временно недоступны. Повторите позже." };
          throw new ApiError(response.status, value?.error?.code ?? "request_failed",
            messages[response.status] ?? `Ошибка сервера (${response.status}). Повторите позже.`);
        }
        if (!value || typeof value !== "object") throw new Error("Сервер вернул некорректный ответ.");
        return value as T;
      } catch (error) {
        if (error instanceof ApiError) throw error;
        throw new Error("Нет ответа от API. Проверьте интернет; для веба также проверьте CORS на сервере.");
      } finally { clearTimeout(timer); }
    }
    throw new ApiError(401, "unauthorized", "Войдите через Google снова.");
  };
}

export function accountSave(local: Save, data: AccountData): Save {
  const history: Result[] = data.history.items.map(item => ({ id: item.session_id, level: item.level_id,
    word: item.word, score: item.score, won: item.won, seconds: item.duration_seconds,
    at: Date.parse(item.recorded_at.replace(" ", "T").replace(/([+-]\d{2})$/, "$1:00")) }));
  return { ...local, hero: data.profile.hero, completed: data.progress.completed, best: data.progress.best,
    demoLamports: data.progress.earned_lamports,
    history: [...history, ...data.history.imported_history.map(item => ({ ...item, id: `guest:${item.id}` }))]
      .sort((a, b) => b.at - a.at).slice(0, 100) };
}
