use async_nats::jetstream::consumer::{self, Config, Consumer};
use async_nats::jetstream::{self, consumer::PullConsumer};
use async_nats::{Client, Subscriber};
use async_trait::async_trait;
use cdc_core::{Connector, ConnectorStatus, DataRecord, Error, Result};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tracing::info;

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
    consumer: Option<Consumer<Config>>,
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

        let stream = jetstream
            .get_or_create_stream(jetstream::stream::Config {
                name: self.config.consumer_group.unwrap(),
                subjects: vec![self.config.subject.clone()], // Listen to all 'orders.*' subjects
                max_messages: 10_000,                        // Limit stream size
                ..Default::default()
            })
            .await?;
        if (self.config.use_jetstream) {
            let jetstreams = jetstream::new(client);
            let consumer = jetstreams
                .create_consumer_strict_on_stream(
                    jetstream::consumer::Config {
                        name: self.config.consumer_name.clone(),
                        ..Default::default()
                    },
                    stream,
                )
                .await?;
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
        let subscriber = self
            .subscriber
            .as_mut()
            .ok_or_else(|| Error::Connection("Not connected".to_string()))?;
        info!("NATS run waiting value");
        if self.config.use_jetstream {
            let mut messages = self.consumer.messages().await?.take(10);
            while let Some(msg) = messages.next().await {
                info!("Received message from NATS: {} bytes", msg.payload.len());
                let message = message?;
                println!(
                    "got message on subject {} with payload {:?}",
                    message.subject,
                    from_utf8(&message.payload)?
                );
                message.ack().await?;
            }
            Ok(None)
        } else {
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
                            let err_msg = format!("Failed to deserialize message: {}", e);
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
