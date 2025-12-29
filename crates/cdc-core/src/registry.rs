use crate::{ConnectorFactory, DestinationFactory, Error, Result};
use std::collections::HashMap;
use std::sync::Arc;

/// Registry for connector and destination factories
pub struct Registry {
    connector_factories: HashMap<String, Arc<dyn ConnectorFactory>>,
    destination_factories: HashMap<String, Arc<dyn DestinationFactory>>,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            connector_factories: HashMap::new(),
            destination_factories: HashMap::new(),
        }
    }
    
    /// Register a connector factory
    pub fn register_connector(&mut self, factory: Arc<dyn ConnectorFactory>) {
        let name = factory.name().to_string();
        self.connector_factories.insert(name, factory);
    }
    
    /// Register a destination factory
    pub fn register_destination(&mut self, factory: Arc<dyn DestinationFactory>) {
        let name = factory.name().to_string();
        self.destination_factories.insert(name, factory);
    }
    
    /// Get a connector factory by name
    pub fn get_connector_factory(&self, name: &str) -> Result<Arc<dyn ConnectorFactory>> {
        self.connector_factories
            .get(name)
            .cloned()
            .ok_or_else(|| Error::Configuration(format!("Connector factory '{}' not found", name)))
    }
    
    /// Get a destination factory by name
    pub fn get_destination_factory(&self, name: &str) -> Result<Arc<dyn DestinationFactory>> {
        self.destination_factories
            .get(name)
            .cloned()
            .ok_or_else(|| Error::Configuration(format!("Destination factory '{}' not found", name)))
    }
    
    /// List all registered connector types
    pub fn list_connectors(&self) -> Vec<String> {
        self.connector_factories.keys().cloned().collect()
    }
    
    /// List all registered destination types
    pub fn list_destinations(&self) -> Vec<String> {
        self.destination_factories.keys().cloned().collect()
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}
