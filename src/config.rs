use anyhow::{anyhow, Result};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: Option<String>,
    pub max_connections: u32,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let database_url = std::env::var("DATABASE_URL").ok();
        let max_connections = std::env::var("IMPORTER_DB_MAX_CONNECTIONS")
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(3);

        Self {
            database_url,
            max_connections,
        }
    }

    pub fn database_url_required(&self) -> Result<&str> {
        self.database_url
            .as_deref()
            .ok_or_else(|| anyhow!("DATABASE_URL is required for this command"))
    }
}
