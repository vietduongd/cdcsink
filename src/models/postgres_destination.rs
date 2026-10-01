use std::collections::{HashMap, HashSet};

use base64::{Engine, engine::general_purpose};
use serde_json::Value;
use sqlx::{PgPool, Row, postgres::PgPoolOptions};
use tracing::{error, warn};

use crate::models::{
    ChangeRow, DataModel, WriteError, decimal::decimal_text, sync_config::primary_key_column,
};

/// Table metadata cdcsink tự tạo ở DB đích.
pub const SCHEMA_METADATA_TABLE: &str = "_cdc_schema_metadata";

pub struct PostgresDestination {
    pub database_url: String,
    pub schema_expect: String,
}

impl PostgresDestination {
    const DEFAULT_MAX_CONNECTIONS: u32 = 10;
    pub fn new(database_url: String, schema_expect: String) -> Self {
        PostgresDestination {
            database_url,
            schema_expect,
        }
    }

    pub async fn connect(&self) -> Result<PgPool, String> {
        let pool = PgPoolOptions::new()
            .max_connections(Self::DEFAULT_MAX_CONNECTIONS)
            .connect(&self.database_url)
            .await
            .map_err(|e| format!("Failed to connect to PostgreSQL: {}", e))?;
        Ok(pool)
    }

    pub async fn ensure_schema_metadata_table(&self, pool: &PgPool) -> Result<(), String> {
        let query = format!(
            "CREATE TABLE IF NOT EXISTS {}.{} (
                schema_name TEXT NOT NULL,
                table_name TEXT NOT NULL,
                column_name TEXT NOT NULL,
                data_type TEXT NOT NULL,
                nullable BOOLEAN NOT NULL,
                last_updated TIMESTAMP NOT NULL DEFAULT NOW(),
                PRIMARY KEY (schema_name, table_name, column_name)
            )",
            Self::quote_identifier(&self.schema_expect.clone()),
            Self::quote_identifier(SCHEMA_METADATA_TABLE)
        );

        sqlx::query(&query)
            .execute(pool)
            .await
            .map_err(|e| format!("Can not create schema metadata table: {}", e))?;

        Ok(())
    }

    /// Table/cột đang thực sự có trong schema đích (đọc từ pg_catalog).
    /// Không dựa vào `_cdc_schema_metadata`: table/cột bị drop bằng tay mà metadata vẫn còn
    /// sẽ làm cdcsink bỏ qua CREATE TABLE / ADD COLUMN rồi insert lỗi "does not exist".
    pub async fn get_schema_info(
        &self,
        pool: &PgPool,
    ) -> Result<HashMap<String, HashSet<String>>, String> {
        let rows = sqlx::query(
            r#"
            SELECT c.relname::text AS table_name, a.attname::text AS column_name
            FROM pg_catalog.pg_attribute a
            JOIN pg_catalog.pg_class c ON c.oid = a.attrelid
            JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
            WHERE n.nspname = $1
              AND c.relkind IN ('r', 'p')
              AND a.attnum > 0
              AND NOT a.attisdropped
        "#,
        )
        .bind(&self.schema_expect)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to fetch schema info: {}", e))?;
        let mut result: HashMap<String, HashSet<String>> = HashMap::new();
        for item in rows {
            let column_name = item.get::<String, _>("column_name");
            result
                .entry(item.get("table_name"))
                .or_insert(HashSet::new())
                .insert(column_name);
        }
        self.report_metadata_drift(&result, pool).await;
        Ok(result)
    }

    /// Log table/cột có trong `_cdc_schema_metadata` nhưng không còn ở đích (sẽ được tạo lại).
    async fn report_metadata_drift(
        &self,
        actual: &HashMap<String, HashSet<String>>,
        pool: &PgPool,
    ) {
        let query_raw = format!(
            "SELECT table_name, column_name FROM {}.{} WHERE schema_name = $1",
            Self::quote_identifier(&self.schema_expect),
            Self::quote_identifier(SCHEMA_METADATA_TABLE)
        );
        let rows = match sqlx::query(&query_raw)
            .bind(&self.schema_expect)
            .fetch_all(pool)
            .await
        {
            Ok(rows) => rows,
            Err(e) => {
                error!(table = SCHEMA_METADATA_TABLE, error = %e, "Failed to read schema metadata");
                return;
            }
        };
        let mut missing_tables: HashSet<String> = HashSet::new();
        let mut missing_columns: Vec<String> = Vec::new();
        for row in rows {
            let table_name = row.get::<String, _>("table_name");
            let column_name = row.get::<String, _>("column_name");
            match actual.get(&table_name) {
                None => {
                    missing_tables.insert(table_name);
                }
                Some(columns) if !columns.contains(&column_name) => {
                    missing_columns.push(format!("{}.{}", table_name, column_name));
                }
                Some(_) => {}
            }
        }
        if !missing_tables.is_empty() {
            let mut names: Vec<String> = missing_tables.into_iter().collect();
            names.sort();
            warn!(
                schema = %self.schema_expect,
                tables = %names.join(", "),
                "Tables in {} but missing in schema (will be recreated)",
                SCHEMA_METADATA_TABLE
            );
        }
        if !missing_columns.is_empty() {
            missing_columns.sort();
            warn!(
                schema = %self.schema_expect,
                columns = %missing_columns.join(", "),
                "Columns in {} but missing in schema (will be re-added)",
                SCHEMA_METADATA_TABLE
            );
        }
    }

    pub async fn create_table_if_not_exists_query(
        &self,
        schema_name: &String,
        table_name: &String,
        columns: &HashMap<String, DataModel>,
        pool: &PgPool,
    ) -> Result<(), String> {
        let primary_key = primary_key_column(columns);
        let mut columns_definitions = Vec::new();
        for (col_name, col_type) in columns {
            let col_def = format!(
                "{} {} {} {}",
                Self::quote_identifier(col_name),
                col_type.data_type,
                if col_type.nullable { "" } else { "NOT NULL" },
                if Some(col_name) == primary_key {
                    "PRIMARY KEY"
                } else {
                    ""
                }
            );
            columns_definitions.push(col_def);
        }
        let columns_sql = columns_definitions.join(", ");

        let query_info = format!(
            "CREATE TABLE IF NOT EXISTS {}.{} ({});",
            Self::quote_identifier(schema_name),
            Self::quote_identifier(table_name),
            columns_sql
        );
        sqlx::query(&query_info).execute(pool).await.map_err(|e| {
            format!(
                "Failed to create table {}.{}: {}",
                schema_name, table_name, e
            )
        })?;

        let insert_query = format!(
            r#"INSERT INTO {}.{} (schema_name, table_name, column_name, data_type, nullable)
               VALUES ($1, $2, $3, $4, $5)
               ON CONFLICT (schema_name, table_name, column_name)
               DO UPDATE SET data_type = EXCLUDED.data_type, nullable = EXCLUDED.nullable, last_updated = NOW()"#,
            Self::quote_identifier(&self.schema_expect.clone()),
            Self::quote_identifier(SCHEMA_METADATA_TABLE)
        );

        // Metadata chỉ dùng để đối chiếu lúc khởi động: lỗi ghi metadata không chặn sync
        for (col_name, col_type) in columns {
            if let Err(e) = sqlx::query(&insert_query)
                .bind(&self.schema_expect)
                .bind(table_name)
                .bind(col_name)
                .bind(&col_type.data_type)
                .bind(col_type.nullable)
                .execute(pool)
                .await
            {
                warn!(
                    schema = %schema_name,
                    table = %table_name,
                    column = %col_name,
                    error = %e,
                    "Failed to upsert schema metadata"
                );
            }
        }
        Ok(())
    }

    pub async fn add_column_if_not_exists(
        &self,
        schema_name: &str,
        table_name: &str,
        col_name: &str,
        col_type: &DataModel,
        pool: &PgPool,
    ) -> Result<(), String> {
        let alter_query = format!(
            "ALTER TABLE {}.{} ADD COLUMN IF NOT EXISTS {} {}",
            Self::quote_identifier(schema_name),
            Self::quote_identifier(table_name),
            Self::quote_identifier(col_name),
            col_type.data_type,
        );
        sqlx::query(&alter_query).execute(pool).await.map_err(|e| {
            format!(
                "Failed to add column {}.{}.{}: {}",
                schema_name, table_name, col_name, e
            )
        })?;

        // Cập nhật metadata
        let insert_query = format!(
            r#"INSERT INTO {}.{} (schema_name, table_name, column_name, data_type, nullable)
               VALUES ($1, $2, $3, $4, $5)
               ON CONFLICT (schema_name, table_name, column_name)
               DO UPDATE SET data_type = EXCLUDED.data_type, nullable = EXCLUDED.nullable, last_updated = NOW()"#,
            Self::quote_identifier(&self.schema_expect.clone()),
            Self::quote_identifier(SCHEMA_METADATA_TABLE)
        );
        if let Err(e) = sqlx::query(&insert_query)
            .bind(&self.schema_expect)
            .bind(table_name)
            .bind(col_name)
            .bind(&col_type.data_type)
            .bind(col_type.nullable)
            .execute(pool)
            .await
        {
            warn!(
                schema = %schema_name,
                table = %table_name,
                column = %col_name,
                error = %e,
                "Failed to upsert schema metadata"
            );
        }
        Ok(())
    }

    fn qualified_table(&self, table_name: &str) -> String {
        format!(
            "{}.{}",
            Self::quote_identifier(&self.schema_expect),
            Self::quote_identifier(table_name)
        )
    }

    /// Upsert các dòng của một table bằng một câu lệnh. Mỗi khóa chính chỉ được có một dòng
    /// (xem `latest_per_key`); dòng thiếu cột nào thì cột đó nhận NULL.
    pub async fn upsert_rows(
        &self,
        table_name: &str,
        rows: Vec<&ChangeRow>,
        pool: &PgPool,
    ) -> Result<(), WriteError> {
        if rows.is_empty() {
            return Ok(());
        }
        let column_active = &rows[0].table_value;
        let primary_key = match primary_key_column(column_active) {
            Some(pk) => pk.clone(),
            None => {
                return Err(WriteError::Transient(format!(
                    "Missing id column for upsert into table {}",
                    table_name
                )));
            }
        };
        let mut colum_keys = column_active.keys().cloned().collect::<Vec<String>>();
        colum_keys.sort();

        // Mọi cột bind dạng TEXT[] rồi cast sang kiểu cột trong SELECT: một đường xử lý cho mọi kiểu
        // (NaN/Infinity, infinity, mảng, bytea, numeric không giới hạn chữ số...)
        let column_list = colum_keys
            .iter()
            .map(|column| Self::quote_identifier(column))
            .collect::<Vec<String>>()
            .join(", ");
        let select_list = colum_keys
            .iter()
            .enumerate()
            .map(|(i, column)| format!("u.c{}::{}", i, column_active[column].data_type))
            .collect::<Vec<String>>()
            .join(", ");
        let params = (1..=colum_keys.len())
            .map(|i| format!("${}::TEXT[]", i))
            .collect::<Vec<String>>()
            .join(", ");
        let aliases = (0..colum_keys.len())
            .map(|i| format!("c{}", i))
            .collect::<Vec<String>>()
            .join(", ");
        let updates = colum_keys
            .iter()
            .filter(|column| **column != primary_key)
            .map(|column| {
                let quoted = Self::quote_identifier(column);
                format!("{} = EXCLUDED.{}", quoted, quoted)
            })
            .collect::<Vec<String>>();
        let on_conflict = if updates.is_empty() {
            "DO NOTHING".to_string()
        } else {
            format!("DO UPDATE SET {}", updates.join(", "))
        };
        let s = format!(
            "INSERT INTO {} ({}) SELECT {} FROM unnest({}) AS u({}) ON CONFLICT ({}) {}",
            self.qualified_table(table_name),
            column_list,
            select_list,
            params,
            aliases,
            Self::quote_identifier(&primary_key),
            on_conflict
        );

        let mut query = sqlx::query(&s);
        for column in &colum_keys {
            // Record thiếu cột này thì dùng NULL
            let values: Vec<Option<String>> = rows
                .iter()
                .map(|row| row.table_value.get(column).and_then(Self::to_pg_text))
                .collect();
            query = query.bind(values);
        }
        query.execute(pool).await.map_err(|e| {
            WriteError::from(e).context(&format!("Failed to upsert into table {}", table_name))
        })?;
        Ok(())
    }

    pub async fn delete_rows(
        &self,
        table_name: &str,
        rows: Vec<&ChangeRow>,
        pool: &PgPool,
    ) -> Result<(), WriteError> {
        if rows.is_empty() {
            return Ok(());
        }
        let primary_key = match primary_key_column(&rows[0].table_value) {
            Some(pk) => pk.clone(),
            None => {
                return Err(WriteError::Transient(format!(
                    "Missing id column for delete in table {}",
                    table_name
                )));
            }
        };
        let key_type = &rows[0].table_value[&primary_key].data_type;
        let delete_query_str = format!(
            "DELETE FROM {} WHERE {} = ANY($1::TEXT[]::{}[]);",
            self.qualified_table(table_name),
            Self::quote_identifier(&primary_key),
            key_type
        );
        let ids: Vec<Option<String>> = rows
            .iter()
            .map(|row| row.table_value.get(&primary_key).and_then(Self::to_pg_text))
            .collect();
        sqlx::query(&delete_query_str)
            .bind(ids)
            .execute(pool)
            .await
            .map_err(|e| {
                WriteError::from(e).context(&format!("Failed to delete from table {}", table_name))
            })?;
        Ok(())
    }

    /// Dạng text Postgres đọc được của một ô, sẽ được cast sang kiểu cột khi insert. NULL -> None.
    fn to_pg_text(data: &DataModel) -> Option<String> {
        let value = &data.value;
        if value.is_null() {
            return None;
        }
        match data.simple_type.as_str() {
            "NUMERIC" | "DECIMAL" => decimal_text(value),
            "BYTEA" => {
                let bytes = general_purpose::STANDARD.decode(value.as_str()?).ok()?;
                let hex: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
                Some(format!("\\x{}", hex))
            }
            "JSONB" | "JSON" => Some(match value {
                // Debezium gửi JSON dạng chuỗi; chuỗi không phải JSON hợp lệ thì lưu thành JSON string
                Value::String(s) if serde_json::from_str::<Value>(s).is_ok() => s.clone(),
                other => other.to_string(),
            }),
            "ARRAY" => Some(Self::pg_array_literal(value)),
            _ => Some(match value {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            }),
        }
    }

    /// JSON array -> literal mảng Postgres, vd `{"a","b\"c",NULL}`.
    fn pg_array_literal(value: &Value) -> String {
        match value {
            Value::Array(elements) => {
                let items = elements
                    .iter()
                    .map(|element| match element {
                        Value::Null => "NULL".to_string(),
                        Value::Array(_) => Self::pg_array_literal(element),
                        Value::String(s) => Self::quote_array_element(s),
                        other => Self::quote_array_element(&other.to_string()),
                    })
                    .collect::<Vec<String>>();
                format!("{{{}}}", items.join(","))
            }
            other => other.to_string(),
        }
    }

    fn quote_array_element(s: &str) -> String {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    }

    /// Luôn đặt trong dấu nháy kép: giữ hoa thường, cho phép từ khóa SQL, khoảng trắng, gạch ngang...
    /// Tên viết thường khi quote vẫn trùng tên cũ nên không ảnh hưởng table đã tạo trước đây.
    pub(crate) fn quote_identifier(identifier: &str) -> String {
        format!("\"{}\"", identifier.replace('"', "\"\""))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn model(simple_type: &str, value: Value) -> DataModel {
        DataModel {
            value,
            data_type: simple_type.to_string(),
            nullable: true,
            simple_type: simple_type.to_string(),
        }
    }

    #[test]
    fn quotes_every_identifier() {
        assert_eq!(PostgresDestination::quote_identifier("order"), "\"order\"");
        assert_eq!(
            PostgresDestination::quote_identifier("customer list"),
            "\"customer list\""
        );
        assert_eq!(
            PostgresDestination::quote_identifier("OrderItems"),
            "\"OrderItems\""
        );
        assert_eq!(PostgresDestination::quote_identifier("a\"b"), "\"a\"\"b\"");
    }

    #[test]
    fn text_for_each_type() {
        let text = |t: &str, v: Value| PostgresDestination::to_pg_text(&model(t, v));
        assert_eq!(text("TEXT", Value::Null), None);
        assert_eq!(
            text("TEXT", json!("it's \"x\"")).as_deref(),
            Some("it's \"x\"")
        );
        assert_eq!(
            text("DATE", json!("2024-02-29")).as_deref(),
            Some("2024-02-29")
        );
        assert_eq!(text("DOUBLE PRECISION", json!(3.5)).as_deref(), Some("3.5"));
        assert_eq!(
            text("DOUBLE PRECISION", json!("NaN")).as_deref(),
            Some("NaN")
        );
        assert_eq!(
            text("BIGINT", json!(9223372036854775807i64)).as_deref(),
            Some("9223372036854775807")
        );
        assert_eq!(text("BOOLEAN", json!(true)).as_deref(), Some("true"));
        assert_eq!(
            text("NUMERIC", json!({"scale": 2, "value": "AeI="})).as_deref(),
            Some("4.82")
        );
        assert_eq!(text("BYTEA", json!("AQL/")).as_deref(), Some("\\x0102ff"));
        assert_eq!(
            text("JSONB", json!("{\"a\": 1}")).as_deref(),
            Some("{\"a\": 1}")
        );
        assert_eq!(text("JSONB", json!("\"s\"")).as_deref(), Some("\"s\""));
        assert_eq!(
            text("JSONB", json!("not json")).as_deref(),
            Some("\"not json\"")
        );
        assert_eq!(
            text("ARRAY", json!(["a,b", "c\"d", "e\\f", null, ""])).as_deref(),
            Some(r#"{"a,b","c\"d","e\\f",NULL,""}"#)
        );
        assert_eq!(
            text("ARRAY", json!([[1, 2], [3, 4]])).as_deref(),
            Some(r#"{{"1","2"},{"3","4"}}"#)
        );
    }
}
