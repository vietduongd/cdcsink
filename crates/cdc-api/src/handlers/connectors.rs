use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use cdc_config_store::ConnectorConfigEntry;

use crate::handlers::AppState;

pub async fn list_connectors(State(state): State<AppState>) -> impl IntoResponse {
    let store = state.config_store.read().await;
    let connectors = store
        .list_connectors()
        .await
        .into_iter()
        .collect::<Vec<_>>();
    Json(connectors)
}

pub async fn get_connector(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let store = state.config_store.read().await;

    match store.get_connector(&name).await {
        Some(connector) => Ok(Json(connector)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn create_connector(
    State(state): State<AppState>,
    Json(entry): Json<ConnectorConfigEntry>,
) -> Result<impl IntoResponse, StatusCode> {
    let mut store = state.config_store.write().await;

    store
        .add_connector(entry)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    Ok(StatusCode::CREATED)
}

pub async fn update_connector(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(entry): Json<ConnectorConfigEntry>,
) -> Result<impl IntoResponse, StatusCode> {
    let mut store = state.config_store.write().await;

    store
        .update_connector(&name, entry)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(StatusCode::OK)
}

pub async fn delete_connector(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let mut store = state.config_store.write().await;

    store
        .delete_connector(&name)
        .await
        .map_err(|_| StatusCode::CONFLICT)?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn test_connector(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let store = state.config_store.read().await;

    let connector_entry = store
        .get_connector(&name)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    // Get registry to create connector
    let registry = &state.registry;
    let connector_factory = registry
        .get_connector_factory(&connector_entry.connector_type)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let mut connector = connector_factory
        .create(connector_entry.config.clone())
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    // Try to connect
    match connector.connect().await {
        Ok(_) => {
            connector.disconnect().await.ok();
            Ok((StatusCode::OK, "Connection successful").into_response())
        }
        Err(e) => {
            Ok((StatusCode::BAD_REQUEST, format!("Connection failed: {}", e)).into_response())
        }
    }
}
