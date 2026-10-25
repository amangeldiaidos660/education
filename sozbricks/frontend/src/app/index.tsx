import React from "react";
import { Image, Pressable, useWindowDimensions, View } from "react-native";
import { router } from "expo-router";
import { Button, Card, characters, Eyebrow, Shell, Txt, usePalette } from "../ui";
import { useStore } from "../state";
import { levels } from "../game/words";

export default function Home() {
  const c = usePalette();
  const { width } = useWindowDimensions();
  const wide = width >= 850;
  const { save } = useStore();
  const next = levels.find(l => !save.completed.includes(l.id));
  return <Shell>
    <View style={{ flexDirection: wide ? "row" : "column", gap: 36, alignItems: "center", paddingVertical: wide ? 20 : 0 }}>
      <View style={{ flex: wide ? 1 : undefined, width: wide ? undefined : "100%", gap: 26 }}>
        <Eyebrow>Қазақ тілі · Сөзбен ойна</Eyebrow>
        <View><Txt size={wide ? 66 : 43} weight="extra" style={{ lineHeight: wide ? 72 : 49 }}>Лови буквы.</Txt>
          <Txt size={wide ? 66 : 43} weight="extra" color={c.accent} style={{ lineHeight: wide ? 72 : 49 }}>Собирай</Txt>
          <Txt size={wide ? 66 : 43} weight="extra" style={{ lineHeight: wide ? 72 : 49 }}>победы.</Txt></View>
        <Txt color={c.muted} size={17}>Твоя ловкость. Твой словарный запас.{"\n"}Казахские слова — с каждым отскоком.</Txt>
        <Button onPress={() => router.push("/levels")}>▶ Играть</Button>
        <Txt size={12} color={c.goldText}>✦ 24 уровня     ◇ Слова на казахском</Txt>
        <Pressable accessibilityRole="button" onPress={() => router.push({ pathname: "/game", params: { level: next?.id ?? 1 } })}>
          <Card style={{ padding: 20 }}><View style={{ flexDirection: "row", gap: 18, alignItems: "center" }}>
            <Txt size={30} weight="extra" color={c.goldText}>{String(next?.id ?? 1).padStart(2, "0")}</Txt>
            <View style={{ flex: 1 }}><Txt weight="bold">{save.completed.length ? "Продолжить" : "Начать"}: {next?.word ?? "АТ"}</Txt>
              <Txt size={12} color={c.muted}>{save.completed.length} из 24 уровней пройдено</Txt></View><Txt color={c.accent}>↗</Txt>
          </View></Card>
        </Pressable>
        <View style={{ flexDirection: "row", gap: 28 }}>
          <Pressable onPress={() => router.push("/profile")}><Txt color={c.muted} size={13}>Мой персонаж ↗</Txt></Pressable>
          <Pressable onPress={() => router.push("/settings")}><Txt color={c.muted} size={13}>Настройки ⚙</Txt></Pressable>
        </View>
      </View>
      <View style={{ flex: wide ? 1.1 : undefined, width: wide ? undefined : "100%", gap: 16 }}>
        <View style={{ flexDirection: "row", gap: 12, height: wide ? 475 : 310 }}>
          <Image source={characters.alikhan} resizeMode="contain" style={{ flex: 1, height: "100%", borderRadius: 28, backgroundColor: "#071d22" }} />
          <Image source={characters.aiaru} resizeMode="contain" style={{ flex: 1, height: "100%", borderRadius: 28, backgroundColor: "#071d22", marginTop: 20 }} />
        </View>
        <View style={{ flexDirection: "row", justifyContent: "center", gap: 8, marginTop: 20 }}>
          {Array.from("КІТАП").map((letter, i) => <View key={i} style={{ backgroundColor: c.gold, borderRadius: 12, borderBottomWidth: 4,
            borderColor: "#b67a19", paddingVertical: 9, paddingHorizontal: wide ? 20 : 16, transform: [{ rotate: `${i % 2 ? 3 : -3}deg` }] }}>
            <Txt size={30} weight="extra" color="#6c470e">{letter}</Txt></View>)}
        </View>
      </View>
    </View>
    <View style={{ flexDirection: "row", flexWrap: "wrap", justifyContent: "space-between", gap: 16, borderTopWidth: 1, borderColor: c.line, paddingTop: 24 }}>
      {["01 / РАЗБИВАЙ КИРПИЧИ", "02 / ЛОВИ БУКВЫ", "03 / СОБИРАЙ СЛОВА"].map(t => <Txt key={t} size={10} color={c.muted} style={{ letterSpacing: 1 }}>{t}</Txt>)}
    </View>
  </Shell>;
}
