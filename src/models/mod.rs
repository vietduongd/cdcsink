mod data_record;
mod nats_receive;

pub use data_record::{DataRecord, Schema, Payload, Source};
pub use nats_receive::NatsReceive;