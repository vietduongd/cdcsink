use crate::{DataRecord, Result};
use async_trait::async_trait;

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

#[derive(Debug, Clone, Default)]
pub struct DestinationStatus {
    pub connected: bool,
    pub records_written: u64,
    pub errors: u64,
    pub consecutive_errors: u64,
    pub last_error: Option<String>,
}
