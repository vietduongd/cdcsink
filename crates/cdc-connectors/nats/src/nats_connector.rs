use std::str::from_utf8;

use async_nats::jetstream::consumer::pull::Stream;
use async_nats::jetstream::consumer::PullConsumer;
use async_nats::{jetstream, Client, Subscriber};
use async_trait::async_trait;
use cdc_core::{Connector, ConnectorStatus, DataRecord, Error, Result};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NatsConfig {
    /// NATS server URL(s)
    pub servers: Vec<String>,

    /// Subject to subscribe to
    pub subject: String,

    /// Optional consumer group name
    pub consumer_group: Option<String>,

    /// Optional consumer name (durable name for JetStream)
    pub consumer_name: Option<String>,

    /// Use JetStream (for guaranteed delivery)
    pub use_jetstream: bool,

    /// Optional username for authentication
    #[serde(default)]
    pub username: Option<String>,

    /// Optional password for authentication
    #[serde(default)]
    pub password: Option<String>,

    /// Optional token for authentication
    #[serde(default)]
    pub token: Option<String>,
}

impl Default for NatsConfig {
    fn default() -> Self {
        Self {
            servers: vec!["nats://localhost:4222".to_string()],
            subject: "cdc.events".to_string(),
            consumer_group: None,
            consumer_name: None,
            use_jetstream: false,
            username: None,
            password: None,
            token: None,
        }
    }
}

pub struct NatsConnector {
    config: NatsConfig,
    client: Option<Client>,
    subscriber: Option<Subscriber>,
    consumer: Option<PullConsumer>,
    message_stream: Option<Stream>,
    status: ConnectorStatus,
}

impl NatsConnector {
    pub fn new(config: NatsConfig) -> Self {
        Self {
            config,
            client: None,
            subscriber: None,
            status: ConnectorStatus::default(),
            consumer: None,
            message_stream: None,
        }
    }
}

#[async_trait]
impl Connector for NatsConnector {
    async fn connect(&mut self) -> Result<()> {
        info!("Connecting to NATS servers: {:?}", self.config.servers);

        // Build connection options with authentication if provided
        let mut opts = async_nats::ConnectOptions::new();

        if let Some(ref username) = self.config.username {
            if let Some(ref password) = self.config.password {
                info!("Using username/password authentication");
                opts = opts.user_and_password(username.clone(), password.clone());
            }
        } else if let Some(ref token) = self.config.token {
            info!("Using token authentication");
            opts = opts.token(token.clone());
        }

        let client = opts
            .connect(&self.config.servers[0])
            .await
            .map_err(|e| Error::Connection(format!("Failed to connect to NATS: {}", e)))?;

        info!("Connected to NATS successfully");

        // IMPORTANT: Store client to keep connection alive!
        self.client = Some(client.clone());

        if self.config.use_jetstream {
            let jetstreams = jetstream::new(client.clone());
            let consumer_group = self.config.consumer_group.clone().unwrap_or_default();
            let stream = jetstreams
                .get_or_create_stream(jetstream::stream::Config {
                    name: consumer_group,
                    subjects: vec![self.config.subject.clone()], // Listen to all 'orders.*' subjects
                    max_messages: 10_000,                        // Limit stream size
                    ..Default::default()
                })
                .await
                .map_err(|e| Error::Connection(format!("Failed to create stream: {}", e)))?;
            let consumer_same = self.config.consumer_name.clone().unwrap_or_default();
            info!("Stream created successfully with : {}", consumer_same);
            let consumer: PullConsumer = stream
                .create_consumer(jetstream::consumer::pull::Config {
                    durable_name: self.config.consumer_name.clone(),
                    ..Default::default()
                })
                .await
                .map_err(|e| Error::Connection(format!("Failed to create consumer: {}", e)))?;
            let messages = consumer
                .messages()
                .await
                .map_err(|e| Error::Connection(format!("Failed to get messages: {}", e)))?;
            self.message_stream = Some(messages);
            self.consumer = Some(consumer);
        } else {
            // Subscribe to subject
            let subscriber = if let Some(ref group) = self.config.consumer_group {
                info!(
                    "Subscribing to subject '{}' with consumer group '{}'",
                    self.config.subject, group
                );
                client
                    .queue_subscribe(self.config.subject.clone(), group.clone())
                    .await
                    .map_err(|e| Error::Connection(format!("Failed to subscribe: {}", e)))?
            } else {
                info!("Subscribing to subject '{}'", self.config.subject);
                client
                    .subscribe(self.config.subject.clone())
                    .await
                    .map_err(|e| Error::Connection(format!("Failed to subscribe: {}", e)))?
            };
            self.subscriber = Some(subscriber);
        }

        self.client = Some(client);
        self.status.connected = true;

        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        info!("Disconnecting from NATS");

        if let Some(subscriber) = self.subscriber.take() {
            drop(subscriber);
        }

        if let Some(client) = self.client.take() {
            client
                .flush()
                .await
                .map_err(|e| Error::Connection(format!("Failed to flush: {}", e)))?;
        }

        self.status.connected = false;
        info!("Disconnected from NATS");

        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.status.connected
    }

    async fn receive(&mut self) -> Result<Option<DataRecord>> {
        if self.config.use_jetstream {
            let messages = self
                .message_stream
                .as_mut()
                .ok_or_else(|| Error::Connection("consumer_info is not connected".to_string()))?;

            // Get next message from stream
            if let Some(msg_result) = messages.next().await {
                let msg = msg_result
                    .map_err(|e| Error::Connection(format!("Failed to receive message: {}", e)))?;

                info!("Received message from NATS: {} bytes", msg.payload.len());

                // Convert payload to UTF-8 string
                let payload_str = from_utf8(&msg.payload).map_err(|e| {
                    Error::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("Invalid UTF-8: {}", e),
                    ))
                })?;
                // Parse JSON payload into DataRecord
                let record = match serde_json::from_str::<DataRecord>(payload_str) {
                    Ok(record) => {
                        info!(
                            "Successfully parsed DataRecord: table={}, operation={:?}",
                            record.table, record.operation
                        );
                        record
                    }
                    Err(e) => {
                        error!("Failed to deserialize message into DataRecord: {}", e);
                        // Acknowledge message even if we can't parse it
                        msg.ack().await.map_err(|e| {
                            Error::Connection(format!("Failed to acknowledge message: {}", e))
                        })?;
                        return Err(Error::Serialization(e));
                    }
                };

                // Acknowledge the message
                msg.ack().await.map_err(|e| {
                    Error::Connection(format!("Failed to acknowledge message: {}", e))
                })?;

                return Ok(Some(record));
            }
            info!("receive message 02");
            // No messages available right now
            Ok(None)
        } else {
            let subscriber = self
                .subscriber
                .as_mut()
                .ok_or_else(|| Error::Connection("Not connected".to_string()))?;
            match subscriber.next().await {
                Some(msg) => {
                    info!("Received message from NATS: {} bytes", msg.payload.len());

                    match serde_json::from_slice::<DataRecord>(&msg.payload) {
                        Ok(record) => {
                            self.status.records_received += 1;
                            Ok(Some(record))
                        }
                        Err(e) => {
                            self.status.errors += 1;
                            let payload_preview = String::from_utf8_lossy(&msg.payload);
                            let err_msg = format!(
                                "Failed to deserialize message into DataRecord: {}. Payload: {}",
                                e, payload_preview
                            );
                            self.status.last_error = Some(err_msg.clone());
                            error!("{}", err_msg);
                            Err(Error::Serialization(e))
                        }
                    }
                }
                None => {
                    info!("NATS subscription closed");
                    Ok(None)
                }
            }
        }
    }

    fn status(&self) -> ConnectorStatus {
        self.status.clone()
    }
}

#[async_trait]
impl cdc_core::ConnectorCleanup for NatsConnector {
    async fn cleanup(&self) -> Result<()> {
        crate::cleanup::cleanup_nats_consumer(&self.config).await
    }
}
