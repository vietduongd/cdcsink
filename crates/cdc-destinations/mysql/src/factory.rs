use crate::{MysqlDestination, MysqlConfig};
use cdc_core::{Destination, DestinationFactory, Result};
use serde_json::Value;

pub struct MysqlDestinationFactory;

impl DestinationFactory for MysqlDestinationFactory {
    fn name(&self) -> &str {
        "mysql"
    }
    
    fn create(&self, config: Value) -> Result<Box<dyn Destination>> {
        let config: MysqlConfig = serde_json::from_value(config)?;
        Ok(Box::new(MysqlDestination::new(config)))
    }
}
