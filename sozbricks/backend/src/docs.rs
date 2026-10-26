use axum::{Json, response::Html};
use serde_json::{Value, json};

pub async fn swagger_ui() -> Html<&'static str> {
    Html(
        r#"<!doctype html>
<html lang="ru">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="referrer" content="no-referrer">
<title>Söz Bricks API</title>
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/swagger-ui-dist@5.17.14/swagger-ui.css">
</head>
<body>
<div id="swagger-ui"></div>
<script src="https://cdn.jsdelivr.net/npm/swagger-ui-dist@5.17.14/swagger-ui-bundle.js"></script>
<script>
SwaggerUIBundle({
  url: '/openapi.json',
  dom_id: '#swagger-ui',
  deepLinking: true,
  persistAuthorization: false,
  queryConfigEnabled: false,
  validatorUrl: null,
  supportedSubmitMethods: ['get', 'post', 'patch'],
  presets: [SwaggerUIBundle.presets.apis]
});
</script>
</body>
</html>"#,
    )
}

fn response(description: &str, schema: Value) -> Value {
    json!({
        "description": description,
        "headers": {
            "x-request-id": {"description": "Server-generated request UUID", "schema": {"type": "string", "format": "uuid"}},
            "Cache-Control": {"schema": {"type": "string", "enum": ["no-store"]}}
        },
        "content": {"application/json": {"schema": schema}}
    })
}

fn reference(name: &str) -> Value {
    json!({"$ref": format!("#/components/schemas/{name}")})
}

fn base_spec() -> Value {
    let error = || reference("Error");
    let mut unauthorized = response(
        "Missing, malformed, expired or rejected token; account is not linked to Google",
        error(),
    );
    unauthorized["headers"]["WWW-Authenticate"] =
        json!({"schema": {"type": "string", "enum": ["Bearer"]}});
    json!({
        "openapi": "3.0.3",
        "info": {
            "title": "Söz Bricks API",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "Игра и подсчёт очков выполняются на фронтенде. API хранит переданный результат, профиль, прогресс и начисления. Запросов во время физики игры и проверки replay нет. Google-вход запускается приложением через Supabase Auth. В Authorize вставляйте session.access_token без слова Bearer. Все /v1 маршруты требуют подтверждённый Google-linked аккаунт. При первом обращении создаётся sozbricks.profiles; auth.users управляется самим Supabase. Если Auth и DATABASE_URL относятся к разным проектам, ответ будет 503 auth_database_mismatch. SOL-выплат пока нет: earned_lamports — учёт в БД, не кошелёк."
        },
        "servers": [{"url": "/", "description": "Current API deployment"}],
        "tags": [
            {"name": "Infrastructure", "description": "Public diagnostic routes; readiness checks only the database"},
            {"name": "Authentication", "description": "Verified Supabase users linked to Google"}
        ],
        "paths": {
            "/health/live": {"get": {
                "operationId": "getLiveness", "tags": ["Infrastructure"], "summary": "Process liveness",
                "responses": {"200": response("API process is running", reference("Health"))}
            }},
            "/health/ready": {"get": {
                "operationId": "getReadiness", "tags": ["Infrastructure"], "summary": "Database and schema readiness",
                "description": "Does not check Google OAuth or Supabase Auth availability.",
                "responses": {
                    "200": response("Database and migrations are ready", reference("Readiness")),
                    "503": response("Database or schema is not ready", reference("Readiness"))
                }
            }},
            "/version": {"get": {
                "operationId": "getVersion", "tags": ["Infrastructure"], "summary": "Build version and commit",
                "responses": {"200": response("Public build metadata", reference("Version"))}
            }},
            "/v1/auth/me": {"get": {
                "operationId": "getAuthenticatedUser", "tags": ["Authentication"], "summary": "Current verified Google-linked user",
                "security": [{"supabaseBearer": []}],
                "responses": {
                    "200": response("Identity from Supabase Auth; creates a missing game profile when the database is configured", reference("AuthUser")),
                    "401": unauthorized,
                    "503": response("auth_not_configured or auth_unavailable; retry later without signing the user out", error()),
                    "408": response("API request timeout", error()),
                    "413": response("Request body too large", error())
                }
            }}
        },
        "components": {
            "securitySchemes": {
                "supabaseBearer": {"type": "http", "scheme": "bearer", "bearerFormat": "Supabase access token", "description": "Use session.access_token issued by this project's Supabase Auth after Google sign-in. Tokens are not persisted by Swagger UI."}
            },
            "schemas": {
                "Health": {
                    "type": "object", "required": ["status"],
                    "properties": {"status": {"type": "string", "enum": ["ok"]}}
                },
                "Readiness": {
                    "type": "object", "required": ["status", "stage", "database", "schema"],
                    "properties": {
                        "status": {"type": "string", "enum": ["ready", "not_ready"]},
                        "stage": {"type": "string", "enum": ["database"]},
                        "database": {"type": "string", "enum": ["ok", "not_configured", "unavailable"]},
                        "schema": {"type": "string", "enum": ["ok", "pending", "unknown"]}
                    }
                },
                "Version": {
                    "type": "object", "required": ["name", "version", "revision"],
                    "properties": {"name": {"type": "string"}, "version": {"type": "string"}, "revision": {"type": "string"}}
                },
                "AuthUser": {
                    "type": "object", "required": ["id", "email", "display_name", "provider"],
                    "properties": {
                        "id": {"type": "string", "format": "uuid", "description": "Supabase auth.users.id"},
                        "email": {"type": "string", "nullable": true},
                        "display_name": {"type": "string", "maxLength": 60, "description": "Display-only user metadata; never use for authorization"},
                        "provider": {"type": "string", "enum": ["google"]}
                    }
                },
                "Error": {
                    "type": "object", "required": ["error"],
                    "properties": {"error": {
                        "type": "object", "required": ["code", "message"],
                        "properties": {"code": {"type": "string"}, "message": {"type": "string"}}
                    }}
                }
            }
        }
    })
}

fn object(properties: Value, required: &[&str]) -> Value {
    json!({"type":"object","properties":properties,"required":required})
}

fn array(schema: Value) -> Value { json!({"type":"array","items":schema}) }

fn operation(id: &str, tag: &str, summary: &str, description: &str, result: &str) -> Value {
    json!({"operationId":id,"tags":[tag],"summary":summary,"description":description,
        "security":[{"supabaseBearer":[]}],
        "responses":{
            "200":response("Success",reference(result)),
            "400":response("Malformed JSON",reference("Error")),
            "401":response("A valid Google-linked Supabase session is required",reference("Error")),
            "403":response("level_locked",reference("Error")),
            "404":response("Resource is missing or belongs to another user",reference("Error")),
            "408":response("Request timed out; retry with the same identifiers and body",reference("Error")),
            "409":response("Conflicting request_id, different result for an already completed session, or guest import already used",reference("Error")),
            "410":response("session_expired; create a new attempt",reference("Error")),
            "413":response("Body exceeds MAX_REQUEST_BODY_BYTES; default 65536 bytes",reference("Error")),
            "415":response("Content-Type must be application/json for JSON bodies",reference("Error")),
            "422":response("Invalid fields, ranges or pagination",reference("Error")),
            "503":response("Auth/database unavailable, auth_not_configured, or auth_database_mismatch; do not delete the local save",reference("Error"))
        }})
}

fn with_body(mut operation: Value, name: &str, example: Value) -> Value {
    operation["requestBody"] = json!({"required":true,"content":{"application/json":{"schema":reference(name),"example":example}}});
    operation
}

pub async fn openapi() -> Json<Value> {
    let mut spec = base_spec();
    spec["tags"].as_array_mut().unwrap().extend([
        json!({"name":"Profile","description":"Создание профиля, имя и герой"}),
        json!({"name":"Progress","description":"Сохранение прогресса аккаунта, импорт гостя, история и начисления"}),
        json!({"name":"Game sessions","description":"Два запроса на партию: начало и сохранение готового результата. Физика, слово, победа и очки определяются фронтендом."})
    ]);
    let schemas = spec["components"]["schemas"].as_object_mut().unwrap();
    let number = || json!({"type":"integer","format":"int64","minimum":0});
    let uuid = || json!({"type":"string","format":"uuid"});
    let text = || json!({"type":"string"});
    let boolean = || json!({"type":"boolean"});
    let level_id = || json!({"type":"integer","minimum":1,"maximum":24});
    schemas.insert("Profile".into(),object(json!({"user_id":uuid(),"email":{"type":"string","nullable":true},"display_name":{"type":"string","minLength":1,"maxLength":60},"hero":{"type":"string","enum":["alikhan","aiaru"]},"created_at":text(),"updated_at":text()}), &["user_id","email","display_name","hero","created_at","updated_at"]));
    schemas.insert("Progress".into(),object(json!({
        "completed":array(level_id()),"recorded_completed":array(level_id()),"imported_completed":array(level_id()),
        "best":{"type":"object","additionalProperties":number(),"description":"Лучший счёт по каждому уровню; ключи — строки 1..24. Максимум из сохранённых партий и импорта."},
        "best_score":number(),"next_level_id":{"type":"integer","nullable":true},"earned_lamports":number(),
        "balance_kind":{"type":"string","enum":["database_ledger_not_wallet"]}
    }), &["completed","recorded_completed","imported_completed","best","best_score","next_level_id","earned_lamports","balance_kind"]));
    schemas.insert("ProfileResponse".into(),object(json!({"profile":reference("Profile"),"progress":reference("Progress")}), &["profile","progress"]));
    let mut patch = object(json!({"display_name":{"type":"string","minLength":1,"maxLength":60},"hero":{"type":"string","enum":["alikhan","aiaru"]}}), &[]);
    patch["additionalProperties"] = json!(false);
    patch["minProperties"] = json!(1);
    schemas.insert("ProfilePatch".into(),patch);
    schemas.insert("Level".into(),object(json!({
        "id":level_id(),"word":text(),"translation":text(),"letter_count":number(),"speed":number(),"reward_lamports":number(),"rules_version":number(),
        "unlocked":boolean(),"completed":boolean(),"recorded_completed":boolean(),"imported_completed":boolean(),"best_score":number(),"recorded_best_score":number()
    }), &["id","word","translation","letter_count","speed","reward_lamports","rules_version","unlocked","completed","recorded_completed","imported_completed","best_score","recorded_best_score"]));
    schemas.insert("LevelsResponse".into(),object(json!({"items":array(reference("Level"))}), &["items"]));
    schemas.insert("GuestResult".into(),object(json!({"id":{"type":"string","minLength":1,"maxLength":160},"level":level_id(),"word":{"type":"string","maxLength":32},"score":{"type":"integer","minimum":0,"maximum":100000000},"won":boolean(),"seconds":{"type":"integer","minimum":0,"maximum":86400},"at":{"type":"integer","format":"int64","minimum":0,"description":"Unix milliseconds, as in the existing frontend save"}}), &["id","level","word","score","won","seconds","at"]));
    schemas.insert("GuestSave".into(),object(json!({
        "version":{"type":"integer","enum":[1]},"hero":{"type":"string","enum":["alikhan","aiaru"]},"completed":{"type":"array","maxItems":24,"items":level_id()},
        "best":{"type":"object","maxProperties":24,"additionalProperties":{"type":"integer","minimum":0,"maximum":100000000}},
        "history":{"type":"array","maxItems":100,"items":reference("GuestResult")}
    }), &["version","completed","best","history"]));
    schemas.insert("GuestImport".into(),object(json!({"import_id":uuid(),"save":reference("GuestSave")}), &["import_id","save"]));
    schemas.insert("ImportResponse".into(),object(json!({"imported":boolean(),"repeated":boolean(),"awarded_lamports":{"type":"integer","enum":[0]},"progress":reference("Progress")}), &["imported","repeated","awarded_lamports","progress"]));
    schemas.insert("StartSession".into(),object(json!({"level_id":level_id(),"request_id":uuid()}), &["level_id","request_id"]));
    schemas.insert("Session".into(),object(json!({
        "id":uuid(),"request_id":uuid(),"level_id":level_id(),"seed":number(),"rules_version":number(),
        "status":{"type":"string","enum":["started","completed","expired","abandoned"]},"started_at":text(),"expires_at":text(),
        "word":text(),"speed":number(),"reward_lamports":number(),"result_source":{"type":"string","enum":["frontend_reported"]}
    }), &["id","request_id","level_id","seed","rules_version","status","started_at","expires_at","word","speed","reward_lamports","result_source"]));
    let mut finish = object(json!({"won":boolean(),"score":{"type":"integer","minimum":0,"maximum":100000000},"collected_word":{"type":"string","maxLength":32},"duration_seconds":{"type":"integer","minimum":0,"maximum":7200}}), &["won","score","collected_word","duration_seconds"]);
    finish["additionalProperties"] = json!(false);
    schemas.insert("FinishSession".into(),finish);
    schemas.insert("FinishResponse".into(),object(json!({"session_id":uuid(),"level_id":level_id(),"won":boolean(),"score":number(),"collected_word":text(),"duration_seconds":number(),"source":{"type":"string","enum":["frontend_reported"]},"awarded_lamports":number(),"repeated":boolean(),"progress":reference("Progress")}), &["session_id","level_id","won","score","collected_word","duration_seconds","source","awarded_lamports","repeated","progress"]));
    schemas.insert("HistoryResult".into(),object(json!({"session_id":uuid(),"level_id":level_id(),"won":boolean(),"score":number(),"word":text(),"collected_word":text(),"duration_seconds":number(),"recorded_at":text(),"source":{"type":"string","enum":["frontend_reported"]}}), &["session_id","level_id","won","score","word","collected_word","duration_seconds","recorded_at","source"]));
    schemas.insert("HistoryResponse".into(),object(json!({"items":array(reference("HistoryResult")),"limit":number(),"offset":number(),"imported_history":array(reference("GuestResult")),"imported_history_verified":{"type":"boolean","enum":[false]}}), &["items","limit","offset","imported_history","imported_history_verified"]));
    schemas.insert("Reward".into(),object(json!({"id":uuid(),"level_id":level_id(),"session_id":uuid(),"amount_lamports":number(),"created_at":text(),"status":{"type":"string","enum":["recorded_not_paid"]}}), &["id","level_id","session_id","amount_lamports","created_at","status"]));
    schemas.insert("RewardsResponse".into(),object(json!({"items":array(reference("Reward")),"limit":number(),"offset":number(),"total_lamports":number(),"solana_payments_enabled":{"type":"boolean","enum":[false]}}), &["items","limit","offset","total_lamports","solana_payments_enabled"]));
    let paths = spec["paths"].as_object_mut().unwrap();
    paths.insert("/v1/profile".into(),json!({
        "get":operation("getProfile","Profile","Профиль и статистика","Вызывайте после входа или восстановления сессии. Профиль создаётся автоматически; ранее изменённые имя и герой не перезаписываются данными Google.","ProfileResponse"),
        "patch":with_body(operation("patchProfile","Profile","Изменить имя или героя","Хотя бы одно поле. display_name обрезается по краям и должен содержать 1..60 печатных символов. Тема, звук и вибрация остаются локальными настройками.","ProfileResponse"),"ProfilePatch",json!({"hero":"aiaru"}))
    }));
    paths.insert("/v1/levels".into(),json!({"get":operation("getLevels","Progress","Уровни и доступность","24 уровня. Первый открыт; следующий открывается после победы на предыдущем или импорта гостевого прохождения. Клиент получает слово, перевод, скорость, сумму награды и счёт.","LevelsResponse")}));
    paths.insert("/v1/progress".into(),json!({"get":operation("getProgress","Progress","Прогресс текущего аккаунта","completed — объединение импорта и переданных результатов; best — максимум по уровню. earned_lamports — сумма записей ledger, не баланс Solana. При выходе или смене пользователя фронтенд должен переключить локальное хранилище по user_id.","Progress")}));
    paths.insert("/v1/progress/import".into(),json!({"post":with_body(operation("importGuestProgress","Progress","Однократно перенести гостевое сохранение","После подтверждённого входа отправьте старый Save из sozbricks.local.v1 и новый import_id UUID. Сохраните этот UUID и неизменный payload для повторов. Один импорт на аккаунт; другой payload после импорта вернёт 409. completed объединяются, best сохраняется по максимуму. history максимум 100. Неизвестные поля Save (dark, sound, vibration, demoLamports) игнорируются; demoLamports не переносится в начисления. Импортированные победы открывают уровни, но наград за импорт нет. Локальный гостевой save очищайте/помечайте перенесённым только после 200, чтобы не импортировать его другому аккаунту. После переключения аккаунта загружайте его /v1/progress, не смешивайте кэши.","ImportResponse"),"GuestImport",json!({"import_id":"7ce08d91-b2d7-4638-ab7a-9eb353106d45","save":{"version":1,"hero":"alikhan","completed":[1],"best":{"1":1200},"history":[]}}))}));
    paths.insert("/v1/game-sessions".into(),json!({"post":with_body(operation("startSession","Game sessions","Создать попытку перед запуском игры","Один запрос перед партией. Генерируйте request_id UUID один раз на попытку; сетевой повтор с тем же request_id возвращает ту же попытку. Для новой игры нужен новый UUID. Другая level_id при повторном UUID даёт 409. Одновременно одна активная попытка на аккаунт; новая попытка закрывает предыдущую как abandoned. Срок — 2 часа. seed совпадает с текущей формулой фронта level_id*12345+17, но сервер физику не воспроизводит.","Session"),"StartSession",json!({"level_id":1,"request_id":"6e7e83e4-938b-46fd-bb5d-85ed264d26f7"}))}));
    paths.insert("/v1/game-sessions/{id}".into(),json!({"get":operation("getSession","Game sessions","Состояние своей попытки","Для восстановления идентификатора и статуса. Полное состояние шариков на сервере не хранится. Чужой UUID возвращает 404.","Session")}));
    paths.insert("/v1/game-sessions/{id}/finish".into(),json!({"post":with_body(operation("finishSession","Game sessions","Сохранить готовый результат партии","Один запрос после победы/проигрыша. Фронт сам вычисляет won, score, collected_word и целые duration_seconds (Math.floor(game.elapsed)). Бэкенд проверяет типы, диапазоны, владельца и статус попытки; победу, буквы и очки не пересчитывает. Первая победа, переданная через этот маршрут на каждом уровне, создаёт одно начисление по сохранённой сервером сумме уровня; повторное прохождение обновляет best, но не баланс. Импорт не лишает награды за первую последующую победу через API. Тот же payload для завершённой попытки возвращает 200 repeated=true, другую версию результата сервер не принимает (409). В одной транзакции сохраняются результат, прогресс и ledger. При сетевой ошибке повторяйте тот же id и payload. Реальных SOL-выплат нет.","FinishResponse"),"FinishSession",json!({"won":true,"score":1200,"collected_word":"АТ","duration_seconds":35}))}));
    paths.insert("/v1/game-sessions/{id}/abandon".into(),json!({"post":operation("abandonSession","Game sessions","Закрыть незавершённую попытку","Вызывайте при выходе из игры, если не сохраняете проигрыш через finish. Тело не требуется. Повтор безопасен. Completed-попытку закрыть нельзя.","Session")}));
    paths.insert("/v1/history".into(),json!({"get":operation("getHistory","Progress","История переданных партий и старое сохранение","items — страницы новых результатов, сначала свежие. imported_history — отдельные старые записи, максимум 100, возвращаются без пагинации; не смешивайте их с серверными ID. Все результаты сообщены фронтендом; античит-проверки нет.","HistoryResponse")}));
    paths.insert("/v1/rewards".into(),json!({"get":operation("getRewards","Progress","Начисления за первые победы","amount_lamports — целое число; 1 SOL = 1000000000 lamports. total_lamports — общий итог независимо от страницы. status=recorded_not_paid. Этот маршрут ничего не переводит и не принимает сумму от клиента.","RewardsResponse")}));
    for path in ["/v1/history","/v1/rewards"] {
        paths.get_mut(path).unwrap()["get"]["parameters"] = json!([
            {"name":"limit","in":"query","schema":{"type":"integer","default":20,"minimum":1,"maximum":100}},
            {"name":"offset","in":"query","schema":{"type":"integer","default":0,"minimum":0,"maximum":10000}}
        ]);
    }
    for path in ["/v1/game-sessions/{id}","/v1/game-sessions/{id}/finish","/v1/game-sessions/{id}/abandon"] {
        paths.get_mut(path).unwrap()["parameters"] = json!([{"name":"id","in":"path","required":true,"schema":uuid()}]);
    }
    spec["info"]["description"] = json!(format!("{}\n\nПорядок подключения фронта: Google OAuth → /v1/auth/me → при наличии гостевого Save /v1/progress/import → /v1/profile и /v1/levels → POST /v1/game-sessions → игра целиком на клиенте → POST finish → обновить состояние из progress ответа. Никакого потока движений или WebSocket. Сетевой сбой сохраняйте в локальной очереди под user_id; при смене аккаунта не отправляйте чужие результаты. Гость продолжает локальную игру.\n\nРабота с БД: миграции используют DATABASE_URL, игровые операции — отдельный пул с SET ROLE sozbricks_runtime и RLS по подтверждённому user_id. При отсутствии RUNTIME_DATABASE_URL этот пул использует те же credentials, но понижает SQL-роль после подключения. Для отдельного LOGIN задайте RUNTIME_DATABASE_URL и вручную выдайте ему membership sozbricks_runtime. Схему sozbricks не добавляйте в exposed schemas. В Render должны быть SUPABASE_URL и SUPABASE_PUBLISHABLE_KEY. CORS_ALLOWED_ORIGINS — точные origins веба. MAX_REQUEST_BODY_BYTES по умолчанию 65536. При 503 не очищайте пользовательские сохранения. При истечении сессии 410 нужна новая попытка; для уже завершённой попытки неизменный finish можно повторять.",spec["info"]["description"].as_str().unwrap()));
    Json(spec)
}
