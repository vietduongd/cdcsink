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
}

fn default_max_connections() -> u32 {
    10
}

fn default_schema() -> String {
    "public".to_string()
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
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConflictResolution {
    /// Use INSERT ... ON CONFLICT DO UPDATE (upsert)
    Upsert,
    /// Replace existing records
    Replace,
    /// Ignore conflicts
    Ignore,
}

impl Default for ConflictResolution {
    fn default() -> Self {
        ConflictResolution::Upsert
    }
}

impl Default for PostgresConfig {
    fn default() -> Self {
        Self {
            url: "postgresql://localhost/cdc".to_string(),
            max_connections: 10,
            schema: "public".to_string(),
            conflict_resolution: ConflictResolution::Upsert,
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

    async fn insert_record(&self, pool: &PgPool, record: &DataRecord) -> Result<()> {
        let table_name = format!("{}.{}", self.config.schema, record.table);

        // Extract column names and values
        let mut columns = Vec::new();
        let mut placeholders = Vec::new();
        let mut values: Vec<&serde_json::Value> = Vec::new();

        for (i, (key, value)) in record.data.iter().enumerate() {
            columns.push(key.clone());
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
