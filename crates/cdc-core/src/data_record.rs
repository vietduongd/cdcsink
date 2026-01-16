use std::collections::HashMap;

use base64::{prelude::BASE64_STANDARD, Engine};
use chrono::{DateTime, Utc};
use etl::types::{Cell, PgNumeric};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::Result;

/// CDC Operation types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Operation {
    Insert,
    Update,
    Delete,
    Read,
    Snapshot,
}

impl Operation {
    /// Parse operation from string (case-insensitive)
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "insert" => Operation::Insert,
            "update" => Operation::Update,
            "delete" => Operation::Delete,
            "read" => Operation::Read,
            "snapshot" => Operation::Snapshot,
            _ => Operation::Read, // Default fallback
        }
    }
}

impl std::fmt::Display for Operation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Operation::Insert => write!(f, "insert"),
            Operation::Update => write!(f, "update"),
            Operation::Delete => write!(f, "delete"),
            Operation::Read => write!(f, "read"),
            Operation::Snapshot => write!(f, "snapshot"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnSchemaSer {
    pub name: String,
    pub typ: String,
    pub modifier: i32,
    pub nullable: bool,
    pub primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TableMetadata {
    pub schema: String,
    pub name: String,
    pub column_schemas: Vec<ColumnSchemaSer>,
}

impl TableMetadata {
    pub fn init() -> Self {
        Self::default()
    }

    pub fn add_column_schema(&mut self, column_schema: ColumnSchemaSer) {
        self.column_schemas.push(column_schema);
    }

    pub fn update_table_name(&mut self, schema: String, name: String) {
        self.schema = schema;
        self.name = name;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataRecord {
    /// Unique identifier for this record
    id: Uuid,

    /// Timestamp when the event occurred
    timestamp: DateTime<Utc>,

    /// The actual data payload
    pub data: HashMap<String, serde_json::Value>,

    /// Table metadata (schema structure)
    pub table_metadata: TableMetadata,

    /// CDC operation type
    pub operation: String,
}

impl DataRecord {
    pub fn new() -> Self {
        Self {
            id: Uuid::now_v7(),
            timestamp: Utc::now(),
            data: HashMap::new(),
            table_metadata: TableMetadata::init(),
            operation: String::new(),
        }
    }

    pub fn init(table_metadata: TableMetadata, operation: String) -> Self {
        Self {
            id: Uuid::now_v7(),
            timestamp: Utc::now(),
            data: HashMap::new(),
            table_metadata: table_metadata,
            operation,
        }
    }

    pub fn add_column(&mut self, column_name: String, column_value: serde_json::Value) {
        self.data.insert(column_name, column_value);
    }

    pub fn get_info(&mut self) -> &mut Self {
        self
    }

    /// Get table name from metadata
    pub fn table_name(&self) -> Option<String> {
        if self.table_metadata.name.is_empty() {
            None
        } else {
            Some(self.table_metadata.name.clone())
        }
    }

    /// Get operation type
    pub fn operation(&self) -> Operation {
        Operation::from_str(&self.operation)
    }

    /// Parse record data (returns the data HashMap)
    pub fn parse_record(&self) -> Result<HashMap<String, serde_json::Value>> {
        Ok(self.data.clone())
    }

    /// Parse changes (for update operations, returns None as we don't track changes separately)
    pub fn parse_changes(&self) -> Result<Option<HashMap<String, serde_json::Value>>> {
        // We don't have a separate changes field, so return None
        // The data field contains the full record
        Ok(None)
    }

    pub fn value_cell_to_json(source: &Cell) -> Value {
        match source {
            Cell::Null => Value::Null,
            Cell::Bool(v) => Value::Bool(*v),
            Cell::String(v) => Value::String(v.clone()),
            Cell::I16(v) => json!(*v),
            Cell::I32(v) => json!(*v),
            Cell::U32(v) => json!(*v),
            Cell::I64(v) => json!(*v),
            Cell::F32(v) => json!(*v),
            Cell::F64(v) => json!(*v),

            Cell::Numeric(num) => match num {
                PgNumeric::NaN => Value::Null,
                _ => json!(num.to_string()),
            },

            Cell::Date(v) => json!(v.to_string()), // "2026-01-14"
            Cell::Time(v) => json!(v.to_string()), // "12:34:56"
            Cell::Timestamp(v) => json!(v.to_string()), // "2026-01-14T12:34:56"
            Cell::TimestampTz(v) => json!(v.to_rfc3339()), // "2026-01-14T12:34:56Z"
            Cell::Uuid(v) => json!(v.to_string()),
            Cell::Json(v) => v.clone(),
            Cell::Bytes(v) => {
                // Encode base64
                json!(BASE64_STANDARD.encode(v.clone()))
            }

            Cell::Array(_arr) => json!("demo".to_string()),
        }
    }
}
