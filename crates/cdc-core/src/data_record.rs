use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Represents a single data change event from PeerDB CDC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataRecord {
    /// Unique identifier for this record
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,

    /// Timestamp when the event was processed
    #[serde(default = "Utc::now")]
    pub timestamp: DateTime<Utc>,

    /// The full record data as JSON string
    pub record: String,

    /// Metadata about the CDC event as JSON string
    pub metadata: String,

    /// Operation type (insert, update, delete)
    pub action: String,

    /// Changed fields as JSON string (for updates)
    #[serde(default)]
    pub changes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Operation {
    Insert,
    Update,
    Delete,
    Snapshot,
}

impl DataRecord {
    /// Create a new DataRecord from PeerDB CDC format
    pub fn new(record: String, metadata: String, action: String, changes: Option<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            record,
            metadata,
            action,
            changes,
        }
    }

    /// Parse the record JSON string into a HashMap
    pub fn parse_record(&self) -> Result<HashMap<String, serde_json::Value>, serde_json::Error> {
        serde_json::from_str(&self.record)
    }

    /// Parse the metadata JSON string into a HashMap
    pub fn parse_metadata(&self) -> Result<HashMap<String, serde_json::Value>, serde_json::Error> {
        serde_json::from_str(&self.metadata)
    }

    /// Parse the changes JSON string into a HashMap
    pub fn parse_changes(
        &self,
    ) -> Result<Option<HashMap<String, serde_json::Value>>, serde_json::Error> {
        match &self.changes {
            Some(changes_str) => {
                let parsed: HashMap<String, serde_json::Value> = serde_json::from_str(changes_str)?;
                Ok(Some(parsed))
            }
            None => Ok(None),
        }
    }

    /// Get the table name from metadata
    pub fn table_name(&self) -> Option<String> {
        self.parse_metadata()
            .ok()
            .and_then(|meta| meta.get("table_name")?.as_str().map(String::from))
    }

    /// Get the database name from metadata
    pub fn database_name(&self) -> Option<String> {
        self.parse_metadata()
            .ok()
            .and_then(|meta| meta.get("database_name")?.as_str().map(String::from))
    }

    /// Convert action string to Operation enum
    pub fn operation(&self) -> Operation {
        match self.action.to_lowercase().as_str() {
            "insert" => Operation::Insert,
            "update" => Operation::Update,
            "delete" => Operation::Delete,
            _ => Operation::Snapshot,
        }
    }
}
