-- numeric 'NaN': Debezium (decimal.handling.mode=precise) không biểu diễn được, ghi log lỗi và gửi NULL.
-- Đây là giới hạn của Debezium, không phải cdcsink. Dùng decimal.handling.mode=string nếu cần giữ NaN.
CREATE TABLE numeric_nan (
    id    integer PRIMARY KEY,
    c_num numeric
);
ALTER TABLE numeric_nan REPLICA IDENTITY FULL;

INSERT INTO numeric_nan VALUES (1, 1.5), (2, 'NaN');
