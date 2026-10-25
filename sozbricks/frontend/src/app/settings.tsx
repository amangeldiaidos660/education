import React from "react";
import { Platform, Switch, View } from "react-native";
import { Button, Card, Eyebrow, Shell, Txt, usePalette } from "../ui";
import { useStore } from "../state";

export default function Settings() {
  const c = usePalette();
  const { save, update } = useStore();
  return <Shell><Eyebrow>Ваш ритм игры</Eyebrow><Txt size={34} weight="extra">Настройки</Txt>
    <Card style={{ gap: 24, maxWidth: 850, width: "100%", alignSelf: "center" }}>
      <View style={{ flexDirection: "row", justifyContent: "space-between", alignItems: "center" }}><Txt>Тема оформления</Txt><Button small secondary onPress={() => update({ dark: !save.dark })}>{save.dark ? "☀ Светлая" : "☾ Тёмная"}</Button></View>
      <View style={{ height: 1, backgroundColor: c.line }} />
      <View style={{ flexDirection: "row", justifyContent: "space-between", alignItems: "center" }}><Txt>Звуковые эффекты</Txt><Switch accessibilityLabel="Звуковые эффекты" value={save.sound} onValueChange={sound => update({ sound })} trackColor={{ false: c.line, true: c.teal }} /></View>
      <View style={{ height: 1, backgroundColor: c.line }} />
      <View style={{ flexDirection: "row", justifyContent: "space-between", alignItems: "center" }}><View><Txt>Вибрация при ловле букв</Txt><Txt size={11} color={c.muted}>{Platform.OS === "web" ? "Доступна в Expo Go на телефоне" : "Зависит от устройства"}</Txt></View><Switch accessibilityLabel="Вибрация" value={save.vibration} onValueChange={vibration => update({ vibration })} trackColor={{ false: c.line, true: c.teal }} /></View>
      <View style={{ height: 1, backgroundColor: c.line }} />
      <Txt>Язык интерфейса: русский</Txt><Txt size={13} color={c.muted}>Игровые слова всегда на казахском. Фоновая музыка в эту версию не включена.</Txt>
      <Txt size={24} weight="extra">Как управлять</Txt>
      <Txt weight="bold">Веб-браузер</Txt><Txt color={c.muted}>← / → или A / D — движение платформы.{"\n"}Пробел — запуск шарика. Esc — пауза.{"\n"}Мышь не управляет платформой. Работает и при русской раскладке.</Txt>
      <Txt weight="bold">Android · Expo Go</Txt><Txt color={c.muted}>Двигайте пальцем по игровой области экрана — платформа повторяет горизонтальное движение без скачка к пальцу. Короткое нажатие запускает шарик. Кнопка Ⅱ ставит игру на паузу.</Txt>
      <Txt weight="bold">Скорость шарика</Txt><Txt color={c.muted}>Стартовая скорость растёт с игровыми уровнями. Каждые 20 секунд активной игры шарики ускоряются на 8% от стартовой скорости. Предел — +50% или 600 игровых единиц в секунду, что наступит раньше. Пауза не учитывается. После отскока и при появлении дополнительного шарика текущая скорость сохраняется.</Txt>
      <Txt weight="bold">Правила</Txt><Txt color={c.muted}>Соберите заданное слово по порядку. Лишнюю букву можно пропустить. Пойманная буква остаётся в ячейке до конца попытки. Неверная комбинация после заполнения всех ячеек — проигрыш.{"\n\n"}После пойманной буквы следующая нужная выпадает не раньше чем через 6–10 секунд и после разрушения ещё 3–5 кирпичей. Все выпадения разделены интервалом не менее 2,5 секунды. Пропущенная нужная буква может выпасть повторно.{"\n\n"}Каждые 7 разрушенных кирпичей добавляют шарик, максимум — три одновременно. Если на поле не осталось шариков, попытка завершена. Запаса жизней нет.{"\n\n"}При сворачивании приложения или переключении вкладки игра автоматически встаёт на паузу.</Txt>
    </Card></Shell>;
}
