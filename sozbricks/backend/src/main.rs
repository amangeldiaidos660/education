use std::{error::Error, process::ExitCode};

use sozbricks_api::{app_with_database, config::Config, database::Database};
use std::time::Duration;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("sozbricks-api failed: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn Error>> {
    let filter = match std::env::var("RUST_LOG") {
        Ok(value) => EnvFilter::try_new(value)?,
        Err(std::env::VarError::NotPresent) => EnvFilter::new("sozbricks_api=info"),
        Err(error) => return Err(error.into()),
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .json()
        .try_init()
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    let config = Config::from_env()?;
    let database = Database::new(config.database.as_ref());
    let listener = tokio::net::TcpListener::bind(config.address).await?;
    let worker_database = database.clone();
    let worker = tokio::spawn(async move {
        if !worker_database.configured() {
            tracing::warn!("database is not configured; readiness will return 503");
            return;
        }
        loop {
            match tokio::time::timeout(Duration::from_secs(60), worker_database.prepare()).await {
                Ok(Ok(())) => {
                    tracing::info!("database schema is ready");
                    break;
                }
                Ok(Err(error)) => {
                    tracing::error!(reason = %error, "database initialization failed; retrying")
                }
                Err(_) => tracing::error!("database initialization timed out; retrying"),
            }
            tokio::time::sleep(Duration::from_secs(15)).await;
        }
    });
    tracing::info!(address = %listener.local_addr()?, revision = %config.revision, "API started");
    let result = axum::serve(listener, app_with_database(config, database.clone()))
        .with_graceful_shutdown(shutdown_signal())
        .await;
    worker.abort();
    let _ = worker.await;
    let _ = tokio::time::timeout(Duration::from_secs(5), database.close()).await;
    result?;
    tracing::info!("API stopped");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("could not install Ctrl+C handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("could not install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! { () = ctrl_c => {}, () = terminate => {} }
    tracing::info!("shutdown requested");
}
