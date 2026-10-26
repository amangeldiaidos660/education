use std::collections::HashMap;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::Value;
use sozbricks_api::{
    app_with_database,
    config::Config,
    database::{Database, MIGRATOR},
};
use sqlx::{PgPool, Row};
use tower::ServiceExt;
use uuid::Uuid;

fn values(url: &str) -> HashMap<String, String> {
    HashMap::from([
        ("DATABASE_URL".into(), url.into()),
        ("DATABASE_SSL_MODE".into(), "disable".into()),
    ])
}

#[test]
fn database_config_rejects_invalid_values_without_exposing_credentials() {
    let secret_url = "postgresql://postgres:never-print-this@db.example/postgres";
    for (key, value) in [
        ("DATABASE_URL", "https://project.supabase.co"),
        (
            "DATABASE_URL",
            "postgresql://postgres:[YOUR-PASSWORD]@localhost/postgres",
        ),
        ("DATABASE_SSL_MODE", "prefer"),
        ("DATABASE_MAX_CONNECTIONS", "0"),
        ("DATABASE_MAX_CONNECTIONS", "11"),
        ("DATABASE_ACQUIRE_TIMEOUT_SECONDS", "0"),
        ("DATABASE_HEALTH_TIMEOUT_SECONDS", "6"),
        ("DATABASE_RUN_MIGRATIONS", "yes"),
        ("RUNTIME_DATABASE_URL", "https://project.supabase.co"),
        ("RUNTIME_DATABASE_URL", "postgresql://postgres:[YOUR-PASSWORD]@localhost/postgres"),
    ] {
        let mut vars = HashMap::from([("DATABASE_URL".into(), secret_url.into())]);
        vars.insert(key.into(), value.into());
        let error = Config::from_values(&vars)
            .err()
            .expect("configuration must be rejected")
            .to_string();
        assert!(error.contains(key));
        assert!(!error.contains("never-print-this"));
        assert!(!error.contains(secret_url));
    }
    let mut vars = values(secret_url);
    assert!(Config::from_values(&vars).is_err());
    vars.insert("DATABASE_SSL_MODE".into(), "require".into());
    let config = Config::from_values(&vars).unwrap();
    let db = config.database.unwrap();
    assert_eq!(db.max_connections, 3);
    assert!(matches!(
        db.options.get_ssl_mode(),
        sqlx::postgres::PgSslMode::Require
    ));
}

async fn create_result(pool: &PgPool, owner: Uuid, level: i16, won: bool) -> Uuid {
    let session = Uuid::new_v4();
    sqlx::query("INSERT INTO sozbricks.game_sessions(id,user_id,level_id,rules_version,seed,status,expires_at,finished_at) VALUES($1,$2,$3,1,17,'completed',now()+interval '1 hour',now())")
        .bind(session).bind(owner).bind(level).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO sozbricks.game_results(session_id,user_id,level_id,won,collected_word,score,duration_ticks) VALUES($1,$2,$3,$4,'АТ',1000,3600)")
        .bind(session).bind(owner).bind(level).bind(won).execute(pool).await.unwrap();
    session
}

async fn reward(
    pool: &PgPool,
    owner: Uuid,
    level: i16,
    session: Uuid,
    amount: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO sozbricks.reward_ledger(user_id,level_id,session_id,amount_lamports) VALUES($1,$2,$3,$4)")
        .bind(owner).bind(level).bind(session).bind(amount).execute(pool).await.map(|_| ())
}

fn sqlstate(result: Result<(), sqlx::Error>, expected: &str) {
    let error = result.expect_err("database must reject invalid data");
    assert_eq!(error.as_database_error().unwrap().code().unwrap(), expected);
}

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL database on the cloud CI runner"]
async fn postgres_schema_and_readiness_contract() {
    assert_eq!(std::env::var("CI").as_deref(), Ok("true"));
    let url = std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL must be set in CI");
    let config = Config::from_values(&values(&url)).unwrap();
    let host = config.database.as_ref().unwrap().options.get_host();
    assert!(matches!(host, "127.0.0.1" | "localhost"));
    let database = Database::new(config.database.as_ref());
    let pool = database.pool().unwrap();
    sqlx::raw_sql("CREATE SCHEMA auth; CREATE TABLE auth.users(id uuid PRIMARY KEY); CREATE ROLE anon NOLOGIN; CREATE ROLE ci_reader NOLOGIN;").execute(pool).await.unwrap();
    assert!(database.readiness().await.is_err());
    database.prepare().await.unwrap();
    database.prepare().await.unwrap();
    database.readiness().await.unwrap();
    let levels = sqlx::query(
        "SELECT id,word,letter_count,reward_lamports,speed FROM sozbricks.levels ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    assert_eq!(levels.len(), 24);
    assert_eq!(levels[2].get::<String, _>("word"), "КІТАП");
    assert_eq!(levels[23].get::<i16, _>("letter_count"), 11);
    assert_eq!(levels[23].get::<i32, _>("speed"), 510);
    for row in &levels {
        let length = row.get::<i16, _>("letter_count");
        let expected = if length <= 3 {
            100_000_000
        } else if length <= 5 {
            300_000_000
        } else if length <= 7 {
            400_000_000
        } else {
            500_000_000
        };
        assert_eq!(row.get::<i64, _>("reward_lamports"), expected);
    }
    let rls: i64 = sqlx::query_scalar("SELECT count(*) FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='sozbricks' AND c.relkind='r' AND c.relrowsecurity")
        .fetch_one(pool).await.unwrap();
    assert_eq!(rls, 8);
    let users = [Uuid::new_v4(), Uuid::new_v4()];
    for user in users {
        sqlx::query("INSERT INTO auth.users(id) VALUES($1)")
            .bind(user)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO sozbricks.profiles(user_id,display_name) VALUES($1,'Игрок')")
            .bind(user)
            .execute(pool)
            .await
            .unwrap();
    }
    let first = create_result(pool, users[0], 1, true).await;
    reward(pool, users[0], 1, first, 100_000_000).await.unwrap();
    let repeated = create_result(pool, users[0], 1, true).await;
    sqlstate(
        reward(pool, users[0], 1, repeated, 100_000_000).await,
        "23505",
    );
    sqlstate(
        reward(pool, users[1], 1, repeated, 100_000_000).await,
        "23503",
    );
    let lost = create_result(pool, users[0], 3, false).await;
    sqlstate(reward(pool, users[0], 3, lost, 300_000_000).await, "23503");
    sqlstate(reward(pool, users[0], 3, lost, -1).await, "23514");
    let progress = sqlx::query("INSERT INTO sozbricks.level_progress(user_id,level_id,completed_at,first_win_session_id) VALUES($1,3,now(),$2)")
        .bind(users[0]).bind(lost).execute(pool).await.map(|_| ());
    sqlstate(progress, "23503");
    let racing_a = create_result(pool, users[0], 2, true).await;
    let racing_b = create_result(pool, users[0], 2, true).await;
    let (a, b) = tokio::join!(
        reward(pool, users[0], 2, racing_a, 100_000_000),
        reward(pool, users[0], 2, racing_b, 100_000_000)
    );
    assert_ne!(a.is_ok(), b.is_ok());
    let ledger_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM sozbricks.reward_ledger WHERE user_id=$1 AND level_id=2",
    )
    .bind(users[0])
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(ledger_count, 1);
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SET LOCAL ROLE anon")
        .execute(&mut *tx)
        .await
        .unwrap();
    let denied = sqlx::query("SELECT * FROM sozbricks.profiles")
        .execute(&mut *tx)
        .await
        .map(|_| ());
    sqlstate(denied, "42501");
    tx.rollback().await.unwrap();
    sqlx::raw_sql("GRANT USAGE ON SCHEMA sozbricks TO ci_reader; GRANT SELECT ON ALL TABLES IN SCHEMA sozbricks TO ci_reader;").execute(pool).await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SET LOCAL ROLE ci_reader")
        .execute(&mut *tx)
        .await
        .unwrap();
    let visible: i64 = sqlx::query_scalar("SELECT count(*) FROM sozbricks.profiles")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(visible, 0);
    tx.rollback().await.unwrap();
    let router = app_with_database(config.clone(), database.clone());
    let response = router
        .clone()
        .oneshot(Request::get("/health/ready").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
    assert_eq!(json["database"], "ok");
    assert_eq!(json["schema"], "ok");
    sqlx::query("UPDATE public._sqlx_migrations SET checksum=$1 WHERE version=$2")
        .bind(vec![0_u8; 48])
        .bind(MIGRATOR.iter().next().unwrap().version)
        .execute(pool)
        .await
        .unwrap();
    assert!(database.readiness().await.is_err());
    assert!(database.prepare().await.is_err());
    database.close().await;
    let response = router
        .oneshot(Request::get("/health/ready").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}
