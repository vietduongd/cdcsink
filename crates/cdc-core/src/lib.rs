mod connector;
mod data_record;
mod destination;
mod error;
mod factory;
mod flow;
mod notification;
mod pipeline;
mod registry;

pub use connector::{Connector, ConnectorCleanup, ConnectorStatus};
pub use data_record::{DataRecord, Operation, TableMetadata};
pub use destination::{Destination, DestinationStatus};
pub use error::{Error, Result};
pub use factory::{ConnectorFactory, DestinationFactory};
pub use flow::{
    ConnectorConfig, DestinationConfig, Flow, FlowBuilder, FlowCommand, FlowConfig, FlowHandle,
    FlowOrchestrator, FlowStatus,
};
pub use notification::{EmailNotifier, NoOpNotifier, Notifier};
pub use pipeline::{Pipeline, PipelineStatus};
pub use registry::Registry;
