import React, { useEffect, useRef, useState } from "react";
import { Platform } from "react-native";
import { router, useLocalSearchParams } from "expo-router";
import { useAuth } from "../../auth";
import { errorText } from "../../api";
import { Button, Card, Shell, Txt } from "../../ui";

export default function Callback() {
  const params = useLocalSearchParams<{ code?: string; error?: string; sb_flow_id?: string }>();
  const { complete } = useAuth();
  const completeRef = useRef(complete);
  const [error, setError] = useState("");
  useEffect(() => {
    let alive = true;
    if (params.error || !params.code) return;
    void completeRef.current(params.code, params.sb_flow_id).then(() => {
      if (alive) {
        if (Platform.OS === "web") window.history.replaceState(null, "", "/auth/callback");
        router.replace("/profile");
      }
    }).catch(error => { if (alive) setError(errorText(error)); });
    return () => { alive = false; };
  }, [params.code, params.error, params.sb_flow_id]);
  const message = error || (params.error || !params.code ? "Вход не завершён. Вернитесь в кабинет и попробуйте снова." : "");
  return <Shell><Card><Txt>{message || "Завершаем вход через Google…"}</Txt>
    {!!message && <Button onPress={() => router.replace("/profile")}>В кабинет</Button>}</Card></Shell>;
}
