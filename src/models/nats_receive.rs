use std::str::from_utf8;

use async_nats::{
    connect,
    jetstream::{self},
};

use async_nats::jetstream::consumer::PullConsumer;
use futures_util::{Stream, StreamExt};

pub struct NatsReceive {
    pub url: String,
    pub durable_name: String,
    pub topic_name: String,
    pub stream: String,
}

impl NatsReceive {
    pub fn new(url: String, durable_name: String, topic_name: String, stream: String) -> Self {
        NatsReceive {
            url,
            durable_name,
            topic_name,
            stream,
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
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| format!("Failed to get or create consumer: {}", e))?;
        Ok(consumer)
    }

    pub async fn receive_messages(&self, consumer: &mut PullConsumer) -> Result<(), String> {
        let mut messages = consumer
            .messages()
            .await
            .map_err(|e| format!("Failed to receive messages: {}", e))?
            .take(1);
        while let Some(Ok(message)) = messages.next().await {
            println!("got message {:?}", message.payload);

             let payload_str = from_utf8(&message.payload).map_err(|e| {
                   format!("Invalid UTF-8: {}", e)
                })?;
            println!("Message payload as string: {}", payload_str);
            message
                .ack()
                .await
                .map_err(|e| format!("Failed to ack message: {}", e))?;
        }

        Ok(())
    }
}
