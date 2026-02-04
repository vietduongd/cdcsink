use std::{env, error::Error};

use chrono::Local;
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

    let nats_info =
        models::NatsReceive::new(nats_url, nats_consumer_name, topic_name, nats_stream_name);

    let mut consumer = nats_info.connected().await?;
    let mut counter = 0;
    loop {
        println!("Waiting for messages at {}", Local::now());
        let messages = nats_info.receive_messages(&mut consumer).await?;
        for (key, vec_msgs) in &messages {
            println!("Key = {}", key);

            for msg in vec_msgs {
                println!("Table: {}", msg.table_name);
                if let Some(data_model) = msg.table_value.get("Time01") {
                    println!(
                        "Time01 - Type: {}, Value: {:?}, Nullable: {}",
                        data_model.data_type, data_model.value, data_model.nullable
                    );
                }
            }
        }
        for item in messages {
            nats_info.ack_message(&item.1).await?;
        }
        println!("Waiting for messages end at {}", Local::now());
        counter += 1;
        println!("Loop count: {}", counter);
    }

    Ok(())
}
