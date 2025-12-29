use crate::models::{ConnectorConfigEntry, DestinationConfigEntry, FlowConfigEntry};
use anyhow::Result;
use async_trait::async_trait;

/// Trait for configuration storage backends
#[async_trait]
pub trait ConfigStoreBackend: Send + Sync {
    // Connector operations
    async fn add_connector(&self, entry: ConnectorConfigEntry) -> Result<()>;
    async fn update_connector(&self, name: &str, entry: ConnectorConfigEntry) -> Result<()>;
    async fn delete_connector(&self, name: &str) -> Result<()>;
    async fn get_connector(&self, name: &str) -> Result<Option<ConnectorConfigEntry>>;
    async fn list_connectors(&self) -> Result<Vec<ConnectorConfigEntry>>;
    
    // Destination operations
    async fn add_destination(&self, entry: DestinationConfigEntry) -> Result<()>;
    async fn update_destination(&self, name: &str, entry: DestinationConfigEntry) -> Result<()>;
    async fn delete_destination(&self, name: &str) -> Result<()>;
    async fn get_destination(&self, name: &str) -> Result<Option<DestinationConfigEntry>>;
    async fn list_destinations(&self) -> Result<Vec<DestinationConfigEntry>>;
    
    // Flow operations
    async fn add_flow(&self, entry: FlowConfigEntry) -> Result<()>;
    async fn update_flow(&self, name: &str, entry: FlowConfigEntry) -> Result<()>;
    async fn delete_flow(&self, name: &str) -> Result<()>;
    async fn get_flow(&self, name: &str) -> Result<Option<FlowConfigEntry>>;
    async fn list_flows(&self) -> Result<Vec<FlowConfigEntry>>;
    
    // Validation
    async fn validate_flow(&self, flow: &FlowConfigEntry) -> Result<()>;
}
