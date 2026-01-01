use crate::handlers::{
    connectors, destinations, flows, get_stats, health_check, reset_stats, AppState,
};
use axum::{
    routing::{get, post, put},
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

pub struct ApiServer {
    host: String,
    port: u16,
    cors_enabled: bool,
    state: AppState,
}

impl ApiServer {
    pub fn new(host: String, port: u16, cors_enabled: bool, state: AppState) -> Self {
        Self {
            host,
            port,
            cors_enabled,
            state,
        }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let mut app = Router::new()
            // Health and stats
            .route("/health", get(health_check))
            .route("/api/stats", get(get_stats))
            .route("/api/stats/reset", post(reset_stats))
            // Connector management
            .route(
                "/api/connectors",
                get(connectors::list_connectors).post(connectors::create_connector),
            )
            .route(
                "/api/connectors/{name}",
                get(connectors::get_connector)
                    .put(connectors::update_connector)
                    .delete(connectors::delete_connector),
            )
            .route(
                "/api/connectors/test-config",
                post(connectors::test_connector_config),
            )
            .route(
                "/api/connectors/{name}/test",
                post(connectors::test_connector),
            )
            // Destination management
            .route(
                "/api/destinations",
                get(destinations::list_destinations).post(destinations::create_destination),
            )
            .route(
                "/api/destinations/{name}",
                get(destinations::get_destination)
                    .put(destinations::update_destination)
                    .delete(destinations::delete_destination),
            )
            .route(
                "/api/destinations/test-config",
                post(destinations::test_destination_config),
            )
            .route(
                "/api/destinations/{name}/test",
                post(destinations::test_destination),
            )
            // Flow management
            .route(
                "/api/flows",
                get(flows::list_flows).post(flows::create_flow),
            )
            .route(
                "/api/flows/{name}",
                get(flows::get_flow).delete(flows::delete_flow),
            )
            .route("/api/flows/{name}/start", put(flows::start_flow))
            .route("/api/flows/{name}/stop", put(flows::stop_flow))
            .route("/api/flows/{name}/restart", put(flows::restart_flow))
            .route("/api/flows/{name}/pause", put(flows::pause_flow))
            .route("/api/flows/{name}/resume", put(flows::resume_flow))
            .with_state(self.state);

        if self.cors_enabled {
            let cors = CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any);

            app = app.layer(cors);
        }

        let addr = format!("{}:{}", self.host, self.port);
        info!("Starting API server on {}", addr);

        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }
}
