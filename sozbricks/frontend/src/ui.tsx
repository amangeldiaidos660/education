import React from "react";
import { Image, Platform, Pressable, ScrollView, StyleSheet, Text, TextStyle, useWindowDimensions, View, ViewStyle } from "react-native";
import { router, usePathname } from "expo-router";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import { useStore } from "./state";

export const characters = { alikhan: require("../assets/characters/alikhan.png"), aiaru: require("../assets/characters/aiaru.png") };
export function usePalette() {
  const { save } = useStore();
  return save.dark ? { bg: "#081c33", card: "#102e4b", soft: "#173b56", text: "#f2f6ff", muted: "#a2b9d1",
    line: "#244961", accent: "#19e1c2", teal: "#20d9be", gold: "#ffd16b", goldText: "#ffd16b", danger: "#ff9b8d", field: "#0a2039" }
    : { bg: "#edf7ff", card: "#ffffff", soft: "#e3f2f8", text: "#173850", muted: "#5f7f97", line: "#c9e0ec",
      accent: "#008b83", teal: "#16d6b9", gold: "#ffd16b", goldText: "#aa6a00", danger: "#bd4339", field: "#e1f3fb" };
}

export function Txt({ children, size = 16, weight = "regular", color, style, ...props }: {
  children?: React.ReactNode; size?: number; weight?: "regular" | "semi" | "bold" | "extra"; color?: string;
  style?: TextStyle | TextStyle[]; selectable?: boolean; numberOfLines?: number;
}) {
  const c = usePalette();
  return <Text {...props} style={[{ color: color ?? c.text, fontSize: size,
    fontFamily: ({ regular: "NunitoSans_400Regular", semi: "NunitoSans_600SemiBold", bold: "NunitoSans_700Bold", extra: "NunitoSans_800ExtraBold" })[weight],
    lineHeight: size * 1.4 }, style]}>{children}</Text>;
}

export function Button({ children, onPress, secondary, disabled, small, label }: {
  children: React.ReactNode; onPress: () => void; secondary?: boolean; disabled?: boolean; small?: boolean; label?: string;
}) {
  const c = usePalette();
  return <Pressable accessibilityRole="button" accessibilityLabel={label} disabled={disabled} onPress={onPress}
    style={({ pressed }) => [{ paddingHorizontal: small ? 16 : 24, paddingVertical: small ? 11 : 15,
      borderRadius: 15, borderWidth: 1, borderBottomWidth: pressed ? 1 : 4,
      borderColor: secondary ? c.line : "#008f84", backgroundColor: secondary ? c.soft : c.teal,
      opacity: disabled ? 0.4 : pressed ? 0.88 : 1, alignItems: "center", justifyContent: "center",
      transform: [{ translateY: pressed ? 2 : 0 }], minHeight: small ? 44 : 54 }]}>
    <Txt weight="extra" size={small ? 14 : 16} color={secondary ? c.text : "#073e42"}>{children}</Txt>
  </Pressable>;
}

export function Card({ children, style }: { children: React.ReactNode; style?: ViewStyle | ViewStyle[] }) {
  const c = usePalette();
  return <View style={[{ backgroundColor: c.card, borderWidth: 1, borderColor: c.line, borderRadius: 24, padding: 24 }, style]}>{children}</View>;
}

export function Eyebrow({ children }: { children: string }) {
  const c = usePalette();
  return <Txt size={11} weight="extra" color={c.accent} style={{ letterSpacing: 2.2 }}>{children.toUpperCase()}</Txt>;
}

export function DevnetNotice() {
  const c = usePalette();
  return <View style={{ gap: 6 }}><Txt size={13} color={c.goldText} weight="bold">DEVNET · Локальная демонстрация</Txt>
    <Txt size={13} color={c.muted}>Devnet SOL — тестовая валюта, не реальные деньги.</Txt>
    <Txt size={11} color={c.muted}>Кошелёк не подключён. Начисления не отправляются в блокчейн.</Txt></View>;
}

export function Logo() {
  const c = usePalette();
  return <View style={{ flexDirection: "row", gap: 10, alignItems: "center" }}>
    <View style={{ flexDirection: "row", flexWrap: "wrap", width: 30, gap: 3, transform: [{ rotate: "-8deg" }] }}>
      {[0, 1, 2, 3].map(i => <View key={i} style={{ width: 12, height: 12, backgroundColor: i === 3 ? c.gold : c.teal, borderRadius: 3, borderBottomWidth: 2, borderColor: "#008e82" }} />)}
    </View><Txt size={23} weight="extra">Söz Bricks<Txt color={c.accent}> ·</Txt></Txt></View>;
}

export function Shell({ children, game = false }: { children: React.ReactNode; game?: boolean }) {
  const c = usePalette();
  const { save, update, storageError } = useStore();
  const { width } = useWindowDimensions();
  const insets = useSafeAreaInsets();
  const pathname = usePathname();
  const desktop = width >= 850;
  const nav = [{ title: "Главная", path: "/" }, { title: "Играть", path: "/levels" }, { title: "Настройки", path: "/settings" }] as const;
  const header = <View style={{ paddingHorizontal: desktop ? 36 : 16, maxWidth: 1420, width: "100%", alignSelf: "center" }}>
    <View style={{ height: game && !desktop ? 50 : 82, flexDirection: "row", alignItems: "center", justifyContent: "space-between", borderBottomWidth: 1, borderColor: c.line }}>
      <Pressable accessibilityRole="link" accessibilityLabel="Главная Söz Bricks" onPress={() => router.navigate("/")}><Logo /></Pressable>
      <View style={{ flexDirection: "row", alignItems: "center", gap: desktop ? 28 : 12 }}>
        {desktop && nav.map(n => <Pressable key={n.path} onPress={() => router.navigate(n.path)} accessibilityRole="link">
          <Txt size={14} weight="bold" color={pathname === n.path ? c.accent : c.muted}>{n.title}</Txt></Pressable>)}
        <Pressable accessibilityRole="button" accessibilityLabel="Переключить тему" onPress={() => update({ dark: !save.dark })} style={{ padding: 8 }}>
          <Txt size={25} color={c.goldText}>{save.dark ? "☀" : "☾"}</Txt></Pressable>
        <Pressable accessibilityRole="link" accessibilityLabel="Кабинет игрока" onPress={() => router.navigate("/profile")}>
          <View style={{ width: 42, height: 42, borderRadius: 21, overflow: "hidden", borderWidth: 2, borderColor: c.teal, backgroundColor: "#06232c" }}>
            <Image source={characters[save.hero]} style={{ width: 50, height: 75, position: "absolute", top: 0, left: -4 }} /></View>
        </Pressable>
      </View>
    </View>
    {!!storageError && <Txt size={12} color={c.danger}>{storageError}</Txt>}
  </View>;
  return <View style={{ flex: 1, backgroundColor: c.bg, paddingTop: insets.top, paddingBottom: insets.bottom }}>
    {header}
    {game ? <View style={{ flex: 1 }}>{children}</View> : <ScrollView contentContainerStyle={{ paddingHorizontal: desktop ? 48 : 20, paddingTop: desktop ? 44 : 28, paddingBottom: 30, width: "100%", maxWidth: 1380, alignSelf: "center", gap: 28 }}>
      {children}
      {!desktop && <View style={{ flexDirection: "row", justifyContent: "space-around", gap: 8, paddingTop: 20, borderTopWidth: 1, borderColor: c.line }}>
        {nav.map(n => <Pressable key={n.path} accessibilityRole="link" onPress={() => router.navigate(n.path)}><Txt size={13} color={c.accent} weight="bold">{n.title}</Txt></Pressable>)}
      </View>}
      <Txt size={10} color={c.muted} style={{ textAlign: "center", letterSpacing: 2, marginTop: 20 }}>SÖZ BRICKS / {Platform.OS === "web" ? "WEB" : "ANDROID"} / ИГРАЙ СО СЛОВАМИ</Txt>
    </ScrollView>}
  </View>;
}

export const ui = StyleSheet.create({ row: { flexDirection: "row", alignItems: "center", gap: 12 }, gap: { gap: 18 } });
