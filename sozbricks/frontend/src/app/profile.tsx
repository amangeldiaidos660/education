import React from "react";
import { Image, useWindowDimensions, View } from "react-native";
import { router } from "expo-router";
import { Button, Card, characters, DevnetNotice, Eyebrow, Shell, Txt, usePalette } from "../ui";
import { useStore } from "../state";
import { formatSol, levels } from "../game/words";

export default function Profile() {
  const c = usePalette();
  const { save, update } = useStore();
  const { width } = useWindowDimensions();
  const wide = width >= 850;
  const next = levels.find(l => !save.completed.includes(l.id));
  return <Shell><Eyebrow>Твой герой. Твой прогресс.</Eyebrow><Txt size={34} weight="extra">Кабинет игрока ✦</Txt>
    <Txt size={12} color={c.muted}>Локальный профиль · Прогресс хранится на этом устройстве</Txt>
    <View style={{ flexDirection: wide ? "row" : "column", gap: 22 }}>
      <Card style={{ flex: 1, gap: 16, alignItems: "center" }}>
        <Eyebrow>Исследователь слов</Eyebrow>
        <Image source={characters[save.hero]} resizeMode="contain" style={{ width: "100%", height: wide ? 360 : 300, borderRadius: 20, backgroundColor: "#071d22" }} />
        <Txt size={30} weight="extra">{save.hero === "alikhan" ? "Алихан" : "Айару"}</Txt>
        <Txt size={12} color={c.muted}>Твой спутник в мире казахских слов</Txt>
        <View style={{ flexDirection: "row", gap: 10 }}>
          <Button small secondary={save.hero !== "alikhan"} onPress={() => update({ hero: "alikhan" })}>Алихан</Button>
          <Button small secondary={save.hero !== "aiaru"} onPress={() => update({ hero: "aiaru" })}>Айару</Button>
        </View>
      </Card>
      <View style={{ flex: 1.4, gap: 22 }}>
        <Card style={{ gap: 24 }}><Txt size={23} weight="extra">Твой следующий рекорд</Txt>
          <View style={{ flexDirection: "row", justifyContent: "space-between", gap: 12 }}>
            <View><Txt size={34} weight="extra">{save.completed.length}<Txt color={c.muted} size={18}>/24</Txt></Txt><Txt size={11} color={c.muted}>Уровней пройдено</Txt></View>
            <View style={{ flexShrink: 1 }}><Txt size={34} weight="extra">{Math.max(0, ...Object.values(save.best)).toLocaleString("ru-RU")}</Txt><Txt size={11} color={c.muted}>Рекорд за одну игру, очков</Txt></View>
          </View>
          <Txt size={12} color={c.muted}>Максимум очков за одну завершённую попытку среди всех уровней, включая проигранные. Это не сумма всех игр и не SOL.</Txt>
          <Txt size={12} color={c.muted}>Обычный кирпич: +40 · Золотистый: +80 · Пойманная буква: +100 · Собранное слово: +500.</Txt>
          <View style={{ height: 7, backgroundColor: c.soft, borderRadius: 7 }}><View style={{ width: `${save.completed.length / 24 * 100}%`, height: 7, backgroundColor: c.teal, borderRadius: 7 }} /></View>
          <Txt color={c.goldText} weight="bold">{next ? `${String(next.id).padStart(2, "0")} · ${next.word}` : "Все уровни пройдены!"}</Txt>
          <Button onPress={() => router.push({ pathname: "/game", params: { level: next?.id ?? 1 } })}>Продолжить →</Button>
        </Card>
        <Card style={{ gap: 20 }}><View style={{ flexDirection: "row", justifyContent: "space-between" }}><Txt size={23} weight="extra">Демо-баланс</Txt><Txt color={c.goldText} size={12}>DEVNET</Txt></View>
          <Txt size={40} weight="extra">{formatSol(save.demoLamports)} <Txt size={18} color={c.goldText}>devnet SOL</Txt></Txt>
          <DevnetNotice /><View style={{ height: 1, backgroundColor: c.line }} />
          <Txt size={13} color={c.muted}>Кошелёк и получение наград появятся на следующем этапе. Это локальный счётчик, а не баланс Solana.</Txt>
          <Txt size={11} color={c.muted}>Демо-награда выдаётся только за первую победу на каждом уровне. При проигрыше накопленное сохраняется.</Txt>
        </Card>
      </View>
    </View>
    <Card style={{ gap: 18 }}><Txt size={24} weight="extra">История игр</Txt>
      {!save.history.length && <Txt color={c.muted}>Пока здесь пусто. Первая история начнётся с первой попытки.</Txt>}
      {save.history.slice(0, 15).map(r => <View key={r.id} style={{ borderTopWidth: 1, borderColor: c.line, paddingTop: 14, gap: 6 }}>
        <View style={{ flexDirection: "row", justifyContent: "space-between", gap: 8 }}><Txt weight="bold">{r.word} · {String(r.level).padStart(2, "0")}</Txt>
          <Txt size={13} color={r.won ? c.accent : c.danger}>{r.won ? "Пройден" : "Попытка"}</Txt></View>
        <Txt size={12} color={c.muted}>{r.score} очков · {r.seconds} с · {new Date(r.at).toLocaleString("ru-RU")}</Txt>
      </View>)}
    </Card></Shell>;
}
