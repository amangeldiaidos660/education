import { AccountData, Api, ApiError, errorText, Finished, History, Outcome, Profile, Progress, Rewards, ServerLevel, Session } from "./api";
import type { Save } from "./model";

type Storage = { getItem: (key: string) => Promise<string | null>; setItem: (key: string, value: string) => Promise<void> };
type Attempt = { requestId: string; levelId: number; session?: Session; action?: "finish" | "abandon"; outcome?: Outcome; rejected?: string };
type Import = { import_id: string; save: Pick<Save, "version" | "hero" | "completed" | "best" | "history">; done?: boolean; rejected?: string };
type Disk = { version: 1; owner: string; data?: AccountData; attempt?: Attempt; imported?: Import; receipt?: Finished };
export type SyncView = { loaded: boolean; busy: boolean; error: string; disk: Disk };

export class AccountSync {
  private disk: Disk;
  private view: SyncView;
  private tail: Promise<unknown> = Promise.resolve();
  private listeners = new Set<(view: SyncView) => void>();
  private initialized = false;
  private key: string;
  constructor(private owner: string, private api: Api, private storage: Storage, private uuid: () => string) {
    this.key = `sozbricks.account.v1.${owner}`;
    this.disk = { version: 1, owner };
    this.view = { loaded: false, busy: false, error: "", disk: this.disk };
  }
  snapshot() { return this.view; }
  subscribe(listener: (view: SyncView) => void) { this.listeners.add(listener); return () => { this.listeners.delete(listener); }; }
  private emit(patch: Partial<SyncView> = {}) {
    this.view = { ...this.view, ...patch, disk: this.disk };
    this.listeners.forEach(listener => listener(this.view));
  }
  private async persist(disk: Disk) {
    await this.storage.setItem(this.key, JSON.stringify(disk));
    this.disk = disk;
    this.emit();
  }
  private async init() {
    if (this.initialized) return;
    const raw = await this.storage.getItem(this.key);
    if (raw) {
      const disk: Disk = JSON.parse(raw);
      if (disk.version !== 1 || disk.owner !== this.owner) throw new Error("Не удалось прочитать сохранение аккаунта.");
      this.disk = disk;
    }
    this.initialized = true;
    this.emit({ loaded: true });
  }
  private run<T>(action: () => Promise<T>): Promise<T> {
    const result = this.tail.then(async () => {
      this.emit({ busy: true, error: "" });
      try { await this.init(); return await action(); }
      catch (error) { this.emit({ error: errorText(error) }); throw error; }
      finally { this.emit({ busy: false }); }
    });
    this.tail = result.catch(() => {});
    return result;
  }
  private async load() {
    const [profile, levels, history, rewards] = await Promise.all([
      this.api<{ profile: Profile; progress: Progress }>("/v1/profile"),
      this.api<{ items: ServerLevel[] }>("/v1/levels"),
      this.api<History>("/v1/history?limit=100"), this.api<Rewards>("/v1/rewards?limit=100"),
    ]);
    await this.persist({ ...this.disk, data: { ...profile, levels: levels.items, history, rewards } });
  }
  private async resolveSession(attempt: Attempt): Promise<Session> {
    if (attempt.session) return attempt.session;
    const session = await this.api<Session>("/v1/game-sessions", "POST", { level_id: attempt.levelId, request_id: attempt.requestId });
    await this.persist({ ...this.disk, attempt: { ...attempt, session } });
    return session;
  }
  private async flushAttempt() {
    const attempt = this.disk.attempt;
    if (!attempt?.action || attempt.rejected) return;
    try {
      const session = await this.resolveSession(attempt);
      if (attempt.action === "finish") {
        const receipt = await this.api<Finished>(`/v1/game-sessions/${session.id}/finish`, "POST", attempt.outcome);
        const data = this.disk.data ? { ...this.disk.data, progress: receipt.progress } : undefined;
        await this.persist({ ...this.disk, attempt: undefined, receipt, data });
      } else {
        await this.api(`/v1/game-sessions/${session.id}/abandon`, "POST");
        await this.persist({ ...this.disk, attempt: undefined });
      }
    } catch (error) {
      if (error instanceof ApiError && error.terminal) {
        await this.persist({ ...this.disk, attempt: { ...this.disk.attempt!, rejected: error.message } });
      }
      throw error;
    }
  }
  private async flushImport() {
    const imported = this.disk.imported;
    if (!imported || imported.done || imported.rejected) return;
    try {
      await this.api("/v1/progress/import", "POST", { import_id: imported.import_id, save: imported.save });
      await this.persist({ ...this.disk, imported: { ...imported, done: true } });
    } catch (error) {
      if (error instanceof ApiError && error.terminal) {
        await this.persist({ ...this.disk, imported: { ...imported, rejected: error.message } });
      }
      throw error;
    }
  }
  refresh() { return this.run(async () => { await this.flushAttempt(); await this.flushImport(); await this.load(); }); }
  start(levelId: number, requestId: string) {
    return this.run(async () => {
      await this.flushAttempt();
      if (this.disk.attempt?.rejected) throw new Error("Предыдущий результат отклонён. Откройте кабинет и подтвердите продолжение без него.");
      if (this.disk.attempt && this.disk.attempt.requestId !== requestId) {
        await this.persist({ ...this.disk, attempt: { ...this.disk.attempt, action: "abandon" } });
        await this.flushAttempt();
      }
      if (!this.disk.attempt) await this.persist({ ...this.disk, attempt: { levelId, requestId } });
      const session = await this.resolveSession(this.disk.attempt!);
      if (session.status !== "started" || session.rules_version !== 1) throw new Error("Попытка недоступна. Вернитесь к уровням и начните новую.");
      return session;
    });
  }
  finish(id: string, outcome: Outcome) {
    return this.run(async () => {
      if (this.disk.receipt?.session_id === id) return this.disk.receipt;
      const attempt = this.disk.attempt;
      if (attempt?.session?.id !== id) throw new Error("Эта попытка не принадлежит текущему сохранению.");
      if (attempt.action && (attempt.action !== "finish" || JSON.stringify(attempt.outcome) !== JSON.stringify(outcome))) {
        throw new Error("Результат попытки уже зафиксирован и не может быть изменён.");
      }
      await this.persist({ ...this.disk, attempt: { ...attempt, action: "finish", outcome: { ...outcome } } });
      await this.flushAttempt();
      if (this.disk.attempt?.rejected) throw new Error(this.disk.attempt.rejected);
      return this.disk.receipt!;
    });
  }
  abandon(id: string) {
    return this.run(async () => {
      if (this.disk.attempt?.session?.id !== id || this.disk.attempt.action === "finish") return;
      await this.persist({ ...this.disk, attempt: { ...this.disk.attempt, action: "abandon" } });
      await this.flushAttempt();
    });
  }
  patchProfile(patch: { display_name?: string; hero?: Save["hero"] }) {
    return this.run(async () => {
      const result = await this.api<{ profile: Profile; progress: Progress }>("/v1/profile", "PATCH", patch);
      if (this.disk.data) await this.persist({ ...this.disk, data: { ...this.disk.data, ...result } });
      else await this.load();
    });
  }
  importGuest(save: Save) {
    return this.run(async () => {
      const key = "sozbricks.guest-import-owner.v1";
      const owner = await this.storage.getItem(key);
      if (owner && owner !== this.owner) throw new Error("Гостевое сохранение на этом устройстве уже связано с другим аккаунтом.");
      await this.storage.setItem(key, this.owner);
      if (!this.disk.imported) {
        const { version, hero, completed, best, history } = save;
        const imported = JSON.parse(JSON.stringify({ import_id: this.uuid(), save: { version, hero, completed, best, history } })) as Import;
        await this.persist({ ...this.disk, imported });
      }
      if (this.disk.imported?.rejected) throw new Error(this.disk.imported.rejected);
      await this.flushImport();
      await this.load();
    });
  }
  dismissRejected() {
    return this.run(async () => {
      if (!this.disk.attempt?.rejected) return;
      const key = `${this.key}.rejected.${this.disk.attempt.requestId}`;
      await this.storage.setItem(key, JSON.stringify(this.disk.attempt));
      await this.persist({ ...this.disk, attempt: undefined });
      await this.load();
    });
  }
}
