import React from "react";
import { Pressable, useWindowDimensions, View } from "react-native";
import { router } from "expo-router";
import { Card, Eyebrow, Shell, Txt, usePalette } from "../ui";
import { useStore } from "../state";
import { letters, levels } from "../game/words";

export default function Levels() {
  const c = usePalette();
  const { width } = useWindowDimensions();
  const { save } = useStore();
  const columns = width >= 1050 ? 4 : width >= 650 ? 3 : 2;
  const cardWidth = (Math.min(width, 1380) - (width >= 850 ? 96 : 40) - (columns - 1) * 14) / columns;
  return <Shell><Eyebrow>Ваш путь к новым словам</Eyebrow><Txt size={34} weight="extra">Выберите уровень</Txt>
    <Txt color={c.muted}>{save.completed.length} / 24 пройдено · Повторяйте открытые уровни</Txt>
    <View style={{ flexDirection: "row", flexWrap: "wrap", gap: 14 }}>
      {levels.map(level => {
        const done = save.completed.includes(level.id);
        const open = level.id === 1 || save.completed.includes(level.id - 1);
        return <Pressable key={level.id} accessibilityRole="button" accessibilityLabel={`Уровень ${level.id}: ${level.word}${open ? "" : ", закрыт"}`}
          disabled={!open} onPress={() => router.push({ pathname: "/game", params: { level: level.id } })}
          style={({ pressed }) => ({ width: cardWidth, opacity: open ? pressed ? 0.85 : 1 : 0.45 })}>
          <Card style={{ padding: width < 500 ? 16 : 24, minHeight: 210, gap: 20, borderColor: open && !done ? c.accent : c.line,
            borderBottomWidth: 4 }}>
            <Txt size={10} color={c.accent} weight="extra">{done ? "ПРОЙДЕН · ✦" : open ? "ДОСТУПЕН" : "ЗАКРЫТ · ◇"}</Txt>
            <Txt size={44} weight="extra" color={open && !done ? c.accent : c.text}>{String(level.id).padStart(2, "0")}</Txt>
            <View><Txt size={18} weight="extra">{open ? level.word : "Новое слово"}</Txt>
              <Txt size={12} color={c.muted}>{letters(level.word).length} букв{!open ? " · Пройдите предыдущий" : ""}</Txt></View>
          </Card>
        </Pressable>;
      })}
    </View></Shell>;
}
