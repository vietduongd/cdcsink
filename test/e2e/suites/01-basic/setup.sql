-- Dữ liệu nguồn khớp với test/e2e/sync_config.yaml.
-- REPLICA IDENTITY FULL để message update/delete có đủ cột trong "before".

CREATE TABLE orders (
    id          BIGSERIAL PRIMARY KEY,
    tenant_id   INTEGER NOT NULL,
    customer_id INTEGER NOT NULL,
    total       NUMERIC(12,2),
    status      TEXT NOT NULL,
    note        TEXT,
    created_at  TIMESTAMP NOT NULL DEFAULT now(),
    deleted_at  TIMESTAMP
);
ALTER TABLE orders REPLICA IDENTITY FULL;

CREATE TABLE users (
    id            BIGSERIAL PRIMARY KEY,
    email         TEXT NOT NULL,
    full_name     TEXT,
    password_hash TEXT,
    secret_token  TEXT,
    is_active     BOOLEAN NOT NULL DEFAULT true,
    profile       JSONB,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
ALTER TABLE users REPLICA IDENTITY FULL;

CREATE TABLE "OrderItems" (
    id          BIGSERIAL PRIMARY KEY,
    "OrderId"   BIGINT NOT NULL,
    "ProductId" INTEGER NOT NULL,
    "Quantity"  INTEGER NOT NULL,
    "UnitPrice" NUMERIC(12,2) NOT NULL,
    "TenantId"  INTEGER NOT NULL,
    "Status"    TEXT NOT NULL,
    "Internal"  TEXT
);
ALTER TABLE "OrderItems" REPLICA IDENTITY FULL;

CREATE TABLE "UserAccounts" (
    id              UUID PRIMARY KEY,
    "UserName"      TEXT NOT NULL,
    "PasswordHash"  TEXT,
    "SecurityStamp" TEXT,
    "BirthDate"     DATE
);
ALTER TABLE "UserAccounts" REPLICA IDENTITY FULL;

-- Không có trong sync_config -> sync toàn bộ cột/dòng
CREATE TABLE products (
    id       SERIAL PRIMARY KEY,
    sku      TEXT NOT NULL,
    name     TEXT NOT NULL,
    price    NUMERIC(12,2),
    in_stock BOOLEAN NOT NULL DEFAULT true
);
ALTER TABLE products REPLICA IDENTITY FULL;

-- ---- Seed: đi qua snapshot của Debezium (op = "r") ----
INSERT INTO orders (tenant_id, customer_id, total, status, deleted_at) VALUES
    (5, 101, 150.00, 'paid',    NULL),   -- id 1: sync
    (5, 102, 99.90,  'shipped', NULL),   -- id 2: sync
    (5, 103, 20.00,  'pending', NULL),   -- id 3: loại (status)
    (7, 104, 500.00, 'paid',    NULL),   -- id 4: loại (tenant)
    (5, 105, 75.50,  'paid',    now());  -- id 5: loại (deleted_at)

INSERT INTO users (email, full_name, password_hash, secret_token, profile) VALUES
    ('a@example.com', 'Alice', 'hash-a', 'tok-a', '{"lang":"vi"}'),
    ('b@example.com', 'Bob',   'hash-b', 'tok-b', NULL);

INSERT INTO "OrderItems" ("OrderId", "ProductId", "Quantity", "UnitPrice", "TenantId", "Status", "Internal") VALUES
    (1, 1, 2, 50.00,  5, 'Paid',    'x'),  -- id 1: sync
    (2, 2, 1, 99.90,  5, 'Shipped', 'x'),  -- id 2: sync
    (3, 1, 1, 20.00,  5, 'Pending', 'x'),  -- id 3: loại
    (4, 3, 5, 100.00, 7, 'Paid',    'x');  -- id 4: loại

INSERT INTO "UserAccounts" (id, "UserName", "PasswordHash", "SecurityStamp", "BirthDate") VALUES
    ('11111111-1111-1111-1111-111111111111', 'alice', 'ph-a', 'ss-a', '1990-01-15'),
    ('22222222-2222-2222-2222-222222222222', 'bob',   'ph-b', 'ss-b', NULL);

INSERT INTO products (sku, name, price) VALUES
    ('SKU-1', 'Keyboard', 50.00),
    ('SKU-2', 'Mouse',    99.90),
    ('SKU-3', 'Monitor',  100.00);
