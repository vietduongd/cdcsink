use axum::{extract::State, response::IntoResponse};
use cdc_config_store::UnifiedConfigStore;
use cdc_core::{FlowOrchestrator, Registry};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::ApiResponse;

pub mod connectors;
pub mod destinations;
pub mod flows;

#[derive(Clone)]
pub struct AppState {
    pub stats: Arc<RwLock<SystemStats>>,
    pub config_store: Arc<RwLock<UnifiedConfigStore>>,
    pub orchestrator: Arc<FlowOrchestrator>,
    pub registry: Arc<Registry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SystemStats {
    pub records_received: u64,
    pub records_written: u64,
    pub errors: u64,
    pub uptime_seconds: u64,
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub uptime_seconds: u64,
}

#[derive(Serialize)]
pub struct StatsResponse {
    pub records_received: u64,
    pub records_written: u64,
    pub errors: u64,
    pub uptime_seconds: u64,
}

pub async fn health_check(State(state): State<AppState>) -> impl IntoResponse {
    let stats = state.stats.read().await;

    let response = HealthResponse {
        status: "healthy".to_string(),
        uptime_seconds: stats.uptime_seconds,
    };

    ApiResponse::success(response, "System is healthy")
}

pub async fn get_stats(State(state): State<AppState>) -> impl IntoResponse {
    let stats = state.stats.read().await;

    let response = StatsResponse {
        records_received: stats.records_received,
        records_written: stats.records_written,
        errors: stats.errors,
        uptime_seconds: stats.uptime_seconds,
    };

    ApiResponse::success(response, "Stats retrieved successfully")
}

pub async fn reset_stats(State(state): State<AppState>) -> ApiResponse<()> {
    let mut stats = state.stats.write().await;
    *stats = SystemStats::default();

    ApiResponse::<()>::success_no_data("Stats reset successfully")
}
