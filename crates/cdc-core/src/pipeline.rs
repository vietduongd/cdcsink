use std::time::Duration;

use crate::{Connector, DataRecord, Destination, Result};
use tokio::time::sleep;
use tracing::{error, info, warn};

/// Main pipeline orchestrating data flow from connector to destination
pub struct Pipeline {
    connector: Box<dyn Connector>,
    destination: Box<dyn Destination>,
    batch_size: usize,
    buffer: Vec<DataRecord>,
}

impl Pipeline {
    pub fn new(
        connector: Box<dyn Connector>,
        destination: Box<dyn Destination>,
        batch_size: usize,
    ) -> Self {
        Self {
            connector,
            destination,
            batch_size,
            buffer: Vec::with_capacity(batch_size),
        }
    }

    /// Start the pipeline
    pub async fn run(&mut self) -> Result<()> {
        info!("Starting CDC pipeline");

        // Connect to source and destination
        self.connector.connect().await?;
        self.destination.connect().await?;

        info!("Pipeline connected successfully");

        loop {
            match self.connector.receive().await {
                Ok(Some(record)) => {
                    self.buffer.push(record);

                    if self.buffer.len() >= self.batch_size {
                        self.flush().await?;
                    }
                }
                Ok(None) => {
                    warn!("Connector stream closed");
                    break;
                }
                Err(e) => {
                    error!("Error receiving record 1: {}", e);
                    // Continue processing other records
                }
            }
            sleep(Duration::from_millis(100)).await;
        }

        // Flush remaining records
        if !self.buffer.is_empty() {
            self.flush().await?;
        }

        // Disconnect
        self.connector.disconnect().await?;
        self.destination.disconnect().await?;

        info!("Pipeline stopped");
        Ok(())
    }

    async fn flush(&mut self) -> Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        let records = std::mem::take(&mut self.buffer);
        let count = records.len();

        match self.destination.write_batch(records).await {
            Ok(_) => {
                info!("Flushed {} records to destination", count);
                Ok(())
            }
            Err(e) => {
                error!("Failed to flush records: {}", e);
                Err(e)
            }
        }
    }

    pub fn get_status(&self) -> PipelineStatus {
        PipelineStatus {
            connector_status: self.connector.status(),
            destination_status: self.destination.status(),
            buffer_size: self.buffer.len(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PipelineStatus {
    pub connector_status: crate::connector::ConnectorStatus,
    pub destination_status: crate::destination::DestinationStatus,
    pub buffer_size: usize,
}
