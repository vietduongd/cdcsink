use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use cdc_config_store::FlowConfigEntry;
use cdc_core::{FlowBuilder, FlowStatus};
use serde::Serialize;

use crate::handlers::AppState;

#[derive(Serialize)]
pub struct FlowInfo {
    pub name: String,
    pub connector_name: String,
    pub destination_count: usize,
    pub status: FlowStatus,
}

pub async fn list_flows(State(state): State<AppState>) -> impl IntoResponse {
    // Get all flows from orchestrator
    let flow_statuses = state.orchestrator.list_flows().await;

    // Get flow configs from store
    let store = state.config_store.read().await;

    let mut flows = Vec::new();
    for (name, status) in flow_statuses {
        if let Some(flow_config) = store.get_flow(&name).await {
            flows.push(FlowInfo {
                name: name.clone(),
                connector_name: flow_config.connector_name.clone(),
                destination_count: flow_config.destination_names.len(),
                status,
            });
        }
    }

    Json(flows)
}

pub async fn get_flow(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let store = state.config_store.read().await;
    let flow_config = store.get_flow(&name).await.ok_or(StatusCode::NOT_FOUND)?;

    let status = state
        .orchestrator
        .get_flow_status(&name)
        .await
        .unwrap_or(FlowStatus::Stopped);

    let info = FlowInfo {
        name: name.clone(),
        connector_name: flow_config.connector_name.clone(),
        destination_count: flow_config.destination_names.len(),
        status,
    };

    Ok(Json(info))
}

pub async fn create_flow(
    State(state): State<AppState>,
    Json(entry): Json<FlowConfigEntry>,
) -> Result<impl IntoResponse, StatusCode> {
    // Add to config store
    let mut store = state.config_store.write().await;
    store
        .add_flow(entry.clone())
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    // Build and start flow if auto_start
    if entry.auto_start {
        drop(store); // Release lock

        let store_read = state.config_store.read().await;

        // Get connector config
        let connector_entry = store_read
            .get_connector(&entry.connector_name)
            .await
            .ok_or(StatusCode::BAD_REQUEST)?;

        // Get destination configs
        let mut dest_entries = Vec::new();
        for dest_name in &entry.destination_names {
            let dest_entry = store_read
                .get_destination(dest_name)
                .await
                .ok_or(StatusCode::BAD_REQUEST)?;
            dest_entries.push(dest_entry);
        }

        let dest_configs: Vec<_> = dest_entries
            .iter()
            .map(|d| (d.destination_type.as_str(), &d.config))
            .collect();

        // Build flow
        let builder = FlowBuilder::new(state.registry.clone());
        let flow = builder
            .build_from_refs(
                entry.name.clone(),
                &connector_entry.connector_type,
                &connector_entry.config,
                dest_configs,
                entry.batch_size,
            )
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        // Start flow
        state
            .orchestrator
            .add_flow(flow)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    Ok(StatusCode::CREATED)
}

pub async fn delete_flow(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    // Stop flow first
    state.orchestrator.stop_flow(&name).await.ok();

    // Wait a bit for graceful shutdown
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Remove from orchestrator
    state.orchestrator.remove_flow(&name).await.ok();

    // Remove from config store
    let mut store = state.config_store.write().await;
    store
        .delete_flow(&name)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn start_flow(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let store = state.config_store.read().await;
    let entry = store.get_flow(&name).await.ok_or(StatusCode::NOT_FOUND)?;

    // Get connector config
    let connector_entry = store
        .get_connector(&entry.connector_name)
        .await
        .ok_or(StatusCode::BAD_REQUEST)?;

    // Get destination configs
    let mut dest_entries = Vec::new();
    for dest_name in &entry.destination_names {
        let dest_entry = store
            .get_destination(dest_name)
            .await
            .ok_or(StatusCode::BAD_REQUEST)?;
        dest_entries.push(dest_entry);
    }

    let dest_configs: Vec<_> = dest_entries
        .iter()
        .map(|d| (d.destination_type.as_str(), &d.config))
        .collect();

    drop(store);

    // Build flow
    let builder = FlowBuilder::new(state.registry.clone());
    let flow = builder
        .build_from_refs(
            entry.name.clone(),
            &connector_entry.connector_type,
            &connector_entry.config,
            dest_configs,
            entry.batch_size,
        )
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Start flow
    state
        .orchestrator
        .add_flow(flow)
        .await
        .map_err(|_| StatusCode::CONFLICT)?;

    Ok(StatusCode::OK)
}

pub async fn stop_flow(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    state
        .orchestrator
        .stop_flow(&name)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(StatusCode::OK)
}
