# Environment Variables Guide

## Quick Start

### Development (File-based Storage)

```bash
# Copy example to .env
cp .env.example .env

# Run with file storage (default)
cargo run --bin cdc-cli start
```

### Development (PostgreSQL Storage)

```bash
# Edit .env and set:
CONFIG_STORAGE=postgres

# Start infrastructure
docker compose -f docker-compose.dev.yml up -d

# Run app locally
cargo run --bin cdc-cli start
```

### Production (Docker)

```bash
# Edit .env for production settings
# Make sure CONFIG_STORAGE=postgres

# Start all services
docker compose up -d
```

## Environment Variables Reference

### Application Configuration

| Variable         | Default                                             | Description                                                      |
| ---------------- | --------------------------------------------------- | ---------------------------------------------------------------- |
| `CONFIG_STORAGE` | `files`                                             | Storage backend: `files`, `postgres`, `postgresql`, or `db`      |
| `DATABASE_URL`   | `postgresql://postgres:postgres@localhost:5432/cdc` | PostgreSQL connection string (used when CONFIG_STORAGE=postgres) |
| `CONFIG_DIR`     | `config`                                            | Directory for YAML config files (used when CONFIG_STORAGE=files) |

### Logging

| Variable   | Default | Description                                          |
| ---------- | ------- | ---------------------------------------------------- |
| `RUST_LOG` | `info`  | Log level: `trace`, `debug`, `info`, `warn`, `error` |

### API Server

| Variable       | Default   | Description             |
| -------------- | --------- | ----------------------- |
| `API_HOST`     | `0.0.0.0` | API server bind address |
| `API_PORT`     | `3000`    | API server port         |
| `CORS_ENABLED` | `true`    | Enable CORS             |

### PostgreSQL (Docker)

| Variable            | Default    | Description              |
| ------------------- | ---------- | ------------------------ |
| `POSTGRES_USER`     | `postgres` | PostgreSQL username      |
| `POSTGRES_PASSWORD` | `postgres` | PostgreSQL password      |
| `POSTGRES_DB`       | `cdc`      | PostgreSQL database name |

### NATS

| Variable   | Default                 | Description         |
| ---------- | ----------------------- | ------------------- |
| `NATS_URL` | `nats://localhost:4222` | NATS connection URL |

### PgAdmin

| Variable           | Default           | Description            |
| ------------------ | ----------------- | ---------------------- |
| `PGADMIN_EMAIL`    | `admin@cdc.local` | PgAdmin login email    |
| `PGADMIN_PASSWORD` | `admin`           | PgAdmin login password |

## Configuration Scenarios

### Scenario 1: Local Development with Files

**Use case:** Quick development, testing, no database required

```bash
# .env
CONFIG_STORAGE=files
CONFIG_DIR=config
RUST_LOG=debug
```

**Start:**

```bash
cargo run --bin cdc-cli start
```

### Scenario 2: Local Development with PostgreSQL

**Use case:** Testing database features, multi-instance scenarios

```bash
# .env
CONFIG_STORAGE=postgres
DATABASE_URL=postgresql://postgres:postgres@localhost:5432/cdc
RUST_LOG=debug
```

**Setup:**

```bash
# Start PostgreSQL
docker compose -f docker-compose.dev.yml up -d postgres

# Initialize database
psql -U postgres -d cdc -f init-db.sql
psql -U postgres -d cdc -f init-config-data.sql

# Run app
cargo run --bin cdc-cli start
```

### Scenario 3: Full Docker Development

**Use case:** Testing complete Docker setup

```bash
# .env
CONFIG_STORAGE=postgres
DATABASE_URL=postgresql://postgres:postgres@postgres:5432/cdc
RUST_LOG=info
```

**Start:**

```bash
docker compose up -d
```

### Scenario 4: Production Deployment

**Use case:** Production environment

```bash
# .env (or set in deployment platform)
CONFIG_STORAGE=postgres
DATABASE_URL=postgresql://prod_user:secure_password@prod-db.example.com:5432/cdc_prod
RUST_LOG=warn
API_PORT=3000
CORS_ENABLED=false

# Strong passwords!
POSTGRES_PASSWORD=your_strong_password
PGADMIN_PASSWORD=your_strong_password
```

**Deploy:**

```bash
# Use environment-specific .env
docker compose --env-file .env.production up -d
```

## Override Priority

Environment variables are resolved in this order (highest to lowest priority):

1. **Shell environment** - `export CONFIG_STORAGE=postgres`
2. **`.env` file** - In project root
3. **docker-compose.yml** - Default values `${VAR:-default}`
4. **Application defaults** - Hardcoded in Rust code

Example:

```bash
# .env has CONFIG_STORAGE=files
# But shell override:
CONFIG_STORAGE=postgres cargo run
# Will use postgres!
```

## Security Best Practices

### For Development

✅ Use `.env` for local settings  
✅ Keep passwords simple (e.g., `postgres`)  
✅ Commit `.env.example` but NOT `.env`

### For Production

❌ **Never commit** actual `.env` files  
✅ Use secrets management (e.g., Docker Secrets, Kubernetes Secrets)  
✅ Use strong, random passwords  
✅ Use environment-specific files (`.env.production`)  
✅ Restrict `CORS_ENABLED=false` for production  
✅ Use `RUST_LOG=warn` or `error` to reduce noise

## Troubleshooting

### Issue: "Failed to connect to PostgreSQL"

**Solution:**

```bash
# Check DATABASE_URL
echo $DATABASE_URL

# Verify PostgreSQL is running
docker compose ps postgres

# Test connection
psql $DATABASE_URL -c "SELECT 1"
```

### Issue: "Configuration files not found"

**Solution:**

```bash
# Check CONFIG_DIR
echo $CONFIG_DIR

# Verify files exist
ls -la $CONFIG_DIR/*.yaml

# Or switch to PostgreSQL
export CONFIG_STORAGE=postgres
```

### Issue: Environment variables not loading

**Solution:**

```bash
# Load .env manually
set -a
source .env
set +a

# Or use with cargo
cargo run --bin cdc-cli start

# Check if loaded
env | grep CONFIG_STORAGE
```

## Examples

### Switch between backends on the fly

```bash
# Use files
CONFIG_STORAGE=files cargo run --bin cdc-cli start

# Use PostgreSQL
CONFIG_STORAGE=postgres cargo run --bin cdc-cli start
```

### Different log levels for debugging

```bash
# Verbose logging
RUST_LOG=debug cargo run --bin cdc-cli start

# Trace everything
RUST_LOG=trace cargo run --bin cdc-cli start

# Only errors
RUST_LOG=error cargo run --bin cdc-cli start
```

### Custom database connection

```bash
# Different database
DATABASE_URL=postgresql://user:pass@custom-db:5432/mydb cargo run

# Local PostgreSQL
DATABASE_URL=postgresql:///cdc cargo run
```

## Tips

💡 **Tip 1:** Use `.env` for local development - simple and fast!

💡 **Tip 2:** Use PostgreSQL for production - better reliability and concurrent access

💡 **Tip 3:** Use `docker-compose.dev.yml` to run only infrastructure, build app locally for faster iteration

💡 **Tip 4:** Create multiple `.env.*` files for different environments but don't commit them!

💡 **Tip 5:** Use `RUST_LOG=target=level` for granular logging:

```bash
RUST_LOG=cdc_core=debug,cdc_api=info cargo run
```
