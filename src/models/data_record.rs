use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, result};

use crate::models::DataModel;

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
    pub fn get_table_name(&self) -> Option<String> {
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
    pub fn get_table_structure(&self) -> Option<HashMap<String, DataModel>> {
        let after_schema = self
            .schema
            .fields
            .iter()
            .find(|x| x.field == Some("after".to_string()));
        if after_schema.is_none() {
            return None;
        }
        let mut structure: HashMap<String, DataModel> = HashMap::new();
        let raw_data = self.get_table_data();
        for item in after_schema.unwrap().fields.as_ref().unwrap() {
            let field_name = item.field.as_ref().unwrap().to_string();

            let field_type = if item.name.is_some() {
                item.name.clone().unwrap()
            } else {
                item.field_type.clone()
            };
            let value = raw_data.get(&field_name);
            if value.is_none() {
                continue;
            }
            let data_type = DataRecord::look_up_data_type(&field_type, value.unwrap()).unwrap_or((
                "TEXT".to_string(),
                "TEXT".to_string(),
                Value::Null,
            ));

            structure.insert(
                field_name,
                DataModel {
                    value: data_type.2,
                    data_type: data_type.0,
                    nullable: item.optional,
                    simple_type: data_type.1,
                },
            );
        }
        Some(structure)
    }

    fn get_table_data(&self) -> HashMap<String, Value> {
        match self.payload.op.as_str() {
            "c" | "r" | "u" => {
                if let Some(after) = &self.payload.after {
                    after.clone()
                } else {
                    HashMap::new()
                }
            }
            "d" => {
                if let Some(before) = &self.payload.before {
                    before.clone()
                } else {
                    HashMap::new()
                }
            }
            _ => HashMap::new(),
        }
    }

    fn look_up_data_type(data_type: &str, value: &Value) -> Option<(String, String, Value)> {
        match data_type {
            "int8" => Some(("BIGINT".to_string(), "BIGINT".to_string(), value.clone())),
            "int16" => Some((
                "SMALLINT".to_string(),
                "SMALLINT".to_string(),
                value.clone(),
            )),
            "int32" => Some(("INTEGER".to_string(), "INTEGER".to_string(), value.clone())),
            "int64" => Some(("BIGINT".to_string(), "BIGINT".to_string(), value.clone())),
            "float" => Some(("BIGINT".to_string(), "BIGINT".to_string(), value.clone())),
            "float32" => Some(("REAL".to_string(), "REAL".to_string(), value.clone())),
            "float64" => Some((
                "DOUBLE PRECISION".to_string(),
                "DOUBLE PRECISION".to_string(),
                value.clone(),
            )),
            "boolean" => Some(("BOOLEAN".to_string(), "BOOLEAN".to_string(), value.clone())),
            "string" => Some(("TEXT".to_string(), "TEXT".to_string(), value.clone())),
            "bytes" => Some(("BYTEA".to_string(), "BYTEA".to_string(), value.clone())),
            "date" => Some(("DATE".to_string(), "DATE".to_string(), value.clone())),
            "time" => Some(("TIME".to_string(), "TIME".to_string(), value.clone())),
            "timestamp" => Some((
                "TIMESTAMP".to_string(),
                "TIMESTAMP".to_string(),
                value.clone(),
            )),
            "decimal" => Some(("NUMERIC".to_string(), "NUMERIC".to_string(), value.clone())),
            "io.debezium.data.VariableScaleDecimal" => {
                Some(("NUMERIC".to_string(), "NUMERIC".to_string(), value.clone()))
            }
            "io.debezium.time.Date" => {
                let result = match value {
                    Value::Null => value.clone(),
                    Value::Number(n) => {
                        if let Some(days) = n.as_i64() {
                            let epoch = chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
                            let date = epoch + chrono::Duration::days(days);
                            Value::String(date.format("%Y-%m-%d").to_string())
                        } else {
                            value.clone()
                        }
                    }
                    _ => value.clone(),
                };
                Some(("DATE".to_string(), "DATE".to_string(), result))
            }
            "io.debezium.time.Time" => {
                Some(("TIME".to_string(), "TIME".to_string(), value.clone()))
            }
            "io.debezium.time.Timestamp"
            | "io.debezium.time.MicroTimestamp"
            | "io.debezium.time.NanoTimestamp"
            | "io.debezium.time.ZonedTimestamp" => {
                let result = match value {
                    Value::Null => value.clone(),
                    Value::Number(n) => {
                        if let Some(timestamp) = n.as_i64() {
                            let datetime = match data_type {
                                "io.debezium.time.Timestamp" | "io.debezium.time.ZonedTimestamp" => {
                                    // milliseconds since epoch
                                    chrono::DateTime::from_timestamp_millis(timestamp)
                                }
                                "io.debezium.time.MicroTimestamp" => {
                                    // microseconds since epoch
                                    chrono::DateTime::from_timestamp_micros(timestamp)
                                }
                                "io.debezium.time.NanoTimestamp" => {
                                    // nanoseconds since epoch
                                    let secs = timestamp / 1_000_000_000;
                                    let nsecs = (timestamp % 1_000_000_000) as u32;
                                    chrono::DateTime::from_timestamp(secs, nsecs)
                                }
                                _ => None,
                            };
                            if let Some(dt) = datetime {
                                Value::String(dt.format("%Y-%m-%d %H:%M:%S").to_string())
                            } else {
                                value.clone()
                            }
                        } else {
                            value.clone()
                        }
                    }
                    _ => value.clone(),
                };
                if data_type == "io.debezium.time.ZonedTimestamp" {
                    Some((
                        "TIMESTAMP WITH TIME ZONE".to_string(),
                        "TIMESTAMPTZ".to_string(),
                        result,
                    ))
                } else {
                    Some(("TIMESTAMP".to_string(), "TIMESTAMP".to_string(), result))
                }
            }
            "io.debezium.data.Json" | "io.debezium.data.Jsonb" => {
                Some(("JSONB".to_string(), "JSONB".to_string(), value.clone()))
            }
            "io.debezium.data.Uuid" => {
                Some(("UUID".to_string(), "UUID".to_string(), value.clone()))
            }
            "io.debezium.data.Enum" => {
                Some(("TEXT".to_string(), "TEXT".to_string(), value.clone()))
            }
            _ => None,
        }
    }
}
