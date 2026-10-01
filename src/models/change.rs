use std::{collections::HashMap, sync::Arc};

use serde_json::Value;

use crate::models::{
    DataModel, DataRecord, RowAction, SyncConfig,
    dead_letter::DEAD_LETTER_TABLE,
    postgres_destination::SCHEMA_METADATA_TABLE,
    sync_config::{MatchOutcome, classify, primary_key_column},
};

/// Nguồn gốc của một dòng: message NATS, hoặc một dòng `_cdc_dead_letter` khi replay.
#[derive(Debug, Clone)]
pub struct RowSource {
    pub subject: String,
    pub stream_sequence: Option<u64>,
    /// Payload gốc Debezium. Byte không phải UTF-8 và byte NUL được thay bằng U+FFFD.
    pub payload: Arc<str>,
    /// Some khi dòng đến từ replay dead-letter.
    pub dead_letter_id: Option<i64>,
}

impl RowSource {
    pub fn new(subject: &str, stream_sequence: Option<u64>, payload: &[u8]) -> Self {
        RowSource {
            subject: subject.to_string(),
            stream_sequence,
            // TEXT của Postgres không nhận byte NUL: thay bằng U+FFFD để luôn ghi được vào dead-letter
            payload: String::from_utf8_lossy(payload)
                .replace('\0', "\u{FFFD}")
                .into(),
            dead_letter_id: None,
        }
    }
}

/// Thay đổi của một dòng, đã áp sync_config, sẵn sàng ghi vào đích.
#[derive(Debug)]
pub struct ChangeRow {
    pub table_name: String,
    pub table_value: HashMap<String, DataModel>,
    /// Khóa chính dạng text: chuỗi không kèm dấu nháy, kiểu khác theo JSON (vd `42`).
    pub primary_key: String,
    pub action: RowAction,
    /// Thứ tự trong batch: cùng khóa chính thì index lớn hơn là bản mới hơn.
    pub index: i64,
    pub source: RowSource,
}

/// Message không parse được, sẽ được ghi vào dead-letter.
#[derive(Debug)]
pub struct PoisonMessage {
    pub source: RowSource,
    pub reason: String,
}

/// Message bỏ qua có chủ đích: ack luôn, không vào dead-letter.
#[derive(Debug, PartialEq)]
pub enum SkipReason {
    /// Tombstone của Debezium (tombstones.on.delete=true): không mang dữ liệu
    Tombstone,
    IgnoredTable(String),
    NoPrimaryKey(String),
}

#[derive(Debug)]
pub enum Parsed {
    Row {
        row: ChangeRow,
        outcome: MatchOutcome,
    },
    Skip(SkipReason),
    Poison(PoisonMessage),
}

/// Bỏ hậu tố `_resync` để message resync dùng chung table và config với table gốc.
pub fn normalize_table_name(name: &str) -> String {
    name.strip_suffix("_resync").unwrap_or(name).to_string()
}

/// Table không bao giờ sync: table nội bộ cdcsink tự tạo ở DB đích (`_cdc_schema_metadata`,
/// `_cdc_dead_letter`). Khi DB đích lại là nguồn của một tầng CDC khác (cdcsink nối tiếp cdcsink),
/// sync các table này sẽ ghi đè dữ liệu nội bộ của tầng sau.
pub fn is_ignored_table(name: &str) -> bool {
    name.eq_ignore_ascii_case(SCHEMA_METADATA_TABLE) || name.eq_ignore_ascii_case(DEAD_LETTER_TABLE)
}

/// Parse payload Debezium thành `ChangeRow`. Dùng chung cho message NATS và replay dead-letter.
pub fn parse_change(source: RowSource, index: i64, sync_config: Option<&SyncConfig>) -> Parsed {
    if source.payload.trim().is_empty() {
        return Parsed::Skip(SkipReason::Tombstone);
    }
    let data_record: DataRecord = match serde_json::from_str(&source.payload) {
        Ok(record) => record,
        Err(e) => return poison(source, format!("invalid payload: {}", e)),
    };
    let Some(raw_table_name) = data_record.get_table_name() else {
        return poison(source, "missing table name".to_string());
    };
    let table_name = normalize_table_name(&raw_table_name);
    if is_ignored_table(&table_name) {
        return Parsed::Skip(SkipReason::IgnoredTable(table_name));
    }
    let Some(mut table_value) = data_record.get_table_structure() else {
        return poison(source, "missing table structure".to_string());
    };
    let Some(primary_key) =
        primary_key_column(&table_value).map(|column| key_text(&table_value[column].value))
    else {
        return Parsed::Skip(SkipReason::NoPrimaryKey(table_name));
    };

    let table_config = sync_config.and_then(|config| config.table(&table_name));
    // Debezium xóa dòng bằng op "d"; "before" có thể chỉ chứa khóa chính nên không xét where
    let (action, outcome) = if data_record.payload.op == "d" {
        (RowAction::Delete, MatchOutcome::default())
    } else {
        classify(&table_value, table_config)
    };
    if let Some(config) = table_config {
        config.retain_columns(&mut table_value);
    }
    Parsed::Row {
        row: ChangeRow {
            table_name,
            table_value,
            primary_key,
            action,
            index,
            source,
        },
        outcome,
    }
}

/// Mỗi khóa chính chỉ giữ bản mới nhất (index lớn nhất), sắp theo index để kết quả ổn định.
pub fn latest_per_key<'a>(rows: &[&'a ChangeRow]) -> Vec<&'a ChangeRow> {
    let mut latest: HashMap<&'a str, &'a ChangeRow> = HashMap::new();
    for &row in rows {
        let key = row.primary_key.as_str();
        if latest
            .get(key)
            .is_none_or(|existing| existing.index < row.index)
        {
            latest.insert(key, row);
        }
    }
    let mut result: Vec<&'a ChangeRow> = latest.into_values().collect();
    result.sort_by_key(|row| row.index);
    result
}

fn poison(source: RowSource, reason: String) -> Parsed {
    Parsed::Poison(PoisonMessage { source, reason })
}

fn key_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Payload Debezium tối thiểu: cột khóa `key_field` kiểu `key_type` và cột `name`.
    fn payload(table: &str, op: &str, key_field: &str, key_type: &str, row: Value) -> String {
        let (before, after) = if op == "d" {
            (row, Value::Null)
        } else {
            (Value::Null, row)
        };
        json!({
            "schema": {
                "type": "struct", "optional": false, "name": "t.Envelope", "version": 1,
                "fields": [
                    { "type": "struct", "optional": true, "field": "after", "fields": [
                        { "type": key_type, "optional": false, "field": key_field },
                        { "type": "string", "optional": true, "field": "name" }
                    ]}
                ]
            },
            "payload": {
                "before": before,
                "after": after,
                "source": {
                    "version": "2", "connector": "postgresql", "name": "s", "ts_ms": 0,
                    "snapshot": "false", "db": "db", "schema": "public", "table": table
                },
                "transaction": null,
                "op": op
            }
        })
        .to_string()
    }

    fn parse(payload: &str) -> Parsed {
        let source = RowSource::new("debezium.public.accounts", Some(7), payload.as_bytes());
        parse_change(source, 3, None)
    }

    fn expect_row(parsed: Parsed) -> ChangeRow {
        match parsed {
            Parsed::Row { row, .. } => row,
            other => panic!("expected Row, got {:?}", other),
        }
    }

    fn expect_poison(parsed: Parsed) -> PoisonMessage {
        match parsed {
            Parsed::Poison(message) => message,
            other => panic!("expected Poison, got {:?}", other),
        }
    }

    fn expect_skip(parsed: Parsed) -> SkipReason {
        match parsed {
            Parsed::Skip(reason) => reason,
            other => panic!("expected Skip, got {:?}", other),
        }
    }

    #[test]
    fn valid_payload_becomes_row() {
        let text = payload(
            "accounts",
            "c",
            "id",
            "int32",
            json!({ "id": 5, "name": "a" }),
        );
        let row = expect_row(parse(&text));
        assert_eq!(row.table_name, "accounts");
        assert_eq!(row.primary_key, "5");
        assert_eq!(row.action, RowAction::Upsert);
        assert_eq!(row.index, 3);
        assert_eq!(row.source.subject, "debezium.public.accounts");
        assert_eq!(row.source.stream_sequence, Some(7));
        assert_eq!(&*row.source.payload, text.as_str());
        assert_eq!(row.source.dead_letter_id, None);
        assert_eq!(row.table_value["name"].value, json!("a"));
    }

    // Review Focus #1: khóa chuỗi không kèm dấu nháy JSON
    #[test]
    fn string_primary_key_has_no_json_quotes() {
        let text = payload(
            "accounts",
            "c",
            "id",
            "string",
            json!({ "id": "abc", "name": "a" }),
        );
        assert_eq!(expect_row(parse(&text)).primary_key, "abc");
    }

    #[test]
    fn delete_op_is_delete() {
        let text = payload(
            "accounts",
            "d",
            "id",
            "int32",
            json!({ "id": 5, "name": "a" }),
        );
        let row = expect_row(parse(&text));
        assert_eq!(row.action, RowAction::Delete);
        assert_eq!(row.primary_key, "5");
    }

    #[test]
    fn resync_suffix_is_stripped() {
        let text = payload(
            "accounts_resync",
            "r",
            "id",
            "int32",
            json!({ "id": 1, "name": "a" }),
        );
        assert_eq!(expect_row(parse(&text)).table_name, "accounts");
    }

    #[test]
    fn invalid_json_is_poison() {
        let poison = expect_poison(parse("not json at all"));
        assert!(
            poison.reason.starts_with("invalid payload:"),
            "{}",
            poison.reason
        );
        assert_eq!(&*poison.source.payload, "not json at all");
        assert_eq!(poison.source.stream_sequence, Some(7));
    }

    // Final review #1: TEXT của Postgres không nhận byte NUL (SQLSTATE 22021)
    #[test]
    fn nul_bytes_are_replaced_so_postgres_text_accepts_payload() {
        let source = RowSource::new("s", None, b"a\0b");
        assert!(!source.payload.contains('\0'));
        assert_eq!(&*source.payload, "a\u{FFFD}b");
    }

    // Review Focus #5: payload không phải UTF-8
    #[test]
    fn non_utf8_payload_is_poison_not_panic() {
        let source = RowSource::new("debezium.public.x", None, &[0xff, 0xfe, b'{']);
        assert!(source.payload.contains('\u{FFFD}'));
        let poison = expect_poison(parse_change(source, 0, None));
        assert!(poison.reason.starts_with("invalid payload:"));
    }

    #[test]
    fn missing_after_schema_is_poison() {
        let text = json!({
            "schema": { "type": "struct", "optional": false, "name": "t", "version": 1, "fields": [] },
            "payload": {
                "before": null, "after": { "id": 1 },
                "source": {
                    "version": "2", "connector": "postgresql", "name": "s", "ts_ms": 0,
                    "snapshot": "false", "db": "db", "schema": "public", "table": "accounts"
                },
                "transaction": null, "op": "c"
            }
        })
        .to_string();
        assert_eq!(
            expect_poison(parse(&text)).reason,
            "missing table structure"
        );
    }

    #[test]
    fn empty_payload_is_tombstone() {
        assert_eq!(expect_skip(parse("")), SkipReason::Tombstone);
        assert_eq!(expect_skip(parse("  \n")), SkipReason::Tombstone);
    }

    #[test]
    fn metadata_table_is_skipped() {
        let text = payload(
            "_cdc_schema_metadata",
            "c",
            "id",
            "int32",
            json!({ "id": 1, "name": "a" }),
        );
        assert_eq!(
            expect_skip(parse(&text)),
            SkipReason::IgnoredTable("_cdc_schema_metadata".to_string())
        );
    }

    #[test]
    fn table_without_id_is_skipped() {
        let text = payload(
            "accounts",
            "c",
            "code",
            "int32",
            json!({ "code": 1, "name": "a" }),
        );
        assert_eq!(
            expect_skip(parse(&text)),
            SkipReason::NoPrimaryKey("accounts".to_string())
        );
    }

    fn row(primary_key: &str, index: i64) -> ChangeRow {
        ChangeRow {
            table_name: "accounts".to_string(),
            table_value: HashMap::new(),
            primary_key: primary_key.to_string(),
            action: RowAction::Upsert,
            index,
            source: RowSource::new("s", None, b"{}"),
        }
    }

    // Review Focus #2: nhiều bản cùng khóa chính trong một batch
    #[test]
    fn latest_per_key_keeps_newest_in_index_order() {
        let rows = [
            row("1", 0),
            row("2", 1),
            row("1", 2),
            row("3", 3),
            row("2", 4),
        ];
        let refs: Vec<&ChangeRow> = rows.iter().collect();
        let latest: Vec<(&str, i64)> = latest_per_key(&refs)
            .into_iter()
            .map(|r| (r.primary_key.as_str(), r.index))
            .collect();
        assert_eq!(latest, vec![("1", 2), ("3", 3), ("2", 4)]);
    }

    // Chuyển từ nats_receive.rs
    #[test]
    fn strips_resync_suffix() {
        assert_eq!(normalize_table_name("orders_resync"), "orders");
        assert_eq!(normalize_table_name("OrderItems_resync"), "OrderItems");
        assert_eq!(normalize_table_name("orders"), "orders");
        assert_eq!(normalize_table_name("resync_log"), "resync_log");
    }

    #[test]
    fn ignores_cdc_internal_tables() {
        assert!(is_ignored_table("_cdc_schema_metadata"));
        assert!(is_ignored_table("_CDC_Schema_Metadata"));
        // bản _resync được chuẩn hóa trước khi kiểm tra
        assert!(is_ignored_table(&normalize_table_name(
            "_cdc_schema_metadata_resync"
        )));
        assert!(is_ignored_table("_cdc_dead_letter"));
        assert!(is_ignored_table(&normalize_table_name(
            "_cdc_dead_letter_resync"
        )));
        assert!(!is_ignored_table("cdc_schema_metadata"));
        assert!(!is_ignored_table("orders"));
    }
}
