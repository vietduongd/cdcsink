use async_nats::{jetstream, Client};
use cdc_core::{Error, Result};
use tracing::{info, warn};

use crate::NatsConfig;

/// Clean up NATS JetStream consumer
///
/// This function connects to the NATS server and deletes the JetStream consumer
/// if the configuration specifies JetStream usage and a consumer name.
///
/// # Arguments
/// * `config` - NATS configuration containing connection details and consumer info
///
/// # Returns
/// * `Ok(())` if cleanup succeeded or was not needed
/// * `Err` if connection or deletion failed
pub async fn cleanup_nats_consumer(config: &NatsConfig) -> Result<()> {
    // Only cleanup if using JetStream and consumer name is specified
    if !config.use_jetstream {
        info!("NATS connector not using JetStream, skipping cleanup");
        return Ok(());
    }

    let consumer_name = match &config.consumer_name {
        Some(name) => name,
        None => {
            info!("No consumer name specified, skipping cleanup");
            return Ok(());
        }
    };

    let consumer_group = config
        .consumer_group
        .as_ref()
        .ok_or_else(|| Error::Configuration("Consumer group required for JetStream".to_string()))?;

    info!(
        "Cleaning up NATS JetStream consumer '{}' from stream '{}'",
        consumer_name, consumer_group
    );

    // Build connection options with authentication
    let mut opts = async_nats::ConnectOptions::new();

    if let Some(ref username) = config.username {
        if let Some(ref password) = config.password {
            info!("Using username/password authentication for cleanup");
            opts = opts.user_and_password(username.clone(), password.clone());
        }
    } else if let Some(ref token) = config.token {
        info!("Using token authentication for cleanup");
        opts = opts.token(token.clone());
    }

    // Connect to NATS
    let client: Client = opts
        .connect(&config.servers[0])
        .await
        .map_err(|e| Error::Connection(format!("Failed to connect to NATS for cleanup: {}", e)))?;

    info!("Connected to NATS for cleanup");

    // Get JetStream context
    let jetstream = jetstream::new(client.clone());

    // Get the stream
    let stream = jetstream.get_stream(consumer_group).await.map_err(|e| {
        warn!("Failed to get stream '{}': {}", consumer_group, e);
        Error::Connection(format!("Failed to get stream: {}", e))
    })?;

    // Delete the consumer
    match stream.delete_consumer(consumer_name).await {
        Ok(_) => {
            info!(
                "Successfully deleted NATS consumer '{}' from stream '{}'",
                consumer_name, consumer_group
            );
            Ok(())
        }
        Err(e) => {
            // Check if consumer doesn't exist (not an error)
            let err_str = e.to_string();
            if err_str.contains("not found") || err_str.contains("does not exist") {
                info!(
                    "Consumer '{}' not found in stream '{}', already deleted",
                    consumer_name, consumer_group
                );
                Ok(())
            } else {
                warn!(
                    "Failed to delete consumer '{}' from stream '{}': {}",
                    consumer_name, consumer_group, e
                );
                Err(Error::Connection(format!(
                    "Failed to delete consumer: {}",
                    e
                )))
            }
        }
    }
}
