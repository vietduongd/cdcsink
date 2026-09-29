-- Vòng đời thay đổi: update dồn dập, delete, insert+delete cùng transaction, đổi khóa chính, khối lượng lớn.
CREATE TABLE accounts (
    id      integer PRIMARY KEY,
    balance numeric(12,2) NOT NULL,
    status  text NOT NULL,
    version integer NOT NULL
);
ALTER TABLE accounts REPLICA IDENTITY FULL;

CREATE TABLE text_pk (
    id  text PRIMARY KEY,
    val text
);
ALTER TABLE text_pk REPLICA IDENTITY FULL;

CREATE TABLE events (
    id         bigint PRIMARY KEY,
    kind       text NOT NULL,
    amount     numeric(10,2),
    created_at timestamp NOT NULL DEFAULT '2025-01-01 00:00:00'
);
ALTER TABLE events REPLICA IDENTITY FULL;

INSERT INTO accounts SELECT g, 100.00, 'active', 1 FROM generate_series(1, 10) g;
INSERT INTO text_pk VALUES ('a', '1'), ('b', '2'), ('with space', '3'), ('ü-ñ', '4');
