use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use sqlx::{
    PgPool, Row,
    migrate::{MigrateError, Migrator},
    postgres::PgPoolOptions,
};

use crate::config::DatabaseConfig;

pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

#[derive(Clone)]
pub struct Database {
    pool: Option<PgPool>,
    runtime: Option<PgPool>,
    initialized: Arc<AtomicBool>,
    run_migrations: bool,
    health_timeout: Duration,
}

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("database is not configured")]
    NotConfigured,
    #[error("database connection or query failed")]
    Unavailable,
    #[error("database migration failed")]
    Migration,
    #[error("database schema is not ready")]
    Schema,
}

impl Database {
    pub fn new(config: Option<&DatabaseConfig>) -> Self {
        Self {
            runtime: config.map(|config| {
                PgPoolOptions::new().max_connections(config.max_connections).min_connections(0)
                    .acquire_timeout(config.acquire_timeout).idle_timeout(Duration::from_secs(60))
                    .after_connect(|connection, _| Box::pin(async move {
                        sqlx::query("SET ROLE sozbricks_runtime").execute(connection).await?;
                        Ok(())
                    }))
                    .connect_lazy_with(config.runtime_options.clone().unwrap_or_else(|| config.options.clone()))
            }),
            pool: config.map(|config| {
                PgPoolOptions::new()
                    .max_connections(config.max_connections)
                    .min_connections(0)
                    .acquire_timeout(config.acquire_timeout)
                    .idle_timeout(Duration::from_secs(60))
                    .connect_lazy_with(config.options.clone())
            }),
            initialized: Arc::new(AtomicBool::new(false)),
            run_migrations: config.is_some_and(|config| config.run_migrations),
            health_timeout: config.map_or(Duration::from_secs(3), |config| config.health_timeout),
        }
    }

    pub fn configured(&self) -> bool {
        self.pool.is_some()
    }

    pub fn pool(&self) -> Option<&PgPool> {
        self.pool.as_ref()
    }

    pub fn runtime_pool(&self) -> Option<&PgPool> {
        if self.initialized.load(Ordering::Acquire) { self.runtime.as_ref() } else { None }
    }

    pub async fn prepare(&self) -> Result<(), DatabaseError> {
        let pool = self.pool.as_ref().ok_or(DatabaseError::NotConfigured)?;
        if self.run_migrations {
            let mut connection = pool
                .acquire()
                .await
                .map_err(|_| DatabaseError::Unavailable)?;
            connection.close_on_drop();
            MIGRATOR
                .run_direct(&mut *connection)
                .await
                .map_err(|error| {
                    let kind = match error {
                        MigrateError::VersionMismatch(_) => "checksum_mismatch",
                        MigrateError::VersionMissing(_) => "missing_migration",
                        _ => "migration_failed",
                    };
                    tracing::error!(kind, "database migration rejected");
                    DatabaseError::Migration
                })?;
        }
        verify_schema(pool).await?;
        let runtime = self.runtime.as_ref().ok_or(DatabaseError::NotConfigured)?;
        let restricted: bool = sqlx::query_scalar("SELECT current_user = 'sozbricks_runtime' AND NOT rolsuper AND NOT rolbypassrls FROM pg_roles WHERE rolname = current_user")
            .fetch_one(runtime).await.map_err(|_| DatabaseError::Unavailable)?;
        if !restricted { return Err(DatabaseError::Schema); }
        self.initialized.store(true, Ordering::Release);
        Ok(())
    }

    pub async fn readiness(&self) -> Result<(), DatabaseError> {
        let pool = self.pool.as_ref().ok_or(DatabaseError::NotConfigured)?;
        tokio::time::timeout(self.health_timeout, async {
            sqlx::query_scalar::<_, i32>("SELECT 1")
                .fetch_one(pool)
                .await
                .map_err(|_| DatabaseError::Unavailable)?;
            if !self.initialized.load(Ordering::Acquire) {
                return Err(DatabaseError::Schema);
            }
            let runtime = self.runtime.as_ref().ok_or(DatabaseError::NotConfigured)?;
            sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(runtime).await.map_err(|_|DatabaseError::Unavailable)?;
            verify_schema(pool).await
        })
        .await
        .map_err(|_| DatabaseError::Unavailable)?
    }

    pub async fn close(&self) {
        if let Some(pool) = &self.runtime { pool.close().await; }
        if let Some(pool) = &self.pool {
            pool.close().await;
        }
    }
}

async fn verify_schema(pool: &PgPool) -> Result<(), DatabaseError> {
    let records = sqlx::query("SELECT version, checksum, success FROM public._sqlx_migrations")
        .fetch_all(pool)
        .await
        .map_err(|_| DatabaseError::Schema)?;
    if records.len() != MIGRATOR.iter().count() {
        return Err(DatabaseError::Schema);
    }
    for migration in MIGRATOR.iter() {
        let valid = records.iter().any(|row| {
            row.get::<i64, _>("version") == migration.version
                && row.get::<bool, _>("success")
                && row.get::<Vec<u8>, _>("checksum").as_slice() == migration.checksum.as_ref()
        });
        if !valid {
            return Err(DatabaseError::Schema);
        }
    }
    let exists: bool = sqlx::query_scalar("SELECT to_regclass('sozbricks.profiles') IS NOT NULL AND to_regclass('sozbricks.levels') IS NOT NULL AND to_regclass('sozbricks.game_sessions') IS NOT NULL AND to_regclass('sozbricks.game_results') IS NOT NULL AND to_regclass('sozbricks.level_progress') IS NOT NULL AND to_regclass('sozbricks.reward_ledger') IS NOT NULL")
        .fetch_one(pool).await.map_err(|_| DatabaseError::Schema)?;
    let guest_tables: bool = sqlx::query_scalar("SELECT to_regclass('sozbricks.guest_imports') IS NOT NULL AND to_regclass('sozbricks.guest_progress') IS NOT NULL")
        .fetch_one(pool).await.map_err(|_| DatabaseError::Schema)?;
    if !exists || !guest_tables {
        return Err(DatabaseError::Schema);
    }
    Ok(())
}
