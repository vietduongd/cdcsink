-- Mọi kiểu dữ liệu phổ biến + giá trị biên. Table không có trong sync_config -> phải giống nguồn 100%.
CREATE TYPE mood AS ENUM ('happy', 'sad', 'ok');

CREATE TABLE type_zoo (
    id          bigint PRIMARY KEY,
    c_smallint  smallint,
    c_int       integer,
    c_bigint    bigint,
    c_real      real,
    c_double    double precision,
    c_num_12_2  numeric(12,2),
    c_num_38_10 numeric(38,10),
    c_num_free  numeric,
    c_money     money,
    c_bool      boolean,
    c_text      text,
    c_varchar   varchar(20),
    c_char      char(5),
    c_uuid      uuid,
    c_date      date,
    c_time      time,
    c_timetz    timetz,
    c_ts        timestamp,
    c_ts0       timestamp(0),
    c_tstz      timestamptz,
    c_interval  interval,
    c_json      json,
    c_jsonb     jsonb,
    c_bytea     bytea,
    c_int_arr   integer[],
    c_text_arr  text[],
    c_enum      mood,
    c_inet      inet,
    c_macaddr   macaddr,
    c_xml       xml
);
ALTER TABLE type_zoo REPLICA IDENTITY FULL;

-- 1: giá trị thường
INSERT INTO type_zoo VALUES (1,
    12, 123456, 1234567890123, 1.5, 3.141592653589793,
    1234.56, 42.1234567891, 2.5, 12.34, true,
    'hello', 'varchar', 'ab', 'a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11',
    '2024-02-29', '13:45:30', '13:45:30+07', '2024-02-29 13:45:30', '2024-02-29 13:45:30', '2024-02-29 13:45:30+07',
    '1 day 02:03:04', '{"a": 1, "b": [1, 2]}', '{"lang": "vi", "n": 1.5}', '\x0102ff',
    '{1,2,3}', '{"x","y"}', 'happy', '192.168.1.10', '08:00:2b:01:02:03', '<a>1</a>');

-- 2: toàn NULL
INSERT INTO type_zoo (id) VALUES (2);

-- 3: giá trị biên lớn
INSERT INTO type_zoo VALUES (3,
    32767, 2147483647, 9223372036854775807, 3.4e38, 1.7976931348623157e308,
    9999999999.99, 1234567890123456789012345678.0123456789, 3.14159265358979323846264338327950288, 92233720368547758.07, false,
    repeat('abc', 70000), '12345678901234567890', 'abcde', 'ffffffff-ffff-ffff-ffff-ffffffffffff',
    '9999-12-31', '23:59:59.999999', '23:59:59.999999+14', '9999-12-31 23:59:59.999999', '9999-12-31 23:59:59', '9999-12-31 23:59:59.999999+00',
    '178000000 years', '[]', '[]', '\x',
    '{}', '{}', 'sad', '::1', 'ff:ff:ff:ff:ff:ff', '<a/>');

-- 4: giá trị biên nhỏ / âm / trước 1970
INSERT INTO type_zoo VALUES (4,
    -32768, -2147483648, -9223372036854775808, -1.17549435e-38, -2.2250738585072014e-308,
    -9999999999.99, -0.0000000001, -0.000000000000000000001, -92233720368547758.08, false,
    '', '', '', '00000000-0000-0000-0000-000000000000',
    '0001-01-01', '00:00:00', '00:00:00-12', '1969-12-31 23:59:59.123456', '1900-01-01 00:00:00', '1800-06-15 12:00:00+00',
    '-5 days -00:00:01', 'null', 'null', '\x00',
    '{NULL,1}', '{NULL,""}', 'ok', '10.0.0.0/8', '00:00:00:00:00:00', '');

-- 5: chuỗi đặc biệt
INSERT INTO type_zoo (id, c_text, c_varchar, c_char, c_json, c_jsonb, c_text_arr) VALUES (5,
    E'Tiếng Việt có dấu 😀 \'quote\' "double" back\\slash\nnewline\ttab',
    'null', 'NULL',
    '{"nested": {"deep": [1, {"x": null}]}, "unicode": "Đà Nẵng"}',
    '"chỉ là string"',
    '{"a,b","c\"d","e\\f"}');

-- 6: số sát ngưỡng chính xác (f64 chỉ giữ ~15-17 chữ số)
INSERT INTO type_zoo (id, c_bigint, c_num_12_2, c_num_38_10, c_num_free, c_double, c_real) VALUES (6,
    9007199254740993, 9876543210.99, 99999999.9999999999, 12345678901234567890.123456789,
    0.1, 0.1);

-- 7: thời gian quanh epoch, có microsecond
INSERT INTO type_zoo (id, c_date, c_time, c_ts, c_ts0, c_tstz) VALUES (7,
    '1970-01-01', '12:00:00.5', '1970-01-01 00:00:00.000001', '1970-01-01 00:00:00', '1970-01-01 00:00:00.5+00');
