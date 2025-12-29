use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use cdc_config_store::DestinationConfigEntry;

use crate::handlers::AppState;

pub async fn list_destinations(State(state): State<AppState>) -> impl IntoResponse {
    let store = state.config_store.read().await;
    let destinations = store
        .list_destinations()
        .await
        .into_iter()
        .collect::<Vec<_>>();
    Json(destinations)
}

pub async fn get_destination(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let store = state.config_store.read().await;

    match store.get_destination(&name).await {
        Some(destination) => Ok(Json(destination)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn create_destination(
    State(state): State<AppState>,
    Json(entry): Json<DestinationConfigEntry>,
) -> Result<impl IntoResponse, StatusCode> {
    let mut store = state.config_store.write().await;

    store
        .add_destination(entry)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    Ok(StatusCode::CREATED)
}

pub async fn update_destination(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(entry): Json<DestinationConfigEntry>,
) -> Result<impl IntoResponse, StatusCode> {
    let mut store = state.config_store.write().await;

    store
        .update_destination(&name, entry)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(StatusCode::OK)
}

pub async fn delete_destination(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let mut store = state.config_store.write().await;

    store
        .delete_destination(&name)
        .await
        .map_err(|_| StatusCode::CONFLICT)?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn test_destination(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let store = state.config_store.read().await;

    let dest_entry = store
        .get_destination(&name)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    // Get registry to create destination
    let registry = &state.registry;
    let dest_factory = registry
        .get_destination_factory(&dest_entry.destination_type)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let mut destination = dest_factory
        .create(dest_entry.config.clone())
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    // Try to connect
    match destination.connect().await {
        Ok(_) => {
            destination.disconnect().await.ok();
            Ok((StatusCode::OK, "Connection successful").into_response())
        }
        Err(e) => {
            Ok((StatusCode::BAD_REQUEST, format!("Connection failed: {}", e)).into_response())
        }
    }
}
