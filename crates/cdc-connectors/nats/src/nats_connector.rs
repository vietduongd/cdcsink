use async_nats::{Client, Subscriber};
use async_trait::async_trait;
use futures::StreamExt;
use cdc_core::{Connector, ConnectorStatus, DataRecord, Error, Result};
use serde::{Deserialize, Serialize};
use tracing::{info, error, debug};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NatsConfig {
    /// NATS server URL(s)
    pub servers: Vec<String>,
    
    /// Subject to subscribe to
    pub subject: String,
    
    /// Optional consumer group name
    pub consumer_group: Option<String>,
    
    /// Use JetStream (for guaranteed delivery)
    pub use_jetstream: bool,
}

impl Default for NatsConfig {
    fn default() -> Self {
        Self {
            servers: vec!["nats://localhost:4222".to_string()],
            subject: "cdc.events".to_string(),
            consumer_group: None,
            use_jetstream: false,
        }
    }
}

pub struct NatsConnector {
    config: NatsConfig,
    client: Option<Client>,
    subscriber: Option<Subscriber>,
    status: ConnectorStatus,
}

impl NatsConnector {
    pub fn new(config: NatsConfig) -> Self {
        Self {
            config,
            client: None,
            subscriber: None,
            status: ConnectorStatus::default(),
        }
    }
}

#[async_trait]
impl Connector for NatsConnector {
    async fn connect(&mut self) -> Result<()> {
        info!("Connecting to NATS servers: {:?}", self.config.servers);
        
        let client = async_nats::connect(&self.config.servers[0])
            .await
            .map_err(|e| Error::Connection(format!("Failed to connect to NATS: {}", e)))?;
        
        info!("Connected to NATS successfully");
        
        // Subscribe to subject
        let subscriber = if let Some(ref group) = self.config.consumer_group {
            info!("Subscribing to subject '{}' with consumer group '{}'", self.config.subject, group);
            client.queue_subscribe(self.config.subject.clone(), group.clone())
                .await
                .map_err(|e| Error::Connection(format!("Failed to subscribe: {}", e)))?
        } else {
            info!("Subscribing to subject '{}'", self.config.subject);
            client.subscribe(self.config.subject.clone())
                .await
                .map_err(|e| Error::Connection(format!("Failed to subscribe: {}", e)))?
        };
        
        self.client = Some(client);
        self.subscriber = Some(subscriber);
        self.status.connected = true;
        
        Ok(())
    }
    
    async fn disconnect(&mut self) -> Result<()> {
        info!("Disconnecting from NATS");
        
        if let Some(subscriber) = self.subscriber.take() {
            drop(subscriber);
        }
        
        if let Some(client) = self.client.take() {
            client.flush().await
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
        let subscriber = self.subscriber.as_mut()
            .ok_or_else(|| Error::Connection("Not connected".to_string()))?;
        
        match subscriber.next().await {
            Some(msg) => {
                debug!("Received message from NATS: {} bytes", msg.payload.len());
                
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
    
    fn status(&self) -> ConnectorStatus {
        self.status.clone()
    }
}
