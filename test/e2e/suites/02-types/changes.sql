-- Streaming (op = c/u): copy toàn bộ dòng snapshot sang id + 100 -> cùng giá trị đi qua đường insert
INSERT INTO type_zoo
SELECT (jsonb_populate_record(NULL::type_zoo, to_jsonb(t) || jsonb_build_object('id', t.id + 100))).*
FROM type_zoo t;

-- Giá trị -> NULL
UPDATE type_zoo SET c_text = NULL, c_int = NULL, c_jsonb = NULL, c_num_12_2 = NULL, c_tstz = NULL WHERE id = 1;

-- NULL -> giá trị
UPDATE type_zoo SET c_int = 5, c_text = 'từ NULL', c_num_12_2 = 0.01, c_date = '2000-01-01', c_bool = false WHERE id = 2;

-- Đổi nhiều cột cùng lúc
UPDATE type_zoo SET c_smallint = -1, c_real = 2.25, c_uuid = gen_random_uuid(), c_enum = 'ok',
                    c_ts = '2030-05-05 05:05:05.555555', c_jsonb = '{"k": [true, false, null]}'
WHERE id = 101;
