use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use cdc_config_store::ConnectorConfigEntry;

use crate::{handlers::AppState, ApiResponse};

pub async fn list_connectors(State(state): State<AppState>) -> impl IntoResponse {
    let store = state.config_store.read().await;
    let connectors = store
        .list_connectors()
        .await
        .into_iter()
        .collect::<Vec<_>>();
    ApiResponse::success(connectors, "Connectors retrieved successfully")
}

pub async fn get_connector(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let store = state.config_store.read().await;

    let connector = match store.get_connector(&name).await {
        Some(c) => c,
        None => return ApiResponse::not_found("Connector"),
    };

    ApiResponse::success(connector, "Connector retrieved successfully")
}

pub async fn create_connector(
    State(state): State<AppState>,
    Json(entry): Json<ConnectorConfigEntry>,
) -> ApiResponse<()> {
    let mut store = state.config_store.write().await;

    match store.add_connector(entry).await {
        Ok(_) => ApiResponse::<()>::success_no_data("Connector created successfully"),
        Err(e) => ApiResponse::<()>::bad_request(format!("Failed to create connector: {}", e)),
    }
}

pub async fn update_connector(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(entry): Json<ConnectorConfigEntry>,
) -> ApiResponse<()> {
    let mut store = state.config_store.write().await;

    match store.update_connector(&name, entry).await {
        Ok(_) => ApiResponse::<()>::success_no_data("Connector updated successfully"),
        Err(_) => ApiResponse::<()>::not_found("Connector"),
    }
}

pub async fn delete_connector(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> ApiResponse<()> {
    // Check if connector is used in any flow
    let store = state.config_store.read().await;
    if store.is_connector_in_use(&name).await {
        return ApiResponse::<()>::conflict(
            "Cannot delete connector: it is being used in one or more flows".to_string(),
        );
    }
    drop(store);

    // Proceed with deletion
    let mut store = state.config_store.write().await;
    match store.delete_connector(&name).await {
        Ok(_) => ApiResponse::<()>::success_no_data("Connector deleted successfully"),
        Err(e) => ApiResponse::<()>::conflict(format!("Failed to delete connector: {}", e)),
    }
}

pub async fn test_connector(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let store = state.config_store.read().await;

    let connector_entry = match store.get_connector(&name).await {
        Some(entry) => entry,
        None => return ApiResponse::not_found("Connector"),
    };

    // Get registry to create connector
    let registry = &state.registry;
    let connector_factory = match registry.get_connector_factory(&connector_entry.connector_type) {
        Ok(factory) => factory,
        Err(_) => return ApiResponse::bad_request("Invalid connector type"),
    };

    let mut connector = match connector_factory.create(connector_entry.config.clone()) {
        Ok(conn) => conn,
        Err(e) => return ApiResponse::bad_request(format!("Failed to create connector: {}", e)),
    };

    // Try to connect
    match connector.connect().await {
        Ok(_) => {
            connector.disconnect().await.ok();
            ApiResponse::<()>::success_no_data("Connection successful")
        }
        Err(e) => ApiResponse::bad_request(format!("Connection failed: {}", e)),
    }
}

pub async fn test_connector_config(
    State(state): State<AppState>,
    Json(entry): Json<ConnectorConfigEntry>,
) -> impl IntoResponse {
    // Get registry to create connector
    let registry = &state.registry;
    let connector_factory = match registry.get_connector_factory(&entry.connector_type) {
        Ok(factory) => factory,
        Err(_) => return ApiResponse::bad_request("Invalid connector type"),
    };

    let mut connector = match connector_factory.create(entry.config.clone()) {
        Ok(conn) => conn,
        Err(e) => return ApiResponse::bad_request(format!("Failed to create connector: {}", e)),
    };

    // Try to connect
    match connector.connect().await {
        Ok(_) => {
            connector.disconnect().await.ok();
            ApiResponse::<()>::success_no_data("Connection successful")
        }
        Err(e) => ApiResponse::bad_request(format!("Connection failed: {}", e)),
    }
}
