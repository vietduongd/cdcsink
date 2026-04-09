use std::{collections::HashMap, env, error::Error};

use chrono::Local;
use dotenvy::dotenv;

use crate::models::{NatMessageReceive, PostgresDestination};

mod models;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Starting CDC Sink application...");

    dotenv().ok();

    println!("Loading environment variables...");
    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL not set");
    let nats_url = env::var("NATS_URL").expect("NATS_URL not set");
    let topic_name = env::var("TOPIC_NAME").expect("TOPIC_NAME not set");
    let database_schema_expected =
        env::var("DATABASE_SCHEMA_EXPECT").unwrap_or("public".to_string());
    let number_pull_object = env::var("NATS_PULL_NUMBER_OBJECT")
        .unwrap_or("100".to_string())
        .parse::<usize>()?;

    let nats_consumer_name =
        env::var("NATS_CONSUMER_NAME").unwrap_or("cdcsink_consumer".to_string());
    let nats_stream_name = env::var("NATS_STREAM_NAME").expect("NATS_STREAM_NAME not set");

    println!("Configuration loaded successfully");
    println!("NATS URL: {}", nats_url);
    println!("Topic: {}", topic_name);
    println!("Stream: {}", nats_stream_name);
    println!("Consumer: {}", nats_consumer_name);
    println!("Schema: {}", database_schema_expected);

    let nats_info = models::NatsReceive::new(
        nats_url,
        nats_consumer_name,
        topic_name,
        nats_stream_name,
        number_pull_object,
    );

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

    let mut schema_cache = pg_destination
        .get_schema_info(&pg_pool)
        .await
        .map_err(|e| Box::<dyn Error>::from(e))?;
    loop {
        let messages = nats_info.receive_messages(&mut consumer).await?;
        if messages.is_empty() {
            continue;
        }
        println!("Received {} messages at {}", messages.len(), Local::now());
        let mut message_active: HashMap<String, Vec<&NatMessageReceive>> = HashMap::new();
        for msg in &messages {
            let table_name = &msg.table_name;
            if table_name.ends_with("_resync") {
                continue;
            }
            if !schema_cache.contains_key(table_name) {
                // Table chưa tồn tại: nếu message_active đang có dữ liệu thì insert trước
                if let Some(buffered) = message_active.remove(table_name) {
                    if !buffered.is_empty() {
                        pg_destination
                            .insert_value(table_name, &buffered, &pg_pool)
                            .await;
                    }
                }
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
            } else {
                // Table đã tồn tại: kiểm tra xem có column mới không
                let cached_columns = schema_cache.get_mut(table_name).unwrap();
                let new_columns: Vec<(&String, &crate::models::DataModel)> = msg
                    .table_value
                    .iter()
                    .filter(|(col_name, _)| !cached_columns.contains(*col_name))
                    .collect();

                if !new_columns.is_empty() {
                    // Có column mới: nếu message_active đang có dữ liệu thì insert trước
                    if let Some(buffered) = message_active.remove(table_name) {
                        if !buffered.is_empty() {
                            pg_destination
                                .insert_value(table_name, &buffered, &pg_pool)
                                .await;
                        }
                    }
                    // Tạo các column mới
                    for (col_name, col_type) in &new_columns {
                        pg_destination
                            .add_column_if_not_exists(
                                &database_schema_expected,
                                table_name,
                                col_name,
                                col_type,
                                &pg_pool,
                            )
                            .await;
                        cached_columns.insert(col_name.to_string());
                    }
                }
            }
            message_active
                .entry(table_name.clone())
                .or_insert(Vec::new())
                .push(msg);
        }
        for active_item in message_active {
            pg_destination
                .insert_value(&active_item.0, &active_item.1, &pg_pool)
                .await;
        }
        nats_info.ack_message(&messages).await?;
    }
}
