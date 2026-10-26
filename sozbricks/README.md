```powershell
cd D:\Downloads\solana\8-15\sozbricks\frontend
npm ci --cache ..\.local\npm-cache
npm start
```

```powershell
cd D:\Downloads\solana\8-15\sozbricks\frontend
npm run web
```

## Бэкенд локально (PowerShell)

Нужны Rust через [rustup](https://rustup.rs/) и Visual Studio Build Tools с компонентом «Разработка классических приложений на C++» и Windows SDK.

```powershell
cd D:\Downloads\solana\8-15\sozbricks\backend
rustup toolchain install 1.98.1 --profile minimal
```

В том же терминале укажи подключение к существующей базе Supabase и публичный ключ из Settings → API Keys (`sb_publishable_…`):

```powershell
$env:DATABASE_URL = Read-Host 'Вставь PostgreSQL URI из Supabase Connect → Session pooler, порт 5432'
$env:SUPABASE_URL = 'https://dvnqejwtzglzgwrmqffb.supabase.co'
$env:SUPABASE_PUBLISHABLE_KEY = Read-Host 'Вставь publishable key Supabase'
$env:DATABASE_SSL_MODE = 'require'
$env:HOST = '127.0.0.1'
$env:PORT = '8080'
$env:APP_REVISION = 'local'
$env:CORS_ALLOWED_ORIGINS = 'http://localhost:8081,http://localhost:19006'
cargo +1.98.1 run --locked
```

В DATABASE_URL должен быть настоящий пароль без квадратных скобок. Спецсимволы в пароле кодируются для URL: `#` → `%23`, `@` → `%40`, `%` → `%25`. Значения `.env` автоматически не загружаются. Миграции применяются при запуске; отдельный PostgreSQL на компьютере не нужен.

- Swagger: http://127.0.0.1:8080/docs
- Проверка процесса: http://127.0.0.1:8080/health/live
- Проверка базы: http://127.0.0.1:8080/health/ready — ожидается `database: ok`, `schema: ok`.
- Для фронтенда адрес API: `http://localhost:8080`. На Android-телефоне используй LAN-IP компьютера и запусти API с `$env:HOST = '0.0.0.0'`; телефон и компьютер должны быть в одной сети, порт 8080 разрешён в брандмауэре.

Для `/v1/auth/me` нужен Supabase `session.access_token` после Google-входа. В Swagger нажми Authorize и вставь токен без слова `Bearer`. Без токена ожидается 401.

Остановка: `Ctrl+C`. Повторный запуск в этом же терминале: `cargo +1.98.1 run --locked`.
