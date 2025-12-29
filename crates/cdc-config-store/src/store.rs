use crate::models::{ConnectorConfigEntry, DestinationConfigEntry, FlowConfigEntry};
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Configuration store for managing connectors, destinations, and flows
#[derive(Debug, Clone)]
pub struct ConfigStore {
    connectors: HashMap<String, ConnectorConfigEntry>,
    destinations: HashMap<String, DestinationConfigEntry>,
    flows: HashMap<String, FlowConfigEntry>,
    storage_dir: PathBuf,
}

impl ConfigStore {
    /// Create a new empty config store
    pub fn new(storage_dir: impl Into<PathBuf>) -> Self {
        Self {
            connectors: HashMap::new(),
            destinations: HashMap::new(),
            flows: HashMap::new(),
            storage_dir: storage_dir.into(),
        }
    }
    
    /// Load config store from storage directory
    pub fn load(storage_dir: impl AsRef<Path>) -> Result<Self> {
        let storage_dir = storage_dir.as_ref();
        
        // Load connectors
        let connectors_path = storage_dir.join("connectors.yaml");
        let connectors = if connectors_path.exists() {
            let content = std::fs::read_to_string(&connectors_path)
                .context("Failed to read connectors.yaml")?;
            let list: Vec<ConnectorConfigEntry> = serde_yaml::from_str(&content)
                .context("Failed to parse connectors.yaml")?;
            list.into_iter().map(|e| (e.name.clone(), e)).collect()
        } else {
            HashMap::new()
        };
        
        // Load destinations
        let destinations_path = storage_dir.join("destinations.yaml");
        let destinations = if destinations_path.exists() {
            let content = std::fs::read_to_string(&destinations_path)
                .context("Failed to read destinations.yaml")?;
            let list: Vec<DestinationConfigEntry> = serde_yaml::from_str(&content)
                .context("Failed to parse destinations.yaml")?;
            list.into_iter().map(|e| (e.name.clone(), e)).collect()
        } else {
            HashMap::new()
        };
        
        // Load flows
        let flows_path = storage_dir.join("flows.yaml");
        let flows = if flows_path.exists() {
            let content = std::fs::read_to_string(&flows_path)
                .context("Failed to read flows.yaml")?;
            let list: Vec<FlowConfigEntry> = serde_yaml::from_str(&content)
                .context("Failed to parse flows.yaml")?;
            list.into_iter().map(|e| (e.name.clone(), e)).collect()
        } else {
            HashMap::new()
        };
        
        Ok(Self {
            connectors,
            destinations,
            flows,
            storage_dir: storage_dir.to_path_buf(),
        })
    }
    
    /// Save config store to storage directory
    pub fn save(&self) -> Result<()> {
        // Ensure directory exists
        std::fs::create_dir_all(&self.storage_dir)
            .context("Failed to create storage directory")?;
        
        // Save connectors
        let connectors_list: Vec<_> = self.connectors.values().cloned().collect();
        let connectors_yaml = serde_yaml::to_string(&connectors_list)
            .context("Failed to serialize connectors")?;
        std::fs::write(self.storage_dir.join("connectors.yaml"), connectors_yaml)
            .context("Failed to write connectors.yaml")?;
        
        // Save destinations
        let destinations_list: Vec<_> = self.destinations.values().cloned().collect();
        let destinations_yaml = serde_yaml::to_string(&destinations_list)
            .context("Failed to serialize destinations")?;
        std::fs::write(self.storage_dir.join("destinations.yaml"), destinations_yaml)
            .context("Failed to write destinations.yaml")?;
        
        // Save flows
        let flows_list: Vec<_> = self.flows.values().cloned().collect();
        let flows_yaml = serde_yaml::to_string(&flows_list)
            .context("Failed to serialize flows")?;
        std::fs::write(self.storage_dir.join("flows.yaml"), flows_yaml)
            .context("Failed to write flows.yaml")?;
        
        Ok(())
    }
    
    // ========== Connector Management ==========
    
    pub fn add_connector(&mut self, mut entry: ConnectorConfigEntry) -> Result<()> {
        if self.connectors.contains_key(&entry.name) {
            return Err(anyhow!("Connector '{}' already exists", entry.name));
        }
        
        entry.created_at = Utc::now();
        entry.updated_at = Utc::now();
        self.connectors.insert(entry.name.clone(), entry);
        Ok(())
    }
    
    pub fn update_connector(&mut self, name: &str, mut entry: ConnectorConfigEntry) -> Result<()> {
        if !self.connectors.contains_key(name) {
            return Err(anyhow!("Connector '{}' not found", name));
        }
        
        // Preserve created_at, update updated_at
        if let Some(existing) = self.connectors.get(name) {
            entry.created_at = existing.created_at;
        }
        entry.updated_at = Utc::now();
        entry.name = name.to_string();
        
        self.connectors.insert(name.to_string(), entry);
        Ok(())
    }
    
    pub fn delete_connector(&mut self, name: &str) -> Result<()> {
        // Check if used by any flow
        let flows_using: Vec<_> = self.flows.values()
            .filter(|f| f.connector_name == name)
            .map(|f| f.name.as_str())
            .collect();
        
        if !flows_using.is_empty() {
            return Err(anyhow!(
                "Connector '{}' is used by flows: {}",
                name,
                flows_using.join(", ")
            ));
        }
        
        self.connectors.remove(name)
            .ok_or_else(|| anyhow!("Connector '{}' not found", name))?;
        Ok(())
    }
    
    pub fn get_connector(&self, name: &str) -> Option<&ConnectorConfigEntry> {
        self.connectors.get(name)
    }
    
    pub fn list_connectors(&self) -> Vec<&ConnectorConfigEntry> {
        self.connectors.values().collect()
    }
    
    // ========== Destination Management ==========
    
    pub fn add_destination(&mut self, mut entry: DestinationConfigEntry) -> Result<()> {
        if self.destinations.contains_key(&entry.name) {
            return Err(anyhow!("Destination '{}' already exists", entry.name));
        }
        
        entry.created_at = Utc::now();
        entry.updated_at = Utc::now();
        self.destinations.insert(entry.name.clone(), entry);
        Ok(())
    }
    
    pub fn update_destination(&mut self, name: &str, mut entry: DestinationConfigEntry) -> Result<()> {
        if !self.destinations.contains_key(name) {
            return Err(anyhow!("Destination '{}' not found", name));
        }
        
        // Preserve created_at, update updated_at
        if let Some(existing) = self.destinations.get(name) {
            entry.created_at = existing.created_at;
        }
        entry.updated_at = Utc::now();
        entry.name = name.to_string();
        
        self.destinations.insert(name.to_string(), entry);
        Ok(())
    }
    
    pub fn delete_destination(&mut self, name: &str) -> Result<()> {
        // Check if used by any flow
        let flows_using: Vec<_> = self.flows.values()
            .filter(|f| f.destination_names.contains(&name.to_string()))
            .map(|f| f.name.as_str())
            .collect();
        
        if !flows_using.is_empty() {
            return Err(anyhow!(
                "Destination '{}' is used by flows: {}",
                name,
                flows_using.join(", ")
            ));
        }
        
        self.destinations.remove(name)
            .ok_or_else(|| anyhow!("Destination '{}' not found", name))?;
        Ok(())
    }
    
    pub fn get_destination(&self, name: &str) -> Option<&DestinationConfigEntry> {
        self.destinations.get(name)
    }
    
    pub fn list_destinations(&self) -> Vec<&DestinationConfigEntry> {
        self.destinations.values().collect()
    }
    
    // ========== Flow Management ==========
    
    pub fn add_flow(&mut self, mut entry: FlowConfigEntry) -> Result<()> {
        if self.flows.contains_key(&entry.name) {
            return Err(anyhow!("Flow '{}' already exists", entry.name));
        }
        
        // Validate references
        self.validate_flow(&entry)?;
        
        entry.created_at = Utc::now();
        entry.updated_at = Utc::now();
        self.flows.insert(entry.name.clone(), entry);
        Ok(())
    }
    
    pub fn update_flow(&mut self, name: &str, mut entry: FlowConfigEntry) -> Result<()> {
        if !self.flows.contains_key(name) {
            return Err(anyhow!("Flow '{}' not found", name));
        }
        
        // Validate references
        self.validate_flow(&entry)?;
        
        // Preserve created_at, update updated_at
        if let Some(existing) = self.flows.get(name) {
            entry.created_at = existing.created_at;
        }
        entry.updated_at = Utc::now();
        entry.name = name.to_string();
        
        self.flows.insert(name.to_string(), entry);
        Ok(())
    }
    
    pub fn delete_flow(&mut self, name: &str) -> Result<()> {
        self.flows.remove(name)
            .ok_or_else(|| anyhow!("Flow '{}' not found", name))?;
        Ok(())
    }
    
    pub fn get_flow(&self, name: &str) -> Option<&FlowConfigEntry> {
        self.flows.get(name)
    }
    
    pub fn list_flows(&self) -> Vec<&FlowConfigEntry> {
        self.flows.values().collect()
    }
    
    // ========== Validation ==========
    
    pub fn validate_flow(&self, flow: &FlowConfigEntry) -> Result<()> {
        // Check connector exists
        if !self.connectors.contains_key(&flow.connector_name) {
            return Err(anyhow!(
                "Connector '{}' not found",
                flow.connector_name
            ));
        }
        
        // Check all destinations exist
        for dest_name in &flow.destination_names {
            if !self.destinations.contains_key(dest_name) {
                return Err(anyhow!(
                    "Destination '{}' not found",
                    dest_name
                ));
            }
        }
        
        // Ensure at least one destination
        if flow.destination_names.is_empty() {
            return Err(anyhow!("Flow must have at least one destination"));
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[test]
    fn test_config_store_crud() {
        let mut store = ConfigStore::new("test_configs");
        
        // Add connector
        let connector = ConnectorConfigEntry::new(
            "test-nats".to_string(),
            "nats".to_string(),
            json!({"servers": ["nats://localhost:4222"]}),
        );
        store.add_connector(connector.clone()).unwrap();
        
        // Add destination
        let destination = DestinationConfigEntry::new(
            "test-pg".to_string(),
            "postgres".to_string(),
            json!({"url": "postgresql://localhost/db"}),
        );
        store.add_destination(destination.clone()).unwrap();
        
        // Add flow
        let flow = FlowConfigEntry::new(
            "test-flow".to_string(),
            "test-nats".to_string(),
            vec!["test-pg".to_string()],
            100,
        );
        store.add_flow(flow.clone()).unwrap();
        
        // Verify
        assert_eq!(store.list_connectors().len(), 1);
        assert_eq!(store.list_destinations().len(), 1);
        assert_eq!(store.list_flows().len(), 1);
        
        // Try to delete connector in use (should fail)
        assert!(store.delete_connector("test-nats").is_err());
        
        // Delete flow first
        store.delete_flow("test-flow").unwrap();
        
        // Now can delete connector
        store.delete_connector("test-nats").unwrap();
        assert_eq!(store.list_connectors().len(), 0);
    }
}
