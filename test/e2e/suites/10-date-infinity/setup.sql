-- date 'infinity' / '-infinity': Debezium gửi số ngày cực lớn.
CREATE TABLE date_infinity (
    id     integer PRIMARY KEY,
    c_date date
);
ALTER TABLE date_infinity REPLICA IDENTITY FULL;

INSERT INTO date_infinity VALUES (1, '2025-01-01'), (2, 'infinity'), (3, '-infinity');
