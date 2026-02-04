use std::{
    collections::{HashMap, HashSet}
};

use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{PgPool, Row, postgres::PgPoolOptions, types::Json};
use uuid::Uuid;

use crate::models::{DataModel, NatMessageReceive};

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
            "CREATE TABLE IF NOT EXISTS {}.\"_cdc_schema_metadata\" (
                schema_name TEXT NOT NULL,
                table_name TEXT NOT NULL,
                column_name TEXT NOT NULL,
                data_type TEXT NOT NULL,
                nullable BOOLEAN NOT NULL,
                last_updated TIMESTAMP NOT NULL DEFAULT NOW(),
                PRIMARY KEY (schema_name, table_name, column_name)
            )",
            "public"
        );

        sqlx::query(&query)
            .execute(pool)
            .await
            .map_err(|e| format!("Can not create schema metadata table: {}", e))?;

        Ok(())
    }

    pub async fn get_schema_info(
        &self,
        pool: &PgPool,
    ) -> Result<HashMap<String, HashSet<String>>, String> {
        let query_raw = format!(
            r#"
            SELECT schema_name, table_name, column_name, data_type, nullable
            FROM {}."_cdc_schema_metadata"
            WHERE schema_name = $1
            ORDER BY schema_name, table_name, column_name
        "#,
            Self::quote_identifier(&self.schema_expect.clone())
        );

        let rows = sqlx::query(&query_raw)
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
        Ok(result)
    }

    pub async fn create_table_if_not_exists_query(
        &self,
        schema_name: &String,
        table_name: &String,
        columns: &HashMap<String, DataModel>,
        pool: &PgPool,
    ) {
        let mut columns_definitions = Vec::new();
        for (col_name, col_type) in columns {
            let col_def = format!(
                "{} {} {} {}",
                Self::quote_identifier(col_name),
                col_type.data_type,
                if col_type.nullable { "" } else { "NOT NULL" },
                if col_name.clone() == "id".to_string() {
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
        sqlx::query(&query_info)
            .execute(pool)
            .await
            .expect("Failed to create table");

        let insert_query = format!(
            r#"INSERT INTO {}."_cdc_schema_metadata" (schema_name, table_name, column_name, data_type, nullable)
               VALUES ($1, $2, $3, $4, $5)
               ON CONFLICT (schema_name, table_name, column_name)
               DO UPDATE SET data_type = EXCLUDED.data_type, nullable = EXCLUDED.nullable, last_updated = NOW()"#,
            Self::quote_identifier(&self.schema_expect.clone())
        );

        for (col_name, col_type) in columns {
            sqlx::query(&insert_query)
                .bind(&self.schema_expect)
                .bind(table_name)
                .bind(col_name)
                .bind(&col_type.data_type)
                .bind(col_type.nullable)
                .execute(pool)
                .await
                .expect("Failed to upsert schema metadata");
        }
    }

    pub async fn insert_value(
        &self,
        table_name: &String,
        columns: &Vec<&NatMessageReceive>,
        pool: &PgPool,
    ) {
        let column_active = columns[0];
        let max_records = column_active.table_value.len();
        let mut s = String::from("INSERT INTO ");
        s.push_str(
            format!(
                "{}.{} (",
                self.schema_expect.clone(),
                Self::quote_identifier(table_name)
            )
            .as_str(),
        );

        let mut colum_keys = column_active
            .table_value
            .keys()
            .cloned()
            .collect::<Vec<String>>();
        colum_keys.sort();
        for (i, column) in colum_keys.clone().iter().enumerate() {
            s.push_str(
                format!(
                    "{}{} ",
                    Self::quote_identifier(column),
                    if i < max_records.clone() - 1 { "," } else { "" }
                )
                .as_str(),
            );
        }
        s.push_str(" )  SELECT * FROM unnest( ");
        for (i, v) in colum_keys.clone().iter().enumerate() {
            let data_model = column_active.table_value.get(v).unwrap();
            s.push_str(
                format!(
                    "${}::{}[] {} ",
                    i + 1,
                    data_model.simple_type,
                    if i < max_records.clone() - 1 { "," } else { "" }
                )
                .as_str(),
            );
        }
        s.push_str(");");
        println!("Insert Query: {}", s);

        // Build typed value vectors based on simple_type
        let mut query = sqlx::query(&s);
        
        for column in colum_keys.clone() {
            let data_model = column_active.table_value.get(&column).unwrap();
            let simple_type = &data_model.simple_type;
            
            println!("Binding column: {} with type: {}", column, simple_type);
            
            // Collect values from all records for this column
            let values: Vec<Value> = columns
                .iter()
                .map(|col_info| col_info.table_value.get(&column).unwrap().value.clone())
                .collect();
            
            println!("Values Map: {:?}", values);
            
            // Bind based on the PostgreSQL type
            match simple_type.as_str() {
                "TEXT" => {
                    let text_values: Vec<String> = values
                        .iter()
                        .map(|v| v.as_str().unwrap_or("").to_string())
                        .collect();
                    query = query.bind(text_values);
                }
                "INTEGER" | "SMALLINT" => {
                    let int_values: Vec<i32> = values
                        .iter()
                        .map(|v| v.as_i64().unwrap_or(0) as i32)
                        .collect();
                    query = query.bind(int_values);
                }
                "BIGINT" => {
                    let bigint_values: Vec<i64> = values
                        .iter()
                        .map(|v| v.as_i64().unwrap_or(0))
                        .collect();
                    query = query.bind(bigint_values);
                }
                "DOUBLE PRECISION" | "REAL" => {
                    let float_values: Vec<f64> = values
                        .iter()
                        .map(|v| v.as_f64().unwrap_or(0.0))
                        .collect();
                    query = query.bind(float_values);
                }
                "BOOLEAN" => {
                    let bool_values: Vec<bool> = values
                        .iter()
                        .map(|v| v.as_bool().unwrap_or(false))
                        .collect();
                    query = query.bind(bool_values);
                }
                "TIMESTAMPTZ" | "TIMESTAMP" => {
                    let timestamp_values: Vec<DateTime<Utc>> = values
                        .iter()
                        .map(|v| {
                            v.as_str()
                                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                                .map(|dt| dt.with_timezone(&Utc))
                                .unwrap_or_else(|| Utc::now())
                        })
                        .collect();
                    query = query.bind(timestamp_values);
                }
                "UUID" => {
                    let uuid_values: Vec<Uuid> = values
                        .iter()
                        .map(|v| {
                            v.as_str()
                                .and_then(|s| Uuid::parse_str(s).ok())
                                .unwrap_or_else(|| Uuid::nil())
                        })
                        .collect();
                    query = query.bind(uuid_values);
                }
                "JSONB" | "JSON" => {
                    let jsonb_values: Vec<Json<Value>> = values
                        .iter()
                        .map(|v| {
                            // If the value is already a JSON object/array, use it directly
                            // If it's a string, try to parse it
                            if v.is_string() {
                                let s = v.as_str().unwrap();
                                serde_json::from_str(s).unwrap_or_else(|_| v.clone())
                            } else {
                                v.clone()
                            }
                        })
                        .map(|v| Json(v))
                        .collect();
                    query = query.bind(jsonb_values);
                }
                _ => {
                    // Default to TEXT for unknown types
                    let text_values: Vec<String> = values
                        .iter()
                        .map(|v| v.to_string())
                        .collect();
                    query = query.bind(text_values);
                }
            }
        }
        
        query.execute(pool).await.expect("Failed to insert values");
    }

    fn quote_identifier(identifier: &str) -> String {
        // Check if identifier contains uppercase letters
        if identifier.chars().any(|c| c.is_uppercase()) {
            format!("\"{}\"", identifier)
        } else {
            identifier.to_string()
        }
    }
}
