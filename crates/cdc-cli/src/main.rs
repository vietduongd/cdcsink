use cdc_api::{handlers::AppState, handlers::SystemStats, ApiServer};
use cdc_config::AppConfig;
use cdc_config_store::{ConfigStore, UnifiedConfigStore};
use cdc_core::{FlowBuilder, FlowOrchestrator, Registry};
use cdc_nats_connector::NatsConnectorFactory;
use cdc_postgres_destination::PostgresDestinationFactory;
use clap::{Parser, Subcommand};
use std::env;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser)]
#[command(name = "cdc-cli")]
#[command(about = "CDC Data Sync System CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the CDC pipeline with config management
    Start {
        /// Path to configuration directory
        #[arg(short, long, default_value = "config")]
        config_dir: String,
    },

    /// Validate configuration files
    Validate {
        /// Path to configuration directory
        #[arg(short, long, default_value = "config")]
        config_dir: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file if present
    dotenvy::dotenv().ok();

    let cli = Cli::parse();

    match cli.command {
        Commands::Start { config_dir } => {
            // Load app config from files and environment variables
            let app_config = AppConfig::load(&config_dir)?;

            // Initialize tracing
            let level = match app_config.logging.level.as_str() {
                "trace" => Level::TRACE,
                "debug" => Level::DEBUG,
                "info" => Level::INFO,
                "warn" => Level::WARN,
                "error" => Level::ERROR,
                _ => Level::INFO,
            };

            let subscriber = FmtSubscriber::builder().with_max_level(level).finish();

            tracing::subscriber::set_global_default(subscriber)?;

            info!("Starting CDC system with config directory: {}", config_dir);

            // Create registry and register all connectors and destinations
            let mut registry = Registry::new();

            // Register connectors
            registry.register_connector(Arc::new(NatsConnectorFactory));
            info!("Registered connector: nats");

            // Register destinations
            registry.register_destination(Arc::new(PostgresDestinationFactory));
            info!("Registered destination: postgres");

            let registry = Arc::new(registry);

            // List all registered plugins
            info!("Available connectors: {:?}", registry.list_connectors());
            info!("Available destinations: {:?}", registry.list_destinations());

            // Load config store - check env variable for storage backend
            let config_store = match env::var("CONFIG_STORAGE").as_deref() {
                Ok("postgres") | Ok("postgresql") | Ok("db") => {
                    info!("Using PostgreSQL config storage");
                    let db_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
                        "postgresql://postgres:postgres@postgres:5432/cdc".to_string()
                    });
                    info!("Connecting to database: {}", db_url);

                    let store = UnifiedConfigStore::from_postgres(&db_url).await?;
                    info!("✓ Connected to PostgreSQL config store");
                    store
                }
                _ => {
                    info!("Using file-based config storage from: {}", config_dir);
                    let store = UnifiedConfigStore::from_files(&config_dir)?;
                    info!("✓ Loaded config from YAML files");
                    store
                }
            };

            info!(
                "Loaded {} connector(s)",
                config_store.list_connectors().await.len()
            );
            info!(
                "Loaded {} destination(s)",
                config_store.list_destinations().await.len()
            );
            info!("Loaded {} flow(s)", config_store.list_flows().await.len());

            let config_store = Arc::new(RwLock::new(config_store));

            // Create flow orchestrator
            let orchestrator = Arc::new(FlowOrchestrator::new(registry.clone()));

            // Start all auto-start flows from config
            {
                let store = config_store.read().await;
                let flow_configs = store.list_flows().await;

                for flow_config in flow_configs {
                    if flow_config.auto_start {
                        info!("Auto-starting flow: {}", flow_config.name);

                        // Get connector config
                        let connector_entry = store
                            .get_connector(&flow_config.connector_name)
                            .await
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "Connector '{}' not found",
                                    flow_config.connector_name
                                )
                            })?;

                        // Get destination configs
                        let mut dest_entries = Vec::new();
                        for dest_name in &flow_config.destination_names {
                            let dest_entry =
                                store.get_destination(dest_name).await.ok_or_else(|| {
                                    anyhow::anyhow!("Destination '{}' not found", dest_name)
                                })?;
                            dest_entries.push(dest_entry);
                        }

                        let dest_configs: Vec<_> = dest_entries
                            .iter()
                            .map(|d| (d.destination_type.as_str(), &d.config))
                            .collect();

                        // Build and start flow
                        let builder = FlowBuilder::new(registry.clone());
                        let flow = builder.build_from_refs(
                            flow_config.name.clone(),
                            &connector_entry.connector_type,
                            &connector_entry.config,
                            dest_configs,
                            flow_config.batch_size,
                        )?;

                        orchestrator.add_flow(flow).await?;
                    }
                }
            }

            // Create shared app state
            let app_state = AppState {
                stats: Arc::new(RwLock::new(SystemStats::default())),
                config_store: config_store.clone(),
                orchestrator: orchestrator.clone(),
                registry: registry.clone(),
            };

            // Start API server
            let api_config = app_config.api.clone();
            let api_state = app_state.clone();
            let server = ApiServer::new(
                api_config.host,
                api_config.port,
                api_config.cors_enabled,
                api_state,
            );

            info!("CDC system started successfully");
            info!(
                "API server available at http://{}:{}",
                app_config.api.host, app_config.api.port
            );

            // Wait for API server or shutdown signal
            tokio::select! {
                res = server.run() => {
                    if let Err(e) = res {
                        error!("API server error: {}", e);
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    info!("Shutting down CDC system...");
                }
            }
        }

        Commands::Validate { config_dir } => {
            info!("Validating configuration in: {}", config_dir);

            let config_store = ConfigStore::load(&config_dir)?;

            println!("✓ Configuration is valid");
            println!("\n📦 Connectors: {}", config_store.list_connectors().len());
            for connector in config_store.list_connectors() {
                println!("  • {} ({})", connector.name, connector.connector_type);
                if let Some(desc) = &connector.description {
                    println!("    {}", desc);
                }
            }

            println!(
                "\n📍 Destinations: {}",
                config_store.list_destinations().len()
            );
            for destination in config_store.list_destinations() {
                println!(
                    "  • {} ({})",
                    destination.name, destination.destination_type
                );
                if let Some(desc) = &destination.description {
                    println!("    {}", desc);
                }
            }

            println!("\n🔄 Flows: {}", config_store.list_flows().len());
            for flow in config_store.list_flows() {
                let auto_start_marker = if flow.auto_start { "🟢" } else { "⚪" };
                println!(
                    "  {} {} ({} → {} destination(s))",
                    auto_start_marker,
                    flow.name,
                    flow.connector_name,
                    flow.destination_names.len()
                );
                if let Some(desc) = &flow.description {
                    println!("    {}", desc);
                }

                // Validate references
                match config_store.validate_flow(flow) {
                    Ok(_) => println!("    ✓ References valid"),
                    Err(e) => println!("    ✗ Error: {}", e),
                }
            }
        }
    }

    Ok(())
}
