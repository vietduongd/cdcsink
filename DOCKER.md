# CDC Data Sync System - Docker Deployment

## 🐳 Quick Start

### Prerequisites

- Docker Desktop installed
- Docker Compose installed

### Start Everything

```bash
# Build and start all services
docker compose up -d --build

# View logs
docker compose logs -f

# View specific service logs
docker compose logs -f cdc-app
```

### Access Services

- **CDC API**: http://localhost:3000
- **NATS Monitoring**: http://localhost:8222
- **PostgreSQL**: localhost:5432
- **PgAdmin**: http://localhost:5050
  - Email: admin@admin.com
  - Password: admin

## 📦 Services

### 1. NATS (Message Broker)

- **Container**: `cdc-nats`
- **Ports**: 4222 (client), 8222 (monitoring)
- **Features**: JetStream enabled

### 2. PostgreSQL (Database)

- **Container**: `cdc-postgres`
- **Port**: 5432
- **Database**: cdc
- **Credentials**: postgres/postgres

### 3. CDC Application (Rust)

- **Container**: `cdc-app`
- **Port**: 3000 (API)
- **Config**: Uses `config/docker.yaml`
- **Auto-restart**: Enabled

### 4. PgAdmin (Database UI)

- **Container**: `cdc-pgadmin`
- **Port**: 5050

### 5. React UI (Optional)

Uncomment in docker-compose.yml to enable

## 🛠️ Commands

### Start services

```bash
docker compose up -d
```

### Stop services

```bash
docker compose down
```

### Rebuild after code changes

```bash
docker compose up -d --build cdc-app
```

### View application logs

```bash
docker compose logs -f cdc-app
```

### Restart a service

```bash
docker compose restart cdc-app
```

### Shell into container

```bash
docker compose exec cdc-app sh
```

### Remove everything (including volumes)

```bash
docker compose down -v
```

## 📝 Configuration

### Using Custom Config

Edit `config/docker.yaml` and rebuild:

```bash
docker compose up -d --build cdc-app
```

### Environment Variables

Set in `docker-compose.yml`:

```yaml
cdc-app:
  environment:
    RUST_LOG: debug # Change log level
```

## 🧪 Testing

### Publish test message to NATS

```bash
# Install NATS CLI
docker run --rm -it --network cdc-network natsio/nats-box:latest

# Inside container
nats pub cdc.events '{"id":"123","timestamp":"2025-12-29T00:00:00Z","source":"test","table":"users","operation":"insert","data":{"id":1,"name":"Test"},"metadata":{}}'
```

### Check PostgreSQL

```bash
docker compose exec postgres psql -U postgres -d cdc

# In psql
SELECT * FROM cdc_events ORDER BY timestamp DESC LIMIT 10;
```

## 🔍 Troubleshooting

### Check service health

```bash
docker compose ps
```

### View all logs

```bash
docker compose logs
```

### Check network connectivity

```bash
docker compose exec cdc-app ping nats
docker compose exec cdc-app ping postgres
```

### Rebuild from scratch

```bash
docker compose down -v
docker compose build --no-cache
docker compose up -d
```

## 📊 Resource Usage

### View resource consumption

```bash
docker stats
```

### Limit resources (add to docker-compose.yml)

```yaml
cdc-app:
  deploy:
    resources:
      limits:
        cpus: "1"
        memory: 512M
```

## 🚀 Production Deployment

### Use production config

```bash
docker compose -f docker-compose.yml -f docker-compose.prod.yml up -d
```

### Enable UI

Uncomment the `ui` service in docker-compose.yml

### Security

- Change default passwords
- Use secrets instead of environment variables
- Enable TLS/SSL
- Use non-root users (already configured)

## 📚 File Structure

```
/
├── Dockerfile              # CDC app container
├── docker-compose.yml      # All services
├── .dockerignore          # Build exclusions
├── config/
│   ├── default.yaml       # Local development
│   └── docker.yaml        # Docker deployment
└── ui/
    ├── Dockerfile         # UI container
    └── nginx.conf        # Nginx config
```
