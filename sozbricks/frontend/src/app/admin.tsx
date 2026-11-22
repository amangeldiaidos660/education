import React, { useEffect, useState } from "react";
import { Linking, View } from "react-native";
import { accessToken, useAuth } from "../auth";
import { createApi, errorText } from "../api";
import { useStore } from "../state";
import { formatSol } from "../game/words";
import { Button, Card, Eyebrow, Shell, Txt, usePalette } from "../ui";

type Account = { player_id: string; display_name: string; archived: boolean; recovery_ready: boolean; wallet_address: string | null;
  wallet_reconfirmation_required: boolean; completed_levels: number; imported_levels: number; paid_lamports: number; pending_lamports: number };
type Reward = { reward_id: string; player_id: string; display_name: string; level_id: number; amount_lamports: number;
  status: string; recipient: string | null; signature: string | null; error_code: string | null; created_at: string; next_attempt_at: string | null };
type Page<T> = { items: T[]; total: number };
type Overview = { summary: { accounts: number; archived_accounts: number; wallets: number; wallets_to_reconfirm: number;
  paid_lamports: number; pending_lamports: number }; statuses: { status: string; count: number }[]; payments_enabled: boolean;
  campaign: { activated_at: string | null; program_address: string | null } | null;
  chain: { status: string; error_code?: string; program_address?: string; treasury_address?: string; reserve_address?: string;
    treasury_lamports?: number; reserve_lamports?: number } };
const statusNames: Record<string, string> = { historical: "Историческая · без выплаты", awaiting_wallet: "Ожидает кошелёк",
  queued: "В очереди", submitted: "Подтверждается сетью", paid: "Выплачено", retry: "Повторная проверка", blocked: "Нужна проверка" };
const pageSize = 25;

export default function Admin() {
  const c = usePalette();
  const { session } = useAuth();
  const { syncView } = useStore();
  const owner = session?.user.id;
  const allowed = syncView?.disk.data?.profile.is_admin === true;
  const [accountOffset, setAccountOffset] = useState(0);
  const [rewardOffset, setRewardOffset] = useState(0);
  const [player, setPlayer] = useState<string>();
  const [refresh, setRefresh] = useState(0);
  const [result, setResult] = useState<{ requestKey: string; owner: string; overview: Overview; accounts: Page<Account>; rewards: Page<Reward>; at: string }>();
  const [completedKey, setCompletedKey] = useState("");
  const requestKey = JSON.stringify([owner, allowed, accountOffset, rewardOffset, player, refresh]);
  const busy = Boolean(owner && allowed && completedKey !== requestKey);
  const [error, setError] = useState("");
  useEffect(() => {
    if (!owner || !allowed) return;
    let alive = true;
    const api = createApi(renew => accessToken(owner, renew));
    void Promise.all([
      api<Overview>("/v1/admin/overview"),
      api<Page<Account>>(`/v1/admin/accounts?limit=${pageSize}&offset=${accountOffset}`),
      api<Page<Reward>>(`/v1/admin/rewards?limit=${pageSize}&offset=${rewardOffset}${player ? `&player_id=${encodeURIComponent(player)}` : ""}`),
    ]).then(([overview, accounts, rewards]) => {
      if (alive) { setError(""); setResult({ requestKey, owner, overview, accounts, rewards, at: new Date().toLocaleTimeString() }); }
    }).catch(err => { if (alive) { setError(errorText(err)); setResult(undefined); } })
      .finally(() => { if (alive) setCompletedKey(requestKey); });
    return () => { alive = false; };
  }, [owner, allowed, accountOffset, rewardOffset, player, refresh, requestKey]);
  const data = result && result.owner === owner && result.requestKey === requestKey && allowed ? result : undefined;
  const pageControls = (offset: number, total: number, setOffset: (n: number) => void) => <View style={{ flexDirection: "row", gap: 12, flexWrap: "wrap", alignItems: "center" }}>
    <Button small secondary disabled={busy || offset === 0} onPress={() => setOffset(Math.max(0, offset - pageSize))}>Назад</Button>
    <Txt size={12}>{total ? `${offset + 1}–${Math.min(offset + pageSize, total)} из ${total}` : "Нет записей"}</Txt>
    <Button small secondary disabled={busy || offset + pageSize >= total} onPress={() => setOffset(offset + pageSize)}>Далее</Button>
  </View>;
  return <Shell><Eyebrow>Управление проектом · только просмотр</Eyebrow><Txt size={32} weight="extra">Админпанель</Txt>
    <Txt color={c.goldText}>Тестовые SOL / Devnet</Txt>
    {!owner ? <Txt>Войдите через Google в кабинет администратора.</Txt> : !allowed ? <Txt>Доступ не предоставлен или профиль ещё загружается.</Txt> : <>
      <Button small disabled={busy} onPress={() => setRefresh(value => value + 1)}>{busy ? "Загружаем…" : "Обновить статусы"}</Button>
      {completedKey === requestKey && !!error && <Txt color={c.danger}>{error}</Txt>}
      {data && <>
        <Txt size={12} color={c.muted}>Обновлено: {data.at}. Статусы не обновляются автоматически.</Txt>
        <Card style={{ gap: 10 }}><Txt size={23} weight="extra">Состояние проекта</Txt>
          <Txt>Обработчик выплат: {data.overview.payments_enabled ? "включён" : "выключен"}</Txt>
          <Txt size={13}>Начисления активированы: {data.overview.campaign?.activated_at ?? "нет"}</Txt>
          <Txt>Аккаунтов: {data.overview.summary.accounts} · Архивных: {data.overview.summary.archived_accounts}</Txt>
          <Txt>Кошельков: {data.overview.summary.wallets} · Требуют подписи: {data.overview.summary.wallets_to_reconfirm}</Txt>
          <Txt>Выплачено: {formatSol(data.overview.summary.paid_lamports)} SOL · Ожидает: {formatSol(data.overview.summary.pending_lamports)} SOL</Txt>
          {data.overview.statuses.map(item => <Txt key={item.status} size={13}>{statusNames[item.status] ?? item.status}: {item.count}</Txt>)}
        </Card>
        <Card style={{ gap: 10 }}><Txt size={23} weight="extra">Кошельки проекта</Txt>
          <Txt selectable size={12}>Главный: {data.overview.chain.treasury_address ?? "не настроен"}</Txt>
          <Txt>Баланс: {data.overview.chain.treasury_lamports == null ? "недоступен" : `${formatSol(data.overview.chain.treasury_lamports)} SOL`}</Txt>
          <Txt selectable size={12}>Резерв PDA: {data.overview.chain.reserve_address ?? "не настроен"}</Txt>
          <Txt>Резерв: {data.overview.chain.reserve_lamports == null ? "недоступен" : `${formatSol(data.overview.chain.reserve_lamports)} SOL`}</Txt>
          <Txt selectable size={12}>Программа: {data.overview.chain.program_address ?? data.overview.campaign?.program_address ?? "не настроена"}</Txt>
          <Txt size={12}>Проверка Devnet: {data.overview.chain.status}{data.overview.chain.error_code ? ` · ${data.overview.chain.error_code}` : ""}</Txt>
        </Card>
        <Card style={{ gap: 16 }}><Txt size={23} weight="extra">Все аккаунты</Txt>
          {pageControls(accountOffset, data.accounts.total, setAccountOffset)}
          {data.accounts.items.map(item => <View key={item.player_id} style={{ gap: 6, borderTopWidth: 1, borderColor: c.line, paddingTop: 14 }}>
            <Txt weight="bold">{item.display_name} · {item.archived ? "Архивный" : "Активный"}</Txt>
            <Txt selectable size={11}>{item.player_id}</Txt>
            <Txt selectable size={12}>Кошелёк: {item.wallet_address ?? "не привязан"}</Txt>
            <Txt size={12}>Восстановление: {item.recovery_ready ? "настроено" : "нужен вход Google"}{item.wallet_reconfirmation_required ? " · нужна подпись кошелька" : ""}</Txt>
            <Txt size={12}>Уровни: {item.completed_levels} · Импорт: {item.imported_levels} · Выплачено: {formatSol(item.paid_lamports)} SOL · Ожидает: {formatSol(item.pending_lamports)} SOL</Txt>
            <Button small secondary disabled={busy} onPress={() => { setPlayer(item.player_id); setRewardOffset(0); }}>Награды этого игрока</Button>
          </View>)}
        </Card>
        <Card style={{ gap: 16 }}><Txt size={23} weight="extra">{player ? "Награды выбранного игрока" : "Награды всех игроков"}</Txt>
          {player && <><Txt selectable size={11}>{player}</Txt><Button small secondary disabled={busy} onPress={() => { setPlayer(undefined); setRewardOffset(0); }}>Показать всех</Button></>}
          {pageControls(rewardOffset, data.rewards.total, setRewardOffset)}
          {data.rewards.items.map(item => <View key={item.reward_id} style={{ gap: 6, borderTopWidth: 1, borderColor: c.line, paddingTop: 14 }}>
            <Txt weight="bold">{item.display_name} · Уровень {item.level_id} · {formatSol(item.amount_lamports)} SOL</Txt>
            <Txt color={item.status === "paid" ? c.accent : c.muted}>{statusNames[item.status] ?? item.status}</Txt>
            <Txt selectable size={11}>Игрок: {item.player_id}</Txt><Txt selectable size={11}>Награда: {item.reward_id}</Txt>
            <Txt selectable size={11}>Получатель: {item.recipient ?? "ещё не назначен"}</Txt>
            <Txt size={11}>{item.created_at}</Txt>
            {!!item.error_code && <Txt color={c.danger} size={12}>{item.error_code} · Следующая проверка: {item.next_attempt_at ?? "—"}</Txt>}
            {!!item.signature && <Button small secondary onPress={() => { void Linking.openURL(`https://explorer.solana.com/tx/${encodeURIComponent(item.signature!)}?cluster=devnet`); }}>Транзакция в Explorer</Button>}
          </View>)}
        </Card>
      </>}
    </>}
  </Shell>;
}
