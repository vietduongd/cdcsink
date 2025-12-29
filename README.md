# CDC Data Sync System

A Rust enterprise-grade Change Data Capture (CDC) system that syncs data from NATS message broker to PostgreSQL database, with **multi-flow support** for running independent data pipelines concurrently.

## Features

- **Multi-Flow Architecture**: Run multiple independent data flows in a single process
- **NATS Connector**: Subscribe to NATS subjects/streams with automatic reconnection
- **PostgreSQL Destination**: Batch inserts with conflict resolution strategies (upsert, replace, ignore)
- **Fan-out Support**: One connector can write to multiple destinations
- **Real-time Monitoring**: React dashboard with live metrics and charts using ECharts
- **REST API**: Health checks, statistics, and system metrics
- **Plugin System**: Factory pattern for easy connector/destination extension
- **Configuration-driven**: YAML-based configuration with environment variable support

## Architecture

```
┌─────────────────────────────────────┐
│       CDC CLI Process               │
│                                     │
│  Flow 1: NATS → PostgreSQL          │
│  Flow 2: NATS → PostgreSQL          │
│  Flow 3: NATS → [PG, PG] (fan-out)  │
│                                     │
│  All flows run concurrently         │
└─────────────────────────────────────┘
         │
         ↓
  REST API Server ← React Dashboard
```

## Quick Start

### 1. Start Infrastructure

```bash
docker-compose up -d
```

This starts:

- NATS server on port 4222
- PostgreSQL on port 5432
- PgAdmin on port 5050

### 2. Generate Configuration

```bash
cargo run --bin cdc-cli generate-config
```

### 3. Start the CDC System

```bash
cargo run --bin cdc-cli start
```

### 4. Start the UI

```bash
cd ui
npm run dev
```

The dashboard will be available at http://localhost:5173

## Configuration

Edit `config/default.yaml`:

```yaml
nats:
  servers:
    - "nats://localhost:4222"
  subject: "cdc.events"

postgres:
  url: "postgresql://postgres:postgres@localhost:5432/cdc"
  conflict_resolution: "upsert"

pipeline:
  batch_size: 100
```

## Project Structure

```
cdc-destination/
├── crates/
│   ├── cdc-core/                    # Core traits, factories, registry
│   ├── cdc-connectors/              # Connector plugins
│   │   ├── nats/                    # NATS connector (production ready)
│   │   └── kafka/                   # Kafka connector (stub/template)
│   ├── cdc-destinations/            # Destination plugins
│   │   ├── postgres/                # PostgreSQL destination (production ready)
│   │   └── mysql/                   # MySQL destination (stub/template)
│   ├── cdc-config/                  # Configuration management
│   ├── cdc-api/                     # REST API server
│   └── cdc-cli/                     # CLI binary
├── ui/                              # React dashboard
├── config/                          # Configuration files
└── docker-compose.yml               # Development environment
```

## API Endpoints

- `GET /health` - Health check
- `GET /api/stats` - System statistics
- `POST /api/stats/reset` - Reset statistics

## Development

### Build the project

```bash
cargo build --workspace
```

### Run tests

```bash
cargo test --workspace
```

### Validate configuration

```bash
cargo run --bin cdc-cli validate -c config/default.yaml
```

## Publishing Test Data to NATS

```bash
# Install NATS CLI
go install github.com/nats-io/natscli/nats@latest

# Publish a test message
nats pub cdc.events '{"id":"550e8400-e29b-41d4-a716-446655440000","timestamp":"2025-12-29T01:00:00Z","source":"test","table":"users","operation":"insert","data":{"id":1,"name":"John Doe"},"metadata":{}}'
```

## License

MIT OR Apache-2.0
