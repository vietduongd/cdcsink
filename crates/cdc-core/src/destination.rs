use async_trait::async_trait;
use crate::{DataRecord, Result};

/// Trait for data destinations
#[async_trait]
pub trait Destination: Send + Sync {
    /// Connect to the destination
    async fn connect(&mut self) -> Result<()>;
    
    /// Disconnect from the destination
    async fn disconnect(&mut self) -> Result<()>;
    
    /// Check if the destination is connected
    fn is_connected(&self) -> bool;
    
    /// Write a single record
    async fn write(&mut self, record: DataRecord) -> Result<()>;
    
    /// Write multiple records in batch
    async fn write_batch(&mut self, records: Vec<DataRecord>) -> Result<()>;
    
    /// Get destination status information
    fn status(&self) -> DestinationStatus;
}

#[derive(Debug, Clone)]
pub struct DestinationStatus {
    pub connected: bool,
    pub records_written: u64,
    pub errors: u64,
    pub last_error: Option<String>,
}

impl Default for DestinationStatus {
    fn default() -> Self {
        Self {
            connected: false,
            records_written: 0,
            errors: 0,
            last_error: None,
        }
    }
}
