-- Tên có khoảng trắng, gạch ngang, dấu tiếng Việt, số ở đầu.
CREATE TABLE "customer list" (
    id             integer PRIMARY KEY,
    "full name"    text,
    "e-mail"       text,
    "tên_khách"    text,
    "1st_order_at" timestamp
);
ALTER TABLE "customer list" REPLICA IDENTITY FULL;

INSERT INTO "customer list" VALUES (1, 'An Nguyễn', 'an@example.com', 'An', '2025-01-01 00:00:00');
