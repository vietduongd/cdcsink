use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Connector configuration entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorConfigEntry {
    /// Unique name for this connector config
    pub name: String,

    /// Type of connector (e.g., "nats", "kafka")
    pub connector_type: String,

    /// Connector-specific configuration
    pub config: Value,

    /// Optional description
    pub description: Option<String>,

    /// Tags for organization
    #[serde(default)]
    pub tags: Vec<String>,

    /// When this config was created
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,

    /// When this config was last updated
    #[serde(default = "Utc::now")]
    pub updated_at: DateTime<Utc>,
}

impl ConnectorConfigEntry {
    pub fn new(name: String, connector_type: String, config: Value) -> Self {
        let now = Utc::now();
        Self {
            name,
            connector_type,
            config,
            description: None,
            tags: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }
}

/// Destination configuration entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DestinationConfigEntry {
    /// Unique name for this destination config
    pub name: String,

    /// Type of destination (e.g., "postgres", "mysql")
    pub destination_type: String,

    /// Destination-specific configuration
    pub config: Value,

    /// Optional description
    pub description: Option<String>,

    /// Tags for organization
    #[serde(default)]
    pub tags: Vec<String>,

    /// Schemas to include for CDC operations
    #[serde(default)]
    pub schemas_includes: Option<Vec<String>>,

    /// When this config was created
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,

    /// When this config was last updated
    #[serde(default = "Utc::now")]
    pub updated_at: DateTime<Utc>,
}

impl DestinationConfigEntry {
    pub fn new(name: String, destination_type: String, config: Value) -> Self {
        let now = Utc::now();
        Self {
            name,
            destination_type,
            config,
            description: None,
            tags: Vec::new(),
            schemas_includes: None,
            created_at: now,
            updated_at: now,
        }
    }
}

/// Flow configuration entry (references connector and destinations)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowConfigEntry {
    /// Unique name for this flow
    pub name: String,

    /// Name of the connector config to use
    pub connector_name: String,

    /// Names of destination configs to use
    pub destination_names: Vec<String>,

    /// Batch size for this flow
    pub batch_size: usize,

    /// Auto-start this flow on system startup
    #[serde(default = "default_auto_start")]
    pub auto_start: bool,

    /// Optional description
    pub description: Option<String>,

    /// When this config was created
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,

    /// When this config was last updated
    #[serde(default = "Utc::now")]
    pub updated_at: DateTime<Utc>,
}

fn default_auto_start() -> bool {
    true
}

impl FlowConfigEntry {
    pub fn new(
        name: String,
        connector_name: String,
        destination_names: Vec<String>,
        batch_size: usize,
    ) -> Self {
        let now = Utc::now();
        Self {
            name,
            connector_name,
            destination_names,
            batch_size,
            auto_start: true,
            description: None,
            created_at: now,
            updated_at: now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_deserialization_defaults() {
        let json_data = json!({
            "name": "test-connector",
            "connector_type": "nats",
            "config": {"servers": ["nats://localhost:4222"]}
        });

        let entry: ConnectorConfigEntry =
            serde_json::from_value(json_data).expect("Failed to deserialize");
        assert_eq!(entry.name, "test-connector");
        // These should be populated by Utc::now() during deserialization
        assert!(entry.created_at.timestamp() > 0);
        assert!(entry.updated_at.timestamp() > 0);
    }
}
