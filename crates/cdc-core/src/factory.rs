use crate::{Connector, Destination, Result};
use serde_json::Value;

/// Factory trait for creating connectors
pub trait ConnectorFactory: Send + Sync {
    /// Get the name/type identifier for this connector
    fn name(&self) -> &str;
    
    /// Create a new connector instance from configuration
    fn create(&self, config: Value) -> Result<Box<dyn Connector>>;
}

/// Factory trait for creating destinations
pub trait DestinationFactory: Send + Sync {
    /// Get the name/type identifier for this destination
    fn name(&self) -> &str;
    
    /// Create a new destination instance from configuration
    fn create(&self, config: Value) -> Result<Box<dyn Destination>>;
}
