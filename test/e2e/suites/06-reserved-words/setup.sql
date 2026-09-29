-- Tên table/cột viết thường nhưng là từ khóa SQL -> phải được quote khi tạo table/insert ở đích.
CREATE TABLE "order" (
    id       integer PRIMARY KEY,
    "select" text,
    "from"   integer,
    "user"   text,
    "group"  text
);
ALTER TABLE "order" REPLICA IDENTITY FULL;

INSERT INTO "order" VALUES (1, 'a', 1, 'u1', 'g1'), (2, 'b', 2, 'u2', 'g2');
