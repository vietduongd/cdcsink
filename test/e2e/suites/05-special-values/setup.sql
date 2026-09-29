-- Giá trị đặc biệt: NaN, Infinity, infinity cho timestamp (date infinity ở suite 10, numeric NaN ở suite 11). Có thể làm Debezium hoặc cdcsink lỗi.
CREATE TABLE special_values (
    id       integer PRIMARY KEY,
    c_double double precision,
    c_real   real,
    c_num    numeric,
    c_ts     timestamp,
    c_tstz   timestamptz
);
ALTER TABLE special_values REPLICA IDENTITY FULL;

INSERT INTO special_values VALUES
    (1, 'NaN',       'NaN',       NULL,  NULL,        NULL),
    (2, 'Infinity',  'Infinity',  NULL,  NULL,        NULL),
    (3, '-Infinity', '-Infinity', NULL,  NULL,        NULL),
    (4, NULL,        NULL,        NULL,  'infinity',  'infinity'),
    (5, NULL,        NULL,        NULL,  '-infinity', '-infinity'),
    (6, 1.5,         1.5,         1.5,   '2025-01-01 00:00:00', '2025-01-01 00:00:00+00');
