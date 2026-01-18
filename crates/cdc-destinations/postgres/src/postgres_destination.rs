use async_trait::async_trait;
use cdc_core::{DataRecord, Destination, DestinationStatus, Error, Operation, Result};
use serde::{Deserialize, Serialize};
use sqlx::postgres::{PgPool, PgPoolOptions};
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone, Serialize)]
pub struct PostgresConfig {
    /// PostgreSQL connection URL (built from individual fields or provided directly)
    pub url: String,

    /// Maximum number of connections in the pool
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,

    /// Target schema name
    #[serde(default = "default_schema")]
    pub schema: String,

    /// Conflict resolution strategy
    #[serde(default)]
    pub conflict_resolution: ConflictResolution,

    /// Automatically create tables if they don't exist
    #[serde(default = "default_auto_create_tables")]
    pub auto_create_tables: bool,

    /// Automatically add columns if they don't exist
    #[serde(default = "default_auto_add_columns")]
    pub auto_add_columns: bool,
}

fn default_max_connections() -> u32 {
    10
}

fn default_schema() -> String {
    "public".to_string()
}

fn default_auto_create_tables() -> bool {
    true
}

fn default_auto_add_columns() -> bool {
    true
}

impl<'de> Deserialize<'de> for PostgresConfig {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        #[derive(Deserialize)]
        struct PostgresConfigHelper {
            // Direct URL format
            url: Option<String>,

            // Individual fields format
            host: Option<String>,
            port: Option<u16>,
            username: Option<String>,
            password: Option<String>,
            database: Option<String>,

            // Optional configuration
            #[serde(default = "default_max_connections")]
            max_connections: u32,
            #[serde(default = "default_schema")]
            schema: String,
            #[serde(default)]
            conflict_resolution: ConflictResolution,
            #[serde(default = "default_auto_create_tables")]
            auto_create_tables: bool,
            #[serde(default = "default_auto_add_columns")]
            auto_add_columns: bool,
        }

        let helper = PostgresConfigHelper::deserialize(deserializer)?;

        // Build URL from either direct URL or individual fields
        let url = if let Some(url) = helper.url {
            // Direct URL provided
            url
        } else if let (Some(host), Some(username)) = (helper.host, helper.username) {
            // Build URL from individual fields
            let port = helper.port.unwrap_or(5432);
            let password = helper.password.unwrap_or_default();
            let database = helper.database.unwrap_or_else(|| "postgres".to_string());

            if password.is_empty() {
                format!("postgresql://{}@{}:{}/{}", username, host, port, database)
            } else {
                format!(
                    "postgresql://{}:{}@{}:{}/{}",
                    username, password, host, port, database
                )
            }
        } else {
            return Err(D::Error::custom(
                "Either 'url' or both 'host' and 'username' must be provided",
            ));
        };

        Ok(PostgresConfig {
            url,
            max_connections: helper.max_connections,
            schema: helper.schema,
            conflict_resolution: helper.conflict_resolution,
            auto_create_tables: helper.auto_create_tables,
            auto_add_columns: helper.auto_add_columns,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ConflictResolution {
    /// Use INSERT ... ON CONFLICT DO UPDATE (upsert)
    #[default]
    Upsert,
    /// Replace existing records
    Replace,
    /// Ignore conflicts
    Ignore,
}

impl Default for PostgresConfig {
    fn default() -> Self {
        Self {
            url: "postgresql://localhost/cdc".to_string(),
            max_connections: 10,
            schema: "public".to_string(),
            conflict_resolution: ConflictResolution::Upsert,
            auto_create_tables: true,
            auto_add_columns: true,
        }
    }
}

pub struct PostgresDestination {
    config: PostgresConfig,
    pool: Option<PgPool>,
    status: DestinationStatus,
}

impl PostgresDestination {
    pub fn new(config: PostgresConfig) -> Self {
        Self {
            config,
            pool: None,
            status: DestinationStatus::default(),
        }
    }

    /// Quote identifier if it contains uppercase letters or needs quoting
    fn quote_identifier(identifier: &str) -> String {
        // Check if identifier contains uppercase letters
        if identifier.chars().any(|c| c.is_uppercase()) {
            format!("\"{}\"", identifier)
        } else {
            identifier.to_string()
        }
    }

    /// Map etl::types type string to PostgreSQL type
    fn map_etl_type_to_postgres(etl_type: &str) -> String {
        match etl_type.to_lowercase().as_str() {
            "bool" => "BOOLEAN".to_string(),
            "int2" | "smallint" => "SMALLINT".to_string(),
            "int4" | "integer" | "int" => "INTEGER".to_string(),
            "int8" | "bigint" => "BIGINT".to_string(),
            "float4" | "real" => "REAL".to_string(),
            "float8" | "double precision" => "DOUBLE PRECISION".to_string(),
            "numeric" | "decimal" => "NUMERIC".to_string(),
            "text" => "TEXT".to_string(),
            "varchar" | "character varying" => "VARCHAR".to_string(),
            "char" | "character" => "CHAR".to_string(),
            "uuid" => "UUID".to_string(),
            "json" => "JSON".to_string(),
            "jsonb" => "JSONB".to_string(),
            "bytea" => "BYTEA".to_string(),
            "date" => "DATE".to_string(),
            "time" => "TIME".to_string(),
            "timestamp" => "TIMESTAMP".to_string(),
            "timestamptz" | "timestamp with time zone" => "TIMESTAMP WITH TIME ZONE".to_string(),
            _ => "TEXT".to_string(), // Default fallback
        }
    }

    /// Infer PostgreSQL type from JSON value and table metadata
    fn infer_postgres_type(
        column_name: &str,
        value: &serde_json::Value,
        table_metadata: Option<&cdc_core::TableMetadata>,
    ) -> String {
        // First, try to get type from table_metadata if available
        if let Some(metadata) = table_metadata {
            if let Some(column_schema) = metadata
                .column_schemas
                .iter()
                .find(|cs| cs.name == column_name)
            {
                // Found column schema, use the type from metadata
                info!(
                    "Using type from metadata for column '{}': {}",
                    column_name, column_schema.typ
                );
                return Self::map_etl_type_to_postgres(&column_schema.typ);
            }
        }

        // Fallback: infer from JSON value if metadata not available
        info!(
            "Inferring type from value for column '{}' (metadata not available)",
            column_name
        );
        match value {
            serde_json::Value::Null => "TEXT".to_string(),
            serde_json::Value::Bool(_) => "BOOLEAN".to_string(),
            serde_json::Value::Number(n) => {
                if n.is_i64() || n.is_u64() {
                    "BIGINT".to_string()
                } else {
                    "DOUBLE PRECISION".to_string()
                }
            }
            serde_json::Value::String(s) => {
                let trimmed = s.trim();

                // Check if it's a UUID (format: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx)
                if trimmed.len() == 36 && trimmed.chars().filter(|c| *c == '-').count() == 4 {
                    // Try to parse as UUID using sqlx's Uuid type
                    if trimmed.parse::<sqlx::types::Uuid>().is_ok() {
                        return "UUID".to_string();
                    }
                }

                // Check if string content is JSON (starts with { or [)
                if (trimmed.starts_with('{') && trimmed.ends_with('}'))
                    || (trimmed.starts_with('[') && trimmed.ends_with(']'))
                {
                    // Verify it's valid JSON by attempting to parse
                    if serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
                        return "JSONB".to_string();
                    }
                }
                "TEXT".to_string()
            }
            serde_json::Value::Array(_) => "JSONB".to_string(),
            serde_json::Value::Object(_) => "JSONB".to_string(),
        }
    }

    /// Ensure the schema metadata table exists
    async fn ensure_schema_metadata_table(&self, pool: &PgPool) -> Result<()> {
        let schema = Self::quote_identifier(&self.config.schema);
        let query = format!(
            "CREATE TABLE IF NOT EXISTS {}.\"_cdc_schema_metadata\" (
                schema_name TEXT NOT NULL,
                table_name TEXT NOT NULL,
                column_name TEXT NOT NULL,
                data_type TEXT NOT NULL,
                last_updated TIMESTAMP NOT NULL DEFAULT NOW(),
                PRIMARY KEY (schema_name, table_name, column_name)
            )",
            schema
        );

        sqlx::query(&query).execute(pool).await.map_err(|e| {
            Error::Generic(anyhow::anyhow!(
                "Failed to create schema metadata table: {}",
                e
            ))
        })?;

        info!("Schema metadata table ensured");
        Ok(())
    }

    /// Check if a table exists
    async fn table_exists(&self, pool: &PgPool, table: &str) -> Result<bool> {
        let query = "SELECT EXISTS (
            SELECT FROM information_schema.tables 
            WHERE table_schema = $1 AND table_name = $2
        )";

        let exists: (bool,) = sqlx::query_as(query)
            .bind(&self.config.schema)
            .bind(table)
            .fetch_one(pool)
            .await
            .map_err(|e| {
                Error::Generic(anyhow::anyhow!("Failed to check table existence: {}", e))
            })?;

        Ok(exists.0)
    }

    /// Get current table schema from information_schema
    async fn get_table_schema(
        &self,
        pool: &PgPool,
        table: &str,
    ) -> Result<std::collections::HashMap<String, String>> {
        let query = "SELECT column_name, data_type 
                     FROM information_schema.columns 
                     WHERE table_schema = $1 AND table_name = $2
                     ORDER BY ordinal_position";

        let rows: Vec<(String, String)> = sqlx::query_as(query)
            .bind(&self.config.schema)
            .bind(table)
            .fetch_all(pool)
            .await
            .map_err(|e| Error::Generic(anyhow::anyhow!("Failed to get table schema: {}", e)))?;

        Ok(rows.into_iter().collect())
    }

    /// Get cached schema from metadata table
    async fn get_cached_schema(
        &self,
        pool: &PgPool,
        table: &str,
    ) -> Result<std::collections::HashMap<String, String>> {
        let schema = Self::quote_identifier(&self.config.schema);
        let query = format!(
            "SELECT column_name, data_type 
             FROM {}.\"_cdc_schema_metadata\" 
             WHERE schema_name = $1 AND table_name = $2
             ORDER BY column_name",
            schema
        );

        let rows: Vec<(String, String)> = sqlx::query_as(&query)
            .bind(&self.config.schema)
            .bind(table)
            .fetch_all(pool)
            .await
            .map_err(|e| Error::Generic(anyhow::anyhow!("Failed to get cached schema: {}", e)))?;

        // If cache is empty, fallback to information_schema
        if rows.is_empty() {
            debug!(
                "Cache miss for table {}, fetching from information_schema",
                table
            );
            return self.get_table_schema(pool, table).await;
        }

        Ok(rows.into_iter().collect())
    }

    /// Update schema metadata cache
    async fn update_schema_metadata(
        &self,
        pool: &PgPool,
        table: &str,
        columns: &std::collections::HashMap<String, String>,
    ) -> Result<()> {
        let schema = Self::quote_identifier(&self.config.schema);

        // Delete existing metadata for this table
        let delete_query = format!(
            "DELETE FROM {}.\"_cdc_schema_metadata\" 
             WHERE schema_name = $1 AND table_name = $2",
            schema
        );
        sqlx::query(&delete_query)
            .bind(&self.config.schema)
            .bind(table)
            .execute(pool)
            .await
            .map_err(|e| {
                Error::Generic(anyhow::anyhow!(
                    "Failed to delete old schema metadata: {}",
                    e
                ))
            })?;

        // Insert new metadata
        for (column_name, data_type) in columns {
            let insert_query = format!(
                "INSERT INTO {}.\"_cdc_schema_metadata\" 
                 (schema_name, table_name, column_name, data_type, last_updated) 
                 VALUES ($1, $2, $3, $4, NOW())",
                schema
            );
            sqlx::query(&insert_query)
                .bind(&self.config.schema)
                .bind(table)
                .bind(column_name)
                .bind(data_type)
                .execute(pool)
                .await
                .map_err(|e| {
                    Error::Generic(anyhow::anyhow!("Failed to insert schema metadata: {}", e))
                })?;
        }

        Ok(())
    }

    /// Create a new table with columns inferred from data and metadata
    async fn create_table(
        &self,
        pool: &PgPool,
        table: &str,
        data: &std::collections::HashMap<String, serde_json::Value>,
        table_metadata: Option<&cdc_core::TableMetadata>,
    ) -> Result<()> {
        let schema = Self::quote_identifier(&self.config.schema);
        let table_quoted = Self::quote_identifier(table);

        // Build column definitions
        let mut column_defs = Vec::new();
        let mut column_types = std::collections::HashMap::new();

        for (col_name, col_value) in data {
            let col_type = Self::infer_postgres_type(col_name, col_value, table_metadata);
            let col_quoted = Self::quote_identifier(col_name);

            // Check if column is nullable from metadata
            let is_nullable = if let Some(metadata) = table_metadata {
                metadata
                    .column_schemas
                    .iter()
                    .find(|cs| cs.name.as_str() == col_name)
                    .map(|cs| cs.nullable)
                    .unwrap_or(true) // Default to nullable if not found
            } else {
                // If no metadata, check if current value is null
                col_value.is_null()
            };

            // Check for id column (case-insensitive) to set as PRIMARY KEY
            if col_name.to_lowercase() == "id" {
                // Primary keys are always NOT NULL
                column_defs.push(format!("{} {} PRIMARY KEY", col_quoted, col_type));
            } else {
                // Add NULL/NOT NULL constraint based on metadata
                let null_constraint = if is_nullable { "NULL" } else { "NOT NULL" };
                column_defs.push(format!("{} {} {}", col_quoted, col_type, null_constraint));
            }
            column_types.insert(col_name.clone(), col_type);
        }

        let columns_str = column_defs.join(", ");
        let query = format!("CREATE TABLE {}.{} ({})", schema, table_quoted, columns_str);

        info!("Creating table: {}", query);
        sqlx::query(&query)
            .execute(pool)
            .await
            .map_err(|e| Error::Generic(anyhow::anyhow!("Failed to create table: {}", e)))?;

        // Update metadata
        self.update_schema_metadata(pool, table, &column_types)
            .await?;

        info!("Table {} created successfully", table);
        Ok(())
    }

    /// Add new columns to an existing table
    async fn add_columns(
        &self,
        pool: &PgPool,
        table: &str,
        new_columns: Vec<(String, String)>,
    ) -> Result<()> {
        if new_columns.is_empty() {
            return Ok(());
        }

        let schema = Self::quote_identifier(&self.config.schema);
        let table_quoted = Self::quote_identifier(table);

        for (col_name, col_type) in &new_columns {
            let col_quoted = Self::quote_identifier(col_name);
            let query = format!(
                "ALTER TABLE {}.{} ADD COLUMN {} {}",
                schema, table_quoted, col_quoted, col_type
            );

            info!("Adding column: {}", query);
            sqlx::query(&query).execute(pool).await.map_err(|e| {
                Error::Generic(anyhow::anyhow!("Failed to add column {}: {}", col_name, e))
            })?;
        }

        // Update metadata with new columns
        let mut current_schema = self.get_table_schema(pool, table).await?;
        for (col_name, col_type) in new_columns {
            current_schema.insert(col_name, col_type);
        }
        self.update_schema_metadata(pool, table, &current_schema)
            .await?;

        info!(
            "Added {} new column(s) to table {}",
            current_schema.len(),
            table
        );
        Ok(())
    }

    /// Ensure table exists and has all required columns
    async fn ensure_table_exists(&self, pool: &PgPool, record: &DataRecord) -> Result<()> {
        let table = &record.table_metadata.name;
        let schema = &record.table_metadata.schema;
        if table.is_empty() || schema.is_empty() {
            return Err(Error::Generic(anyhow::anyhow!(
                "No table_name or schema in metadata"
            )));
        }

        // Parse record data
        let data = record
            .parse_record()
            .map_err(|e| Error::Generic(anyhow::anyhow!("Failed to parse record: {}", e)))?;

        // Check if table exists
        let exists = self.table_exists(pool, &table).await?;

        if !exists {
            // Table doesn't exist
            if self.config.auto_create_tables {
                info!("Table {} does not exist, creating it", table);
                self.create_table(pool, &table, &data, Some(&record.table_metadata))
                    .await?;
            } else {
                return Err(Error::Generic(anyhow::anyhow!(
                    "Table {} does not exist and auto_create_tables is disabled",
                    table
                )));
            }
        } else {
            // Table exists, check for missing columns
            if self.config.auto_add_columns {
                let current_schema = self.get_table_schema(pool, &table).await?;
                let mut missing_columns = Vec::new();

                for (col_name, col_value) in &data {
                    if !current_schema.contains_key(col_name) {
                        let col_type = Self::infer_postgres_type(
                            col_name,
                            col_value,
                            Some(&record.table_metadata),
                        );
                        missing_columns.push((col_name.clone(), col_type));
                    }
                }

                if !missing_columns.is_empty() {
                    info!(
                        "Detected {} missing column(s) in table {}",
                        missing_columns.len(),
                        table
                    );
                    self.add_columns(pool, &table, missing_columns).await?;
                }
            }
        }

        Ok(())
    }

    async fn insert_record<'e, E>(&self, executor: E, record: &DataRecord) -> Result<()>
    where
        E: sqlx::PgExecutor<'e>,
    {
        let schema = Self::quote_identifier(&self.config.schema);
        let table_name_str = record
            .table_name()
            .ok_or_else(|| Error::Generic(anyhow::anyhow!("No table_name in metadata")))?;
        let table = Self::quote_identifier(&table_name_str);
        let table_name = format!("{}.{}", schema, table);

        // Parse record data
        let data = record
            .parse_record()
            .map_err(|e| Error::Generic(anyhow::anyhow!("Failed to parse record: {}", e)))?;

        let operation = record.operation();
        info!("Executing operation {:?} on {}", operation, table_name);

        match operation {
            Operation::Insert | Operation::Snapshot | Operation::Update | Operation::Read => {
                // For all operations, use INSERT ON CONFLICT
                // For UPDATE, we'll merge changes into the full record

                let mut final_data = data.clone();

                // If it's an UPDATE, merge changes
                if matches!(operation, Operation::Update) {
                    if let Ok(Some(changes)) = record.parse_changes() {
                        info!("Merging {} changed fields for UPDATE", changes.len());
                        for (key, value) in changes {
                            final_data.insert(key, value);
                        }
                    }
                }

                // Apply default values for NULL/empty fields using schema metadata
                let pool = self
                    .pool
                    .as_ref()
                    .ok_or_else(|| Error::Connection("Not connected".to_string()))?;
                // self.apply_default_values(pool, &table_name_str, &mut final_data)
                //     .await?;

                // Get table schema to know column types
                let table_schema = self.get_table_schema(pool, &table_name_str).await?;

                // Extract column names and values
                let mut columns = Vec::new();
                let mut placeholders = Vec::new();
                let mut values: Vec<&serde_json::Value> = Vec::new();
                let mut column_names_for_binding: Vec<String> = Vec::new();
                let mut update_sets = Vec::new();
                let mut pk_column = None;

                for (i, (key, value)) in final_data.iter().enumerate() {
                    let quoted = Self::quote_identifier(key);
                    columns.push(quoted.clone());

                    // Check if this column needs type casting based on schema
                    let column_type = table_schema.get(key).map(|s| s.as_str());
                    let placeholder = match column_type {
                        Some("timestamp without time zone") => format!("${}::timestamp", i + 1),
                        Some("timestamp with time zone") | Some("timestamptz") => {
                            format!("${}::timestamptz", i + 1)
                        }
                        Some("date") => format!("${}::date", i + 1),
                        Some("time") | Some("time without time zone") => {
                            format!("${}::time", i + 1)
                        }
                        _ => format!("${}", i + 1),
                    };

                    placeholders.push(placeholder);
                    values.push(value);
                    column_names_for_binding.push(key.clone());

                    if key.to_lowercase() == "id" {
                        pk_column = Some(quoted);
                    } else {
                        update_sets.push(format!("{} = EXCLUDED.{}", quoted, quoted));
                    }
                }

                let pk_column = pk_column.unwrap_or_else(|| Self::quote_identifier("id"));
                let columns_str = columns.join(", ");
                let placeholders_str = placeholders.join(", ");

                let query = match self.config.conflict_resolution {
                    ConflictResolution::Upsert => {
                        format!(
                            "INSERT INTO {} ({}) VALUES ({}) ON CONFLICT ({}) DO UPDATE SET {}",
                            table_name,
                            columns_str,
                            placeholders_str,
                            pk_column,
                            update_sets.join(", ")
                        )
                    }
                    ConflictResolution::Ignore => {
                        format!(
                            "INSERT INTO {} ({}) VALUES ({}) ON CONFLICT ({}) DO NOTHING",
                            table_name, columns_str, placeholders_str, pk_column
                        )
                    }
                    ConflictResolution::Replace => {
                        format!(
                            "INSERT INTO {} ({}) VALUES ({})",
                            table_name, columns_str, placeholders_str
                        )
                    }
                };

                info!("Executing upsert query: {}", query);
                self.execute_query_with_schema(
                    executor,
                    &query,
                    &values,
                    &column_names_for_binding,
                    &table_schema,
                )
                .await?;
            }
            Operation::Delete => {
                // For DELETE, extract ID and delete
                let pk_value = data.get("id").or_else(|| data.get("Id"));

                if let Some(val) = pk_value {
                    let pk_column = Self::quote_identifier("id");
                    let query = format!("DELETE FROM {} WHERE {} = $1", table_name, pk_column);
                    debug!("Executing delete query: {}", query);
                    self.execute_query(executor, &query, &[val]).await?;
                } else {
                    warn!(
                        "Cannot delete record without ID column in table {}",
                        table_name
                    );
                }
            }
        }

        Ok(())
    }

    /// Apply default values for NULL or empty fields based on actual column types from schema
    async fn apply_default_values(
        &self,
        pool: &PgPool,
        table: &str,
        data: &mut std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        // Fetch schema from metadata cache
        let schema = self.get_cached_schema(pool, table).await?;

        let keys: Vec<String> = data.keys().cloned().collect();

        for key in keys {
            let value = data.get(&key).unwrap();

            // Determine if we should apply defaults
            let needs_default = value.is_null()
                || (value.is_string() && value.as_str().unwrap_or("").trim().is_empty());

            if !needs_default {
                continue;
            }

            // Get column type from schema
            let column_type = schema.get(&key);

            if let Some(data_type) = column_type {
                // Use actual data type to generate default
                let default_value = Self::get_default_value_for_type(data_type);
                info!(
                    "Applying default for column '{}' (type: {}): {}",
                    key, data_type, default_value
                );
                data.insert(key.clone(), default_value);
            } else {
                // Column not in schema, use heuristic fallback
                warn!(
                    "Column '{}' not found in schema, using heuristic default",
                    key
                );
                let default_value =
                    if key.to_lowercase().ends_with("id") || key.to_lowercase().ends_with("by") {
                        let uuid = sqlx::types::Uuid::new_v4();
                        serde_json::Value::String(uuid.to_string())
                    } else {
                        serde_json::Value::String(String::new())
                    };
                data.insert(key.clone(), default_value);
            }
        }

        Ok(())
    }

    /// Get default value for a PostgreSQL data type
    fn get_default_value_for_type(data_type: &str) -> serde_json::Value {
        match data_type.to_lowercase().as_str() {
            "uuid" => {
                let uuid = sqlx::types::Uuid::new_v4();
                serde_json::Value::String(uuid.to_string())
            }
            "integer" | "int" | "int4" | "smallint" | "int2" => {
                serde_json::Value::Number(serde_json::Number::from(0))
            }
            "bigint" | "int8" => serde_json::Value::Number(serde_json::Number::from(0i64)),
            "real" | "float4" | "double precision" | "float8" | "numeric" | "decimal" => {
                serde_json::Value::Number(serde_json::Number::from_f64(0.0).unwrap())
            }
            "boolean" | "bool" => serde_json::Value::Bool(false),
            "text" | "varchar" | "character varying" | "char" | "character" => {
                serde_json::Value::String(String::new())
            }
            "jsonb" | "json" => serde_json::json!({}),
            "timestamp"
            | "timestamptz"
            | "timestamp with time zone"
            | "timestamp without time zone"
            | "date"
            | "time" => serde_json::Value::Null,
            _ => {
                // Default to empty string for unknown types
                serde_json::Value::String(String::new())
            }
        }
    }

    async fn execute_query_with_schema<'e, E>(
        &self,
        executor: E,
        query: &str,
        values: &[&serde_json::Value],
        column_names: &[String],
        table_schema: &std::collections::HashMap<String, String>,
    ) -> Result<()>
    where
        E: sqlx::PgExecutor<'e>,
    {
        let mut query_builder = sqlx::query(query);

        for (idx, value) in values.iter().enumerate() {
            let column_name = column_names.get(idx).map(|s| s.as_str()).unwrap_or("");
            let column_type = table_schema.get(column_name).map(|s| s.as_str());

            info!(
                "Binding value for column '{}' (type: {:?}): {}",
                column_name, column_type, value
            );

            // Use column type from schema to determine how to bind
            match column_type {
                Some("integer") | Some("smallint") | Some("bigint") => {
                    // Integer types
                    match *value {
                        serde_json::Value::Number(n) => {
                            if let Some(i) = n.as_i64() {
                                query_builder = query_builder.bind(i);
                            } else {
                                query_builder = query_builder.bind(0i64);
                            }
                        }
                        serde_json::Value::String(s) => {
                            let trimmed = s.trim();
                            if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("null") {
                                query_builder = query_builder.bind(None::<i64>);
                            } else if let Ok(i) = trimmed.parse::<i64>() {
                                query_builder = query_builder.bind(i);
                            } else {
                                warn!(
                                    "Cannot parse '{}' as integer for column '{}', binding NULL",
                                    trimmed, column_name
                                );
                                query_builder = query_builder.bind(None::<i64>);
                            }
                        }
                        serde_json::Value::Null => {
                            query_builder = query_builder.bind(None::<i64>);
                        }
                        _ => {
                            query_builder = query_builder.bind(None::<i64>);
                        }
                    }
                }
                Some("numeric") | Some("decimal") | Some("real") | Some("double precision") => {
                    // Numeric/float types
                    match *value {
                        serde_json::Value::Number(n) => {
                            if let Some(f) = n.as_f64() {
                                query_builder = query_builder.bind(f);
                            } else {
                                query_builder = query_builder.bind(0.0f64);
                            }
                        }
                        serde_json::Value::String(s) => {
                            let trimmed = s.trim();
                            if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("null") {
                                query_builder = query_builder.bind(None::<f64>);
                            } else if let Ok(f) = trimmed.parse::<f64>() {
                                query_builder = query_builder.bind(f);
                            } else {
                                warn!(
                                    "Cannot parse '{}' as float for column '{}', binding NULL",
                                    trimmed, column_name
                                );
                                query_builder = query_builder.bind(None::<f64>);
                            }
                        }
                        serde_json::Value::Null => {
                            query_builder = query_builder.bind(None::<f64>);
                        }
                        _ => {
                            query_builder = query_builder.bind(None::<f64>);
                        }
                    }
                }
                Some("boolean") => {
                    // Boolean type
                    match *value {
                        serde_json::Value::Bool(b) => {
                            query_builder = query_builder.bind(*b);
                        }
                        serde_json::Value::String(s) => {
                            let trimmed = s.trim();
                            if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("null") {
                                query_builder = query_builder.bind(None::<bool>);
                            } else {
                                let bool_val = trimmed.eq_ignore_ascii_case("true")
                                    || trimmed.eq_ignore_ascii_case("1");
                                query_builder = query_builder.bind(bool_val);
                            }
                        }
                        serde_json::Value::Null => {
                            query_builder = query_builder.bind(None::<bool>);
                        }
                        _ => {
                            query_builder = query_builder.bind(false);
                        }
                    }
                }
                Some("uuid") => {
                    // UUID type
                    match *value {
                        serde_json::Value::String(s) => {
                            let trimmed = s.trim();
                            if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("null") {
                                query_builder = query_builder.bind(None::<sqlx::types::Uuid>);
                            } else if let Ok(uuid) = trimmed.parse::<sqlx::types::Uuid>() {
                                query_builder = query_builder.bind(uuid);
                            } else {
                                warn!(
                                    "Cannot parse '{}' as UUID for column '{}', binding NULL",
                                    trimmed, column_name
                                );
                                query_builder = query_builder.bind(None::<sqlx::types::Uuid>);
                            }
                        }
                        serde_json::Value::Null => {
                            query_builder = query_builder.bind(None::<sqlx::types::Uuid>);
                        }
                        _ => {
                            query_builder = query_builder.bind(None::<sqlx::types::Uuid>);
                        }
                    }
                }
                Some("json") | Some("jsonb") => {
                    // JSON type
                    query_builder = query_builder.bind((*value).clone());
                }
                Some("timestamp without time zone")
                | Some("timestamp")
                | Some("timestamp with time zone")
                | Some("timestamptz")
                | Some("date")
                | Some("time") => {
                    // Timestamp/Date/Time types - bind as string and let PostgreSQL parse
                    match *value {
                        serde_json::Value::String(s) => {
                            let trimmed = s.trim();
                            if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("null") {
                                query_builder = query_builder.bind(None::<String>);
                            } else {
                                // Let PostgreSQL handle the timestamp parsing
                                query_builder = query_builder.bind(s.clone());
                            }
                        }
                        serde_json::Value::Null => {
                            query_builder = query_builder.bind(None::<String>);
                        }
                        _ => {
                            // Convert to string and let PostgreSQL parse
                            query_builder = query_builder.bind(value.to_string());
                        }
                    }
                }
                _ => {
                    // Default: text/string types or unknown
                    match *value {
                        serde_json::Value::String(s) => {
                            let trimmed = s.trim();
                            if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("null") {
                                query_builder = query_builder.bind(None::<String>);
                            } else {
                                query_builder = query_builder.bind(s.clone());
                            }
                        }
                        serde_json::Value::Null => {
                            query_builder = query_builder.bind(None::<String>);
                        }
                        _ => {
                            query_builder = query_builder.bind(value.to_string());
                        }
                    }
                }
            }
        }

        query_builder
            .execute(executor)
            .await
            .map_err(|e| Error::Generic(anyhow::anyhow!("Database error: {}", e)))?;

        Ok(())
    }

    async fn execute_query<'e, E>(
        &self,
        executor: E,
        query: &str,
        values: &[&serde_json::Value],
    ) -> Result<()>
    where
        E: sqlx::PgExecutor<'e>,
    {
        let mut query_builder = sqlx::query(query);

        for value in values {
            info!("Binding value: {}", value);
            match *value {
                serde_json::Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        query_builder = query_builder.bind(i);
                    } else if let Some(f) = n.as_f64() {
                        query_builder = query_builder.bind(f);
                    } else {
                        query_builder = query_builder.bind((*value).clone());
                    }
                }
                serde_json::Value::String(s) => {
                    let trimmed = s.trim();

                    // Check if string is empty or literally "null" - bind as NULL
                    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("null") {
                        info!("Binding empty or 'null' string as SQL NULL");
                        query_builder = query_builder.bind(None::<String>);
                        continue;
                    }

                    // Check if it's a UUID first
                    if trimmed.len() == 36 && trimmed.chars().filter(|c| *c == '-').count() == 4 {
                        if let Ok(uuid_val) = trimmed.parse::<sqlx::types::Uuid>() {
                            query_builder = query_builder.bind(uuid_val);
                            continue;
                        }
                    }

                    // Try to parse as numeric (integer or float) before treating as string
                    // This handles cases where numeric values are serialized as strings
                    if let Ok(int_val) = trimmed.parse::<i64>() {
                        info!("Parsing string '{}' as i64: {}", trimmed, int_val);
                        query_builder = query_builder.bind(int_val);
                        continue;
                    }

                    if let Ok(float_val) = trimmed.parse::<f64>() {
                        info!("Parsing string '{}' as f64: {}", trimmed, float_val);
                        query_builder = query_builder.bind(float_val);
                        continue;
                    }

                    // Check if it's JSON
                    if (trimmed.starts_with('{') && trimmed.ends_with('}'))
                        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
                    {
                        if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                            query_builder = query_builder.bind(json_val);
                        } else {
                            query_builder = query_builder.bind(s.clone());
                        }
                    } else {
                        query_builder = query_builder.bind(s.clone());
                    }
                }
                serde_json::Value::Bool(b) => {
                    query_builder = query_builder.bind(*b);
                }
                serde_json::Value::Null => {
                    query_builder = query_builder.bind(None::<String>);
                }
                _ => {
                    query_builder = query_builder.bind((*value).clone());
                }
            }
        }

        query_builder
            .execute(executor)
            .await
            .map_err(|e| Error::Generic(anyhow::anyhow!("Database error: {}", e)))?;

        Ok(())
    }
}

#[async_trait]
impl Destination for PostgresDestination {
    async fn connect(&mut self) -> Result<()> {
        info!("Connecting to PostgreSQL: {}", self.config.url);

        let pool = PgPoolOptions::new()
            .max_connections(self.config.max_connections)
            .connect(&self.config.url)
            .await
            .map_err(|e| Error::Connection(format!("Failed to connect to PostgreSQL: {}", e)))?;

        info!("Connected to PostgreSQL successfully");

        // Ensure schema metadata table exists
        self.ensure_schema_metadata_table(&pool).await?;

        self.pool = Some(pool);
        self.status.connected = true;

        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        info!("Disconnecting from PostgreSQL");

        if let Some(pool) = self.pool.take() {
            pool.close().await;
        }

        self.status.connected = false;
        info!("Disconnected from PostgreSQL");

        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.status.connected
    }

    async fn write(&mut self, record: DataRecord) -> Result<()> {
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| Error::Connection("Not connected".to_string()))?;

        // Ensure table exists and has all required columns
        self.ensure_table_exists(pool, &record).await?;

        match self.insert_record(pool, &record).await {
            Ok(_) => {
                self.status.records_written += 1;
                Ok(())
            }
            Err(e) => {
                self.status.errors += 1;
                self.status.last_error = Some(e.to_string());
                error!("Failed to write record: {}", e);
                Err(e)
            }
        }
    }

    async fn write_batch(&mut self, records: Vec<DataRecord>) -> Result<()> {
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| Error::Connection("Not connected".to_string()))?;

        // Ensure all unique tables exist before processing batch
        let mut processed_tables = std::collections::HashSet::new();
        for record in &records {
            if let Some(table_name) = record.table_name() {
                if !processed_tables.contains(&table_name) {
                    self.ensure_table_exists(pool, record).await?;
                    processed_tables.insert(table_name);
                }
            }
        }

        let mut transaction = pool
            .begin()
            .await
            .map_err(|e| Error::Generic(anyhow::anyhow!("Failed to begin transaction: {}", e)))?;

        for record in &records {
            if let Err(e) = self.insert_record(&mut *transaction, record).await {
                let table_name = record.table_name().unwrap_or_else(|| "unknown".to_string());
                let operation = record.operation();
                error!(
                    "Failed to write record to table '{}' (operation: {:?}): {}",
                    table_name, operation, e
                );
                self.status.errors += 1;
                self.status.last_error = Some(e.to_string());

                transaction
                    .rollback()
                    .await
                    .map_err(|e| Error::Generic(anyhow::anyhow!("Failed to rollback: {}", e)))?;

                return Err(e);
            }
        }

        transaction
            .commit()
            .await
            .map_err(|e| Error::Generic(anyhow::anyhow!("Failed to commit transaction: {}", e)))?;

        self.status.records_written += records.len() as u64;
        info!("Successfully wrote batch of {} records", records.len());

        Ok(())
    }

    fn status(&self) -> DestinationStatus {
        self.status.clone()
    }
}
