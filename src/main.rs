use std::{env, error::Error};

use dotenvy::dotenv;

mod models;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();
    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL not set");
    let nats_url = env::var("NATS_URL").expect("NATS_URL not set");
    let topic_name = env::var("TOPIC_NAME").expect("TOPIC_NAME not set");
    let nats_consumer_name =
        env::var("NATS_CONSUMER_NAME").unwrap_or("cdcsink_consumer".to_string());
    let nats_stream_name = env::var("NATS_STREAM_NAME").expect("NATS_STREAM_NAME not set");

    let nats_info = models::NatsReceive::new(
        nats_url,
        nats_consumer_name,
        topic_name,
        nats_stream_name,
    );

    let mut consumer = nats_info.connected().await?;
    let _messages = nats_info.receive_messages(&mut consumer).await?;

    Ok(())
}
