# Неделя 7 SPL Tokens и Token Extensions

Амангелді Айдос, 3 курс, SE, AlmaU.

Задание: создать собственный токен, выполнить выпуск и перевод, добавить metadata и продемонстрировать взаимодействие через клиент.

## Реализация

TypeScript-клиент использует `@solana/kit` и официальные инструкции `@solana-program/token-2022`. Создаётся mint Token-2022 с `MetadataPointer`, указывающим на сам mint, и `TokenMetadata` с названием, символом и URI. Лампортов выделяется достаточно для rent exemption после добавления metadata. Создание аккаунта, инициализация pointer, mint и metadata выполняются атомарно в одной транзакции.

Создаются два associated token account, выпускается 1 000 000 AIDOS и выполняется `transfer_checked` на 125 AIDOS. Все количества в коде хранятся как `bigint` в минимальных единицах; decimals = 6. Клиент ждёт `finalized`, затем читает mint и оба token account и проверяет владельцев, metadata, supply и балансы. Freeze authority не назначается; mint authority и update authority остаются у учебного кошелька.

Отдельную Anchor-программу для этой недели развёртывать не требуется: клиент напрямую вызывает стандартные Token-2022, Associated Token и System Program. Исходный Anchor starter в корне репозитория сохранён.

## Результат в Devnet

| Параметр | Значение |
| --- | --- |
| Название | Aidos AlmaU Token |
| Символ | AIDOS |
| Decimals | 6 |
| Выпуск | 1 000 000 AIDOS |
| Перевод | 125 AIDOS |
| Баланс отправителя после перевода | 999 875 AIDOS |
| Баланс получателя после перевода | 125 AIDOS |
| Расширения | MetadataPointer, TokenMetadata |

Mint: [`56WoASrH9tLiMZ2tRDGuPf8ZVFy2hhaF6KdLWcikJBEo`](https://explorer.solana.com/address/56WoASrH9tLiMZ2tRDGuPf8ZVFy2hhaF6KdLWcikJBEo?cluster=devnet).

Отправитель: `AfDwj8iYT9DQoh1h1tQRvBkh2VNrDkiJ15ig43qb4t4j`.

Получатель: `HMg9Wz3DLAwU8GAYh91WhRMWQzdhpAsBpUYkZcf9tMtX`.

Адреса token account, signatures всех четырёх транзакций и точные raw amounts сохранены в [results.json](results.json). Приватные ключи в репозитории отсутствуют.

## Проверка опубликованного результата

Проверено с Node.js 22.18.0 и npm 10.9.3. Точные версии SDK и TypeScript зафиксированы в `package.json` и `package-lock.json`.

```powershell
git clone --branch codex/week-07-spl-tokens --single-branch https://github.com/amangeldiaidos660/education.git
cd education/week-07
npm ci
npm run check
npm run verify
```

Для `verify` кошелёк не требуется: выполняется только чтение Devnet и опубликованного metadata JSON. Ожидаемый результат — сообщения PASS для metadata, supply, обоих балансов и четырёх finalized-транзакций. Состояние относится к учебному запуску 21 октября 2026 года; последующие операции владельца и сброс Devnet могут изменить результат проверки.

## Выполнение со своим кошельком

Нужен отдельный Devnet-кошелёк с тестовым SOL и публичный адрес второго кошелька. Укажите свой локальный путь; пример не содержит ключа.

```powershell
$env:SOLANA_KEYPAIR = 'C:/path/to/devnet-keypair.json'
$env:RECIPIENT = '<публичный адрес второго кошелька>'
npm run devnet
```

RPC фиксирован на Devnet, дополнительно проверяется genesis hash. Клиент создаёт новый токен и перезаписывает локальный `results.json` результатами собственного запуска. URI указывает на учебный metadata JSON в этой ветке. Чтобы изменить название и символ, измените `metadata.json` и `metadataUri` в `shared.ts`, опубликуйте собственный JSON и затем запускайте клиент.

В `.local/progress.json` сохраняются только публичные адреса и signatures. При повторном запуске с теми же параметрами уже finalized-операции пропускаются, повторный выпуск не производится. Если сохранённая транзакция не найдена, завершилась ошибкой или ещё не finalized, клиент останавливается: сначала проверьте её через Explorer. Не удаляйте журнал для слепого повторения незавершённой операции. Для нового независимого выполнения после проверки старого запуска можно переименовать `.local`.

Используются только тестовые токены без денежной стоимости. Все изменения состояния, rent и комиссии оплачиваются тестовым SOL. Устанавливать Rust/Anchor для запуска этого клиента не требуется.

## Материалы

- [Solana Token Metadata и Metadata Pointer](https://solana.com/docs/tokens/extensions/metadata)
- [Token-2022 Extension Guide](https://www.solana-program.com/docs/token-2022/extensions)
- [Solana Kit](https://www.solanakit.com/)
