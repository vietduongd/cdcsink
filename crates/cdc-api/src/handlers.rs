use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use cdc_config_store::UnifiedConfigStore;
use cdc_core::{FlowOrchestrator, Registry};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStats {
    pub records_received: u64,
    pub records_written: u64,
    pub errors: u64,
    pub uptime_seconds: u64,
}

impl Default for SystemStats {
    fn default() -> Self {
        Self {
            records_received: 0,
            records_written: 0,
            errors: 0,
            uptime_seconds: 0,
        }
    }
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
    
    Json(HealthResponse {
        status: "healthy".to_string(),
        uptime_seconds: stats.uptime_seconds,
    })
}

pub async fn get_stats(State(state): State<AppState>) -> impl IntoResponse {
    let stats = state.stats.read().await;
    
    Json(StatsResponse {
        records_received: stats.records_received,
        records_written: stats.records_written,
        errors: stats.errors,
        uptime_seconds: stats.uptime_seconds,
    })
}

pub async fn reset_stats(State(state): State<AppState>) -> impl IntoResponse {
    let mut stats = state.stats.write().await;
    *stats = SystemStats::default();
    
    (StatusCode::OK, "Stats reset successfully")
}
