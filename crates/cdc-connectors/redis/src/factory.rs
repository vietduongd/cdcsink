use crate::{RedisConfig, RedisConnector};
use cdc_core::{Connector, ConnectorFactory, Result};
use serde_json::Value;

pub struct RedisConnectorFactory;

impl ConnectorFactory for RedisConnectorFactory {
    fn name(&self) -> &str {
        "redis"
    }

    fn create(&self, config: Value) -> Result<Box<dyn Connector>> {
        let config: RedisConfig = serde_json::from_value(config)?;
        Ok(Box::new(RedisConnector::new(config)))
    }
}
