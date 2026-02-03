use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Operation {
    Insert,
    Update,
    Delete,
    Read,
    Snapshot,
}

/// ===== Root =====
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataRecord {
    pub schema: Schema,
    pub payload: Payload,
}

/// ===== Schema (ít khi dùng, nhưng vẫn map đầy đủ) =====
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    #[serde(rename = "type")]
    pub schema_type: String,
    pub fields: Vec<SchemaField>,
    pub optional: bool,
    pub name: String,
    pub version: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaField {
    #[serde(rename = "type")]
    pub field_type: String,
    pub optional: bool,

    #[serde(default)]
    pub field: Option<String>,

    #[serde(default)]
    pub name: Option<String>,

    #[serde(default)]
    pub version: Option<i32>,

    #[serde(default)]
    pub fields: Option<Vec<SchemaField>>,

    #[serde(default)]
    pub default: Option<Value>,

    #[serde(default)]
    pub parameters: Option<Value>,
}

/// ===== Payload =====
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Payload {
    pub before: Option<HashMap<String, Value>>,
    pub after: Option<HashMap<String, Value>>,
    pub source: Source,
    pub transaction: Option<Transaction>,
    pub op: String,

    #[serde(rename = "ts_ms")]
    pub ts_ms: Option<i64>,
    #[serde(rename = "ts_us")]
    pub ts_us: Option<i64>,
    #[serde(rename = "ts_ns")]
    pub ts_ns: Option<i64>,
}

/// ===== Source =====
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub version: String,
    pub connector: String,
    pub name: String,

    #[serde(rename = "ts_ms")]
    pub ts_ms: i64,

    pub snapshot: String,
    pub db: String,

    pub sequence: Option<String>,

    #[serde(rename = "ts_us")]
    pub ts_us: Option<i64>,
    #[serde(rename = "ts_ns")]
    pub ts_ns: Option<i64>,

    pub schema: String,
    pub table: String,

    #[serde(rename = "txId")]
    pub tx_id: Option<i64>,
    pub lsn: Option<i64>,
    pub xmin: Option<i64>,
}

/// ===== Transaction =====
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String,

    #[serde(rename = "total_order")]
    pub total_order: i64,

    #[serde(rename = "data_collection_order")]
    pub data_collection_order: i64,
}

impl DataRecord {
    pub fn parse_record(&self) -> Result<HashMap<String, Value>, String> {
        match self.payload.op.as_str() {
            "c" | "r" | "u" => {
                if let Some(after) = &self.payload.after {
                    Ok(after.clone())
                } else {
                    Err("No 'after' data for create/read/update operation".to_string())
                }
            }
            "d" => {
                if let Some(before) = &self.payload.before {
                    Ok(before.clone())
                } else {
                    Err("No 'before' data for delete operation".to_string())
                }
            }
            _ => Err(format!("Unknown operation type: {}", self.payload.op)),
        }
    }

    /// Get the table name from source metadata
    pub fn table_name(&self) -> Option<String> {
        Some(self.payload.source.table.clone())
    }

    /// Get the database name from source metadata
    pub fn database_name(&self) -> Option<String> {
        Some(self.payload.source.db.clone())
    }

    /// Get the schema name from source metadata
    pub fn schema_name(&self) -> Option<String> {
        Some(self.payload.source.schema.clone())
    }

    pub fn operation(&self) -> Operation {
        match self.payload.op.as_str() {
            "c" => Operation::Insert,
            "r" => Operation::Read,
            "u" => Operation::Update,
            "d" => Operation::Delete,
            "snapshot" => Operation::Snapshot,
            _ => Operation::Snapshot, // Default fallback or handle unknown operation
        }
    }

    // Get table structure as a HashMap of field names to (data_type, is_nullable)
    pub fn get_table_structure(&self) -> Option<HashMap<String, (String, bool)>> {
        let after_schema = self
            .schema
            .fields
            .iter()
            .find(|x| x.field == Some("after".to_string()));
        if after_schema.is_none() {
            return None;
        }
        let mut structure: HashMap<String, (String, bool)> = HashMap::new();
        for item  in after_schema.unwrap().fields.as_ref().unwrap() {
            let field_name = item.field.as_ref().unwrap().to_string();
            let data_type = DataRecord::look_up_data_type(&item.field_type).unwrap_or("TEXT".to_string());
            let is_nullable = item.optional;
            structure.insert(field_name, (data_type, is_nullable));
        }
        Some(structure)
    }

    // pub fn get_postgres_data_type(&self, field_name: &str) -> Option<String> {
    //     let structure = self.get_table_structure();
    //     structure
    //         .get(field_name)
    //         .map(|(data_type, _)| data_type.clone())
    // }

    fn look_up_data_type(data_type: &str) -> Option<String> {
        match data_type {
            "int8" => Some("BIGINT".to_string()),
            "int16" => Some("SMALLINT".to_string()),
            "int32" => Some("INTEGER".to_string()),
            "int64" => Some("BIGINT".to_string()),
            "float" => Some("BIGINT".to_string()),
            "float32" => Some("REAL".to_string()),
            "float64" => Some("DOUBLE PRECISION".to_string()),
            "boolean" => Some("BOOLEAN".to_string()),
            "string" => Some("TEXT".to_string()),
            "bytes" => Some("BYTEA".to_string()),
            "date" => Some("DATE".to_string()),
            "time" => Some("TIME".to_string()),
            "timestamp" => Some("TIMESTAMP".to_string()),
            "decimal" => Some("NUMERIC".to_string()),
            "io.debezium.data.VariableScaleDecimal" => Some("NUMERIC".to_string()),
            "io.debezium.time.Date" => Some("DATE".to_string()),
            "io.debezium.time.Time" => Some("TIME".to_string()),
            "io.debezium.time.Timestamp" => Some("TIMESTAMP".to_string()),
            "io.debezium.time.MicroTimestamp" => Some("TIMESTAMP".to_string()),
            "io.debezium.time.NanoTimestamp" => Some("TIMESTAMP".to_string()),
            "io.debezium.time.ZonedTimestamp" => Some("TIMESTAMPTZ".to_string()),
            "io.debezium.data.Json" => Some("JSONB".to_string()),
            "io.debezium.data.Jsonb" => Some("JSONB".to_string()),
            "io.debezium.data.Uuid" => Some("UUID".to_string()),
            "io.debezium.data.Enum" => Some("TEXT".to_string()),
            _ => None,
        }
    }
}
