use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use std::collections::HashMap;

/// Represents a single data change event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataRecord {
    /// Unique identifier for this record
    pub id: Uuid,
    
    /// Timestamp when the event occurred
    pub timestamp: DateTime<Utc>,
    
    /// Source system identifier
    pub source: String,
    
    /// Table/collection name
    pub table: String,
    
    /// Operation type
    pub operation: Operation,
    
    /// The actual data payload
    pub data: HashMap<String, serde_json::Value>,
    
    /// Optional metadata
    pub metadata: HashMap<String, String>,
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
    pub fn new(
        source: impl Into<String>,
        table: impl Into<String>,
        operation: Operation,
        data: HashMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            source: source.into(),
            table: table.into(),
            operation,
            data,
            metadata: HashMap::new(),
        }
    }
    
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}
