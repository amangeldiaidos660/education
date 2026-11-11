import React, { useState } from "react";
import { TextInput, View } from "react-native";
import { authConfigured, nativeGo, useAuth } from "./auth";
import { useStore } from "./state";
import { Button, Card, Txt, usePalette } from "./ui";
import { formatSol } from "./game/words";

export function AccountPanel() {
  const c = usePalette();
  const auth = useAuth();
  const { sync, syncView, guest } = useStore();
  const data = syncView?.disk.data;
  const attempt = syncView?.disk.attempt;
  const imported = syncView?.disk.imported;
  const [draft, setDraft] = useState<{ owner: string; base: string; value: string } | null>(null);
  const base = data?.profile.display_name ?? "";
  const owner = auth.session?.user.id ?? "";
  const name = draft?.owner === owner && draft.base === base ? draft.value : base;
  const busy = auth.busy || Boolean(syncView?.busy);
  return <Card style={{ gap: 16 }}><Txt size={23} weight="extra">Учётная запись</Txt>
    {auth.session ? <>
      <Txt weight="bold">{data?.profile.display_name ?? "Аккаунт Google"}</Txt>
      <Txt size={13} color={c.muted}>{auth.session.user.email}</Txt>
      <Txt size={13} color={c.muted}>{syncView?.busy ? "Синхронизация…" : data ? "Прогресс аккаунта загружен" : "Ожидаем данные сервера"}</Txt>
      <View style={{ flexDirection: "row", flexWrap: "wrap", gap: 10 }}>
        <Button small disabled={busy} onPress={() => { void sync?.refresh().catch(() => {}); }}>Обновить данные</Button>
        <Button small secondary disabled={busy} onPress={() => { void auth.signOut(); }}>Выйти</Button>
      </View>
      {!!attempt?.action && !attempt.rejected && <Txt size={13} color={c.goldText}>Есть неотправленная попытка. Она сохранена на устройстве и будет отправлена повторно при восстановлении связи с этим аккаунтом.</Txt>}
      {!!attempt?.rejected && <>
        <Txt size={13} color={c.danger}>{attempt.rejected} Результат не засчитан сервером.</Txt>
        <Button secondary disabled={busy} onPress={() => { void sync?.dismissRejected().catch(() => {}); }}>Продолжить без этого результата</Button>
      </>}
      {data && <>
        <Txt size={13}>Имя игрока</Txt>
        <TextInput accessibilityLabel="Имя игрока" value={name} onChangeText={value => setDraft({ owner, base, value })} maxLength={60}
          style={{ color: c.text, backgroundColor: c.soft, borderColor: c.line, borderWidth: 1, borderRadius: 12, padding: 14 }} />
        <Button small secondary disabled={busy || !name.trim() || name.trim() === data.profile.display_name}
          onPress={() => { void sync?.patchProfile({ display_name: name.trim() }).catch(() => {}); }}>Сохранить имя</Button>
        {!!guest.history.length && !imported?.done && <>
          <Txt size={13} color={c.muted}>На устройстве есть гостевой прогресс: {guest.completed.length} пройденных уровней. Его можно перенести один раз в один аккаунт. Демо-баланс не переносится.</Txt>
          <Button secondary disabled={busy || Boolean(imported?.rejected)} onPress={() => { void sync?.importGuest(guest).catch(() => {}); }}>Перенести гостевой прогресс</Button>
        </>}
        {imported?.done && <Txt size={12} color={c.accent}>Гостевой прогресс перенесён.</Txt>}
        {!!imported?.rejected && <Txt size={12} color={c.danger}>{imported.rejected}</Txt>}
      </>}
    </> : <>
      <Txt color={c.muted}>Войдите через Google, чтобы сохранять результаты и продолжать игру на другом устройстве. Без входа доступен гостевой режим.</Txt>
      <Button disabled={busy || nativeGo || !authConfigured} onPress={() => { void auth.signIn(); }}>{busy ? "Открываем Google…" : "Войти через Google"}</Button>
      {nativeGo && <Txt size={13} color={c.goldText}>В Expo Go доступна гостевая игра. Google-вход работает в веб-версии или отдельной сборке приложения.</Txt>}
      {!authConfigured && <Txt size={13} color={c.danger}>Подключение к Google ещё не настроено.</Txt>}
    </>}
    {!!auth.error && <Txt size={13} color={c.danger}>{auth.error}</Txt>}
    {!!syncView?.error && <Txt size={13} color={c.danger}>{syncView.error}</Txt>}
  </Card>;
}

export function RewardHistory() {
  const c = usePalette();
  const { syncView } = useStore();
  const rewards = syncView?.disk.data?.rewards;
  if (!rewards) return null;
  return <Card style={{ gap: 14 }}><Txt size={23} weight="extra">Начисления в базе</Txt>
    <Txt size={12} color={c.muted}>Тестовый учёт devnet SOL. Переводов в кошелёк пока нет.</Txt>
    {!rewards.items.length && <Txt color={c.muted}>Начислений пока нет. Завершите уровень после входа.</Txt>}
    {rewards.items.slice(0, 24).map(item => <View key={item.id} style={{ flexDirection: "row", justifyContent: "space-between", gap: 12 }}>
      <Txt size={13}>Уровень {item.level_id}</Txt><Txt size={13} color={c.goldText}>+{formatSol(item.amount_lamports)} devnet SOL · не выплачено</Txt>
    </View>)}
  </Card>;
}
