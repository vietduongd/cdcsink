-- Hai cột chỉ khác hoa thường trong cùng table ("code" và "Code").
-- Đã biết: Debezium (pgoutput) làm mất một cột ở snapshot và crash khi gặp UPDATE (IndexOutOfBoundsException).
CREATE TABLE "Codes" (id integer PRIMARY KEY, code text, "Code" text);
ALTER TABLE "Codes" REPLICA IDENTITY FULL;

INSERT INTO "Codes" VALUES (1, 'lower', 'UPPER');
