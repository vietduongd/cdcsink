use std::{
    collections::{HashMap, HashSet},
    str::from_utf8,
};

use async_nats::{
    connect,
    jetstream::{self, Message},
};

use async_nats::jetstream::consumer::PullConsumer;
use futures_util::StreamExt;

use crate::models::{DataModel, DataRecord};

pub struct NatsReceive {
    pub url: String,
    pub durable_name: String,
    pub topic_name: String,
    pub stream: String,
    table_schema: HashMap<String, HashSet<String>>,
}

pub struct NatMessageReceive {
    pub message: Message,
    pub table_name: String,
    pub table_value: HashMap<String, DataModel>,
}

impl NatsReceive {
    pub fn new(url: String, durable_name: String, topic_name: String, stream: String) -> Self {
        NatsReceive {
            url,
            durable_name,
            topic_name,
            stream,
            table_schema: HashMap::new(),
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
    ) -> Result<HashMap<String, Vec<NatMessageReceive>>, String> {
        let mut messages = consumer
            .messages()
            .await
            .map_err(|e| format!("Failed to receive messages: {}", e))?
            .take(10);
        let mut received_messages: HashMap<String, Vec<NatMessageReceive>> = HashMap::new();
        while let Some(Ok(message)) = messages.next().await {
            let data_record: DataRecord = serde_json::from_slice(&message.payload)
                .map_err(|e| format!("Failed to deserialize message payload: {}", e))?;

            let table_name = data_record
                .get_table_name()
                .ok_or("Failed to get table name from data record")?;

            let table_schema = data_record
                .get_table_structure()
                .ok_or("Failed to get table structure from data record")?;

            received_messages
                .entry(table_name.clone())
                .or_insert_with(Vec::new)
                .push(NatMessageReceive {
                    message,
                    table_name,
                    table_value: table_schema,
                });
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
