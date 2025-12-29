use async_trait::async_trait;
use crate::{DataRecord, Result};

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

#[derive(Debug, Clone)]
pub struct ConnectorStatus {
    pub connected: bool,
    pub records_received: u64,
    pub errors: u64,
    pub last_error: Option<String>,
}

impl Default for ConnectorStatus {
    fn default() -> Self {
        Self {
            connected: false,
            records_received: 0,
            errors: 0,
            last_error: None,
        }
    }
}
