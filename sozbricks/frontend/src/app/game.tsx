import React, { useCallback, useEffect, useRef, useState } from "react";
import { AppState, Platform, Pressable, useWindowDimensions, View } from "react-native";
import { router, useFocusEffect, useLocalSearchParams } from "expo-router";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import Svg, { Circle, Defs, G, LinearGradient, Rect, Stop, Text as SvgText } from "react-native-svg";
import * as Haptics from "expo-haptics";
import { setAudioModeAsync, setIsAudioActiveAsync, useAudioPlayer } from "expo-audio";
import { Button, Card, DevnetNotice, Eyebrow, Shell, Txt, usePalette } from "../ui";
import { useStore } from "../state";
import { ballSpeed, createGame, Game, H, launch, movePaddle, PADDLE_W, PADDLE_Y, step, togglePause, W } from "../game/engine";
import { letters, Level, levels } from "../game/words";

function clock(seconds: number) {
  return `${String(Math.floor(seconds / 60)).padStart(2, "0")}:${String(Math.floor(seconds % 60)).padStart(2, "0")}`;
}

function snapshotGame(game: Game): Game {
  return { ...game, balls: game.balls.map(b => ({ ...b })), bricks: game.bricks.map(b => ({ ...b })),
    drops: game.drops.map(d => ({ ...d })), particles: game.particles.map(p => ({ ...p })), collected: [...game.collected] };
}

export default function GameRoute() {
  const { level: param } = useLocalSearchParams<{ level?: string }>();
  const { save } = useStore();
  const level = levels.find(l => l.id === Number(param ?? 1));
  if (!level || (level.id > 1 && !save.completed.includes(level.id - 1))) return <Shell>
    <Card><Txt size={24} weight="extra">Уровень пока недоступен</Txt><Button onPress={() => router.replace("/levels")}>Выбрать уровень</Button></Card></Shell>;
  return <Play key={level.id} level={level} />;
}

function Play({ level }: { level: Level }) {
  const c = usePalette();
  const { save, finish } = useStore();
  const { width, height } = useWindowDimensions();
  const insets = useSafeAreaInsets();
  const wide = width >= 1100;
  const game = useRef(createGame(level));
  const keys = useRef(new Set<string>());
  const [round, setRound] = useState(0);
  const [snapshot, setSnapshot] = useState(() => createGame(level));
  const [runId, setRunId] = useState(() => `${Date.now()}-${Math.random()}`);
  const [awarded, setAwarded] = useState(false);
  const settings = useRef(save);
  const finishRef = useRef(finish);
  useEffect(() => { settings.current = save; finishRef.current = finish; }, [save, finish]);
  const recorded = useRef(false);
  const player = useAudioPlayer(require("../../assets/bounce.wav"), { downloadFirst: true, keepAudioSessionActive: true });
  const audioReady = useRef(false);
  const audioBusy = useRef(false);
  const audioError = useRef(false);
  const lastSound = useRef(0);
  const available = height - insets.top - insets.bottom - (width >= 850 ? 82 : 50) - 18;
  const chromeHeight = 124 + 72 + (Platform.OS === "web" ? 43 : 0) + 2;
  const fieldHeight = Math.min(510, Math.max(120, available - chromeHeight));
  const boardWidth = Math.max(120, Math.min(wide ? width - 680 : width - 26, 430, fieldHeight * W / H));
  const fieldDisplayHeight = boardWidth * H / W;
  const target = letters(level.word);
  const g = snapshot;

  useEffect(() => {
    let alive = true;
    let foreground = AppState.currentState !== "background" && AppState.currentState !== "inactive";
    player.volume = 1;
    player.muted = false;
    if (Platform.OS === "web") {
      audioReady.current = true;
      return () => { audioReady.current = false; };
    }
    const activate = async () => {
      audioReady.current = false;
      try {
        await setAudioModeAsync({ playsInSilentMode: true, interruptionMode: "mixWithOthers",
          allowsRecording: false, shouldPlayInBackground: false, shouldRouteThroughEarpiece: false });
        if (!alive || !foreground) return;
        await setIsAudioActiveAsync(true);
        if (alive && foreground) audioReady.current = true;
      } catch (error) {
        if (alive && !audioError.current) { audioError.current = true; console.warn("Не удалось включить звук игры", error); }
      }
    };
    if (foreground) void activate();
    const sub = AppState.addEventListener("change", state => {
      foreground = state === "active";
      if (foreground) void activate();
      else audioReady.current = false;
    });
    return () => { alive = false; audioReady.current = false; sub.remove(); };
  }, [player]);

  const refresh = () => setSnapshot(snapshotGame(game.current));
  const pause = () => { togglePause(game.current); keys.current.clear(); refresh(); };
  const restart = () => {
    game.current = createGame(level);
    keys.current.clear(); recorded.current = false;
    setAwarded(false);
    setRunId(`${Date.now()}-${Math.random()}`);
    setRound(n => n + 1);
    refresh();
  };
  const autoPause = useCallback(() => {
    const current = game.current;
    keys.current.clear();
    if (current.phase === "playing" || current.phase === "ready") { togglePause(current); setSnapshot(snapshotGame(current)); }
  }, []);
  useFocusEffect(useCallback(() => () => autoPause(), [autoPause]));
  useEffect(() => {
    const sub = AppState.addEventListener("change", state => { if (state !== "active") autoPause(); });
    return () => sub.remove();
  }, [autoPause]);
  useEffect(() => {
    if (Platform.OS !== "web") return;
    const down = (event: KeyboardEvent) => {
      if (["ArrowLeft", "ArrowRight", "KeyA", "KeyD", "Space", "Escape"].includes(event.code)) event.preventDefault();
      keys.current.add(event.code);
      if (event.repeat) return;
      if (event.code === "Space") launch(game.current);
      if (event.code === "Escape") { togglePause(game.current); keys.current.clear(); }
      setSnapshot(snapshotGame(game.current));
    };
    const up = (event: KeyboardEvent) => { keys.current.delete(event.code); };
    const visibility = () => { if (document.hidden) autoPause(); };
    window.addEventListener("keydown", down);
    window.addEventListener("keyup", up);
    window.addEventListener("blur", autoPause);
    document.addEventListener("visibilitychange", visibility);
    return () => {
      window.removeEventListener("keydown", down); window.removeEventListener("keyup", up);
      window.removeEventListener("blur", autoPause); document.removeEventListener("visibilitychange", visibility);
    };
  }, [autoPause]);
  useEffect(() => {
    let request = 0, previous = 0, accumulator = 0, alive = true;
    const frame = (now: number) => {
      if (!alive) return;
      const current = game.current;
      const elapsed = previous ? Math.min((now - previous) / 1000, 0.05) : 0;
      previous = now;
      if (current.phase === "playing" || current.phase === "ready") {
        accumulator += elapsed;
        const before = current.catches;
        let bounced = false;
        const direction = (keys.current.has("ArrowRight") || keys.current.has("KeyD") ? 1 : 0)
          - (keys.current.has("ArrowLeft") || keys.current.has("KeyA") ? 1 : 0);
        while (accumulator >= 1 / 120) { step(current, 1 / 120, direction); bounced ||= current.bounce; accumulator -= 1 / 120; }
        if (settings.current.sound && audioReady.current && player.isLoaded && !audioBusy.current
          && (bounced || before !== current.catches) && now - lastSound.current > 90) {
          lastSound.current = now;
          audioBusy.current = true;
          void player.seekTo(0).then(() => {
            if (alive && audioReady.current && settings.current.sound && game.current.phase !== "paused") player.play();
          }).catch(error => {
            if (alive && !audioError.current) { audioError.current = true; console.warn("Не удалось воспроизвести звук игры", error); }
          }).finally(() => { audioBusy.current = false; });
        }
        if (before !== current.catches && settings.current.vibration && Platform.OS !== "web") void Haptics.selectionAsync().catch(() => {});
        if (["won", "lost"].includes(current.phase) && !recorded.current) {
          recorded.current = true;
          setAwarded(!settings.current.completed.includes(level.id));
          finishRef.current(current, runId);
        }
        setSnapshot(snapshotGame(current));
      } else accumulator = 0;
      request = requestAnimationFrame(frame);
    };
    request = requestAnimationFrame(frame);
    return () => { alive = false; cancelAnimationFrame(request); };
  }, [round, player, runId, level.id]);

  const slotSize = Math.min(42, (boardWidth - 20 - (target.length - 1) * 5) / target.length);
  const touch = useRef<{ id: number; x: number; y: number; distance: number; at: number } | null>(null);
  const slotRow = <View style={{ flexDirection: "row", justifyContent: "center", gap: 5 }}>
    {target.map((letter, i) => {
      const caught = g.collected[i];
      const wrong = caught && caught !== letter;
      return <View key={i} style={{ width: slotSize, height: 40, alignItems: "center", justifyContent: "center",
        borderRadius: 8, backgroundColor: caught ? wrong ? "#633943" : c.soft : "transparent",
        borderWidth: 1, borderColor: caught ? wrong ? c.danger : c.accent : c.line,
        borderStyle: caught ? "solid" : "dashed" }}><Txt size={Math.min(23, slotSize * 0.7)} weight="extra"
          color={wrong ? "#ffc0b8" : caught ? c.accent : c.muted}>{caught ?? "·"}</Txt></View>;
    })}
  </View>;

  return <Shell game><View style={{ flex: 1, flexDirection: "row", justifyContent: "center", alignItems: "center", gap: 28, padding: 8 }}
    onStartShouldSetResponder={() => Platform.OS !== "web" && ["ready", "playing"].includes(game.current.phase)}
    onResponderGrant={event => {
      const e = event.nativeEvent;
      touch.current = e.touches.length === 1
        ? { id: e.identifier, x: e.pageX, y: e.pageY, distance: 0, at: e.timestamp } : null;
    }}
    onResponderMove={event => {
      const e = event.nativeEvent, gesture = touch.current;
      if (!gesture || e.identifier !== gesture.id || e.touches.length !== 1) { touch.current = null; return; }
      if (!["ready", "playing"].includes(game.current.phase)) { touch.current = null; return; }
      const dx = e.pageX - gesture.x, dy = e.pageY - gesture.y;
      gesture.distance += Math.hypot(dx, dy);
      gesture.x = e.pageX; gesture.y = e.pageY;
      movePaddle(game.current, game.current.paddle + dx * W / boardWidth);
      refresh();
    }}
    onResponderRelease={event => {
      const gesture = touch.current;
      touch.current = null;
      if (gesture && event.nativeEvent.identifier === gesture.id && gesture.distance < 8
        && Math.hypot(event.nativeEvent.pageX - gesture.x, event.nativeEvent.pageY - gesture.y) < 8
        && event.nativeEvent.timestamp - gesture.at < 350) launch(game.current);
      refresh();
    }}
    onResponderTerminationRequest={() => true}
    onResponderTerminate={() => { touch.current = null; }}>
    {wide && <View style={{ width: 260, gap: 28 }}><Eyebrow>Слово за словом</Eyebrow>
      <Txt size={34} weight="extra">Лови момент.{"\n"}<Txt size={34} weight="extra" color={c.accent}>Собери слово.</Txt></Txt>
      <Txt color={c.muted}>Разбивайте кирпичи. Ловите буквы заданного слова по порядку. Пропускайте лишние.</Txt>
      <Card><Txt color={c.goldText} weight="bold">Сенің қолыңнан келеді!</Txt><Txt size={13} color={c.muted}>У тебя получится.</Txt></Card>
      <Txt size={13} color={c.muted}>← / → или A / D — движение{"\n"}Пробел — запуск · Esc — пауза</Txt>
      <Button secondary small onPress={() => router.replace("/levels")}>К уровням</Button>
    </View>}
    <View style={{ width: boardWidth, backgroundColor: c.field, borderRadius: 22, borderWidth: 1, borderColor: c.line, overflow: "hidden" }}>
      <View style={{ height: 124, paddingHorizontal: 14, paddingTop: 9, gap: 4 }}>
        <View style={{ flexDirection: "row", justifyContent: "space-between", alignItems: "center" }}>
          <Txt size={12} weight="extra">{String(level.id).padStart(2, "0")} / 24</Txt>
          <Txt size={12} color={c.muted}>{g.score.toLocaleString("ru-RU")} очков · {clock(g.elapsed)}</Txt>
          <Pressable accessibilityRole="button" accessibilityLabel="Пауза" onPress={pause} style={{ padding: 7 }}><Txt color={c.accent} weight="extra">Ⅱ</Txt></Pressable>
        </View><Txt size={10} color={c.muted} style={{ textAlign: "center", letterSpacing: 2 }}>СОБЕРИТЕ СЛОВО</Txt>
        <Txt size={Math.min(24, 260 / target.length)} color={c.goldText} weight="extra" style={{ textAlign: "center", letterSpacing: 3 }}>{level.word}</Txt>
        <Txt size={10} color={c.muted} style={{ textAlign: "center" }}>Скорость ×{(ballSpeed(g) / level.speed).toFixed(2)}</Txt>
      </View>
      <View style={{ width: boardWidth, height: fieldDisplayHeight }}>
        <Svg width={boardWidth} height={fieldDisplayHeight} viewBox={`0 0 ${W} ${H}`} accessibilityLabel="Игровое поле">
          <Defs><LinearGradient id="teal" x1="0" y1="0" x2="0" y2="1"><Stop offset="0" stopColor="#4cf4d5" /><Stop offset="1" stopColor="#09b69f" /></LinearGradient>
            <LinearGradient id="gold" x1="0" y1="0" x2="0" y2="1"><Stop offset="0" stopColor="#ffe99d" /><Stop offset="1" stopColor="#ffc54c" /></LinearGradient></Defs>
          {[80, 160, 240, 320, 400].map(y => <Rect key={y} x="0" y={y} width={W} height="1" fill={c.line} opacity="0.2" />)}
          {g.bricks.filter(b => b.hp > 0).map(b => <G key={b.id}>
            <Rect x={b.x} y={b.y + 3} width="51" height="22" rx="6" fill={b.gold ? "#a67422" : "#007c79"} />
            <Rect x={b.x} y={b.y} width="51" height="21" rx="6" fill={`url(#${b.gold ? "gold" : "teal"})`} opacity={b.hp === 2 ? 0.65 : 1} />
            <Rect x={b.x + 4} y={b.y + 2} width="43" height="1.5" rx="1" fill="white" opacity="0.45" />
          </G>)}
          {g.particles.map(p => <Circle key={p.id} cx={p.x} cy={p.y} r="2.5" fill={p.gold ? c.gold : c.teal} opacity={Math.max(0, p.life / 0.45)} />)}
          {g.drops.map(d => <G key={d.id}><Rect x={d.x - 13} y={d.y - 12} width="27" height="29" rx="6" fill="#ad7e23" />
            <Rect x={d.x - 13} y={d.y - 15} width="27" height="29" rx="6" fill="url(#gold)" stroke="#fff1c0" />
            <SvgText x={d.x} y={d.y + 6} fontSize="20" fontWeight="bold" fontFamily="NunitoSans_700Bold" textAnchor="middle" fill="#26424c">{d.letter}</SvgText></G>)}
          {g.balls.map(b => <G key={b.id}><Circle cx={b.x} cy={b.y} r="11" fill={c.teal} opacity="0.13" />
            <Circle cx={b.x} cy={b.y} r="6" fill="#ffffff" stroke="#67d4cc" strokeWidth="2" /></G>)}
          <Rect x={g.paddle - PADDLE_W / 2} y={PADDLE_Y + 3} width={PADDLE_W} height="12" rx="6" fill="#007c79" />
          <Rect x={g.paddle - PADDLE_W / 2} y={PADDLE_Y} width={PADDLE_W} height="11" rx="6" fill="url(#teal)" />
        </Svg>
        {g.phase === "ready" && <View pointerEvents="none" style={{ position: "absolute", top: "49%", left: 12, right: 12, gap: 8, alignItems: "center" }}>
          <Txt size={22} weight="extra">Готовы?</Txt><Txt size={12} color={c.muted}>{Platform.OS === "web" ? "Пробел или кнопка «Запустить»" : "Нажмите для запуска · Ведите пальцем для движения"}</Txt>
        </View>}
      </View>
      <View style={{ height: 72, justifyContent: "center", gap: 5, paddingHorizontal: 10 }}>
        <View style={{ flexDirection: "row", justifyContent: "space-between" }}><Txt size={9} color={c.accent}>{g.elapsed < g.noticeUntil ? g.notice : `${g.balls.length} шарик(а) в игре`}</Txt>
          <Txt size={9} color={c.muted}>{g.collected.length} / {target.length}</Txt></View>{slotRow}
      </View>
      {Platform.OS === "web" && <View style={{ height: 43, flexDirection: "row", justifyContent: "space-around", alignItems: "center", borderTopWidth: 1, borderColor: c.line }}>
        <Txt size={9} color={c.muted}>← → / A D</Txt><Pressable accessibilityRole="button" onPress={() => { launch(game.current); refresh(); }} style={{ padding: 9 }}>
          <Txt size={11} weight="bold" color={c.accent}>{g.phase === "ready" ? "▶ Запустить" : "Пробел · Esc"}</Txt></Pressable>
        {!wide && <Pressable accessibilityRole="button" onPress={() => router.replace("/levels")}><Txt size={10} color={c.muted}>Уровни</Txt></Pressable>}
      </View>}
      {(g.phase === "paused" || g.phase === "won" || g.phase === "lost") && <View style={{ position: "absolute", top: 0, bottom: 0, left: 0, right: 0,
        backgroundColor: "rgba(2,15,30,0.84)", alignItems: "center", justifyContent: "center", padding: 12 }}>
        <View style={{ width: "100%", padding: boardWidth < 260 ? 12 : 20, backgroundColor: c.card, borderRadius: 22, borderWidth: 1, borderColor: c.accent, gap: 15, alignItems: "center" }}>
          <View style={{ width: 54, height: 54, borderRadius: 27, backgroundColor: c.gold, justifyContent: "center", alignItems: "center" }}>
            <Txt size={28} color="#7d5714">{g.phase === "paused" ? "Ⅱ" : g.phase === "won" ? "✦" : "↻"}</Txt></View>
          <Txt size={24} weight="extra" style={{ textAlign: "center" }}>{g.phase === "paused" ? "Пауза" : g.phase === "won" ? "Слово собрано!" : "Попробуем ещё?"}</Txt>
          {g.phase === "paused" ? <Txt size={13} color={c.muted}>Слово подождёт. Продолжим?</Txt> : <>
            <Txt size={Math.min(26, 230 / target.length)} weight="extra" color={c.goldText}>{g.phase === "won" ? level.word : g.collected.join("") || "· · ·"}</Txt>
            <Txt size={13} color={g.phase === "lost" ? c.danger : c.muted}>{g.phase === "won" ? level.translation : g.reason}</Txt>
            <Txt size={12} color={c.muted}>{g.score} очков · {clock(g.elapsed)}</Txt>
          </>}
          {g.phase === "won" && <View style={{ backgroundColor: c.soft, padding: 12, borderRadius: 12, gap: 5, width: "100%" }}>
            <Txt color={c.goldText} size={13} weight="bold">{!awarded
              ? "Повторное прохождение · без награды" : `Демо-награда +${level.reward.toLocaleString("ru-RU")} devnet SOL`}</Txt><DevnetNotice /></View>}
          <View style={{ width: "100%", gap: 9 }}>
            {g.phase === "paused" ? <Button onPress={pause}>Продолжить</Button>
              : g.phase === "won" && level.id < levels.length ? <Button onPress={() => router.replace({ pathname: "/game", params: { level: level.id + 1 } })}>Следующий уровень →</Button>
                : g.phase === "won" ? <Button onPress={() => router.replace("/profile")}>Все слова собраны! →</Button> : <Button onPress={restart}>Повторить уровень</Button>}
            {g.phase === "paused" && <Button secondary onPress={restart}>Начать заново</Button>}
            <Button secondary onPress={() => router.replace("/levels")}>К уровням</Button>
          </View>
        </View>
      </View>}
    </View>
    {wide && <View style={{ width: 260, gap: 20 }}><Card style={{ gap: 18, borderTopColor: c.accent, borderTopWidth: 3 }}>
      <Eyebrow>Ваш прогресс</Eyebrow><Txt size={26} weight="extra">Буква за буквой</Txt>
      <View style={{ height: 7, borderRadius: 8, backgroundColor: c.soft }}><View style={{ width: `${g.collected.length / target.length * 100}%`, height: 7, backgroundColor: c.teal, borderRadius: 8 }} /></View>
      <Txt size={13} color={c.muted}>{g.collected.length} из {target.length} букв собрано</Txt>
      <Txt size={13} color={c.muted}>Все падающие буквы выглядят одинаково. Выбирайте по символу, а не по цвету.</Txt>
    </Card><Txt size={12} color={c.muted}>✦ Каждые 7 кирпичей — дополнительный шарик. Потерян последний — попытка завершена.</Txt>
      <Txt size={12} color={c.muted}>Рекорд этого уровня: {save.best[level.id] ?? 0} очков</Txt>
      <Txt size={12} color={c.muted}>Темп растёт каждые 20 секунд активной игры и ограничен верхним пределом.</Txt></View>}
  </View></Shell>;
}
