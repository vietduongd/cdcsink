use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use cdc_config_store::DestinationConfigEntry;

use crate::{handlers::AppState, ApiResponse};

pub async fn list_destinations(State(state): State<AppState>) -> impl IntoResponse {
    let store = state.config_store.read().await;
    let destinations = store
        .list_destinations()
        .await
        .into_iter()
        .collect::<Vec<_>>();
    ApiResponse::success(destinations, "Destinations retrieved successfully")
}

pub async fn get_destination(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let store = state.config_store.read().await;

    let destination = match store.get_destination(&name).await {
        Some(d) => d,
        None => return ApiResponse::not_found("Destination"),
    };

    ApiResponse::success(destination, "Destination retrieved successfully")
}

pub async fn create_destination(
    State(state): State<AppState>,
    Json(entry): Json<DestinationConfigEntry>,
) -> ApiResponse<()> {
    let mut store = state.config_store.write().await;

    match store.add_destination(entry).await {
        Ok(_) => ApiResponse::<()>::success_no_data("Destination created successfully"),
        Err(e) => ApiResponse::<()>::bad_request(format!("Failed to create destination: {}", e)),
    }
}

pub async fn update_destination(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(entry): Json<DestinationConfigEntry>,
) -> ApiResponse<()> {
    let mut store = state.config_store.write().await;

    match store.update_destination(&name, entry).await {
        Ok(_) => ApiResponse::<()>::success_no_data("Destination updated successfully"),
        Err(_) => ApiResponse::<()>::not_found("Destination"),
    }
}

pub async fn delete_destination(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> ApiResponse<()> {
    // Check if destination is used in any flow
    let store = state.config_store.read().await;
    if store.is_destination_in_use(&name).await {
        return ApiResponse::<()>::conflict(
            "Cannot delete destination: it is being used in one or more flows".to_string(),
        );
    }
    drop(store);

    // Proceed with deletion
    let mut store = state.config_store.write().await;
    match store.delete_destination(&name).await {
        Ok(_) => ApiResponse::<()>::success_no_data("Destination deleted successfully"),
        Err(e) => ApiResponse::<()>::conflict(format!("Failed to delete destination: {}", e)),
    }
}

pub async fn test_destination(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let store = state.config_store.read().await;

    let dest_entry = match store.get_destination(&name).await {
        Some(entry) => entry,
        None => return ApiResponse::not_found("Destination"),
    };

    // Get registry to create destination
    let registry = &state.registry;
    let dest_factory = match registry.get_destination_factory(&dest_entry.destination_type) {
        Ok(factory) => factory,
        Err(_) => return ApiResponse::bad_request("Invalid destination type"),
    };

    let mut destination = match dest_factory.create(dest_entry.config.clone()) {
        Ok(dest) => dest,
        Err(e) => return ApiResponse::bad_request(format!("Failed to create destination: {}", e)),
    };

    // Try to connect
    match destination.connect().await {
        Ok(_) => {
            destination.disconnect().await.ok();
            ApiResponse::<()>::success_no_data("Connection successful")
        }
        Err(e) => ApiResponse::bad_request(format!("Connection failed: {}", e)),
    }
}

pub async fn test_destination_config(
    State(state): State<AppState>,
    Json(entry): Json<DestinationConfigEntry>,
) -> impl IntoResponse {
    // Get registry to create destination
    let registry = &state.registry;
    let dest_factory = match registry.get_destination_factory(&entry.destination_type) {
        Ok(factory) => factory,
        Err(_) => return ApiResponse::bad_request("Invalid destination type"),
    };

    let mut destination = match dest_factory.create(entry.config.clone()) {
        Ok(dest) => dest,
        Err(e) => return ApiResponse::bad_request(format!("Failed to create destination: {}", e)),
    };

    // Try to connect
    match destination.connect().await {
        Ok(_) => {
            destination.disconnect().await.ok();
            ApiResponse::<()>::success_no_data("Connection successful")
        }
        Err(e) => ApiResponse::bad_request(format!("Connection failed: {}", e)),
    }
}
