-- Table không có trong sync_config: sync toàn bộ cột, toàn bộ dòng.
CREATE TABLE dl_accounts (
    id      integer PRIMARY KEY,
    email   text NOT NULL,
    balance numeric(12,2) NOT NULL
);
ALTER TABLE dl_accounts REPLICA IDENTITY FULL;

INSERT INTO dl_accounts VALUES (1, 'a@x', 10), (2, 'b@x', 20), (3, 'c@x', 30), (4, 'd@x', 40);
