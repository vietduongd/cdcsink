use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::models::DataModel;
use crate::models::decimal::decimal_text;

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

    /// Kiểu phần tử khi `type` là "array"
    #[serde(default)]
    pub items: Option<Box<SchemaField>>,
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
            .find(|x| x.field == Some("after".to_string()))?;
        let mut structure: HashMap<String, DataModel> = HashMap::new();
        let raw_data = self.get_table_data();
        for item in after_schema.fields.as_ref()? {
            let field_name = match item.field.as_ref() {
                Some(name) => name.to_string(),
                None => continue,
            };
            let value = match raw_data.get(&field_name) {
                Some(value) => value,
                None => continue,
            };
            let (data_type, simple_type, value) = DataRecord::convert_field(item, value);
            structure.insert(
                field_name,
                DataModel {
                    value,
                    data_type,
                    nullable: item.optional,
                    simple_type,
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

    /// (kiểu cột Postgres, kiểu rút gọn, giá trị đã chuẩn hóa) của một field.
    /// Kiểu không biết: giữ nguyên giá trị trong cột TEXT thay vì bỏ thành NULL.
    fn convert_field(item: &SchemaField, value: &Value) -> (String, String, Value) {
        let type_name = item.name.as_deref().unwrap_or(&item.field_type);
        if type_name == "org.apache.kafka.connect.data.Decimal" {
            return DataRecord::kafka_decimal(value, item.parameters.as_ref());
        }
        if item.field_type == "array" {
            return DataRecord::array_type(item, value);
        }
        DataRecord::look_up_data_type(type_name, value)
            .unwrap_or_else(|| ("TEXT".to_string(), "TEXT".to_string(), value.clone()))
    }

    /// numeric(p,s) của Debezium: value là base64, scale nằm trong `parameters.scale`.
    /// Chuyển về dạng `{scale, value}` giống VariableScaleDecimal để insert và bộ lọc dùng chung.
    fn kafka_decimal(value: &Value, parameters: Option<&Value>) -> (String, String, Value) {
        let scale = parameters
            .and_then(|p| p.get("scale"))
            .and_then(|s| match s {
                Value::String(s) => s.parse::<i64>().ok(),
                other => other.as_i64(),
            })
            .unwrap_or(0);
        let result = match value {
            Value::String(encoded) => serde_json::json!({ "scale": scale, "value": encoded }),
            _ => value.clone(),
        };
        ("NUMERIC".to_string(), "NUMERIC".to_string(), result)
    }

    /// Mảng Postgres: kiểu cột `<kiểu phần tử>[]`, từng phần tử được chuẩn hóa như cột thường.
    fn array_type(item: &SchemaField, value: &Value) -> (String, String, Value) {
        let (element_type, converted) = match item.items.as_deref() {
            Some(items) => {
                let (element_type, _, _) = DataRecord::convert_field(items, &Value::Null);
                let converted = match value {
                    Value::Array(elements) => Value::Array(
                        elements
                            .iter()
                            .map(|element| {
                                let (_, simple, v) = DataRecord::convert_field(items, element);
                                if simple == "NUMERIC" && !v.is_null() {
                                    decimal_text(&v).map(Value::String).unwrap_or(v)
                                } else {
                                    v
                                }
                            })
                            .collect(),
                    ),
                    other => other.clone(),
                };
                (element_type, converted)
            }
            None => ("TEXT".to_string(), value.clone()),
        };
        (
            format!("{}[]", element_type),
            "ARRAY".to_string(),
            converted,
        )
    }

    fn look_up_data_type(data_type: &str, value: &Value) -> Option<(String, String, Value)> {
        let simple =
            |pg_type: &str| Some((pg_type.to_string(), pg_type.to_string(), value.clone()));
        let converted = |pg_type: &str, simple_type: &str, v: Value| {
            Some((pg_type.to_string(), simple_type.to_string(), v))
        };
        match data_type {
            "int8" | "int16" => simple("SMALLINT"),
            "int32" | "int" => simple("INTEGER"),
            "int64" => simple("BIGINT"),
            // Kafka Connect JSON: "float" = float32, "double" = float64. NaN/Infinity đến dạng chuỗi.
            "float" | "float32" => simple("REAL"),
            "double" | "float64" => simple("DOUBLE PRECISION"),
            "boolean" => simple("BOOLEAN"),
            "string" => simple("TEXT"),
            "bytes" => simple("BYTEA"),
            "date" => simple("DATE"),
            "time" => simple("TIME"),
            "timestamp" => simple("TIMESTAMP"),
            "decimal" | "io.debezium.data.VariableScaleDecimal" => simple("NUMERIC"),
            "io.debezium.time.Date" => converted("DATE", "DATE", DataRecord::convert_date(value)),
            "io.debezium.time.Time" => converted(
                "TIME",
                "TIME",
                DataRecord::convert_time(value, TimeUnit::Milli),
            ),
            "io.debezium.time.MicroTime" => converted(
                "TIME",
                "TIME",
                DataRecord::convert_time(value, TimeUnit::Micro),
            ),
            "io.debezium.time.NanoTime" => converted(
                "TIME",
                "TIME",
                DataRecord::convert_time(value, TimeUnit::Nano),
            ),
            "io.debezium.time.ZonedTime" => {
                converted("TIME WITH TIME ZONE", "TIMETZ", value.clone())
            }
            "io.debezium.time.Timestamp" => converted(
                "TIMESTAMP",
                "TIMESTAMP",
                DataRecord::convert_timestamp(value, TimeUnit::Milli),
            ),
            "io.debezium.time.MicroTimestamp" => converted(
                "TIMESTAMP",
                "TIMESTAMP",
                DataRecord::convert_timestamp(value, TimeUnit::Micro),
            ),
            "io.debezium.time.NanoTimestamp" => converted(
                "TIMESTAMP",
                "TIMESTAMP",
                DataRecord::convert_timestamp(value, TimeUnit::Nano),
            ),
            // Chuỗi ISO-8601, kể cả "infinity" / "-infinity"
            "io.debezium.time.ZonedTimestamp" => {
                converted("TIMESTAMP WITH TIME ZONE", "TIMESTAMPTZ", value.clone())
            }
            // interval.handling.mode=numeric: số microsecond (Debezium tính tháng xấp xỉ 30 ngày)
            "io.debezium.time.MicroDuration" => {
                let result = match value.as_i64() {
                    Some(micros) => Value::String(format!("{} microseconds", micros)),
                    None => value.clone(),
                };
                converted("INTERVAL", "INTERVAL", result)
            }
            // interval.handling.mode=string: chuỗi ISO-8601 (P1Y2M3DT4H5M6.78S), Postgres đọc trực tiếp
            "io.debezium.time.Interval" => simple("INTERVAL"),
            "io.debezium.data.Json" | "io.debezium.data.Jsonb" => simple("JSONB"),
            "io.debezium.data.Uuid" => simple("UUID"),
            "io.debezium.data.Enum" | "io.debezium.data.EnumSet" => simple("TEXT"),
            "io.debezium.data.Xml" => simple("XML"),
            _ => None,
        }
    }

    /// Số ngày từ 1970-01-01 -> "YYYY-MM-DD".
    fn convert_date(value: &Value) -> Value {
        // Debezium không có giá trị riêng cho date 'infinity' và mã hóa khác nhau theo đường đọc:
        // - streaming (pgoutput): cộng 10957 ngày vào hằng số int32 của Postgres nên bị tràn
        const STREAM_POSITIVE_INFINITY: i64 = i32::MAX as i64 + 10957 - (1i64 << 32); // -2147472692
        const STREAM_NEGATIVE_INFINITY: i64 = i32::MIN as i64 + 10957; // -2147472691
        // - snapshot (JDBC): hằng số infinity của driver đổi qua LocalDate
        const SNAPSHOT_POSITIVE_INFINITY: i64 = -622191234;
        const SNAPSHOT_NEGATIVE_INFINITY: i64 = -625821272;
        let days = match value.as_i64() {
            Some(days) => days,
            None => return value.clone(),
        };
        match days {
            STREAM_POSITIVE_INFINITY | SNAPSHOT_POSITIVE_INFINITY => {
                return Value::String("infinity".to_string());
            }
            STREAM_NEGATIVE_INFINITY | SNAPSHOT_NEGATIVE_INFINITY => {
                return Value::String("-infinity".to_string());
            }
            _ => {}
        }
        let epoch = chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
        match chrono::TimeDelta::try_days(days).and_then(|delta| epoch.checked_add_signed(delta)) {
            Some(date) => Value::String(date.format("%Y-%m-%d").to_string()),
            None => {
                tracing::warn!(days, "Date out of range (days since epoch), stored as NULL");
                Value::Null
            }
        }
    }

    /// Thời gian trong ngày -> "HH:MM:SS.ffffff".
    fn convert_time(value: &Value, unit: TimeUnit) -> Value {
        let n = match value.as_i64() {
            Some(n) => n,
            None => return value.clone(),
        };
        let micros = match unit {
            TimeUnit::Milli => n * 1_000,
            TimeUnit::Micro => n,
            TimeUnit::Nano => n / 1_000,
        };
        let seconds = micros.div_euclid(1_000_000);
        Value::String(format!(
            "{:02}:{:02}:{:02}.{:06}",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60,
            micros.rem_euclid(1_000_000)
        ))
    }

    /// Thời điểm tính từ epoch -> "YYYY-MM-DD HH:MM:SS[.ffffff]", giữ nguyên phần lẻ giây.
    fn convert_timestamp(value: &Value, unit: TimeUnit) -> Value {
        let n = match value.as_i64() {
            Some(n) => n,
            None => return value.clone(),
        };
        let datetime = match unit {
            TimeUnit::Milli => chrono::DateTime::from_timestamp_millis(n),
            TimeUnit::Micro => chrono::DateTime::from_timestamp_micros(n),
            TimeUnit::Nano => chrono::DateTime::from_timestamp(
                n.div_euclid(1_000_000_000),
                n.rem_euclid(1_000_000_000) as u32,
            ),
        };
        match datetime {
            Some(dt) => Value::String(dt.format("%Y-%m-%d %H:%M:%S%.f").to_string()),
            // Debezium biểu diễn 'infinity' / '-infinity' bằng số ngoài phạm vi (±9223372036825200000)
            None if n > 0 => Value::String("infinity".to_string()),
            None => Value::String("-infinity".to_string()),
        }
    }
}

#[derive(Clone, Copy)]
enum TimeUnit {
    Milli,
    Micro,
    Nano,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn record_with_field(field_schema: Value, after: Value) -> DataRecord {
        serde_json::from_value(json!({
            "schema": {
                "type": "struct", "optional": false, "name": "t.Envelope", "version": 1,
                "fields": [
                    { "type": "struct", "optional": true, "field": "after", "fields": [
                        { "type": "int32", "optional": false, "field": "id" },
                        field_schema
                    ]}
                ]
            },
            "payload": {
                "before": null,
                "after": after,
                "source": {
                    "version": "2", "connector": "postgresql", "name": "s", "ts_ms": 0,
                    "snapshot": "false", "db": "db", "schema": "public", "table": "orders"
                },
                "transaction": null,
                "op": "c"
            }
        }))
        .expect("valid Debezium record")
    }

    // numeric(p,s) của Debezium: base64 + scale trong parameters, không được thành NULL
    #[test]
    fn kafka_connect_decimal_keeps_value_and_scale() {
        let record = record_with_field(
            json!({
                "type": "bytes", "optional": true, "field": "total",
                "name": "org.apache.kafka.connect.data.Decimal", "version": 1,
                "parameters": { "scale": "2", "connect.decimal.precision": "10" }
            }),
            json!({ "id": 1, "total": "AeI=" }),
        );
        let structure = record.get_table_structure().unwrap();
        let total = structure.get("total").unwrap();
        assert_eq!(total.simple_type, "NUMERIC");
        assert_eq!(total.value, json!({ "scale": 2, "value": "AeI=" }));
    }

    #[test]
    fn kafka_connect_decimal_null_stays_null() {
        let record = record_with_field(
            json!({
                "type": "bytes", "optional": true, "field": "total",
                "name": "org.apache.kafka.connect.data.Decimal", "version": 1,
                "parameters": { "scale": "2" }
            }),
            json!({ "id": 1, "total": null }),
        );
        let structure = record.get_table_structure().unwrap();
        assert_eq!(structure.get("total").unwrap().value, Value::Null);
        assert_eq!(structure.get("total").unwrap().simple_type, "NUMERIC");
    }

    fn converted(field_schema: Value, value: Value) -> DataModel {
        let record = record_with_field(field_schema, json!({ "id": 1, "v": value }));
        record.get_table_structure().unwrap().remove("v").unwrap()
    }

    // JsonConverter ghi float64 là "double": trước đây rơi vào TEXT và mất giá trị
    #[test]
    fn double_maps_to_double_precision() {
        let v = converted(
            json!({ "type": "double", "optional": true, "field": "v" }),
            json!(3.5),
        );
        assert_eq!(v.data_type, "DOUBLE PRECISION");
        assert_eq!(v.value, json!(3.5));
        let v = converted(
            json!({ "type": "double", "optional": true, "field": "v" }),
            json!("NaN"),
        );
        assert_eq!(v.value, json!("NaN"));
        let v = converted(
            json!({ "type": "float", "optional": true, "field": "v" }),
            json!(1.5),
        );
        assert_eq!(v.data_type, "REAL");
    }

    #[test]
    fn micro_timestamp_keeps_fraction() {
        let schema = json!({ "type": "int64", "optional": true, "field": "v", "name": "io.debezium.time.MicroTimestamp" });
        assert_eq!(
            converted(schema.clone(), json!(1904187905555555i64)).value,
            json!("2030-05-05 05:05:05.555555")
        );
        assert_eq!(
            converted(schema.clone(), json!(0)).value,
            json!("1970-01-01 00:00:00")
        );
        assert_eq!(
            converted(schema.clone(), json!(-876544i64)).value,
            json!("1969-12-31 23:59:59.123456")
        );
        assert_eq!(
            converted(schema.clone(), json!(9223372036825200000i64)).value,
            json!("infinity")
        );
        assert_eq!(
            converted(schema, json!(-9223372036832400000i64)).value,
            json!("-infinity")
        );
    }

    #[test]
    fn date_handles_infinity_and_range() {
        let schema = json!({ "type": "int32", "optional": true, "field": "v", "name": "io.debezium.time.Date" });
        assert_eq!(
            converted(schema.clone(), json!(19782)).value,
            json!("2024-02-29")
        );
        assert_eq!(
            converted(schema.clone(), json!(-719162)).value,
            json!("0001-01-01")
        );
        assert_eq!(
            converted(schema.clone(), json!(-2147472692i64)).value,
            json!("infinity")
        );
        assert_eq!(
            converted(schema.clone(), json!(-2147472691i64)).value,
            json!("-infinity")
        );
        assert_eq!(
            converted(schema.clone(), json!(-622191234)).value,
            json!("infinity")
        );
        assert_eq!(
            converted(schema.clone(), json!(-625821272)).value,
            json!("-infinity")
        );
        assert_eq!(converted(schema, json!(i64::MAX)).value, Value::Null);
    }

    #[test]
    fn time_types_are_converted() {
        let micro = json!({ "type": "int64", "optional": true, "field": "v", "name": "io.debezium.time.MicroTime" });
        let v = converted(micro, json!(49530500000i64));
        assert_eq!(
            (v.data_type.as_str(), v.value),
            ("TIME", json!("13:45:30.500000"))
        );
        let milli = json!({ "type": "int32", "optional": true, "field": "v", "name": "io.debezium.time.Time" });
        assert_eq!(
            converted(milli, json!(1000)).value,
            json!("00:00:01.000000")
        );
        let zoned = json!({ "type": "string", "optional": true, "field": "v", "name": "io.debezium.time.ZonedTime" });
        assert_eq!(
            converted(zoned, json!("06:45:30Z")).data_type,
            "TIME WITH TIME ZONE"
        );
        let duration = json!({ "type": "int64", "optional": true, "field": "v", "name": "io.debezium.time.MicroDuration" });
        let v = converted(duration, json!(93784000000i64));
        assert_eq!(
            (v.data_type.as_str(), v.value),
            ("INTERVAL", json!("93784000000 microseconds"))
        );
    }

    #[test]
    fn arrays_keep_elements() {
        let schema = json!({ "type": "array", "optional": true, "field": "v", "items": { "type": "int32", "optional": true } });
        let v = converted(schema, json!([1, null, 3]));
        assert_eq!(
            (v.data_type.as_str(), v.simple_type.as_str()),
            ("INTEGER[]", "ARRAY")
        );
        assert_eq!(v.value, json!([1, null, 3]));
        let schema = json!({ "type": "array", "optional": true, "field": "v", "items": {
            "type": "bytes", "optional": true, "name": "org.apache.kafka.connect.data.Decimal", "parameters": { "scale": "2" } } });
        let v = converted(schema, json!(["AeI="]));
        assert_eq!(
            (v.data_type.as_str(), v.value),
            ("NUMERIC[]", json!(["4.82"]))
        );
    }

    // Kiểu chưa biết: giữ giá trị (trước đây thành NULL)
    #[test]
    fn unknown_type_keeps_value_as_text() {
        let v = converted(
            json!({ "type": "struct", "optional": true, "field": "v", "name": "io.debezium.data.geometry.Point" }),
            json!({ "x": 1.0, "y": 2.0 }),
        );
        assert_eq!(v.data_type, "TEXT");
        assert_eq!(v.value, json!({ "x": 1.0, "y": 2.0 }));
    }
}
