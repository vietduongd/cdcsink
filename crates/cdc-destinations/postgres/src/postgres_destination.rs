use async_trait::async_trait;
use cdc_core::{DataRecord, Destination, DestinationStatus, Error, Operation, Result};
use serde::{Deserialize, Serialize};
use sqlx::postgres::{PgPool, PgPoolOptions};
use tracing::{debug, error, info};

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

    /// Infer PostgreSQL type from JSON value
    fn infer_postgres_type(value: &serde_json::Value) -> String {
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
                // Check if string content is JSON (starts with { or [)
                let trimmed = s.trim();
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

    /// Create a new table with columns inferred from data
    async fn create_table(
        &self,
        pool: &PgPool,
        table: &str,
        data: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let schema = Self::quote_identifier(&self.config.schema);
        let table_quoted = Self::quote_identifier(table);

        // Build column definitions
        let mut column_defs = Vec::new();
        let mut column_types = std::collections::HashMap::new();

        for (col_name, col_value) in data {
            let col_type = Self::infer_postgres_type(col_value);
            let col_quoted = Self::quote_identifier(col_name);
            column_defs.push(format!("{} {}", col_quoted, col_type));
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
        let table = &record.table;

        // Check if table exists
        let exists = self.table_exists(pool, table).await?;

        if !exists {
            // Table doesn't exist
            if self.config.auto_create_tables {
                info!("Table {} does not exist, creating it", table);
                self.create_table(pool, table, &record.data).await?;
            } else {
                return Err(Error::Generic(anyhow::anyhow!(
                    "Table {} does not exist and auto_create_tables is disabled",
                    table
                )));
            }
        } else {
            // Table exists, check for missing columns
            if self.config.auto_add_columns {
                let current_schema = self.get_table_schema(pool, table).await?;
                let mut missing_columns = Vec::new();

                for (col_name, col_value) in &record.data {
                    if !current_schema.contains_key(col_name) {
                        let col_type = Self::infer_postgres_type(col_value);
                        missing_columns.push((col_name.clone(), col_type));
                    }
                }

                if !missing_columns.is_empty() {
                    info!(
                        "Detected {} missing column(s) in table {}",
                        missing_columns.len(),
                        table
                    );
                    self.add_columns(pool, table, missing_columns).await?;
                }
            }
        }

        Ok(())
    }

    async fn insert_record(&self, pool: &PgPool, record: &DataRecord) -> Result<()> {
        let schema = Self::quote_identifier(&self.config.schema);
        let table = Self::quote_identifier(&record.table);
        let table_name = format!("{}.{}", schema, table);

        // Extract column names and values
        let mut columns = Vec::new();
        let mut placeholders = Vec::new();
        let mut values: Vec<&serde_json::Value> = Vec::new();

        for (i, (key, value)) in record.data.iter().enumerate() {
            columns.push(Self::quote_identifier(key));
            placeholders.push(format!("${}", i + 1));
            values.push(value);
        }

        let columns_str = columns.join(", ");
        let placeholders_str = placeholders.join(", ");

        match record.operation {
            Operation::Insert | Operation::Snapshot => {
                let query = match self.config.conflict_resolution {
                    ConflictResolution::Upsert => {
                        format!(
                            "INSERT INTO {} ({}) VALUES ({}) ON CONFLICT DO UPDATE SET {}",
                            table_name,
                            columns_str,
                            placeholders_str,
                            columns
                                .iter()
                                .map(|c| format!("{} = EXCLUDED.{}", c, c))
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    }
                    ConflictResolution::Ignore => {
                        format!(
                            "INSERT INTO {} ({}) VALUES ({}) ON CONFLICT DO NOTHING",
                            table_name, columns_str, placeholders_str
                        )
                    }
                    ConflictResolution::Replace => {
                        format!(
                            "INSERT INTO {} ({}) VALUES ({})",
                            table_name, columns_str, placeholders_str
                        )
                    }
                };

                debug!("Executing insert query: {}", query);
                self.execute_query(pool, &query, &values).await?;
            }
            Operation::Update => {
                let set_clause = columns
                    .iter()
                    .enumerate()
                    .map(|(i, c)| format!("{} = ${}", c, i + 1))
                    .collect::<Vec<_>>()
                    .join(", ");

                let query = format!("UPDATE {} SET {}", table_name, set_clause);
                debug!("Executing update query: {}", query);
                self.execute_query(pool, &query, &values).await?;
            }
            Operation::Delete => {
                // For delete, we expect a WHERE clause from metadata
                let query = format!("DELETE FROM {}", table_name);
                debug!("Executing delete query: {}", query);
                self.execute_query(pool, &query, &values).await?;
            }
        }

        Ok(())
    }

    async fn execute_query(
        &self,
        pool: &PgPool,
        query: &str,
        values: &[&serde_json::Value],
    ) -> Result<()> {
        let mut query_builder = sqlx::query(query);

        for value in values {
            query_builder = query_builder.bind(value);
        }

        query_builder
            .execute(pool)
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
            if !processed_tables.contains(&record.table) {
                self.ensure_table_exists(pool, record).await?;
                processed_tables.insert(record.table.clone());
            }
        }

        let transaction = pool
            .begin()
            .await
            .map_err(|e| Error::Generic(anyhow::anyhow!("Failed to begin transaction: {}", e)))?;

        for record in &records {
            if let Err(e) = self.insert_record(pool, record).await {
                error!("Failed to write record in batch: {}", e);
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
