use std::{
    collections::{HashMap, HashSet},
    env,
    error::Error,
};

use chrono::Local;
use dotenvy::dotenv;

use crate::models::{NatMessageReceive, PostgresDestination};

mod models;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();
    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL not set");
    let nats_url = env::var("NATS_URL").expect("NATS_URL not set");
    let topic_name = env::var("TOPIC_NAME").expect("TOPIC_NAME not set");
    let database_schema_expected =
        env::var("DATABASE_SCHEMA_EXPECT").unwrap_or("public".to_string());

    let nats_consumer_name =
        env::var("NATS_CONSUMER_NAME").unwrap_or("cdcsink_consumer".to_string());
    let nats_stream_name = env::var("NATS_STREAM_NAME").expect("NATS_STREAM_NAME not set");

    let nats_info =
        models::NatsReceive::new(nats_url, nats_consumer_name, topic_name, nats_stream_name);

    let mut consumer = nats_info.connected().await?;

    let pg_destination = PostgresDestination::new(db_url, database_schema_expected.clone());
    let pg_pool = pg_destination
        .connect()
        .await
        .map_err(|e| Box::<dyn Error>::from(e))?;

    pg_destination
        .ensure_schema_metadata_table(&pg_pool)
        .await
        .map_err(|e| Box::<dyn Error>::from(e))?;

    let mut counter = 0;
    let mut schema_cache = pg_destination
        .get_schema_info(&pg_pool)
        .await
        .map_err(|e| Box::<dyn Error>::from(e))?;
    loop {
        println!("Waiting for messages at {}", Local::now());
        let messages = nats_info.receive_messages(&mut consumer).await?;
        let mut message_active: HashMap<String, Vec<&NatMessageReceive>> = HashMap::new();
        for msg in &messages {
            let table_name = &msg.table_name;
            if !schema_cache.contains_key(table_name) {
                pg_destination
                    .create_table_if_not_exists_query(
                        &database_schema_expected.clone(),
                        table_name,
                        &msg.table_value,
                        &pg_pool,
                    )
                    .await;
                schema_cache.insert(
                    table_name.clone(),
                    msg.table_value.keys().cloned().collect(),
                );
            }
            message_active
                .entry(table_name.clone())
                .or_insert(Vec::new())
                .push(msg);
        }
        for active_item in message_active {
            pg_destination.insert_value(&active_item.0, &active_item.1, &pg_pool).await;
        }
        nats_info.ack_message(&messages).await?;
        counter += 1;
        println!("Loop count: {}, at {}", counter, Local::now());
    }
    Ok(())
}
