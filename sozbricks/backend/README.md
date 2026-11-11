# Söz Bricks: главный кошелёк Devnet

Адрес проекта: `615vW8VZFB8fLGrPTcxQ2FWBWkevahkjX5kCbtoosyQw`.
Все суммы ниже относятся только к тестовым SOL в Solana Devnet.

## 1. Локальный ключ и резервная копия

Рабочий ключ: `C:\Users\amang\.config\solana\sozbricks-treasury-devnet.json`.
Зашифрованная резервная копия: `D:\SolanaBackups\sozbricks-treasury-devnet.encrypted.json`.
Инструкция восстановления: `D:\SolanaBackups\RECOVERY.txt`.
Резервная копия проверена расшифрованием, совпадением адреса и проверкой подписи.
Пароль копии хранится отдельно. Приватный ключ и резервная копия не входят в репозиторий.

## 2. Размещение на Render

Код загружает ключ один раз при запуске и проверяет соответствие публичному адресу.
`GET /health/treasury` возвращает адрес, баланс в lamports с commitment `finalized`,
проверку сети Devnet и подписанное диагностическое сообщение.
`proof_signature` — Ed25519-подпись UTF-8 строки `proof_message`, закодированная в base58.
Сообщение содержит новый UUID при каждом запуске процесса. Приватный ключ не возвращается.
Этот этап не включает маршруты перевода или выплат игрокам: `payments_enabled` остаётся `false`.
Без обеих настроек кошелька остальной API работает, а маршрут возвращает `503 treasury_not_configured`.
Если задана только одна настройка или ключ неверен, запуск отклоняется с ошибкой без содержимого ключа.

1. Загрузить изменения кода в ветку, которую действительно использует сервис `sozbricks-api-tests`.
   В `render.yaml` указана `task/14-tests`; фактическую ветку проверить в Render → Settings.
   Дождаться успешных GitHub Actions. Коммит и push выполняет владелец проекта.
2. В Render → сервис → Environment → Secret Files добавить файл
   `sozbricks-treasury-devnet.json`. В Contents вставить содержимое рабочего JSON-файла,
   а не зашифрованной резервной копии. Секрет не добавлять в Git, Dockerfile или логи.
3. Добавить обе переменные и применить настройки:

   ```text
   SOLANA_TREASURY_KEYPAIR_PATH=/etc/secrets/sozbricks-treasury-devnet.json
   SOLANA_TREASURY_ADDRESS=615vW8VZFB8fLGrPTcxQ2FWBWkevahkjX5kCbtoosyQw
   ```

   Существующие DATABASE_URL, SUPABASE_* и CORS_ALLOWED_ORIGINS сохранить.
4. Выполнить деплой текущего проверенного коммита в Render. Убедиться, что `/version`
   показывает этот коммит, `/health/ready` — `ready`, а `/health/treasury` — `ready`.

Команды публикации для ветки `task/14-tests`, указанной в `render.yaml`:

```powershell
Set-Location 'C:\Users\amang\Documents\Workspace\education'
git add -- sozbricks/backend/Cargo.toml sozbricks/backend/Cargo.lock sozbricks/backend/src/treasury.rs sozbricks/backend/src/config.rs sozbricks/backend/src/lib.rs sozbricks/backend/src/docs.rs sozbricks/backend/tests/api.rs sozbricks/backend/.env.example sozbricks/backend/README.md
git diff --cached --stat
git commit -m "Add Devnet treasury key and balance checks"
git push origin HEAD:task/14-tests
```

Если фактическая ветка сервиса другая, в последней команде указать её. При отказе push
из-за расхождения истории остановиться и согласовать изменения; `--force` не использовать.

Копирование ключа в буфер обмена для вставки только в Secret File Render:

```powershell
Get-Content -LiteralPath 'C:\Users\amang\.config\solana\sozbricks-treasury-devnet.json' -Raw | Set-Clipboard
```

После вставки очистить текущий буфер (это не очищает ранее включённую историю буфера Windows):

```powershell
Set-Clipboard -Value ''
```

Проверка сервера перед переводом, в PowerShell:

```powershell
$api = 'https://sozbricks-api-tests.onrender.com'
$treasury = '615vW8VZFB8fLGrPTcxQ2FWBWkevahkjX5kCbtoosyQw'
Invoke-RestMethod "$api/version"
Invoke-RestMethod "$api/health/ready"
$status = Invoke-RestMethod "$api/health/treasury" -TimeoutSec 90
if ($status.status -ne 'ready' -or $status.cluster -ne 'devnet' -or $status.address -ne $treasury -or $status.signer_verified -ne $true) { throw 'Кошелёк Render не прошёл проверку' }
$status
```

Если Render ещё запускается или RPC отвечает 503/429, повторить только проверку после паузы.
Проверка баланса не отправляет транзакции. Сохранить значение `proof_message`, выполнить
ручной повторный деплой той же версии и проверить, что адрес прежний, а `proof_message`
новый: это подтверждает загрузку того же ключа новым процессом.

## 3. Перевод сначала 5 SOL, затем остатка

Выполнять только после успешной проверки Render. Все команды запускает владелец проекта.

Подготовить переменные и проверить исходный адрес:

```powershell
$solana = 'C:\Users\amang\.local\share\solana\install\releases\3.1.10\solana-release\bin\solana.exe'
$sourceKey = 'C:\Users\amang\.config\solana\id.json'
$sourceAddress = 'HMg9Wz3DLAwU8GAYh91WhRMWQzdhpAsBpUYkZcf9tMtX'
$treasury = '615vW8VZFB8fLGrPTcxQ2FWBWkevahkjX5kCbtoosyQw'
$actualSource = & $solana address --keypair $sourceKey
if ($LASTEXITCODE -ne 0 -or $actualSource.Trim() -cne $sourceAddress) { throw 'Не совпадает исходный кошелёк' }
& $solana balance $sourceAddress --url devnet --commitment finalized
& $solana balance $treasury --url devnet --commitment finalized
$before = Invoke-RestMethod 'https://sozbricks-api-tests.onrender.com/health/treasury' -TimeoutSec 90
if ($before.status -ne 'ready' -or $before.cluster -ne 'devnet' -or $before.address -ne $treasury -or $before.signer_verified -ne $true) { throw 'Кошелёк Render не прошёл проверку' }
```

Первая транзакция — ровно 5 devnet SOL:

```powershell
& $solana transfer $treasury 5 --from $sourceKey --fee-payer $sourceKey --url devnet --commitment finalized --allow-unfunded-recipient
if ($LASTEXITCODE -ne 0) { throw 'Сначала выясни результат транзакции; не повторяй перевод вслепую' }
```

Сохранить полученную подпись транзакции. При сетевом таймауте проверить её статус и баланс,
а не отправлять 5 SOL повторно. Ссылка: `https://explorer.solana.com/tx/ПОДПИСЬ?cluster=devnet`.

```powershell
$afterTest = Invoke-RestMethod 'https://sozbricks-api-tests.onrender.com/health/treasury' -TimeoutSec 90
if ($afterTest.status -ne 'ready' -or $afterTest.cluster -ne 'devnet' -or $afterTest.address -ne $treasury -or $afterTest.signer_verified -ne $true) { throw 'Кошелёк Render не прошёл проверку' }
if ([long]$afterTest.balance_lamports -ne ([long]$before.balance_lamports + 5000000000L)) { throw 'Ожидаемое пополнение 5 SOL пока не подтверждено; остаток не переводить' }
$afterTest
& $solana balance $treasury --url devnet --commitment finalized
```

После проверки первой транзакции отдельной командой перевести весь оставшийся баланс.
`ALL` учитывает комиссию; исходный кошелёк останется без SOL на новые операции.
Его файл ключа и права обновления прежней программы при этом сохраняются.

```powershell
& $solana transfer $treasury ALL --from $sourceKey --fee-payer $sourceKey --url devnet --commitment finalized
if ($LASTEXITCODE -ne 0) { throw 'Проверь подпись и балансы перед любым повтором' }
& $solana balance $sourceAddress --url devnet --commitment finalized
& $solana balance $treasury --url devnet --commitment finalized
Invoke-RestMethod 'https://sozbricks-api-tests.onrender.com/health/treasury' -TimeoutSec 90
```

Сохранить обе подписи и итоговые балансы. Успех шага: исходный баланс 0,
резерв проекта пополнен, Render показывает тот же адрес и подтверждённый баланс.

## Проверки кода

```powershell
cargo test --locked --lib --test api --test auth
```

Тесты используют искусственный ключ и локальный RPC-сервер. Настоящие переводы не выполняются.
Полные тесты с PostgreSQL выполняются существующим GitHub Actions workflow.
