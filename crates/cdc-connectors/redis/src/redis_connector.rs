use ::async_trait::async_trait;
use cdc_core::{Connector, ConnectorStatus, DataRecord, Error, Result};
use redis::{aio::MultiplexedConnection, AsyncCommands, Client, ConnectionInfo};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use tracing::{debug, error, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// Redis connection URL (e.g., redis://localhost:6379)
    #[serde(default = "default_url")]
    pub url: String,

    /// Stream key to read from
    #[serde(default = "default_stream_key")]
    pub stream_key: String,

    /// Consumer group name
    #[serde(default = "default_consumer_group")]
    pub consumer_group: String,

    /// Consumer name (unique per instance)
    #[serde(default = "default_consumer_name")]
    pub consumer_name: String,

    /// Block time in milliseconds for XREADGROUP
    #[serde(default = "default_block_time")]
    pub block_time_ms: u64,

    /// Number of messages to read per batch
    #[serde(default = "default_count")]
    pub count: usize,

    /// Redis database number
    #[serde(default = "default_database")]
    pub database: i64,
}

fn default_url() -> String {
    "redis://localhost:6379".to_string()
}

fn default_stream_key() -> String {
    "cdc_events".to_string()
}

fn default_consumer_group() -> String {
    "cdc_group".to_string()
}

fn default_consumer_name() -> String {
    "cdc_consumer".to_string()
}

fn default_block_time() -> u64 {
    100 // 100ms
}

fn default_count() -> usize {
    10
}

fn default_database() -> i64 {
    0
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://localhost:6379".to_string(),
            stream_key: "cdc_events".to_string(),
            consumer_group: "cdc_group".to_string(),
            consumer_name: "cdc_consumer".to_string(),
            block_time_ms: 100,
            count: 10,
            database: 0,
        }
    }
}

pub struct RedisConnector {
    config: RedisConfig,
    connection: Option<MultiplexedConnection>,
    status: ConnectorStatus,
    last_id: String,
    buffer: VecDeque<DataRecord>,
}

impl RedisConnector {
    pub fn new(config: RedisConfig) -> Self {
        Self {
            config,
            connection: None,
            status: ConnectorStatus::default(),
            last_id: ">".to_string(), // Start from new messages
            buffer: VecDeque::new(),
        }
    }

    /// Ensure consumer group exists
    async fn ensure_consumer_group(&self, conn: &mut MultiplexedConnection) -> Result<()> {
        // Try to create consumer group, ignore error if it already exists
        let result: redis::RedisResult<String> = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(&self.config.stream_key)
            .arg(&self.config.consumer_group)
            .arg("0") // Start from beginning
            .arg("MKSTREAM") // Create stream if not exists
            .query_async(conn)
            .await;

        match result {
            Ok(_) => {
                info!(
                    "Created consumer group '{}' for stream '{}'",
                    self.config.consumer_group, self.config.stream_key
                );
                Ok(())
            }
            Err(e) => {
                let err_msg = e.to_string();
                if err_msg.contains("BUSYGROUP") {
                    // Group already exists, this is fine
                    debug!(
                        "Consumer group '{}' already exists",
                        self.config.consumer_group
                    );
                    Ok(())
                } else {
                    error!("Failed to create consumer group: {}", e);
                    Err(Error::Connection(format!(
                        "Failed to create consumer group: {}",
                        e
                    )))
                }
            }
        }
    }

    /// Parse Redis stream entry into DataRecord
    fn parse_stream_entry(entry: &redis::streams::StreamId) -> Result<DataRecord> {
        let map = &entry.map;

        // Extract fields from stream entry
        let record_str = map
            .get("record")
            .and_then(|v| match v {
                redis::Value::Data(bytes) => String::from_utf8(bytes.clone()).ok(),
                _ => None,
            })
            .ok_or_else(|| Error::Generic(anyhow::anyhow!("Missing 'record' field")))?;

        let metadata_str = map
            .get("metadata")
            .and_then(|v| match v {
                redis::Value::Data(bytes) => String::from_utf8(bytes.clone()).ok(),
                _ => None,
            })
            .ok_or_else(|| Error::Generic(anyhow::anyhow!("Missing 'metadata' field")))?;

        let action = map
            .get("action")
            .and_then(|v| match v {
                redis::Value::Data(bytes) => String::from_utf8(bytes.clone()).ok(),
                _ => None,
            })
            .ok_or_else(|| Error::Generic(anyhow::anyhow!("Missing 'action' field")))?;

        let changes_str = map.get("changes").and_then(|v| match v {
            redis::Value::Data(bytes) => String::from_utf8(bytes.clone()).ok(),
            _ => None,
        });

        // Parse strings to JSON values
        let record = serde_json::from_str(&record_str).map_err(|e| Error::Serialization(e))?;
        let metadata = serde_json::from_str(&metadata_str).map_err(|e| Error::Serialization(e))?;
        let changes = if let Some(changes_s) = changes_str {
            Some(serde_json::from_str(&changes_s).map_err(|e| Error::Serialization(e))?)
        } else {
            None
        };

        Ok(DataRecord::new(record, metadata, action, changes))
    }
}

#[async_trait]
impl Connector for RedisConnector {
    async fn connect(&mut self) -> Result<()> {
        info!(
            "Connecting to Redis: {} (db={})",
            self.config.url, self.config.database
        );

        let mut connection_info: ConnectionInfo = Client::open(self.config.url.as_str())
            .map_err(|e| Error::Connection(format!("Invalid Redis URL: {}", e)))?
            .get_connection_info()
            .clone();

        // Override database if specified
        connection_info.redis.db = self.config.database;

        let client = Client::open(connection_info)
            .map_err(|e| Error::Connection(format!("Failed to create Redis client: {}", e)))?;

        let mut conn = client
            .get_multiplexed_tokio_connection()
            .await
            .map_err(|e| Error::Connection(format!("Failed to connect to Redis: {}", e)))?;

        info!("Connected to Redis successfully");

        // Ensure consumer group exists
        self.ensure_consumer_group(&mut conn).await?;

        self.connection = Some(conn);
        self.status.connected = true;

        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        info!("Disconnecting from Redis");
        self.connection = None;
        self.status.connected = false;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.status.connected
    }

    async fn receive(&mut self) -> Result<Option<DataRecord>> {
        // 1. Return from buffer if available
        if let Some(record) = self.buffer.pop_front() {
            return Ok(Some(record));
        }

        // 2. Ensure connection
        if self.connection.is_none() {
            debug!("Connection lost, attempting to reconnect...");
            self.connect().await?;
        }

        let conn = self
            .connection
            .as_mut()
            .ok_or_else(|| Error::Connection("Not connected".to_string()))?;

        // 3. Read from Redis
        let mut cmd = redis::cmd("XREADGROUP");
        cmd.arg("GROUP")
            .arg(&self.config.consumer_group)
            .arg(&self.config.consumer_name)
            .arg("COUNT")
            .arg(self.config.count);

        if self.config.block_time_ms > 0 {
            cmd.arg("BLOCK").arg(self.config.block_time_ms);
        }

        cmd.arg("STREAMS")
            .arg(&self.config.stream_key)
            .arg(&self.last_id);

        let result: redis::RedisResult<redis::streams::StreamReadReply> =
            cmd.query_async(conn).await;

        match result {
            Ok(reply) => {
                if reply.keys.is_empty() {
                    return Ok(None);
                }

                for stream_key in &reply.keys {
                    for entry in &stream_key.ids {
                        let entry_id = entry.id.clone();

                        match Self::parse_stream_entry(entry) {
                            Ok(record) => {
                                // Acknowledge the message
                                let _: redis::RedisResult<i32> = conn
                                    .xack(
                                        &self.config.stream_key,
                                        &self.config.consumer_group,
                                        &[&entry_id],
                                    )
                                    .await;

                                // Only update last_id if we are not in "new messages" mode (>)
                                if self.last_id != ">" {
                                    self.last_id = entry_id;
                                }

                                self.status.records_received += 1;
                                self.buffer.push_back(record);
                            }
                            Err(e) => {
                                error!("Failed to parse stream entry {}: {}", entry_id, e);
                                self.status.errors += 1;
                                self.status.last_error = Some(e.to_string());
                                continue;
                            }
                        }
                    }
                }

                Ok(self.buffer.pop_front())
            }
            Err(e) => {
                error!("Failed to read from Redis stream: {} (connection reset)", e);
                self.status.errors += 1;
                self.status.last_error = Some(e.to_string());

                self.connection = None;
                self.status.connected = false;

                Err(Error::Connection(format!(
                    "Failed to read from stream: {}",
                    e
                )))
            }
        }
    }

    fn status(&self) -> ConnectorStatus {
        self.status.clone()
    }
}
