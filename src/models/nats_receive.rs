use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use async_nats::{
    connect,
    jetstream::{self, Message},
};

use async_nats::jetstream::consumer::PullConsumer;
use futures_util::StreamExt;

use crate::models::{DataModel, DataRecord, RowAction, SyncConfig, sync_config::classify};

pub struct NatsReceive {
    pub url: String,
    pub durable_name: String,
    pub topic_name: String,
    pub stream: String,
    pub number_pull_object: usize,
}

pub struct NatMessageReceive {
    pub index: i64,
    pub message: Message,
    pub table_name: String,
    pub table_value: HashMap<String, DataModel>,
    pub primary_key: Option<String>,
    pub action: RowAction,
}

/// Bỏ hậu tố `_resync` để message resync dùng chung table và config với table gốc.
pub fn normalize_table_name(name: &str) -> String {
    name.strip_suffix("_resync").unwrap_or(name).to_string()
}

impl NatsReceive {
    pub fn new(
        url: String,
        durable_name: String,
        topic_name: String,
        stream: String,
        number_pull_object: usize,
    ) -> Self {
        NatsReceive {
            url,
            durable_name,
            topic_name,
            stream,
            number_pull_object,
        }
    }

    pub async fn connected(&self) -> Result<PullConsumer, String> {
        let client = connect(&self.url)
            .await
            .map_err(|e| format!("Failed to connect to NATS server: {}", e))?;

        let jetstream_context = jetstream::new(client);
        let stream_info = jetstream_context
            .get_or_create_stream(async_nats::jetstream::stream::Config {
                name: self.stream.clone(),
                ..Default::default()
            })
            .await
            .map_err(|e| format!("Failed to get or create stream: {}", e))?;

        let consumer: PullConsumer = stream_info
            .get_or_create_consumer(
                &self.durable_name.clone(),
                async_nats::jetstream::consumer::pull::Config {
                    durable_name: Some(self.durable_name.clone()),
                    filter_subject: self.topic_name.clone(),
                    ack_wait: std::time::Duration::from_secs(10),
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| format!("Failed to get or create consumer: {}", e))?;
        Ok(consumer)
    }

    pub async fn receive_messages(
        &self,
        consumer: &mut PullConsumer,
        sync_config: Option<&SyncConfig>,
        logged_type_errors: &mut HashSet<String>,
    ) -> Result<Vec<NatMessageReceive>, String> {
        let mut messages = consumer
            .fetch()
            .max_messages(self.number_pull_object)
            .expires(Duration::from_secs(5)) // 👈 MaxWait
            .messages()
            .await
            .map_err(|e| format!("Failed to receive messages: {}", e))?;

        let mut received_messages: Vec<NatMessageReceive> = Vec::new();
        // table -> (tổng số message, số message bị loại vì lỗi kiểu hoặc thiếu cột)
        let mut type_rejections: HashMap<String, (usize, usize)> = HashMap::new();
        let mut counter = 0;
        while let Some(Ok(message)) = messages.next().await {
            let data_record: DataRecord = serde_json::from_slice(&message.payload)
                .map_err(|e| format!("Failed to deserialize message payload: {}", e))?;

            let table_name = normalize_table_name(
                &data_record
                    .get_table_name()
                    .ok_or("Failed to get table name from data record")?,
            );

            let mut table_value = data_record
                .get_table_structure()
                .ok_or("Failed to get table structure from data record")?;

            let primary_key = match table_value.get("id") {
                Some(op) => Some(op.value.to_string()),
                None => continue,
            };

            let table_config = sync_config.and_then(|config| config.table(&table_name));
            let (action, outcome) = classify(&table_value, table_config);

            for error in &outcome.type_errors {
                let key = format!("{}|{}|{:?}", table_name, error.column, error.op);
                if logged_type_errors.insert(key) {
                    eprintln!(
                        "Sync filter type mismatch: table {} column {} op {:?}: {}",
                        table_name, error.column, error.op, error.detail
                    );
                }
            }
            for column in &outcome.missing_columns {
                let key = format!("{}|{}|missing", table_name, column);
                if logged_type_errors.insert(key) {
                    eprintln!(
                        "Sync filter column not found: table {} column {} (check the name in where; rows are deleted)",
                        table_name, column
                    );
                }
            }
            let stats = type_rejections.entry(table_name.clone()).or_insert((0, 0));
            stats.0 += 1;
            if !outcome.matched
                && (!outcome.type_errors.is_empty() || !outcome.missing_columns.is_empty())
            {
                stats.1 += 1;
            }

            if let Some(config) = table_config {
                config.retain_columns(&mut table_value);
            }

            received_messages.push(NatMessageReceive {
                message,
                table_name,
                table_value,
                index: counter,
                primary_key,
                action,
            });
            counter += 1;
        }

        for (table_name, (total, rejected)) in &type_rejections {
            if *total > 0 && total == rejected {
                eprintln!(
                    "WARNING: all {} rows of table {} rejected due to type mismatch or missing column in where",
                    total, table_name
                );
            }
        }

        Ok(received_messages)
    }

    pub async fn ack_message(&self, nats_message: &Vec<NatMessageReceive>) -> Result<(), String> {
        for message in nats_message {
            message
                .message
                .ack()
                .await
                .map_err(|e| format!("Failed to acknowledge message: {}", e))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Review Focus #5: table _resync dùng chung config/tên với table gốc
    #[test]
    fn strips_resync_suffix() {
        assert_eq!(normalize_table_name("orders_resync"), "orders");
        assert_eq!(normalize_table_name("OrderItems_resync"), "OrderItems");
        assert_eq!(normalize_table_name("orders"), "orders");
        assert_eq!(normalize_table_name("resync_log"), "resync_log");
    }
}
