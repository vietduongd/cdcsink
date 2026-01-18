use crate::models::{ConnectorConfigEntry, DestinationConfigEntry, FlowConfigEntry};
use anyhow::{anyhow, Context, Result};
use sqlx::{PgPool, Row};

pub struct PgConfigStore {
    pool: PgPool,
}

impl PgConfigStore {
    /// Create a new PostgreSQL config store
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = PgPool::connect(database_url)
            .await
            .context("Failed to connect to PostgreSQL")?;

        Ok(Self { pool })
    }

    // ========== Connector Management ==========

    pub async fn add_connector(&self, entry: &ConnectorConfigEntry) -> Result<()> {
        sqlx::query(
            "INSERT INTO connectors (name, connector_type, config, description, tags)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(&entry.name)
        .bind(&entry.connector_type)
        .bind(&entry.config)
        .bind(&entry.description)
        .bind(&entry.tags)
        .execute(&self.pool)
        .await
        .context("Failed to insert connector")?;

        Ok(())
    }

    pub async fn update_connector(&self, name: &str, entry: &ConnectorConfigEntry) -> Result<()> {
        let result = sqlx::query(
            "UPDATE connectors 
             SET connector_type = $2, config = $3, description = $4, tags = $5
             WHERE name = $1",
        )
        .bind(name)
        .bind(&entry.connector_type)
        .bind(&entry.config)
        .bind(&entry.description)
        .bind(&entry.tags)
        .execute(&self.pool)
        .await
        .context("Failed to update connector")?;

        if result.rows_affected() == 0 {
            return Err(anyhow!("Connector '{}' not found", name));
        }

        Ok(())
    }

    pub async fn delete_connector(&self, name: &str) -> Result<()> {
        // Check if used by any flow
        let flows_using =
            sqlx::query("SELECT name FROM flows WHERE connector_name = $1 ORDER BY name")
                .bind(name)
                .fetch_all(&self.pool)
                .await
                .context("Failed to check connector usage")?;

        if !flows_using.is_empty() {
            let flow_names: Vec<String> = flows_using
                .iter()
                .map(|row| row.get::<String, _>("name"))
                .collect();
            return Err(anyhow!(
                "Connector '{}' is used by flows: {}",
                name,
                flow_names.join(", ")
            ));
        }

        let result = sqlx::query("DELETE FROM connectors WHERE name = $1")
            .bind(name)
            .execute(&self.pool)
            .await
            .context("Failed to delete connector")?;

        if result.rows_affected() == 0 {
            return Err(anyhow!("Connector '{}' not found", name));
        }

        Ok(())
    }

    pub async fn get_connector(&self, name: &str) -> Result<Option<ConnectorConfigEntry>> {
        let row = sqlx::query(
            "SELECT name, connector_type, config, description, tags, created_at, updated_at
             FROM connectors
             WHERE name = $1",
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get connector")?;

        Ok(row.map(|r| ConnectorConfigEntry {
            name: r.get("name"),
            connector_type: r.get("connector_type"),
            config: r.get("config"),
            description: r.get("description"),
            tags: r.get("tags"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        }))
    }

    pub async fn list_connectors(&self) -> Result<Vec<ConnectorConfigEntry>> {
        let rows = sqlx::query(
            "SELECT name, connector_type, config, description, tags, created_at, updated_at
             FROM connectors
             ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to list connectors")?;

        Ok(rows
            .into_iter()
            .map(|r| ConnectorConfigEntry {
                name: r.get("name"),
                connector_type: r.get("connector_type"),
                config: r.get("config"),
                description: r.get("description"),
                tags: r.get("tags"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
            })
            .collect())
    }

    // ========== Destination Management ==========

    pub async fn add_destination(&self, entry: &DestinationConfigEntry) -> Result<()> {
        sqlx::query(
            "INSERT INTO destinations (name, destination_type, config, description, tags, schemas_includes)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(&entry.name)
        .bind(&entry.destination_type)
        .bind(&entry.config)
        .bind(&entry.description)
        .bind(&entry.tags)
        .bind(&entry.schemas_includes)
        .execute(&self.pool)
        .await
        .context("Failed to insert destination")?;

        Ok(())
    }

    pub async fn update_destination(
        &self,
        name: &str,
        entry: &DestinationConfigEntry,
    ) -> Result<()> {
        let result = sqlx::query(
            "UPDATE destinations 
             SET destination_type = $2, config = $3, description = $4, tags = $5, schemas_includes = $6
             WHERE name = $1",
        )
        .bind(name)
        .bind(&entry.destination_type)
        .bind(&entry.config)
        .bind(&entry.description)
        .bind(&entry.tags)
        .bind(&entry.schemas_includes)
        .execute(&self.pool)
        .await
        .context("Failed to update destination")?;

        if result.rows_affected() == 0 {
            return Err(anyhow!("Destination '{}' not found", name));
        }

        Ok(())
    }

    pub async fn delete_destination(&self, name: &str) -> Result<()> {
        // Check if used by any flow
        let flows_using =
            sqlx::query("SELECT name FROM flows WHERE $1 = ANY(destination_names) ORDER BY name")
                .bind(name)
                .fetch_all(&self.pool)
                .await
                .context("Failed to check destination usage")?;

        if !flows_using.is_empty() {
            let flow_names: Vec<String> = flows_using
                .iter()
                .map(|row| row.get::<String, _>("name"))
                .collect();
            return Err(anyhow!(
                "Destination '{}' is used by flows: {}",
                name,
                flow_names.join(", ")
            ));
        }

        let result = sqlx::query("DELETE FROM destinations WHERE name = $1")
            .bind(name)
            .execute(&self.pool)
            .await
            .context("Failed to delete destination")?;

        if result.rows_affected() == 0 {
            return Err(anyhow!("Destination '{}' not found", name));
        }

        Ok(())
    }

    pub async fn get_destination(&self, name: &str) -> Result<Option<DestinationConfigEntry>> {
        let row = sqlx::query(
            "SELECT name, destination_type, config, description, tags, schemas_includes, created_at, updated_at
             FROM destinations
             WHERE name = $1",
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get destination")?;

        Ok(row.map(|r| DestinationConfigEntry {
            name: r.get("name"),
            destination_type: r.get("destination_type"),
            config: r.get("config"),
            description: r.get("description"),
            tags: r.get("tags"),
            schemas_includes: r.get("schemas_includes"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        }))
    }

    pub async fn list_destinations(&self) -> Result<Vec<DestinationConfigEntry>> {
        let rows = sqlx::query(
            "SELECT name, destination_type, config, description, tags, schemas_includes, created_at, updated_at
             FROM destinations
             ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to list destinations")?;

        Ok(rows
            .into_iter()
            .map(|r| DestinationConfigEntry {
                name: r.get("name"),
                destination_type: r.get("destination_type"),
                config: r.get("config"),
                description: r.get("description"),
                tags: r.get("tags"),
                schemas_includes: r.get("schemas_includes"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
            })
            .collect())
    }

    // ========== Flow Management ==========

    pub async fn add_flow(&self, entry: &FlowConfigEntry) -> Result<()> {
        sqlx::query(
            "INSERT INTO flows (name, connector_name, destination_names, batch_size, auto_start, description)
             VALUES ($1, $2, $3, $4, $5, $6)"
        )
        .bind(&entry.name)
        .bind(&entry.connector_name)
        .bind(&entry.destination_names)
        .bind(entry.batch_size as i32)
        .bind(entry.auto_start)
        .bind(&entry.description)
        .execute(&self.pool)
        .await
        .context("Failed to insert flow")?;

        Ok(())
    }

    pub async fn update_flow(&self, name: &str, entry: &FlowConfigEntry) -> Result<()> {
        let result = sqlx::query(
            "UPDATE flows 
             SET connector_name = $2, destination_names = $3, batch_size = $4, 
                 auto_start = $5, description = $6
             WHERE name = $1",
        )
        .bind(name)
        .bind(&entry.connector_name)
        .bind(&entry.destination_names)
        .bind(entry.batch_size as i32)
        .bind(entry.auto_start)
        .bind(&entry.description)
        .execute(&self.pool)
        .await
        .context("Failed to update flow")?;

        if result.rows_affected() == 0 {
            return Err(anyhow!("Flow '{}' not found", name));
        }

        Ok(())
    }

    pub async fn delete_flow(&self, name: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM flows WHERE name = $1")
            .bind(name)
            .execute(&self.pool)
            .await
            .context("Failed to delete flow")?;

        if result.rows_affected() == 0 {
            return Err(anyhow!("Flow '{}' not found", name));
        }

        Ok(())
    }

    pub async fn get_flow(&self, name: &str) -> Result<Option<FlowConfigEntry>> {
        let row = sqlx::query(
            "SELECT name, connector_name, destination_names, batch_size, auto_start, 
                    description, created_at, updated_at
             FROM flows
             WHERE name = $1",
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get flow")?;

        Ok(row.map(|r| FlowConfigEntry {
            name: r.get("name"),
            connector_name: r.get("connector_name"),
            destination_names: r.get("destination_names"),
            batch_size: r.get::<i32, _>("batch_size") as usize,
            auto_start: r.get("auto_start"),
            description: r.get("description"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        }))
    }

    pub async fn list_flows(&self) -> Result<Vec<FlowConfigEntry>> {
        let rows = sqlx::query(
            "SELECT name, connector_name, destination_names, batch_size, auto_start,
                    description, created_at, updated_at
             FROM flows
             ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to list flows")?;

        Ok(rows
            .into_iter()
            .map(|r| FlowConfigEntry {
                name: r.get("name"),
                connector_name: r.get("connector_name"),
                destination_names: r.get("destination_names"),
                batch_size: r.get::<i32, _>("batch_size") as usize,
                auto_start: r.get("auto_start"),
                description: r.get("description"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
            })
            .collect())
    }

    // ========== Validation ==========

    pub async fn validate_flow(&self, flow: &FlowConfigEntry) -> Result<()> {
        // Check connector exists
        let connector_exists = sqlx::query("SELECT 1 FROM connectors WHERE name = $1")
            .bind(&flow.connector_name)
            .fetch_optional(&self.pool)
            .await?
            .is_some();

        if !connector_exists {
            return Err(anyhow!("Connector '{}' not found", flow.connector_name));
        }

        // Check all destinations exist
        for dest_name in &flow.destination_names {
            let dest_exists = sqlx::query("SELECT 1 FROM destinations WHERE name = $1")
                .bind(dest_name)
                .fetch_optional(&self.pool)
                .await?
                .is_some();

            if !dest_exists {
                return Err(anyhow!("Destination '{}' not found", dest_name));
            }
        }

        // Ensure at least one destination
        if flow.destination_names.is_empty() {
            return Err(anyhow!("Flow must have at least one destination"));
        }

        Ok(())
    }
}
