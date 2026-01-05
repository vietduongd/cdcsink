# CDC System Architecture - Component Roles

## 🎯 Overview

CDC (Change Data Capture) System với 3-tier config management và dynamic flow control.

## 📊 Architecture Diagram

```
┌─────────────────────────────────────────────────────────┐
│                    CDC APPLICATION                      │
│  ┌────────────────────────────────────────────────┐     │
│  │           Configuration Storage                │     │
│  │  ┌──────────────┐      ┌──────────────┐        │     │
│  │  │ YAML Files   │  OR  │  PostgreSQL  │        │     │
│  │  │ (Dev)        │      │ (Production) │        │     │
│  │  └──────────────┘      └──────────────┘        │     │
│  │         ↓                      ↓               │     │
│  │     Connectors Config, Destinations Config,    │     │
│  │              Flows Config                      │     │
│  └────────────────────────────────────────────────┘     │
│                        ↓                                │
│  ┌────────────────────────────────────────────────┐     │
│  │         Flow Orchestrator (Runtime)            │     │
│  │  • Start/Stop flows dynamically                │     │
│  │  • No restart required                         │     │
│  │  • Control channels for each flow              │     │
│  └────────────────────────────────────────────────┘     │
│                        ↓                                │
│  ┌────────────────────────────────────────────────┐     │
│  │              Active Flows                      │     │
│  │                                                │     │
│  │  Flow 1: NATS → PostgreSQL                     │     │
│  │  Flow 2: Kafka → MySQL                         │     │
│  │  Flow 3: NATS → PostgreSQL + MySQL             │     │
│  └────────────────────────────────────────────────┘     │
└─────────────────────────────────────────────────────────┘
         ↑                                    ↓
    [Data Sources]                      [Destinations]
    • NATS                              • PostgreSQL
    • Kafka                             • MySQL
    • RabbitMQ                          • MongoDB
```

## 🔧 Component Roles

### 1. CDC Application (Core)

**Vai trò:** Orchestrates data flows từ sources đến destinations

**Chức năng:**

- Đọc config từ files hoặc database
- Tạo và quản lý flows runtime
- Start/stop flows động mà không restart
- Xử lý errors và retry logic

**Port:** 3000 (API)

---

### 2. PostgreSQL Database

**Vai trò:** Dual role - vừa là destination, vừa là config storage

#### Role A: CDC Events Destination (PRIMARY)

```
NATS events → CDC App → PostgreSQL (cdc_events table)
```

- **Table:** `cdc_events`
- **Purpose:** Store actual CDC events/data
- **Example:** User updates từ source system

#### Role B: Config Storage (OPTIONAL)

```
Connector/Destination/Flow configs → PostgreSQL tables
```

- **Tables:** `connectors`, `destinations`, `flows`
- **Purpose:** Store configuration metadata
- **When:** `CONFIG_STORAGE=postgres`
- **Alternative:** YAML files in `config/` directory

**Port:** 5432

**Summary:**

```
PostgreSQL làm 2 việc:
1. ✅ Nhận CDC events (LUÔN LUÔN dùng)
2. 🔧 Lưu configs (TÙY CHỌN - hoặc dùng YAML)
```

---

### 3. NATS Message Broker

**Vai trò:** Data source connector (example)

**Chức năng:**

- Publish events về changes trong source systems
- CDC app subscribe và nhận events
- Acts as message buffer

**Flow:**

```
Source System → Publishes to NATS → CDC subscribes → Writes to PostgreSQL
```

**Example:**

- Source: User service publishes "user.updated" events
- NATS: Message broker
- CDC: Consumes and transforms
- PostgreSQL: Final storage

**Port:** 4222 (client), 8222 (monitoring)

**Không bắt buộc:** Có thể thêm Kafka, RabbitMQ, hoặc sources khác

---

### 4. PgAdmin (Optional Dev Tool)

**Vai trò:** Database management UI

**Chức năng:**

- View data trong PostgreSQL
- Debug flows
- Check configs (nếu dùng Postgres storage)
- Query CDC events

**Port:** 5050

**Web UI:** http://localhost:5050

**Login:**

- Email: `admin@cdc.local`
- Password: `admin`

**Summary:** Chỉ để DEV, không cần cho production

---

## 💡 Configuration Storage Options

### Option 1: YAML Files (Default, Dev-Friendly)

```bash
CONFIG_STORAGE=files
```

**Pros:**

- ✅ Simple, human-editable
- ✅ Git-friendly
- ✅ No extra infrastructure
- ✅ Fast for development

**Cons:**

- ❌ Single instance only
- ❌ No concurrent writes
- ❌ Manual deployment

**Files:**

```
config/
├── connectors.yaml     # Data source configs
├── destinations.yaml   # Data sink configs
└── flows.yaml         # Flow definitions
```

---

### Option 2: PostgreSQL Database (Production)

```bash
CONFIG_STORAGE=postgres
DATABASE_URL=postgresql://localhost/cdc
```

**Pros:**

- ✅ Multi-instance safe
- ✅ ACID transactions
- ✅ Concurrent access
- ✅ Referential integrity
- ✅ Audit trail (timestamps)

**Cons:**

- ❌ Requires PostgreSQL
- ❌ More complexity

**Tables:**

```sql
connectors      -- Connector configs
destinations    -- Destination configs
flows          -- Flow configs (refs to connectors/destinations)
```

---

## 🚀 Data Flow Examples

### Example 1: Simple NATS → PostgreSQL

```yaml
# connectors.yaml
- name: nats-local
  connector_type: nats
  config:
    servers: ["nats://nats:4222"]
    subject: "user.events"

# destinations.yaml
- name: postgres-local
  destination_type: postgres
  config:
    url: "postgresql://postgres@postgres/cdc"

# flows.yaml
- name: user-events-flow
  connector_name: nats-local
  destination_names: [postgres-local]
  auto_start: true
```

**Runtime:**

```
1. User service publishes: NATS("user.events", {id: 123, name: "John"})
2. CDC consumes from NATS
3. CDC writes to PostgreSQL.cdc_events
4. Done!
```

---

### Example 2: Fan-out (1 source → multiple destinations)

```yaml
flows:
  - name: events-to-analytics
    connector_name: nats-production
    destination_names:
      - postgres-primary # Real-time DB
      - postgres-analytics # Analytics DB
      - elasticsearch # Search index
```

**Runtime:**

```
NATS event
  ↓
CDC App
  ├→ PostgreSQL (primary)
  ├→ PostgreSQL (analytics)
  └→ Elasticsearch
```

---

## 🎮 Control Flow

### Start System

```bash
# Development
cargo run --bin cdc-cli start

# Docker
docker compose up -d
```

**What happens:**

1. CDC app starts
2. Loads configs (files or PostgreSQL)
3. Auto-starts flows with `auto_start: true`
4. API server listens on :3000

### Add Flow Runtime (No Restart!)

```bash
# Create connector
curl -X POST http://localhost:3000/api/connectors -d '{
  "name": "kafka-events",
  "connector_type": "kafka",
  "config": {...}
}'

# Create flow
curl -X POST http://localhost:3000/api/flows -d '{
  "name": "kafka-to-postgres",
  "connector_name": "kafka-events",
  "destination_names": ["postgres-local"],
  "auto_start": true
}'

# Flow starts immediately!
```

---

## 📋 Summary

| Component      | Role                         | Required?               | Purpose                    |
| -------------- | ---------------------------- | ----------------------- | -------------------------- |
| **CDC App**    | Core                         | ✅ Yes                  | Orchestrate data flows     |
| **PostgreSQL** | Destination + Config Storage | ✅ Yes (as destination) | Store CDC events + configs |
| **NATS**       | Data Source                  | ⚠️ Example              | Publish source events      |
| **PgAdmin**    | Dev Tool                     | ❌ No                   | View database data         |

**Key Points:**

1. **PostgreSQL = 2 roles:**

   - Primary: Store CDC events (always needed)
   - Optional: Store configs (if `CONFIG_STORAGE=postgres`)

2. **NATS = Example data source:**

   - Bạn có thể thay bằng Kafka, RabbitMQ, etc.
   - Hoặc thêm nhiều sources cùng lúc
   - CDC app subscribe để nhận events

3. **PgAdmin = Dev convenience:**

   - Chỉ để view/debug data
   - Không ảnh hưởng CDC logic
   - Có thể tắt trong production

4. **Config Storage = Flexible:**
   - Dev: Dùng YAML files (simple)
   - Prod: Dùng PostgreSQL (safe)
   - Switch bằng env variable: `CONFIG_STORAGE`

---

## 🔄 Typical Workflows

### Development Workflow

```bash
# 1. Start infrastructure
docker compose -f docker-compose.dev.yml up -d

# 2. Edit configs
vim config/flows.yaml

# 3. Run CDC locally
CONFIG_STORAGE=files cargo run

# 4. View data
# → PgAdmin at http://localhost:5050
# → Check cdc_events table
```

### Production Workflow

```bash
# 1. Set env vars
export CONFIG_STORAGE=postgres
export DATABASE_URL=postgresql://prod-db/cdc

# 2. Deploy
docker compose up -d

# 3. Add flows via API
curl -X POST http://prod:3000/api/flows -d {...}

# 4. Monitor
curl http://prod:3000/api/flows  # Check status
```

---

Bây giờ clear chưa? 😊

- **NATS** = Data source (example connector)
- **PostgreSQL** = Destination AND config storage
- **PgAdmin** = Dev tool để xem data
