mod postgres_destination;
mod factory;

pub use postgres_destination::{PostgresDestination, PostgresConfig, ConflictResolution};
pub use factory::PostgresDestinationFactory;
