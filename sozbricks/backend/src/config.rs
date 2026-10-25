use std::{collections::HashMap, env, net::{IpAddr, SocketAddr}, time::Duration};

use axum::http::{HeaderValue, Uri};
use thiserror::Error;

#[derive(Clone)]
pub struct Config {
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
        for variable in ["HOST", "PORT", "CORS_ALLOWED_ORIGINS", "REQUEST_TIMEOUT_SECONDS", "MAX_REQUEST_BODY_BYTES", "APP_REVISION", "RENDER_GIT_COMMIT"] {
            match env::var(variable) {
                Ok(value) => { values.insert(variable.to_owned(), value); }
                Err(env::VarError::NotPresent) => {}
                Err(env::VarError::NotUnicode(_)) => return Err(invalid(variable, "must contain valid Unicode")),
            }
        }
        Self::from_values(&values)
    }

    pub fn from_values(values: &HashMap<String, String>) -> Result<Self, ConfigError> {
        let value = |key: &str| values.get(key).map(|v| v.trim());
        let host: IpAddr = value("HOST").unwrap_or("0.0.0.0").parse()
            .map_err(|_| invalid("HOST", "must be an IP address"))?;
        let port = number(value("PORT"), "PORT", 8080, 1, 65535)? as u16;
        let seconds = number(value("REQUEST_TIMEOUT_SECONDS"), "REQUEST_TIMEOUT_SECONDS", 30, 1, 120)?;
        let max_request_body_bytes = number(value("MAX_REQUEST_BODY_BYTES"), "MAX_REQUEST_BODY_BYTES", 65536, 1, 1048576)?;
        let mut cors_origins = Vec::new();
        for origin in value("CORS_ALLOWED_ORIGINS").unwrap_or("").split(',').map(str::trim).filter(|origin| !origin.is_empty()) {
            let uri: Uri = origin.parse().map_err(|_| invalid("CORS_ALLOWED_ORIGINS", "must contain HTTP(S) origins"))?;
            if !matches!(uri.scheme_str(), Some("http" | "https"))
                || uri.authority().is_none_or(|a| a.as_str().contains('@') || a.as_str().contains('*'))
                || uri.path() != "/" || uri.query().is_some() {
                return Err(invalid("CORS_ALLOWED_ORIGINS", "must contain origins without paths, credentials, queries or wildcards"));
            }
            cors_origins.push(HeaderValue::from_str(origin.trim_end_matches('/'))
                .map_err(|_| invalid("CORS_ALLOWED_ORIGINS", "must contain valid header values"))?);
        }
        let revision = value("APP_REVISION").or_else(|| value("RENDER_GIT_COMMIT")).unwrap_or(option_env!("GIT_SHA").unwrap_or("unknown"));
        if revision.is_empty() || revision.len() > 128 || !revision.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)) {
            return Err(invalid("APP_REVISION", "must be 1-128 ASCII letters, digits, dots, underscores or hyphens"));
        }
        Ok(Self { address: SocketAddr::new(host, port), cors_origins,
            request_timeout: Duration::from_secs(seconds as u64), max_request_body_bytes, revision: revision.to_owned() })
    }
}

fn number(value: Option<&str>, variable: &'static str, default: usize, minimum: usize, maximum: usize) -> Result<usize, ConfigError> {
    let parsed = match value {
        Some(value) => value.parse().map_err(|_| invalid(variable, "must be an integer in the documented range"))?,
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
