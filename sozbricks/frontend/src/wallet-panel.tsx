import React, { useEffect, useRef, useState } from "react";
import { Platform, View } from "react-native";
import { accessToken, nativeGo, useAuth } from "./auth";
import { createApi, errorText } from "./api";
import { useStore } from "./state";
import { Button, Card, Txt, usePalette } from "./ui";
import { connectWallet } from "./wallet";
import type { Challenge, WalletChoice } from "./wallet-types";

export function WalletPanel() {
  const c = usePalette();
  const auth = useAuth();
  const { sync, syncView } = useStore();
  const owner = auth.session?.user.id;
  const activeOwner = useRef(owner);
  useEffect(() => { activeOwner.current = owner; return () => { activeOwner.current = undefined; }; }, [owner]);
  const inFlight = useRef(false);
  const [state, setState] = useState<{ owner?: string; busy: boolean; error?: string }>({ busy: false });
  const busy = state.busy;
  const address = syncView?.disk.data?.rewards.wallet_address;
  async function connect(choice: WalletChoice) {
    if (!owner || inFlight.current) return;
    inFlight.current = true;
    setState({ owner, busy: true });
    const api = createApi(refresh => accessToken(owner, refresh));
    try {
      const proof = await connectWallet(choice, address => api<Challenge>("/v1/wallet/challenge", "POST", { address }));
      if (activeOwner.current !== owner) throw new Error("Аккаунт изменился. Подключите кошелёк после входа.");
      await api("/v1/wallet/verify", "POST", { challenge_id: proof.challengeId, signature: proof.signature });
      await sync?.refresh();
      setState({ owner, busy: false });
    } catch (error) { setState({ owner, busy: false, error: errorText(error) }); }
    finally { inFlight.current = false; }
  }
  if (!owner) return null;
  return <Card style={{ gap: 14 }}><Txt size={23} weight="extra">Кошелёк игрока</Txt>
    <Txt selectable color={address ? c.accent : c.muted}>{address ?? "Личный кошелёк ещё не привязан"}</Txt>
    <Txt size={12} color={c.muted}>Подпишите сообщение для подтверждения владения. Награды выплачиваются в тестовых SOL сети Devnet.</Txt>
    <View style={{ flexDirection: "row", flexWrap: "wrap", gap: 10 }}>
      <Button small disabled={busy || nativeGo} onPress={() => { void connect("phantom"); }}>{busy ? "Ожидаем кошелёк…" : Platform.OS === "web" ? "Подключить Phantom" : "Подключить кошелёк"}</Button>
      {Platform.OS === "web" && <Button small secondary disabled={busy} onPress={() => { void connect("solflare"); }}>Подключить Solflare</Button>}
    </View>
    {address && <Txt size={12} color={c.muted}>Можно привязать другой адрес новой подписью. Уже подготовленные выплаты отправятся на прежний адрес, указанный в истории.</Txt>}
    {nativeGo && <Txt size={12} color={c.muted}>Для подключения нужен отдельный APK.</Txt>}
    {state.owner === owner && !!state.error && <Txt size={13} color={c.danger}>{state.error}</Txt>}
  </Card>;
}
