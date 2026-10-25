# Söz Bricks API — Supabase Auth и PostgreSQL

Rust API разворачивается и проверяется в облаке. Ветка разработки и тестирования — `task/14-tests`. В `main` попадает только принятая демонстрационная версия. Папка `frontend` в рамках этого этапа не изменяется.

## Готовый каркас

- Axum и Tokio, JSON-ответы и JSON-логи в stdout.
- Проверяемые переменные окружения, CORS по списку разрешённых origins.
- Ограничение размера тела запроса, тайм-аут обработки, серверный `x-request-id`.
- Корректное завершение по SIGTERM и Ctrl+C.
- Docker-образ с непривилегированным пользователем.
- Облачные проверки конфигурации, HTTP-контракта и контейнера в GitHub Actions.
- PostgreSQL через SQLx, TLS, ограниченный пул подключений и контроль готовности схемы.
- Версионированные миграции и 24 уровня с казахскими словами.

Подключение базы Supabase настраивается через DATABASE_URL. Rust проверяет пользовательские сессии через Supabase Auth. Google OAuth настраивается в Google Cloud и Supabase, а запускается из клиентского приложения. Игровые HTTP-маршруты и Solana пока не реализованы. Сервис не запрашивает и не хранит ключи кошельков.

## Маршруты

| Метод | Путь | Ответ |
| --- | --- | --- |
| GET / HEAD | `/health/live` | `{"status":"ok"}` |
| GET / HEAD | `/health/ready` | `{"status":"ready","stage":"database","database":"ok","schema":"ok"}` |
| GET / HEAD | `/version` | `name`, `version`, `revision` |
| GET / HEAD | `/v1/auth/me` | Подтверждённая личность: `id`, `email`, `display_name`, `provider`; нужен Bearer-токен |
| GET / HEAD | `/docs` | Swagger UI |
| GET / HEAD | `/openapi.json` | Спецификация OpenAPI 3.0.3 |

Готовность возвращает HTTP 200 только после успешного SELECT 1 и проверки таблиц, версий и контрольных сумм миграций. При отсутствии DATABASE_URL, недоступной базе или неготовой схеме возвращается HTTP 503. Ливнес остаётся HTTP 200, пока работает процесс. Готовность базы не означает готовность Google OAuth или игровой логики.

Ошибки возвращаются в виде `{"error":{"code":"not_found","message":"Route not found"}}`. Предусмотрены 404, 405, 413, 400 при ошибке чтения тела и 408 при тайм-ауте. Ответ содержит `x-request-id`; запросы и ошибки не кешируются. Логи содержат путь без query-параметров, метод, статус и длительность; заголовки авторизации и тела запросов не логируются.

## Конфигурация

Файл `.env.example` — справочник. Приложение читает переменные окружения, а не загружает `.env` автоматически. На Render задавайте значения в Environment.

| Переменная | По умолчанию | Правило |
| --- | --- | --- |
| `HOST` | `0.0.0.0` | IP-адрес для прослушивания |
| `PORT` | `8080` | 1–65535; на Render используется выданный платформой порт |
| `RUST_LOG` | `sozbricks_api=info` | Фильтр tracing; неправильное значение останавливает запуск |
| `CORS_ALLOWED_ORIGINS` | пусто | Origins через запятую, например `https://game.example`; без путей, credentials и `*` |
| `REQUEST_TIMEOUT_SECONDS` | `30` | 1–120, включая чтение тела и выполнение обработчика |
| `MAX_REQUEST_BODY_BYTES` | `65536` | 1–1048576 |
| `APP_REVISION` | build-time `GIT_SHA` или `unknown` | 1–128 букв ASCII, цифр, `.`, `_`, `-` |
| `DATABASE_URL` | не задана | PostgreSQL URI из Supabase Connect → Session pooler; без неё readiness возвращает 503 |
| `DATABASE_SSL_MODE` | `require` | `require` или `verify-full`; `disable` допустим только для loopback PostgreSQL на cloud CI runner |
| `DATABASE_MAX_CONNECTIONS` | `3` | 1–10; pool min_connections = 0, idle_timeout = 60 секунд |
| `DATABASE_ACQUIRE_TIMEOUT_SECONDS` | `5` | 1–30 |
| `DATABASE_HEALTH_TIMEOUT_SECONDS` | `3` | 1–5; ограничивает полную проверку готовности |
| `DATABASE_RUN_MIGRATIONS` | `true` | Применять миграции при запуске; `false` только после отдельного применения миграций |
| `SUPABASE_URL` | не задана | HTTPS origin проекта, без путей, query-параметров и credentials |
| `SUPABASE_PUBLISHABLE_KEY` | не задана | Публичный API-ключ `sb_publishable_…` из Supabase Settings → API Keys; требует SUPABASE_URL |
| `AUTH_TIMEOUT_SECONDS` | `5` | 1–10; полное время запроса к Supabase Auth, включая чтение ответа |

DATABASE_URL хранится только в Render Environment. Не сохраняйте её в репозиторий и не передавайте в GitHub Actions. URI и низкоуровневые ошибки подключения не выводятся в логи или HTTP-ответы. Пароль со специальными символами должен быть URL-кодирован. Для текущего проекта используется session pooler на порту 5432; transaction pooler не подходит для session advisory locks мигратора.

TLS принудительно включён по умолчанию, даже если в URI указан более слабый режим. Для полной проверки сертификата используйте verify-full с доверенным корневым сертификатом. Полный SQL-текст и параметры запросов не логируются.

## Google-вход и проверка сессии

Google Client ID и Client Secret хранятся в настройках Google-провайдера Supabase. В Render добавляются SUPABASE_URL и SUPABASE_PUBLISHABLE_KEY. Rust не получает Google Client Secret, JWT signing secret или service_role. Если publishable key не задан, сервис продолжает работать: валидный по формату Bearer-заголовок на /v1/auth/me получает HTTP 503 с auth_not_configured. Database readiness независимо от этого может быть успешной.

Клиент входит через Supabase Auth и передаёт session.access_token в Authorization: Bearer. Rust обращается к фиксированному /auth/v1/user своего проекта с publishable key и этим токеном. Пользовательские URL не принимаются, redirects HTTP-клиента отключены. Каждый защищённый запрос проверяется заново; сервер не хранит пользовательские токены и не возвращает refresh token или весь ответ Supabase. Лимит ответа Auth — 64 KiB. Настоящие запросы в Supabase происходят при использовании API; тесты CI используют временный HTTP-сервер с тестовыми ответами.

Доступ разрешён только для role=authenticated, неанонимного пользователя с ненулевым UUID и Google в app_metadata.providers. Это подтверждает привязку Google к аккаунту, но не доказывает, что именно текущая сессия создана через Google, если у аккаунта есть другие способы входа. Если нужен исключительно Google-вход, отключите ненужные провайдеры в Supabase. Имя берётся из пользовательских metadata, обрезается до 60 символов и используется только для отображения. На фронтенде показывайте его обычным текстом, не HTML.

Успешный GET /v1/auth/me возвращает:

```json
{
  "id": "17bcb386-6840-4d10-8715-a45f73b92e6d",
  "email": "player@example.com",
  "display_name": "Қазақ ойыншысы",
  "provider": "google"
}
```

id соответствует auth.users.id. email может быть null. Этот маршрут ничего не записывает в sozbricks.profiles; игровой профиль будет отдельным API. Не отправляйте user_id из формы как подтверждение личности — будущие защищённые операции должны брать владельца только из результата серверной проверки.

| HTTP | error.code | Действие клиента |
| --- | --- | --- |
| 401 | unauthorized | Токен отсутствует, имеет неверный формат, отклонён Auth либо аккаунт не соответствует условиям; попробовать обновление сессии, затем повторный вход |
| 503 | auth_not_configured | Проверить переменные Render |
| 503 | auth_unavailable | Auth недоступен, ограничил запросы или вернул неожиданный ответ; повторить позже, не разлогинивать пользователя |
| 408 | request_timeout | Истёк общий тайм-аут API; повторить позже |

Supabase Auth остаётся владельцем сессий и сроков JWT. Выход в Supabase не обязательно немедленно отзывает уже выданный access token: он может действовать до истечения срока. Rust не обещает немедленное прекращение всех сессий и не реализует собственный механизм refresh/logout. Клиентский Supabase SDK управляет обновлением токенов и выходом.

## Подключение веба и Expo

Папку frontend этот этап не изменяет. Для последующей интеграции понадобятся клиентские переменные EXPO_PUBLIC_SUPABASE_URL, EXPO_PUBLIC_SUPABASE_PUBLISHABLE_KEY и EXPO_PUBLIC_API_URL. Первые две относятся к Supabase Auth, последняя — https://sozbricks-api-tests.onrender.com. DATABASE_URL и пароля базы в приложении быть не должно.

В Supabase Authentication → URL Configuration задайте Site URL опубликованного веб-приложения и добавьте его точный callback URL в Redirect URLs. В Render CORS_ALLOWED_ORIGINS добавьте точный origin фронтенда без пути. Callback Supabase, указанный в Google Cloud, остаётся https://PROJECT_REF.supabase.co/auth/v1/callback. Это разные адреса: сначала Google возвращает пользователя в Supabase, затем Supabase возвращает его в приложение.

Для веба с @supabase/supabase-js используйте один экземпляр клиента с flowType=pkce, detectSessionInUrl=true, persistSession=true и autoRefreshToken=true. Вход запускается так:

```typescript
const { error } = await supabase.auth.signInWithOAuth({
  provider: 'google',
  options: { redirectTo: `${window.location.origin}/auth/callback` },
});
```

Фронтенд должен обслуживать /auth/callback. После возврата SDK обрабатывает PKCE-код; дождитесь восстановления сессии через getSession или onAuthStateChange, обработайте ошибку входа и только затем вызывайте Rust API:

```typescript
const { data: { session }, error } = await supabase.auth.getSession();
if (error || !session) throw new Error('Требуется вход');
const response = await fetch(`${API_URL}/v1/auth/me`, {
  headers: { Authorization: `Bearer ${session.access_token}` },
});
const data = await response.json();
if (!response.ok) throw new Error(data.error?.code ?? 'request_failed');
```

Для Android через Expo потребуется браузерный OAuth с PKCE и callback в приложение: зарегистрированная scheme, точный deep link в Supabase Redirect URLs, skipBrowserRedirect=true, открытие через expo-web-browser.openAuthSessionAsync и обмен вернувшегося code через exchangeCodeForSession. Хранилище сессии и PKCE verifier должно переживать открытие браузера. Эти настройки требуют отдельной интеграции фронтенда и проверки на устройстве; пример веба нельзя копировать в Android без изменений. Возврат в приложение проверяйте на development/standalone build с его scheme. Токены не передавайте через query-параметры Rust API, в логи или чат.

## Swagger и ручная проверка

После успешного CI и деплоя пользователь открывает /docs. OpenAPI доступен отдельно на /openapi.json, сервер запросов — текущий origin. Интерфейс Swagger загружает закреплённую версию swagger-ui-dist 5.17.14 с jsDelivr; для отображения UI браузеру нужен доступ к CDN. JSON-спецификация обслуживается самим API и доступна независимо от CDN.

Сначала проверьте /v1/auth/me без токена: ожидается 401 unauthorized и WWW-Authenticate: Bearer. После Google-входа в приложении возьмите session.access_token, нажмите Authorize и вставьте только токен, без префикса Bearer. Выполните GET /v1/auth/me: ожидается 200 с id аккаунта. Swagger не запускает Google-вход и не обменивает refresh token. Авторизация не сохраняется после перезагрузки страницы, внешний validator выключен. Проверки реального Render и Supabase выполняет пользователь; агент не опрашивает эти сервисы.

Официальные инструкции: [Google OAuth](https://supabase.com/docs/guides/auth/social-sign-in/auth-google), [Expo и social sign-in](https://supabase.com/docs/guides/auth/quickstarts/with-expo-react-native-social-auth), [PKCE](https://supabase.com/docs/guides/auth/sessions/pkce-flow).

## Схема и миграции

Миграции из migrations встроены в бинарный файл. При запуске фоновая задача применяет их с блокировкой SQLx на выделенном соединении. Соединение закрывается после миграции, в том числе при ошибке или отмене, чтобы не оставлять session lock в пуле. При неудаче попытка повторяется через 15 секунд. HTTP-сервер доступен для диагностики, но readiness остаётся 503. Render health check пока использует /health/live.

| Таблица в схеме sozbricks | Назначение |
| --- | --- |
| profiles | Профиль, связанный с auth.users.id, имя и герой |
| levels | 24 уровня: слово, перевод, длина, скорость, награда в lamports |
| game_sessions | Попытка, владелец, уровень, seed, версия правил и сроки |
| game_results | Один результат на попытку, слово, очки, длительность в тиках 1/120 секунды |
| level_progress | Лучший результат и первая победа пользователя на уровне |
| reward_ledger | Одно начисление за первую победу на уровне; это запись в БД, не выплата SOL |

Схема sozbricks не предназначена для Supabase Data API. Права PUBLIC отозваны, на всех шести таблицах включён RLS без разрешающих клиентских политик. Не добавляйте схему в exposed schemas и не выдавайте anon/authenticated прямой доступ к таблицам. Будущие операции проходят через проверку пользователя в Rust.

Внешние ключи связывают результат с владельцем и уровнем попытки. Начисление и отметка первой победы могут ссылаться только на выигранный результат. Уникальность пары пользователь/уровень в ledger защищает от двойного начисления, включая параллельные запросы. Полный расчёт результата и размера награды будет реализован на шаге игровой логики.

На первом подключении DATABASE_URL использует административную роль postgres для создания схемы. Эта роль обходит RLS. Перед открытием игровых операций нужна отдельная ограниченная LOGIN-роль для runtime и отдельный порядок применения миграций: DATABASE_RUN_MIGRATIONS=false у runtime. Этот переход выполняется до добавления пользовательских операций, не заменяется включением RLS.

Не редактируйте уже применённые миграции: добавляйте новые файлы. Несовпадение checksum оставляет readiness в 503. Удаление таблиц, очистка данных и откат миграций автоматически не выполняются.

Пустой CORS-список не разрешает браузерные cross-origin запросы. Это не механизм авторизации: технические маршруты публичны, /v1/auth/me требует Bearer-токен. При подключении фронтенда добавьте его точный origin; разрешены GET/HEAD, заголовки Authorization/Content-Type, без cookie credentials.

## Облачные проверки

Workflow `.github/workflows/sozbricks-backend.yml` запускается при изменениях бэкенда в `task/14-tests`, для PR в эту ветку и вручную. Он проверяет форматирование, Clippy без предупреждений, тесты и реальный HTTP-запуск Docker-контейнера на GitHub runner. Для интеграционных тестов создаётся временный PostgreSQL 17 с минимальной auth.users fixture. Доступ к реальной Supabase-базе и её секретам CI не нужен. Контейнер также проверяется на корректный выход после SIGTERM, при остановке PostgreSQL readiness становится 503, а после его восстановления возвращается 200. Локальный Rust-сервер и Docker для этого не нужны.

CI подготавливает Cargo.lock командой cargo update --workspace и форматирование через rustfmt. Существующие сторонние зависимости остаются зафиксированы, если их изменение не требуется новым зависимостям workspace. Если есть изменения, GitHub Actions сохраняет только Cargo.lock, src и tests в task/14-tests от имени github-actions[bot]. Для PR автоматическая запись запрещена, изменения должны быть подготовлены заранее. Main не изменяется.

Clippy и тесты используют --locked, Docker собирается из подготовленного коммита. Push от GITHUB_TOKEN не запускает повторный workflow: поэтому для созданного CI-коммита публикуется статус PostgreSQL cloud validation, который становится успешным только после Rust-проверок и контейнерных тестов. Render должен использовать After CI Checks Pass. При ошибке любого тестового job статус не разрешает деплой подготовленного коммита.

## Render Free

Тестовый API: https://sozbricks-api-tests.onrender.com. Проверки: `/health/live`, `/health/ready`, `/version`.

1. Подключить GitHub-репозиторий `amangeldiaidos660/education` к Render.
2. Создать Blueprint из ветки `task/14-tests`, указав путь `sozbricks/render.yaml`.
3. Проверить план **Free**, ветку **task/14-tests** и автодеплой **After CI Checks Pass**.
4. После успешного CI проверить публичные `/health/live`, `/health/ready`, `/version`.

Пути Docker в Blueprint заданы от корня Git-репозитория `8-15`, не от папки файла Blueprint. Сервис, созданный вручную, использует Root Directory `sozbricks/backend`, Dockerfile `./Dockerfile`, Docker context `.` и health check `/health/live`. Это эквивалентно путям Blueprint без Root Directory. При ручном создании сервис не становится управляемым Blueprint автоматически: настройки ветки и автодеплоя проверяются в Render Dashboard.

Render публикует commit в `RENDER_GIT_COMMIT`; API использует его в `/version`, если `APP_REVISION` не задан. В CI revision передаётся как Docker build arg `GIT_SHA`.

Не включайте дополнительный автодеплой по каждому push в обход CI. Изменения старых недель и фронтенда не должны пересобирать API. На Free сервис засыпает после простоя: первый запрос может потребовать ожидания пробуждения. Данные на файловой системе контейнера не сохраняются.

## Следующий шаг

После ручного подтверждения авторизации — ограниченная runtime-роль, API игрового профиля, выдачи уровней и попыток, проверки результатов, прогресса и истории начислений. Наличие таблиц ещё не означает готовность этих операций. Изменения фронтенда выполняются отдельно. Переход к Solana требует отдельного подтверждения готовности второго этапа.
