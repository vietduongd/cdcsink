-- Tên table và cột CamelCase / camelCase / UPPER / trộn, kiểu EF Core.

-- Không có config: phải sync y nguyên
CREATE TABLE "CustomerProfiles" (
    id               bigint PRIMARY KEY,
    "firstName"      text NOT NULL,
    "lastName"       text,
    "emailAddress"   varchar(200),
    "isVIP"          boolean NOT NULL DEFAULT false,
    "HTMLContent"    text,
    "userID"         integer,
    "createdAt"      timestamptz NOT NULL DEFAULT '2025-01-01 08:00:00+07',
    "updatedAt"      timestamp,
    "birthDate"      date,
    "creditLimit"    numeric(14,2),
    "Metadata"       jsonb,
    snake_case_col   text,
    "mixed_Case_Col" text
);
ALTER TABLE "CustomerProfiles" REPLICA IDENTITY FULL;

-- Có config include/where theo cột camelCase
CREATE TABLE "SalesOrders" (
    id             bigint PRIMARY KEY,
    "orderNo"      text NOT NULL,
    "tenantId"     integer NOT NULL,
    "customerId"   bigint,
    "totalAmount"  numeric(12,2),
    "orderStatus"  text,
    "isActive"     boolean NOT NULL,
    "deletedAt"    timestamp,
    "internalNote" text,
    "createdAt"    timestamp NOT NULL DEFAULT '2025-01-01 00:00:00'
);
ALTER TABLE "SalesOrders" REPLICA IDENTITY FULL;

-- Config key viết thường "salesorderlines", PK uuid
CREATE TABLE "SalesOrderLines" (
    id             uuid PRIMARY KEY,
    "salesOrderId" bigint NOT NULL,
    "productCode"  text NOT NULL,
    qty            integer NOT NULL,
    "unitPrice"    numeric(12,2) NOT NULL,
    "costPrice"    numeric(12,2),
    "internalNote" text
);
ALTER TABLE "SalesOrderLines" REPLICA IDENTITY FULL;

-- Khóa chính "Id" (mặc định của EF Core), không phải "id"
CREATE TABLE "Invoices" (
    "Id"        integer PRIMARY KEY,
    "InvoiceNo" text NOT NULL,
    "Amount"    numeric(12,2) NOT NULL
);
ALTER TABLE "Invoices" REPLICA IDENTITY FULL;

-- Hai table chỉ khác hoa thường
CREATE TABLE "Tags" (id integer PRIMARY KEY, "Name" text NOT NULL);
CREATE TABLE tags   (id integer PRIMARY KEY, name   text NOT NULL);
ALTER TABLE "Tags" REPLICA IDENTITY FULL;
ALTER TABLE tags   REPLICA IDENTITY FULL;

-- Toàn chữ hoa
CREATE TABLE "AUDIT_LOG" (id bigint PRIMARY KEY, "EVENT_TYPE" text NOT NULL, "PAYLOAD" jsonb, "CREATED_AT" timestamp);
ALTER TABLE "AUDIT_LOG" REPLICA IDENTITY FULL;

-- ---- Seed ----
INSERT INTO "CustomerProfiles" (id, "firstName", "lastName", "emailAddress", "isVIP", "HTMLContent", "userID",
                                "updatedAt", "birthDate", "creditLimit", "Metadata", snake_case_col, "mixed_Case_Col") VALUES
    (1, 'An',   'Nguyễn', 'an@example.com',   true,  '<b>VIP</b>', 1001, '2025-03-01 10:00:00', '1990-05-20', 5000.00, '{"tier": "gold"}', 's1', 'm1'),
    (2, 'Bình', NULL,     'binh@example.com', false, NULL,         NULL, NULL,                  NULL,         NULL,    NULL,               NULL, NULL);

INSERT INTO "SalesOrders" (id, "orderNo", "tenantId", "customerId", "totalAmount", "orderStatus", "isActive", "deletedAt", "internalNote") VALUES
    (1, 'SO-001', 5, 1, 150.00, 'Paid',    true,  NULL,  'n'),   -- sync
    (2, 'SO-002', 5, 1, 250.50, 'Shipped', true,  NULL,  'n'),   -- sync
    (3, 'SO-003', 5, 2, 99.99,  'Paid',    true,  NULL,  'n'),   -- loại: < 100
    (4, 'SO-004', 5, 2, 500.00, 'Paid',    false, NULL,  'n'),   -- loại: isActive
    (5, 'SO-005', 6, 2, 500.00, 'Paid',    true,  NULL,  'n'),   -- loại: tenant
    (6, 'SO-006', 5, 2, 500.00, 'Draft',   true,  NULL,  'n'),   -- loại: status
    (7, 'SO-007', 5, 2, 500.00, 'paid',    true,  NULL,  'n'),   -- loại: 'paid' khác 'Paid'
    (8, 'SO-008', 5, 2, 500.00, 'Paid',    true,  now(), 'n'),   -- loại: deletedAt
    (9, 'SO-009', 5, 2, 100.00, 'Paid',    true,  NULL,  'n');   -- sync: đúng bằng 100 (gte)

INSERT INTO "SalesOrderLines" VALUES
    ('aaaaaaaa-0000-0000-0000-000000000001', 1, 'P-01', 2, 50.00, 30.00, 'secret'),
    ('aaaaaaaa-0000-0000-0000-000000000002', 2, 'P-02', 1, 250.50, 200.00, NULL);

INSERT INTO "Invoices" VALUES (1, 'INV-001', 150.00), (2, 'INV-002', 250.50);

INSERT INTO "Tags" VALUES (1, 'Tag Hoa'), (2, 'Tag Hoa 2');
INSERT INTO tags   VALUES (1, 'tag thuong'), (3, 'tag thuong 3');

INSERT INTO "AUDIT_LOG" VALUES (1, 'LOGIN', '{"ip": "1.2.3.4"}', '2025-01-01 00:00:00');
