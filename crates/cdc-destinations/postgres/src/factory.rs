use crate::{PostgresDestination, PostgresConfig};
use cdc_core::{Destination, DestinationFactory, Result};
use serde_json::Value;

pub struct PostgresDestinationFactory;

impl DestinationFactory for PostgresDestinationFactory {
    fn name(&self) -> &str {
        "postgres"
    }
    
    fn create(&self, config: Value) -> Result<Box<dyn Destination>> {
        let config: PostgresConfig = serde_json::from_value(config)?;
        Ok(Box::new(PostgresDestination::new(config)))
    }
}
