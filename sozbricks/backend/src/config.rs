use std::{
    collections::HashMap,
    env,
    net::{IpAddr, SocketAddr},
    time::Duration,
};

use axum::http::{HeaderValue, Uri};
use thiserror::Error;
use sqlx::{ConnectOptions, postgres::{PgConnectOptions, PgSslMode}};
use std::str::FromStr;

#[derive(Clone)]
pub struct DatabaseConfig {
    pub options: PgConnectOptions,
    pub max_connections: u32,
    pub acquire_timeout: Duration,
    pub health_timeout: Duration,
    pub run_migrations: bool,
}

#[derive(Clone)]
pub struct Config {
    pub database: Option<DatabaseConfig>,
    pub address: SocketAddr,
    pub cors_origins: Vec<HeaderValue>,
    pub request_timeout: Duration,
    pub max_request_body_bytes: usize,
    pub revision: String,
}

#[derive(Debug, Error)]
#[error("invalid configuration: {variable} {reason}")]
pub struct ConfigError {
    variable: &'static str,
    reason: &'static str,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let mut values = HashMap::new();
        for variable in [
            "HOST",
            "PORT",
            "CORS_ALLOWED_ORIGINS",
            "REQUEST_TIMEOUT_SECONDS",
            "MAX_REQUEST_BODY_BYTES",
            "APP_REVISION",
            "RENDER_GIT_COMMIT",
            "DATABASE_URL",
            "DATABASE_MAX_CONNECTIONS",
            "DATABASE_ACQUIRE_TIMEOUT_SECONDS",
            "DATABASE_HEALTH_TIMEOUT_SECONDS",
            "DATABASE_RUN_MIGRATIONS",
            "DATABASE_SSL_MODE",
        ] {
            match env::var(variable) {
                Ok(value) => {
                    values.insert(variable.to_owned(), value);
                }
                Err(env::VarError::NotPresent) => {}
                Err(env::VarError::NotUnicode(_)) => {
                    return Err(invalid(variable, "must contain valid Unicode"));
                }
            }
        }
        Self::from_values(&values)
    }

    pub fn from_values(values: &HashMap<String, String>) -> Result<Self, ConfigError> {
        let value = |key: &str| values.get(key).map(|v| v.trim());
        let database = match value("DATABASE_URL") {
            None => None,
            Some(url) => {
                if !(url.starts_with("postgresql://") || url.starts_with("postgres://"))
                    || url.contains("[YOUR-PASSWORD]") || url.contains("YOUR-PASSWORD") {
                    return Err(invalid("DATABASE_URL", "must be a PostgreSQL URI with actual credentials"));
                }
                let options = PgConnectOptions::from_str(url)
                    .map_err(|_| invalid("DATABASE_URL", "must be a valid PostgreSQL URI"))?;
                let ssl_mode = match value("DATABASE_SSL_MODE").unwrap_or("require") {
                    "require" => PgSslMode::Require,
                    "verify-full" => PgSslMode::VerifyFull,
                    "disable" if matches!(options.get_host(), "127.0.0.1" | "localhost" | "::1") => PgSslMode::Disable,
                    _ => return Err(invalid("DATABASE_SSL_MODE", "must be require or verify-full; disable is limited to loopback test databases")),
                };
                let run_migrations = match value("DATABASE_RUN_MIGRATIONS").unwrap_or("true") {
                    "true" => true,
                    "false" => false,
                    _ => return Err(invalid("DATABASE_RUN_MIGRATIONS", "must be true or false")),
                };
                Some(DatabaseConfig {
                    options: options.ssl_mode(ssl_mode).application_name("sozbricks-api")
                        .statement_cache_capacity(0).disable_statement_logging(),
                    max_connections: number(value("DATABASE_MAX_CONNECTIONS"), "DATABASE_MAX_CONNECTIONS", 3, 1, 10)? as u32,
                    acquire_timeout: Duration::from_secs(number(value("DATABASE_ACQUIRE_TIMEOUT_SECONDS"), "DATABASE_ACQUIRE_TIMEOUT_SECONDS", 5, 1, 30)? as u64),
                    health_timeout: Duration::from_secs(number(value("DATABASE_HEALTH_TIMEOUT_SECONDS"), "DATABASE_HEALTH_TIMEOUT_SECONDS", 3, 1, 5)? as u64),
                    run_migrations,
                })
            }
        };
        let host: IpAddr = value("HOST")
            .unwrap_or("0.0.0.0")
            .parse()
            .map_err(|_| invalid("HOST", "must be an IP address"))?;
        let port = number(value("PORT"), "PORT", 8080, 1, 65535)? as u16;
        let seconds = number(
            value("REQUEST_TIMEOUT_SECONDS"),
            "REQUEST_TIMEOUT_SECONDS",
            30,
            1,
            120,
        )?;
        let max_request_body_bytes = number(
            value("MAX_REQUEST_BODY_BYTES"),
            "MAX_REQUEST_BODY_BYTES",
            65536,
            1,
            1048576,
        )?;
        let mut cors_origins = Vec::new();
        for origin in value("CORS_ALLOWED_ORIGINS")
            .unwrap_or("")
            .split(',')
            .map(str::trim)
            .filter(|origin| !origin.is_empty())
        {
            let uri: Uri = origin
                .parse()
                .map_err(|_| invalid("CORS_ALLOWED_ORIGINS", "must contain HTTP(S) origins"))?;
            if !matches!(uri.scheme_str(), Some("http" | "https"))
                || uri
                    .authority()
                    .is_none_or(|a| a.as_str().contains('@') || a.as_str().contains('*'))
                || uri.path() != "/"
                || uri.query().is_some()
            {
                return Err(invalid(
                    "CORS_ALLOWED_ORIGINS",
                    "must contain origins without paths, credentials, queries or wildcards",
                ));
            }
            cors_origins.push(HeaderValue::from_str(origin.trim_end_matches('/')).map_err(
                |_| invalid("CORS_ALLOWED_ORIGINS", "must contain valid header values"),
            )?);
        }
        let revision = value("APP_REVISION")
            .or_else(|| value("RENDER_GIT_COMMIT"))
            .unwrap_or(option_env!("GIT_SHA").unwrap_or("unknown"));
        if revision.is_empty()
            || revision.len() > 128
            || !revision
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            return Err(invalid(
                "APP_REVISION",
                "must be 1-128 ASCII letters, digits, dots, underscores or hyphens",
            ));
        }
        Ok(Self {
            database,
            address: SocketAddr::new(host, port),
            cors_origins,
            request_timeout: Duration::from_secs(seconds as u64),
            max_request_body_bytes,
            revision: revision.to_owned(),
        })
    }
}

fn number(
    value: Option<&str>,
    variable: &'static str,
    default: usize,
    minimum: usize,
    maximum: usize,
) -> Result<usize, ConfigError> {
    let parsed = match value {
        Some(value) => value
            .parse()
            .map_err(|_| invalid(variable, "must be an integer in the documented range"))?,
        None => default,
    };
    if !(minimum..=maximum).contains(&parsed) {
        return Err(invalid(variable, "is outside the documented range"));
    }
    Ok(parsed)
}

fn invalid(variable: &'static str, reason: &'static str) -> ConfigError {
    ConfigError { variable, reason }
}
