use crate::{NatsConnector, NatsConfig};
use cdc_core::{Connector, ConnectorFactory, Result};
use serde_json::Value;

pub struct NatsConnectorFactory;

impl ConnectorFactory for NatsConnectorFactory {
    fn name(&self) -> &str {
        "nats"
    }
    
    fn create(&self, config: Value) -> Result<Box<dyn Connector>> {
        let config: NatsConfig = serde_json::from_value(config)?;
        Ok(Box::new(NatsConnector::new(config)))
    }
}
