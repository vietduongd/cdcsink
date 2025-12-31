use serde::{Deserialize, Serialize};
use std::path::Path;

// Re-export flow config types from cdc-core
pub use cdc_core::{ConnectorConfig, DestinationConfig, FlowConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub flows: Vec<FlowConfig>,
    pub api: ApiConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// API server host
    pub host: String,

    /// API server port
    pub port: u16,

    /// Enable CORS
    pub cors_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error)
    pub level: String,

    /// JSON formatted logs
    pub json: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            flows: vec![FlowConfig::default()],
            api: ApiConfig {
                host: "localhost".to_string(),
                port: 3000,
                cors_enabled: true,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                json: false,
            },
        }
    }
}

impl AppConfig {
    pub fn load(config_dir: impl AsRef<Path>) -> anyhow::Result<Self> {
        let config_dir = config_dir.as_ref();
        let s = config::Config::builder()
            // Start with defaults
            .add_source(config::Config::try_from(&Self::default())?)
            // Add default.yaml
            .add_source(
                config::File::with_name(&config_dir.join("default.yaml").to_string_lossy())
                    .required(false),
            )
            // Add docker.yaml (often used for overrides in containers)
            .add_source(
                config::File::with_name(&config_dir.join("docker.yaml").to_string_lossy())
                    .required(false),
            )
            // Add environment variables (CDC_API__PORT=4000)
            .add_source(config::Environment::with_prefix("CDC").separator("__"))
            .build()?;

        let config = s.try_deserialize()?;
        Ok(config)
    }

    pub fn from_file(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    pub fn to_file(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let content = serde_yaml::to_string(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}
