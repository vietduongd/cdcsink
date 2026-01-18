use crate::{Connector, DataRecord, Destination, Error, NoOpNotifier, Notifier, Registry, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

/// Configuration structures for flows
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowConfig {
    /// Unique name for this flow
    pub name: String,

    /// Connector configuration
    pub connector: ConnectorConfig,

    /// List of destinations for this flow
    pub destinations: Vec<DestinationConfig>,

    /// Batch size for writing to destinations
    pub batch_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorConfig {
    /// Type/name of the connector (e.g., "nats", "kafka")
    #[serde(rename = "type")]
    pub connector_type: String,

    /// Connector-specific configuration
    pub config: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DestinationConfig {
    /// Type/name of the destination (e.g., "postgres", "mysql")
    #[serde(rename = "type")]
    pub destination_type: String,

    /// Destination-specific configuration
    pub config: Value,
}

impl Default for FlowConfig {
    fn default() -> Self {
        Self {
            name: "default-flow".to_string(),
            connector: ConnectorConfig::default(),
            destinations: vec![DestinationConfig::default()],
            batch_size: 100,
        }
    }
}

impl Default for ConnectorConfig {
    fn default() -> Self {
        Self {
            connector_type: "nats".to_string(),
            config: serde_json::json!({
                "servers": ["nats://localhost:4222"],
                "subject": "cdc.events",
                "consumer_group": null,
                "consumer_name": null,
                "use_jetstream": false
            }),
        }
    }
}

impl Default for DestinationConfig {
    fn default() -> Self {
        Self {
            destination_type: "postgres".to_string(),
            config: serde_json::json!({
                "url": "postgresql://postgres:postgres@localhost:5432/cdc",
                "max_connections": 10,
                "schema": "public",
                "conflict_resolution": "upsert"
            }),
        }
    }
}

/// Commands for controlling flow lifecycle
#[derive(Debug, Clone)]
pub enum FlowCommand {
    Stop,
    Pause,
    Resume,
}

/// Flow status
#[derive(Debug, Clone, Serialize)]
pub enum FlowStatus {
    Running,
    Stopped,
    Paused,
    Failed(String),
}

/// Flow represents a single data pipeline from one connector to multiple destinations
pub struct Flow {
    name: String,
    connector: Box<dyn Connector>,
    destinations: Vec<Box<dyn Destination>>,
    batch_size: usize,
    buffer: Vec<DataRecord>,
    control_rx: Option<mpsc::Receiver<FlowCommand>>,
    last_flush: std::time::Instant,
    messages_received: Arc<RwLock<u64>>,
    destination_error_counters: Vec<u64>,
    error_threshold: u64,
    notifier: Arc<dyn Notifier>,
    /// Schema filters for each destination (None or empty = allow all schemas)
    destination_schema_filters: Vec<Option<Vec<String>>>,
}

impl Flow {
    pub fn new(
        name: String,
        connector: Box<dyn Connector>,
        destinations: Vec<Box<dyn Destination>>,
        batch_size: usize,
        destination_schema_filters: Vec<Option<Vec<String>>>,
    ) -> Self {
        let dest_count = destinations.len();
        
        // Log the configured schema filters for debugging
        warn!(
            "[{}] Initializing flow with {} destination(s) and schema filters: {:?}",
            name, dest_count, destination_schema_filters
        );
        
        Self {
            name,
            connector,
            destinations,
            batch_size,
            buffer: Vec::with_capacity(batch_size),
            control_rx: None,
            last_flush: std::time::Instant::now(),
            messages_received: Arc::new(RwLock::new(0)),
            destination_error_counters: vec![0; dest_count],
            error_threshold: 20, // Default threshold
            notifier: Arc::new(NoOpNotifier),
            destination_schema_filters,
        }
    }

    pub fn with_notifier(mut self, notifier: Arc<dyn Notifier>) -> Self {
        self.notifier = notifier;
        self
    }

    pub fn with_control(mut self, control_rx: mpsc::Receiver<FlowCommand>) -> Self {
        self.control_rx = Some(control_rx);
        self
    }

    /// Create a flow from configuration using the registry
    pub fn from_config(config: FlowConfig, registry: &Registry) -> Result<Self> {
        info!("Creating flow '{}'", config.name);

        // Create connector
        let connector_factory = registry.get_connector_factory(&config.connector.connector_type)?;
        let connector = connector_factory.create(config.connector.config)?;

        // Create destinations
        let mut destinations = Vec::new();
        for dest_config in config.destinations {
            let dest_factory = registry.get_destination_factory(&dest_config.destination_type)?;
            let destination = dest_factory.create(dest_config.config)?;
            destinations.push(destination);
        }

        // DestinationConfig doesn't have schemas_includes, so use empty filters
        let schema_filters = vec![None; destinations.len()];

        Ok(Self::new(
            config.name,
            connector,
            destinations,
            config.batch_size,
            schema_filters,
        ))
    }

    /// Run the flow
    pub async fn run(mut self) -> Result<()> {
        info!("[{}] Starting flow", self.name);

        // Connect to source
        self.connector.connect().await?;
        info!("[{}] Connector connected", self.name);

        // Connect to destinations
        for (idx, dest) in self.destinations.iter_mut().enumerate() {
            dest.connect().await?;
            info!("[{}] Destination {} connected", self.name, idx);
        }

        info!("[{}] Flow running", self.name);

        loop {
            // Check for control commands
            if let Some(ref mut rx) = self.control_rx {
                if let Ok(cmd) = rx.try_recv() {
                    match cmd {
                        FlowCommand::Stop => {
                            info!("[{}] Received stop command", self.name);
                            break;
                        }
                        FlowCommand::Pause => {
                            info!("[{}] Paused", self.name);
                            // Wait for resume
                            if let Some(FlowCommand::Resume) = rx.recv().await {
                                info!("[{}] Resumed", self.name);
                            }
                        }
                        FlowCommand::Resume => {
                            // Already running, ignore
                        }
                    }
                }
            }

            // Check for flush timeout (every 5 seconds)
            if !self.buffer.is_empty() && self.last_flush.elapsed() >= Duration::from_secs(5) {
                info!(
                    "[{}] Flush timeout reached, flushing {} records",
                    self.name,
                    self.buffer.len()
                );
                if let Err(e) = self.flush().await {
                    error!("[{}] Flush timeout failed: {}. Continuing...", self.name, e);
                    // To satisfy "ko stop event", we should ensure we don't get stuck in a loop
                    // but for now we just catch and continue.
                }
            }

            // Receive with timeout to allow control and flush checks
            match tokio::time::timeout(Duration::from_millis(500), self.connector.receive()).await {
                Ok(Ok(Some(record))) => {
                    // Increment message counter
                    *self.messages_received.write().await += 1;

                    self.buffer.push(record);

                    if self.buffer.len() >= self.batch_size {
                        if let Err(e) = self.flush().await {
                            error!("[{}] Batch flush failed: {}. Continuing...", self.name, e);
                        }
                    }
                }
                Ok(Ok(None)) | Err(_) => {
                    // No data or timeout - just continue to next iteration
                }
                Ok(Err(e)) => {
                    error!("[{}] Error receiving record: {}", self.name, e);
                    // Continue processing other records
                }
            }
        }

        // Flush remaining records
        if !self.buffer.is_empty() {
            let _ = self.flush().await.map_err(|e| {
                error!("[{}] Final flush failed: {}", self.name, e);
                e
            });
        }

        // Disconnect
        self.connector.disconnect().await?;
        for dest in &mut self.destinations {
            dest.disconnect().await?;
        }

        info!("[{}] Flow stopped", self.name);
        Ok(())
    }

    async fn flush(&mut self) -> Result<()> {
        if self.buffer.is_empty() {
            self.last_flush = std::time::Instant::now();
            return Ok(());
        }

        let records = std::mem::take(&mut self.buffer);
        let count = records.len();

        // Write to all destinations with schema filtering
        for (idx, dest) in self.destinations.iter_mut().enumerate() {
            // Filter records for this destination based on schemas_includes
            // We need to do this inline to avoid borrow checker issues
            let schema_filter = self.destination_schema_filters.get(idx);
            
            debug!(
                "[{}] Destination {} schema filter: {:?}",
                self.name, idx, schema_filter
            );
            
            let filtered_records: Vec<_> = records
                .iter()
                .filter(|r| {
                    let record_schema = &r.table_metadata.schema;
                    
                    // Check if this record should be sent to this destination
                    if let Some(Some(schemas)) = schema_filter {
                        if !schemas.is_empty() {
                            // If record schema is empty, allow it
                            if record_schema.is_empty() {
                                debug!(
                                    "[{}] Dest {}: Allowing record with empty schema (filter: {:?})",
                                    self.name, idx, schemas
                                );
                                return true;
                            }
                            
                            let should_send = schemas.contains(record_schema);
                            debug!(
                                "[{}] Dest {}: Record schema='{}', filter={:?}, allowed={}",
                                self.name, idx, record_schema, schemas, should_send
                            );
                            return should_send;
                        }
                    }
                    // No filter or empty filter = allow all
                    debug!(
                        "[{}] Dest {}: No filter or empty filter, allowing all schemas (record schema: '{}')",
                        self.name, idx, record_schema
                    );
                    true
                })
                .cloned()
                .collect();

            // Skip if no records match this destination's schema filter
            if filtered_records.is_empty() {
                info!(
                    "[{}] No records match schema filter for destination {} (filter: {:?})",
                    self.name, idx, schema_filter
                );
                continue;
            }

            let filtered_count = filtered_records.len();
            info!(
                "[{}] Sending {}/{} records to destination {} (schema filter: {:?})",
                self.name, filtered_count, count, idx, schema_filter
            );

            match dest.write_batch(filtered_records).await {
                Ok(_) => {
                    info!(
                        "[{}] Flushed {} records to destination {}",
                        self.name, filtered_count, idx
                    );
                    // Reset error counter on success
                    self.destination_error_counters[idx] = 0;
                }
                Err(e) => {
                    // Increment consecutive error counter
                    self.destination_error_counters[idx] += 1;
                    let error_count = self.destination_error_counters[idx];

                    error!(
                        "[{}] Failed to flush records to destination {} (error #{}/{}): {}",
                        self.name, idx, error_count, self.error_threshold, e
                    );

                    // Check if error threshold reached
                    if error_count >= self.error_threshold {
                        error!(
                            "[{}] Destination {} reached error threshold ({} consecutive errors). Stopping flow.",
                            self.name, idx, self.error_threshold
                        );

                        // Send email notification
                        let error_details = format!(
                            "Destination {}: {}\nConsecutive errors: {}\nError: {}",
                            idx,
                            if idx < self.destinations.len() {
                                format!("destination_{}", idx)
                            } else {
                                "unknown".to_string()
                            },
                            error_count,
                            e
                        );

                        if let Err(notify_err) = self
                            .notifier
                            .send_error_notification(&self.name, &error_details)
                            .await
                        {
                            error!(
                                "[{}] Failed to send email notification: {}",
                                self.name, notify_err
                            );
                        }

                        // Put records back into buffer to avoid data loss
                        self.buffer = records;
                        return Err(Error::Connection(format!(
                            "Destination {} failed {} consecutive times, threshold reached",
                            idx, self.error_threshold
                        )));
                    }

                    // Put records back into buffer to avoid data loss
                    self.buffer = records;
                    return Err(e);
                }
            }
        }

        self.last_flush = std::time::Instant::now();
        Ok(())
    }
}

/// Flow handle for managing running flow
pub struct FlowHandle {
    pub name: String,
    pub control_tx: mpsc::Sender<FlowCommand>,
    pub task_handle: JoinHandle<Result<()>>,
    pub status: Arc<RwLock<FlowStatus>>,
    pub start_time: std::time::Instant,
    pub records_processed: Arc<RwLock<u64>>,
    pub messages_received: Arc<RwLock<u64>>,
}

/// Flow builder for creating flows from configuration references
pub struct FlowBuilder {
    registry: Arc<Registry>,
}

impl FlowBuilder {
    pub fn new(registry: Arc<Registry>) -> Self {
        Self { registry }
    }

    /// Build flow from connector/destination configs
    pub fn build_from_refs(
        &self,
        name: String,
        connector_type: &str,
        connector_config: &Value,
        destinations: Vec<(&str, &Value)>,
        batch_size: usize,
        destination_schema_filters: Vec<Option<Vec<String>>>,
    ) -> Result<Flow> {
        // Create connector
        let connector_factory = self.registry.get_connector_factory(connector_type)?;
        let connector = connector_factory.create(connector_config.clone())?;

        // Create destinations
        let mut dest_instances = Vec::new();
        for (dest_type, dest_config) in destinations {
            let dest_factory = self.registry.get_destination_factory(dest_type)?;
            let destination = dest_factory.create(dest_config.clone())?;
            dest_instances.push(destination);
        }

        Ok(Flow::new(
            name,
            connector,
            dest_instances,
            batch_size,
            destination_schema_filters,
        ))
    }
}

/// FlowOrchestrator manages multiple concurrent flows with dynamic control
pub struct FlowOrchestrator {
    flows: Arc<Mutex<HashMap<String, FlowHandle>>>,
}

impl FlowOrchestrator {
    pub fn new(_registry: Arc<Registry>) -> Self {
        Self {
            flows: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Add and start a new flow
    pub async fn add_flow(&self, mut flow: Flow) -> Result<()> {
        let name = flow.name.clone();

        let mut flows = self.flows.lock().await;
        if flows.contains_key(&name) {
            return Err(Error::Configuration(format!(
                "Flow '{}' already exists",
                name
            )));
        }

        // Create control channel
        let (tx, rx) = mpsc::channel(32);

        // Clone the messages_received Arc before moving flow
        let messages_received = flow.messages_received.clone();

        flow = flow.with_control(rx);

        // Start flow
        let status = Arc::new(RwLock::new(FlowStatus::Running));
        let status_clone = status.clone();

        let task_handle = tokio::spawn(async move {
            let result = flow.run().await;
            let mut s = status_clone.write().await;
            *s = match &result {
                Ok(_) => FlowStatus::Stopped,
                Err(e) => FlowStatus::Failed(e.to_string()),
            };
            result
        });

        let handle = FlowHandle {
            name: name.clone(),
            control_tx: tx,
            task_handle,
            status,
            start_time: std::time::Instant::now(),
            records_processed: Arc::new(RwLock::new(0)),
            messages_received,
        };

        flows.insert(name.clone(), handle);
        info!("Flow '{}' started", name);
        Ok(())
    }

    /// Stop a running flow
    pub async fn stop_flow(&self, name: &str) -> Result<()> {
        let flows = self.flows.lock().await;
        let handle = flows
            .get(name)
            .ok_or_else(|| Error::Configuration(format!("Flow '{}' not found", name)))?;

        // Send stop command
        handle
            .control_tx
            .send(FlowCommand::Stop)
            .await
            .map_err(|_| Error::Configuration("Failed to send stop command".to_string()))?;

        // Immediately update status to Stopped so API consumers see the change
        {
            let mut status = handle.status.write().await;
            *status = FlowStatus::Stopped;
        }

        info!("Stop command sent to flow '{}'", name);
        Ok(())
    }

    /// Remove a stopped flow
    pub async fn remove_flow(&self, name: &str) -> Result<()> {
        let mut flows = self.flows.lock().await;
        flows
            .remove(name)
            .ok_or_else(|| Error::Configuration(format!("Flow '{}' not found", name)))?;

        info!("Flow '{}' removed", name);
        Ok(())
    }

    /// List all flows
    pub async fn list_flows(&self) -> Vec<(String, FlowStatus)> {
        let flows = self.flows.lock().await;
        let mut result = Vec::new();

        for (name, handle) in flows.iter() {
            let status = handle.status.read().await.clone();
            result.push((name.clone(), status));
        }

        result
    }

    /// Get flow status
    pub async fn get_flow_status(&self, name: &str) -> Option<FlowStatus> {
        let flows = self.flows.lock().await;
        if let Some(handle) = flows.get(name) {
            Some(handle.status.read().await.clone())
        } else {
            None
        }
    }

    /// Get flow metrics (uptime and records processed)
    pub async fn get_flow_metrics(&self, name: &str) -> (Option<u64>, Option<u64>) {
        let flows = self.flows.lock().await;
        if let Some(handle) = flows.get(name) {
            let uptime_seconds = Some(handle.start_time.elapsed().as_secs());
            let records_processed = Some(*handle.records_processed.read().await);
            (uptime_seconds, records_processed)
        } else {
            (None, None)
        }
    }

    /// Get message count for a flow
    pub async fn get_flow_message_count(&self, name: &str) -> Option<u64> {
        let flows = self.flows.lock().await;
        if let Some(handle) = flows.get(name) {
            Some(*handle.messages_received.read().await)
        } else {
            None
        }
    }

    /// Wait for all flows to complete
    pub async fn wait_all(&self) -> Result<()> {
        loop {
            let flows = self.flows.lock().await;
            if flows.is_empty() {
                break;
            }
            drop(flows);
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
        Ok(())
    }
}
