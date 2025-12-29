use crate::{KafkaConnector, KafkaConfig};
use cdc_core::{Connector, ConnectorFactory, Result};
use serde_json::Value;

pub struct KafkaConnectorFactory;

impl ConnectorFactory for KafkaConnectorFactory {
    fn name(&self) -> &str {
        "kafka"
    }
    
    fn create(&self, config: Value) -> Result<Box<dyn Connector>> {
        let config: KafkaConfig = serde_json::from_value(config)?;
        Ok(Box::new(KafkaConnector::new(config)))
    }
}
