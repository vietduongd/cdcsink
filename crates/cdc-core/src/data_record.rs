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

    /// The full record data (can be JSON object or string)
    pub record: serde_json::Value,

    /// Metadata about the CDC event (can be JSON object or string)
    pub metadata: serde_json::Value,

    /// Operation type (insert, update, delete, read)
    pub action: String,

    /// Changed fields (can be JSON object, string, or null)
    #[serde(default)]
    pub changes: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Operation {
    Insert,
    Update,
    Delete,
    Read,
    Snapshot,
}

impl DataRecord {
    /// Create a new DataRecord from PeerDB CDC format
    pub fn new(
        record: serde_json::Value,
        metadata: serde_json::Value,
        action: String,
        changes: Option<serde_json::Value>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            record,
            metadata,
            action,
            changes,
        }
    }

    /// Get record as a JSON string
    pub fn record_as_string(&self) -> String {
        serde_json::to_string(&self.record).unwrap_or_default()
    }

    /// Get metadata as a JSON string
    pub fn metadata_as_string(&self) -> String {
        serde_json::to_string(&self.metadata).unwrap_or_default()
    }

    /// Get changes as a JSON string
    pub fn changes_as_string(&self) -> Option<String> {
        self.changes
            .as_ref()
            .map(|c| serde_json::to_string(c).unwrap_or_default())
    }

    /// Parse the record JSON into a HashMap
    pub fn parse_record(&self) -> Result<HashMap<String, serde_json::Value>, serde_json::Error> {
        if self.record.is_object() {
            serde_json::from_value(self.record.clone())
        } else if self.record.is_string() {
            serde_json::from_str(self.record.as_str().unwrap_or("{}"))
        } else {
            Ok(HashMap::new())
        }
    }

    /// Parse the metadata JSON into a HashMap
    pub fn parse_metadata(&self) -> Result<HashMap<String, serde_json::Value>, serde_json::Error> {
        if self.metadata.is_object() {
            serde_json::from_value(self.metadata.clone())
        } else if self.metadata.is_string() {
            serde_json::from_str(self.metadata.as_str().unwrap_or("{}"))
        } else {
            Ok(HashMap::new())
        }
    }

    /// Parse the changes JSON into a HashMap
    pub fn parse_changes(
        &self,
    ) -> Result<Option<HashMap<String, serde_json::Value>>, serde_json::Error> {
        match &self.changes {
            Some(changes_val) => {
                if changes_val.is_null() {
                    Ok(None)
                } else if changes_val.is_object() {
                    let parsed: HashMap<String, serde_json::Value> =
                        serde_json::from_value(changes_val.clone())?;
                    Ok(Some(parsed))
                } else if changes_val.is_string() {
                    let parsed: HashMap<String, serde_json::Value> =
                        serde_json::from_str(changes_val.as_str().unwrap_or("{}"))?;
                    Ok(Some(parsed))
                } else {
                    Ok(None)
                }
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
            "read" => Operation::Read,
            _ => Operation::Snapshot,
        }
    }
}
