use sqlx::{PgConnection, PgPool, Row};

use crate::models::{ChangeRow, PoisonMessage, PostgresDestination};

/// Table lưu dữ liệu lỗi ở DB đích.
pub const DEAD_LETTER_TABLE: &str = "_cdc_dead_letter";

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeadLetterKind {
    /// Message không parse được
    Poison,
    /// Dòng bị DB đích từ chối vì lỗi dữ liệu
    Rejected,
}

impl DeadLetterKind {
    pub fn as_str(self) -> &'static str {
        match self {
            DeadLetterKind::Poison => "poison",
            DeadLetterKind::Rejected => "rejected",
        }
    }
}

/// Một dòng cần ghi vào `_cdc_dead_letter`.
pub struct NewDeadLetter<'a> {
    pub kind: DeadLetterKind,
    pub subject: &'a str,
    pub stream_sequence: Option<u64>,
    pub table_name: Option<&'a str>,
    pub primary_key: Option<&'a str>,
    pub payload: &'a str,
    pub error_code: Option<&'a str>,
    pub error_message: &'a str,
    /// Some khi dòng đến từ replay: cập nhật dòng cũ thay vì thêm mới.
    pub existing_id: Option<i64>,
}

impl<'a> NewDeadLetter<'a> {
    pub fn rejected(row: &'a ChangeRow, code: &'a str, message: &'a str) -> Self {
        NewDeadLetter {
            kind: DeadLetterKind::Rejected,
            subject: &row.source.subject,
            stream_sequence: row.source.stream_sequence,
            table_name: Some(&row.table_name),
            primary_key: Some(&row.primary_key),
            payload: &row.source.payload,
            error_code: Some(code),
            error_message: message,
            existing_id: row.source.dead_letter_id,
        }
    }

    pub fn poison(message: &'a PoisonMessage) -> Self {
        NewDeadLetter {
            kind: DeadLetterKind::Poison,
            subject: &message.source.subject,
            stream_sequence: message.source.stream_sequence,
            table_name: None,
            primary_key: None,
            payload: &message.source.payload,
            error_code: None,
            error_message: &message.reason,
            existing_id: message.source.dead_letter_id,
        }
    }
}

/// Dòng `retry` lấy ra để replay.
pub struct RetryEntry {
    pub id: i64,
    pub subject: String,
    pub stream_sequence: Option<i64>,
    pub payload: String,
}

pub struct DeadLetterStore {
    /// Tên table đã kèm schema và quote
    table: String,
}

impl DeadLetterStore {
    pub fn new(schema: &str) -> Self {
        DeadLetterStore {
            table: format!(
                "{}.{}",
                PostgresDestination::quote_identifier(schema),
                PostgresDestination::quote_identifier(DEAD_LETTER_TABLE)
            ),
        }
    }

    pub async fn ensure_table(&self, pool: &PgPool) -> Result<(), String> {
        let statements = [
            format!(
                "CREATE TABLE IF NOT EXISTS {} (
                    id              BIGSERIAL PRIMARY KEY,
                    kind            TEXT NOT NULL,
                    subject         TEXT NOT NULL,
                    stream_sequence BIGINT,
                    table_name      TEXT,
                    primary_key     TEXT,
                    payload         TEXT NOT NULL,
                    error_code      TEXT,
                    error_message   TEXT NOT NULL,
                    status          TEXT NOT NULL DEFAULT 'pending',
                    attempts        INT  NOT NULL DEFAULT 1,
                    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
                    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
                )",
                self.table
            ),
            format!(
                "CREATE INDEX IF NOT EXISTS {}_open_idx ON {} (table_name, primary_key) \
                 WHERE status IN ('pending', 'retry')",
                DEAD_LETTER_TABLE, self.table
            ),
            format!(
                "CREATE INDEX IF NOT EXISTS {}_retry_idx ON {} (id) WHERE status = 'retry'",
                DEAD_LETTER_TABLE, self.table
            ),
        ];
        for statement in &statements {
            sqlx::query(statement)
                .execute(pool)
                .await
                .map_err(|e| format!("Can not create dead letter table: {}", e))?;
        }
        Ok(())
    }

    /// Ghi một dòng lỗi, trả về id. Khi replay (`existing_id`) thì đưa dòng cũ về `pending` với
    /// lỗi mới; nếu dòng cũ đã bị xóa bằng tay thì thêm dòng mới.
    pub async fn record(
        &self,
        entry: &NewDeadLetter<'_>,
        conn: &mut PgConnection,
    ) -> Result<i64, String> {
        if let Some(id) = entry.existing_id {
            let updated = sqlx::query(&self.update_existing_sql())
                .bind(id)
                .bind(entry.error_code)
                .bind(entry.error_message)
                .bind(entry.kind.as_str())
                .bind(entry.table_name)
                .bind(entry.primary_key)
                .fetch_optional(&mut *conn)
                .await
                .map_err(|e| format!("Failed to update dead letter {}: {}", id, e))?;
            if let Some(row) = updated {
                return Ok(row.get::<i64, _>("id"));
            }
        }
        let row = sqlx::query(&format!(
            "INSERT INTO {} (kind, subject, stream_sequence, table_name, primary_key, payload, \
             error_code, error_message) VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING id",
            self.table
        ))
        .bind(entry.kind.as_str())
        .bind(entry.subject)
        .bind(entry.stream_sequence.map(|sequence| sequence as i64))
        .bind(entry.table_name)
        .bind(entry.primary_key)
        .bind(entry.payload)
        .bind(entry.error_code)
        .bind(entry.error_message)
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| format!("Failed to record dead letter: {}", e))?;
        Ok(row.get::<i64, _>("id"))
    }

    /// Replay lại lỗi: poison sửa payload xong có thể thành dòng bị từ chối, nên cập nhật cả
    /// kind/table/khóa để dòng này còn bị superseded bởi bản mới hơn.
    fn update_existing_sql(&self) -> String {
        format!(
            "UPDATE {} SET status = 'pending', error_code = $2, error_message = $3, \
             kind = $4, table_name = $5, primary_key = $6, \
             attempts = attempts + 1, updated_at = now() WHERE id = $1 RETURNING id",
            self.table
        )
    }

    /// Các khóa chính vừa có bản mới hơn (ghi được hoặc lại bị từ chối): dòng lỗi cũ của chúng
    /// chuyển sang `superseded`, trừ các id trong `keep_ids`. `below_ids[i]` là Some(id) khi khóa
    /// `primary_keys[i]` đến từ replay: chỉ các dòng lỗi có id nhỏ hơn (cũ hơn) mới bị thay thế.
    pub async fn supersede(
        &self,
        table_name: &str,
        primary_keys: &[String],
        below_ids: &[Option<i64>],
        keep_ids: &[i64],
        conn: &mut PgConnection,
    ) -> Result<u64, String> {
        if primary_keys.is_empty() {
            return Ok(0);
        }
        sqlx::query(&format!(
            "UPDATE {} AS d SET status = 'superseded', updated_at = now() \
             FROM unnest($2::TEXT[], $3::BIGINT[]) AS k(primary_key, below_id) \
             WHERE d.status IN ('pending', 'retry') AND d.table_name = $1 \
               AND d.primary_key = k.primary_key \
               AND (k.below_id IS NULL OR d.id < k.below_id) \
               AND d.id <> ALL($4)",
            self.table
        ))
        .bind(table_name)
        .bind(primary_keys)
        .bind(below_ids)
        .bind(keep_ids)
        .execute(&mut *conn)
        .await
        .map(|result| result.rows_affected())
        .map_err(|e| format!("Failed to supersede dead letters: {}", e))
    }

    pub async fn fetch_retry(&self, limit: i64, pool: &PgPool) -> Result<Vec<RetryEntry>, String> {
        let rows = sqlx::query(&format!(
            "SELECT id, subject, stream_sequence, payload FROM {} \
             WHERE status = 'retry' ORDER BY id LIMIT $1",
            self.table
        ))
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to fetch dead letters to retry: {}", e))?;
        Ok(rows
            .into_iter()
            .map(|row| RetryEntry {
                id: row.get("id"),
                subject: row.get("subject"),
                stream_sequence: row.get("stream_sequence"),
                payload: row.get("payload"),
            })
            .collect())
    }

    pub async fn mark_resolved(&self, ids: &[i64], conn: &mut PgConnection) -> Result<(), String> {
        if ids.is_empty() {
            return Ok(());
        }
        sqlx::query(&format!(
            "UPDATE {} SET status = 'resolved', updated_at = now() \
             WHERE id = ANY($1) AND status IN ('pending', 'retry')",
            self.table
        ))
        .bind(ids)
        .execute(&mut *conn)
        .await
        .map_err(|e| format!("Failed to mark dead letters resolved: {}", e))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{RowAction, RowSource};
    use std::collections::HashMap;

    fn replayed_source() -> RowSource {
        let mut source = RowSource::new("debezium.public.accounts", Some(42), b"{\"x\":1}");
        source.dead_letter_id = Some(9);
        source
    }

    #[test]
    fn kind_names() {
        assert_eq!(DeadLetterKind::Poison.as_str(), "poison");
        assert_eq!(DeadLetterKind::Rejected.as_str(), "rejected");
    }

    #[test]
    fn rejected_entry_copies_row_fields() {
        let row = ChangeRow {
            table_name: "accounts".to_string(),
            table_value: HashMap::new(),
            primary_key: "5".to_string(),
            action: RowAction::Upsert,
            index: 0,
            source: replayed_source(),
        };
        let entry = NewDeadLetter::rejected(&row, "23514", "violates check");
        assert_eq!(entry.kind, DeadLetterKind::Rejected);
        assert_eq!(entry.subject, "debezium.public.accounts");
        assert_eq!(entry.stream_sequence, Some(42));
        assert_eq!(entry.table_name, Some("accounts"));
        assert_eq!(entry.primary_key, Some("5"));
        assert_eq!(entry.payload, "{\"x\":1}");
        assert_eq!(entry.error_code, Some("23514"));
        assert_eq!(entry.error_message, "violates check");
        assert_eq!(entry.existing_id, Some(9));
    }

    #[test]
    fn poison_entry_has_no_table_or_key() {
        let message = PoisonMessage {
            source: RowSource::new("debezium.public.x", None, b"not json"),
            reason: "invalid payload: boom".to_string(),
        };
        let entry = NewDeadLetter::poison(&message);
        assert_eq!(entry.kind, DeadLetterKind::Poison);
        assert_eq!(entry.table_name, None);
        assert_eq!(entry.primary_key, None);
        assert_eq!(entry.error_code, None);
        assert_eq!(entry.error_message, "invalid payload: boom");
        assert_eq!(entry.payload, "not json");
        assert_eq!(entry.existing_id, None);
    }

    // Final review #2: poison replay rồi bị từ chối phải có table/khóa để còn bị superseded
    #[test]
    fn replay_update_refreshes_kind_table_and_key() {
        let sql = DeadLetterStore::new("public").update_existing_sql();
        for column in ["kind = $4", "table_name = $5", "primary_key = $6"] {
            assert!(sql.contains(column), "missing `{}` in {}", column, sql);
        }
    }

    #[test]
    fn table_is_schema_qualified_and_quoted() {
        assert_eq!(
            DeadLetterStore::new("public").table,
            "\"public\".\"_cdc_dead_letter\""
        );
    }
}
