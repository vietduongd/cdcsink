use crate::{DataRecord, Result};
use async_trait::async_trait;

/// Trait for data source connectors
#[async_trait]
pub trait Connector: Send + Sync {
    /// Connect to the data source
    async fn connect(&mut self) -> Result<()>;

    /// Disconnect from the data source
    async fn disconnect(&mut self) -> Result<()>;

    /// Check if the connector is connected
    fn is_connected(&self) -> bool;

    /// Receive the next data record
    /// Returns None if the stream is closed
    async fn receive(&mut self) -> Result<Option<DataRecord>>;

    /// Get connector status information
    fn status(&self) -> ConnectorStatus;
}

#[derive(Debug, Clone, Default)]
pub struct ConnectorStatus {
    pub connected: bool,
    pub records_received: u64,
    pub errors: u64,
    pub last_error: Option<String>,
}

/// Optional trait for connectors that need to clean up external resources
/// when their configuration is deleted from the config store.
///
/// Examples of cleanup operations:
/// - Deleting JetStream consumers from NATS
/// - Removing consumer groups from Kafka
/// - Cleaning up subscriptions or queues
#[async_trait]
pub trait ConnectorCleanup: Send + Sync {
    /// Clean up external resources created by this connector
    /// This is called when the connector configuration is being deleted
    async fn cleanup(&self) -> Result<()>;
}
