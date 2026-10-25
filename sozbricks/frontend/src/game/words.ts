export function letters(value: string): string[] {
  return Array.from(value.normalize("NFC").trim().toUpperCase());
}

export function sameWord(a: string, b: string): boolean {
  return letters(a).join("") === letters(b).join("");
}

export const alphabet = letters("АӘБВГҒДЕЁЖЗИЙКҚЛМНҢОӨПРСТУҰҮФХҺЦЧШЩЪЫІЬЭЮЯ");

export type Level = { id: number; word: string; translation: string; reward: number; speed: number };

const vocabulary = [
  ["АТ", "Лошадь"], ["АНА", "Мама"], ["КІТАП", "Книга"],
  ["ӘКЕ", "Отец"], ["ҚАЛА", "Город"], ["ӨРІК", "Абрикос"],
  ["ҮЙ", "Дом"], ["ҰШАҚ", "Самолёт"], ["АҢ", "Зверь"],
  ["ҒАРЫШ", "Космос"], ["БІЛІМ", "Знания"], ["ДОСТЫҚ", "Дружба"],
  ["МЕКТЕП", "Школа"], ["ЖҮРЕК", "Сердце"], ["ТІЛ", "Язык"],
  ["КӨКТЕМ", "Весна"], ["ТЕҢІЗ", "Море"], ["ЖАҢБЫР", "Дождь"],
  ["ҚҰРМЕТ", "Уважение"], ["ҚАҺАРМАН", "Герой"], ["ОТБАСЫ", "Семья"],
  ["РАҚМЕТ", "Спасибо"], ["ҚАЗАҚСТАН", "Казахстан"], ["ТӘУЕЛСІЗДІК", "Независимость"],
];

export const levels: Level[] = vocabulary.map(([word, translation], index) => {
  const length = letters(word).length;
  return { id: index + 1, word, translation, reward: length <= 3 ? 0.1 : length <= 5 ? 0.3 : length <= 7 ? 0.4 : 0.5,
    speed: index < 8 ? 210 + index * 20 : 350 + (index - 7) * 10 };
});

export const formatSol = (lamports: number) => (lamports / 1e9).toLocaleString("ru-RU", { maximumFractionDigits: 3 });
