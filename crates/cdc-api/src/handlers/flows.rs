use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use cdc_config_store::FlowConfigEntry;
use cdc_core::{FlowBuilder, FlowStatus};
use serde::Serialize;

use crate::{handlers::AppState, ApiResponse};

#[derive(Serialize)]
pub struct FlowInfo {
    pub name: String,
    pub connector_name: String,
    pub destination_names: Vec<String>,
    pub batch_size: usize,
    pub auto_start: bool,
    pub status: FlowStatus,
    #[serde(rename = "active")]
    pub is_active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uptime_seconds: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub records_processed: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub messages_received: Option<u64>,
}

pub async fn list_flows(State(state): State<AppState>) -> impl IntoResponse {
    // Get all flows from orchestrator
    let flow_statuses = state.orchestrator.list_flows().await;

    // Get flow configs from store
    let store = state.config_store.read().await;

    let mut flows = Vec::new();
    for (name, status) in flow_statuses {
        if let Some(flow_config) = store.get_flow(&name).await {
            let is_active = matches!(status, FlowStatus::Running);

            // Get metrics from orchestrator
            let (uptime_seconds, records_processed) =
                state.orchestrator.get_flow_metrics(&name).await;
            let messages_received = state.orchestrator.get_flow_message_count(&name).await;

            flows.push(FlowInfo {
                name: name.clone(),
                connector_name: flow_config.connector_name.clone(),
                destination_names: flow_config.destination_names.clone(),
                batch_size: flow_config.batch_size,
                auto_start: flow_config.auto_start,
                status,
                is_active,
                uptime_seconds,
                records_processed,
                messages_received,
            });
        }
    }

    ApiResponse::success(flows, "Flows retrieved successfully")
}

pub async fn get_flow(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let store = state.config_store.read().await;
    let flow_config = match store.get_flow(&name).await {
        Some(config) => config,
        None => return ApiResponse::not_found("Flow"),
    };

    let status = state
        .orchestrator
        .get_flow_status(&name)
        .await
        .unwrap_or(FlowStatus::Stopped);

    let is_active = matches!(status, FlowStatus::Running);

    // Get metrics from orchestrator
    let (uptime_seconds, records_processed) = state.orchestrator.get_flow_metrics(&name).await;
    let messages_received = state.orchestrator.get_flow_message_count(&name).await;

    let info = FlowInfo {
        name: name.clone(),
        connector_name: flow_config.connector_name.clone(),
        destination_names: flow_config.destination_names.clone(),
        batch_size: flow_config.batch_size,
        auto_start: flow_config.auto_start,
        status,
        is_active,
        uptime_seconds,
        records_processed,
        messages_received,
    };

    ApiResponse::success(info, "Flow retrieved successfully")
}

pub async fn create_flow(
    State(state): State<AppState>,
    Json(entry): Json<FlowConfigEntry>,
) -> ApiResponse<()> {
    // Add to config store
    let mut store = state.config_store.write().await;
    if let Err(e) = store.add_flow(entry.clone()).await {
        return ApiResponse::<()>::bad_request(format!("Failed to create flow: {}", e));
    }

    // Build and start flow if auto_start
    if entry.auto_start {
        drop(store); // Release lock

        let store_read = state.config_store.read().await;

        // Get connector config
        let connector_entry = match store_read.get_connector(&entry.connector_name).await {
            Some(c) => c,
            None => return ApiResponse::<()>::bad_request("Connector not found"),
        };

        // Get destination configs
        let mut dest_entries = Vec::new();
        for dest_name in &entry.destination_names {
            let dest_entry = match store_read.get_destination(dest_name).await {
                Some(d) => d,
                None => {
                    return ApiResponse::<()>::bad_request(format!(
                        "Destination {} not found",
                        dest_name
                    ))
                }
            };
            dest_entries.push(dest_entry);
        }

        let dest_configs: Vec<_> = dest_entries
            .iter()
            .map(|d| (d.destination_type.as_str(), &d.config))
            .collect();

        // Build flow
        let builder = FlowBuilder::new(state.registry.clone());
        let flow = match builder.build_from_refs(
            entry.name.clone(),
            &connector_entry.connector_type,
            &connector_entry.config,
            dest_configs,
            entry.batch_size,
        ) {
            Ok(f) => f,
            Err(e) => {
                return ApiResponse::<()>::internal_error(format!("Failed to build flow: {}", e))
            }
        };

        // Start flow
        if let Err(e) = state.orchestrator.add_flow(flow).await {
            return ApiResponse::<()>::internal_error(format!("Failed to start flow: {}", e));
        }
    }

    ApiResponse::<()>::success_no_data("Flow created successfully")
}

pub async fn delete_flow(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> ApiResponse<()> {
    // Stop flow first
    state.orchestrator.stop_flow(&name).await.ok();

    // Wait a bit for graceful shutdown
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Remove from orchestrator
    state.orchestrator.remove_flow(&name).await.ok();

    // Remove from config store
    let mut store = state.config_store.write().await;
    match store.delete_flow(&name).await {
        Ok(_) => ApiResponse::<()>::success_no_data("Flow deleted successfully"),
        Err(_) => ApiResponse::<()>::not_found("Flow"),
    }
}

pub async fn start_flow(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> ApiResponse<()> {
    let store = state.config_store.read().await;
    let entry = match store.get_flow(&name).await {
        Some(e) => e,
        None => return ApiResponse::<()>::not_found("Flow"),
    };

    // Get connector config
    let connector_entry = match store.get_connector(&entry.connector_name).await {
        Some(c) => c,
        None => {
            return ApiResponse::<()>::bad_request(format!(
                "Connector '{}' not found for flow '{}'",
                entry.connector_name, name
            ))
        }
    };

    // Get destination configs
    let mut dest_entries = Vec::new();
    for dest_name in &entry.destination_names {
        let dest_entry = match store.get_destination(dest_name).await {
            Some(d) => d,
            None => {
                return ApiResponse::<()>::bad_request(format!(
                    "Destination '{}' not found for flow '{}'",
                    dest_name, name
                ))
            }
        };
        dest_entries.push(dest_entry);
    }

    let dest_configs: Vec<_> = dest_entries
        .iter()
        .map(|d| (d.destination_type.as_str(), &d.config))
        .collect();

    drop(store);

    // Remove existing flow if it exists (e.g., if it was stopped)
    state.orchestrator.remove_flow(&name).await.ok();

    // Build flow
    let builder = FlowBuilder::new(state.registry.clone());
    let flow = match builder.build_from_refs(
        entry.name.clone(),
        &connector_entry.connector_type,
        &connector_entry.config,
        dest_configs,
        entry.batch_size,
    ) {
        Ok(f) => f,
        Err(e) => {
            return ApiResponse::<()>::internal_error(format!(
                "Failed to build flow '{}': {}",
                name, e
            ))
        }
    };

    // Start flow
    match state.orchestrator.add_flow(flow).await {
        Ok(_) => ApiResponse::<()>::success_no_data("Flow started successfully"),
        Err(e) => ApiResponse::<()>::conflict(format!("Failed to start flow '{}': {}", name, e)),
    }
}

pub async fn stop_flow(State(state): State<AppState>, Path(name): Path<String>) -> ApiResponse<()> {
    match state.orchestrator.stop_flow(&name).await {
        Ok(_) => ApiResponse::<()>::success_no_data("Flow stopped successfully"),
        Err(_) => ApiResponse::<()>::not_found("Flow"),
    }
}

pub async fn restart_flow(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> ApiResponse<()> {
    // First, stop the flow if it's running
    state.orchestrator.stop_flow(&name).await.ok();

    // Wait a bit for graceful shutdown
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Remove the old flow handle
    state.orchestrator.remove_flow(&name).await.ok();

    // Now start it again using the same logic as start_flow
    let store = state.config_store.read().await;
    let entry = match store.get_flow(&name).await {
        Some(e) => e,
        None => return ApiResponse::<()>::not_found("Flow"),
    };

    // Get connector config
    let connector_entry = match store.get_connector(&entry.connector_name).await {
        Some(c) => c,
        None => {
            return ApiResponse::<()>::bad_request(format!(
                "Connector '{}' not found for flow '{}'",
                entry.connector_name, name
            ))
        }
    };

    // Get destination configs
    let mut dest_entries = Vec::new();
    for dest_name in &entry.destination_names {
        let dest_entry = match store.get_destination(dest_name).await {
            Some(d) => d,
            None => {
                return ApiResponse::<()>::bad_request(format!(
                    "Destination '{}' not found for flow '{}'",
                    dest_name, name
                ))
            }
        };
        dest_entries.push(dest_entry);
    }

    let dest_configs: Vec<_> = dest_entries
        .iter()
        .map(|d| (d.destination_type.as_str(), &d.config))
        .collect();

    drop(store);

    // Build flow
    let builder = FlowBuilder::new(state.registry.clone());
    let flow = match builder.build_from_refs(
        entry.name.clone(),
        &connector_entry.connector_type,
        &connector_entry.config,
        dest_configs,
        entry.batch_size,
    ) {
        Ok(f) => f,
        Err(e) => {
            return ApiResponse::<()>::internal_error(format!(
                "Failed to build flow '{}': {}",
                name, e
            ))
        }
    };

    // Start flow
    match state.orchestrator.add_flow(flow).await {
        Ok(_) => ApiResponse::<()>::success_no_data("Flow restarted successfully"),
        Err(e) => ApiResponse::<()>::conflict(format!("Failed to restart flow '{}': {}", name, e)),
    }
}

pub async fn pause_flow(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> ApiResponse<()> {
    // Note: FlowOrchestrator doesn't have pause_flow method yet, so we'll implement a basic version
    // For now, we can use stop as pause (they're functionally similar)
    match state.orchestrator.stop_flow(&name).await {
        Ok(_) => ApiResponse::<()>::success_no_data("Flow paused successfully"),
        Err(_) => ApiResponse::<()>::not_found("Flow"),
    }
}

pub async fn resume_flow(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> ApiResponse<()> {
    // Resume is essentially starting a stopped flow
    // We'll use the same logic as start_flow
    let store = state.config_store.read().await;
    let entry = match store.get_flow(&name).await {
        Some(e) => e,
        None => return ApiResponse::<()>::not_found("Flow"),
    };

    // Get connector config
    let connector_entry = match store.get_connector(&entry.connector_name).await {
        Some(c) => c,
        None => {
            return ApiResponse::<()>::bad_request(format!(
                "Connector '{}' not found for flow '{}'",
                entry.connector_name, name
            ))
        }
    };

    // Get destination configs
    let mut dest_entries = Vec::new();
    for dest_name in &entry.destination_names {
        let dest_entry = match store.get_destination(dest_name).await {
            Some(d) => d,
            None => {
                return ApiResponse::<()>::bad_request(format!(
                    "Destination '{}' not found for flow '{}'",
                    dest_name, name
                ))
            }
        };
        dest_entries.push(dest_entry);
    }

    let dest_configs: Vec<_> = dest_entries
        .iter()
        .map(|d| (d.destination_type.as_str(), &d.config))
        .collect();

    drop(store);

    // Remove existing flow if it exists
    state.orchestrator.remove_flow(&name).await.ok();

    // Build flow
    let builder = FlowBuilder::new(state.registry.clone());
    let flow = match builder.build_from_refs(
        entry.name.clone(),
        &connector_entry.connector_type,
        &connector_entry.config,
        dest_configs,
        entry.batch_size,
    ) {
        Ok(f) => f,
        Err(e) => {
            return ApiResponse::<()>::internal_error(format!(
                "Failed to build flow '{}': {}",
                name, e
            ))
        }
    };

    // Start flow
    match state.orchestrator.add_flow(flow).await {
        Ok(_) => ApiResponse::<()>::success_no_data("Flow resumed successfully"),
        Err(e) => ApiResponse::<()>::conflict(format!("Failed to resume flow '{}': {}", name, e)),
    }
}
