import React from "react";
import { Stack } from "expo-router";
import { StatusBar } from "expo-status-bar";
import { ActivityIndicator, View } from "react-native";
import { useFonts } from "expo-font";
import { StoreProvider, useStore } from "../state";

function Navigation() {
  const { loaded, save } = useStore();
  if (!loaded) return <View style={{ flex: 1, backgroundColor: "#081c33", justifyContent: "center" }}><ActivityIndicator color="#19e1c2" /></View>;
  return <><StatusBar style={save.dark ? "light" : "dark"} /><Stack screenOptions={{ headerShown: false, animation: "fade", contentStyle: { backgroundColor: save.dark ? "#081c33" : "#edf7ff" } }} /></>;
}

export default function Layout() {
  const [fontsLoaded, fontError] = useFonts({
    NunitoSans_400Regular: require("@expo-google-fonts/nunito-sans/400Regular/NunitoSans_400Regular.ttf"),
    NunitoSans_600SemiBold: require("@expo-google-fonts/nunito-sans/600SemiBold/NunitoSans_600SemiBold.ttf"),
    NunitoSans_700Bold: require("@expo-google-fonts/nunito-sans/700Bold/NunitoSans_700Bold.ttf"),
    NunitoSans_800ExtraBold: require("@expo-google-fonts/nunito-sans/800ExtraBold/NunitoSans_800ExtraBold.ttf"),
  });
  if (!fontsLoaded && !fontError) return <View style={{ flex: 1, backgroundColor: "#081c33", justifyContent: "center" }}><ActivityIndicator color="#19e1c2" /></View>;
  return <StoreProvider><Navigation /></StoreProvider>;
}
