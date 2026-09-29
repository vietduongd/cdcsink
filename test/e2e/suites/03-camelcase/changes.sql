-- CustomerProfiles: update cột camelCase, set NULL, insert mới
UPDATE "CustomerProfiles" SET "lastName" = NULL, "isVIP" = false, "creditLimit" = 7500.25, "updatedAt" = '2025-04-01 09:30:00' WHERE id = 1;
UPDATE "CustomerProfiles" SET "lastName" = 'Trần', "Metadata" = '{"tier": "silver", "tags": ["a", "b"]}' WHERE id = 2;
INSERT INTO "CustomerProfiles" (id, "firstName", "emailAddress", "userID") VALUES (3, 'Chi', 'chi@example.com', 1003);

-- Thêm cột camelCase mới ở nguồn -> đích phải tự ADD COLUMN đúng tên (có dấu nháy)
ALTER TABLE "CustomerProfiles" ADD COLUMN "loyaltyPoints" integer;
ALTER TABLE "CustomerProfiles" ADD COLUMN "LastLoginAt" timestamptz;
UPDATE "CustomerProfiles" SET "loyaltyPoints" = 120, "LastLoginAt" = '2025-05-05 05:05:05+00' WHERE id IN (1, 3);

-- SalesOrders: dòng ra/vào filter
UPDATE "SalesOrders" SET "totalAmount" = 50.00       WHERE id = 1;  -- ra (< 100) -> xóa ở đích
UPDATE "SalesOrders" SET "isActive" = false          WHERE id = 2;  -- ra
UPDATE "SalesOrders" SET "totalAmount" = 100.01      WHERE id = 3;  -- vào
UPDATE "SalesOrders" SET "orderStatus" = 'Shipped'   WHERE id = 6;  -- vào
UPDATE "SalesOrders" SET "internalNote" = 'đổi note' WHERE id = 9;  -- cột không sync
INSERT INTO "SalesOrders" (id, "orderNo", "tenantId", "customerId", "totalAmount", "orderStatus", "isActive")
VALUES (10, 'SO-010', 5, 3, 1000.00, 'Paid', true);                 -- vào

-- SalesOrderLines
UPDATE "SalesOrderLines" SET qty = 5, "costPrice" = 31.00 WHERE "productCode" = 'P-01';
INSERT INTO "SalesOrderLines" VALUES ('aaaaaaaa-0000-0000-0000-000000000003', 10, 'P-03', 10, 100.00, 80.00, 'x');

-- PK "Id"
UPDATE "Invoices" SET "Amount" = 175.00 WHERE "Id" = 1;
INSERT INTO "Invoices" VALUES (3, 'INV-003', 999.99);

-- Table khác hoa thường: đổi cả hai
UPDATE "Tags" SET "Name" = 'Tag Hoa (sửa)' WHERE id = 1;
UPDATE tags   SET name   = 'tag thuong (sua)' WHERE id = 1;
INSERT INTO "Tags" VALUES (3, 'chỉ ở Tags');

INSERT INTO "AUDIT_LOG" VALUES (2, 'LOGOUT', NULL, '2025-01-02 00:00:00');
