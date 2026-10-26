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
  supportedSubmitMethods: ['get'],
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

pub async fn openapi() -> Json<Value> {
    let error = || reference("Error");
    let mut unauthorized = response(
        "Missing, malformed, expired or rejected token; account is not linked to Google",
        error(),
    );
    unauthorized["headers"]["WWW-Authenticate"] =
        json!({"schema": {"type": "string", "enum": ["Bearer"]}});
    Json(json!({
        "openapi": "3.0.3",
        "info": {
            "title": "Söz Bricks API",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "Rust API: инфраструктура и проверка сессии Supabase. Google-вход выполняется через Supabase Auth в приложении. После входа передавайте session.access_token в Authorization: Bearer. В Authorize вставляйте только сам токен, без слова Bearer. Google access_token / id_token и API-ключ проекта здесь не подходят. Refresh token остаётся в клиенте Supabase. Маршрут me возвращает подтверждённую личность, не создаёт игровой профиль и не пишет в БД. Google означает наличие Google в привязанных провайдерах аккаунта. Игровые маршруты, история, прогресс и начисления пока не реализованы."
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
                    "200": response("Verified identity from Supabase Auth; no database writes", reference("AuthUser")),
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
    }))
}
