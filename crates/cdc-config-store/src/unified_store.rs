use crate::models::{ConnectorConfigEntry, DestinationConfigEntry, FlowConfigEntry};
use crate::{ConfigStore, PgConfigStore};
use anyhow::Result;
use std::sync::Arc;

/// Unified config storage that can use either YAML files or PostgreSQL
pub enum UnifiedConfigStore {
    File(ConfigStore),
    Postgres(Arc<PgConfigStore>),
}

impl UnifiedConfigStore {
    /// Create from YAML files
    pub fn from_files(storage_dir: impl AsRef<std::path::Path>) -> Result<Self> {
        Ok(Self::File(ConfigStore::load(storage_dir)?))
    }

    /// Create from PostgreSQL
    pub async fn from_postgres(database_url: &str) -> Result<Self> {
        Ok(Self::Postgres(Arc::new(
            PgConfigStore::new(database_url).await?,
        )))
    }

    // Connector operations
    pub async fn add_connector(&mut self, entry: ConnectorConfigEntry) -> Result<()> {
        match self {
            Self::File(store) => {
                store.add_connector(entry)?;
                store.save()
            }
            Self::Postgres(store) => store.add_connector(&entry).await,
        }
    }

    pub async fn update_connector(
        &mut self,
        name: &str,
        entry: ConnectorConfigEntry,
    ) -> Result<()> {
        match self {
            Self::File(store) => {
                store.update_connector(name, entry)?;
                store.save()
            }
            Self::Postgres(store) => store.update_connector(name, &entry).await,
        }
    }

    pub async fn delete_connector(&mut self, name: &str) -> Result<()> {
        match self {
            Self::File(store) => {
                store.delete_connector(name)?;
                store.save()
            }
            Self::Postgres(store) => store.delete_connector(name).await,
        }
    }

    pub async fn get_connector(&self, name: &str) -> Option<ConnectorConfigEntry> {
        match self {
            Self::File(store) => store.get_connector(name).cloned(),
            Self::Postgres(store) => store.get_connector(name).await.ok().flatten(),
        }
    }

    pub async fn list_connectors(&self) -> Vec<ConnectorConfigEntry> {
        match self {
            Self::File(store) => store.list_connectors().into_iter().cloned().collect(),
            Self::Postgres(store) => store.list_connectors().await.unwrap_or_default(),
        }
    }

    // Destination operations
    pub async fn add_destination(&mut self, entry: DestinationConfigEntry) -> Result<()> {
        match self {
            Self::File(store) => {
                store.add_destination(entry)?;
                store.save()
            }
            Self::Postgres(store) => store.add_destination(&entry).await,
        }
    }

    pub async fn update_destination(
        &mut self,
        name: &str,
        entry: DestinationConfigEntry,
    ) -> Result<()> {
        match self {
            Self::File(store) => {
                store.update_destination(name, entry)?;
                store.save()
            }
            Self::Postgres(store) => store.update_destination(name, &entry).await,
        }
    }

    pub async fn delete_destination(&mut self, name: &str) -> Result<()> {
        match self {
            Self::File(store) => {
                store.delete_destination(name)?;
                store.save()
            }
            Self::Postgres(store) => store.delete_destination(name).await,
        }
    }

    pub async fn get_destination(&self, name: &str) -> Option<DestinationConfigEntry> {
        match self {
            Self::File(store) => store.get_destination(name).cloned(),
            Self::Postgres(store) => store.get_destination(name).await.ok().flatten(),
        }
    }

    pub async fn list_destinations(&self) -> Vec<DestinationConfigEntry> {
        match self {
            Self::File(store) => store.list_destinations().into_iter().cloned().collect(),
            Self::Postgres(store) => store.list_destinations().await.unwrap_or_default(),
        }
    }

    // Flow operations
    pub async fn add_flow(&mut self, entry: FlowConfigEntry) -> Result<()> {
        match self {
            Self::File(store) => {
                store.add_flow(entry)?;
                store.save()
            }
            Self::Postgres(store) => store.add_flow(&entry).await,
        }
    }

    pub async fn update_flow(&mut self, name: &str, entry: FlowConfigEntry) -> Result<()> {
        match self {
            Self::File(store) => {
                store.update_flow(name, entry)?;
                store.save()
            }
            Self::Postgres(store) => store.update_flow(name, &entry).await,
        }
    }

    pub async fn delete_flow(&mut self, name: &str) -> Result<()> {
        match self {
            Self::File(store) => {
                store.delete_flow(name)?;
                store.save()
            }
            Self::Postgres(store) => store.delete_flow(name).await,
        }
    }

    pub async fn get_flow(&self, name: &str) -> Option<FlowConfigEntry> {
        match self {
            Self::File(store) => store.get_flow(name).cloned(),
            Self::Postgres(store) => store.get_flow(name).await.ok().flatten(),
        }
    }

    pub async fn list_flows(&self) -> Vec<FlowConfigEntry> {
        match self {
            Self::File(store) => store.list_flows().into_iter().cloned().collect(),
            Self::Postgres(store) => store.list_flows().await.unwrap_or_default(),
        }
    }

    // Validation
    pub async fn validate_flow(&self, flow: &FlowConfigEntry) -> Result<()> {
        match self {
            Self::File(store) => store.validate_flow(flow),
            Self::Postgres(store) => store.validate_flow(flow).await,
        }
    }
}
