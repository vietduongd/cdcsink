-- Chạy trên DB đích (receive_sync). So sánh từng table với DB nguồn qua dblink.
-- Dữ liệu nguồn là "đáp án": đích phải có đúng các dòng thỏa p_where, đúng các cột sau include/exclude,
-- và giá trị từng ô phải bằng nhau (so bằng jsonb: số so theo giá trị, 150.00 = 150).

CREATE EXTENSION IF NOT EXISTS dblink;

DROP TABLE IF EXISTS e2e_result;
CREATE TABLE e2e_result (
    severity text,   -- ERROR: sai dữ liệu / thiếu / thừa; WARN: kiểu cột khác nguồn
    tbl      text,
    problem  text,
    col      text,
    row_id   text,
    src_val  text,
    sink_val text
);

CREATE OR REPLACE FUNCTION e2e_src_cols(p_table text)
RETURNS TABLE (col text, typ text) LANGUAGE sql AS $f$
    SELECT * FROM dblink(
        'host=pg-source dbname=source_db user=postgres password=postgres',
        format($q$SELECT attname::text, format_type(atttypid, atttypmod) FROM pg_attribute
                  WHERE attrelid = to_regclass(%L) AND attnum > 0 AND NOT attisdropped$q$,
               'public.' || quote_ident(p_table))
    ) AS t(col text, typ text)
$f$;

-- Khác biệt về dạng hiển thị nhưng cùng giá trị:
--   money  : nguồn "$12.34", đích numeric 12.34
--   timetz : Debezium luôn gửi giờ UTC ("06:45:30Z" thay cho "13:45:30+07") -> so theo thời điểm
CREATE OR REPLACE FUNCTION e2e_equivalent(p_src_type text, p_src jsonb, p_dst jsonb)
RETURNS boolean LANGUAGE plpgsql AS $f$
BEGIN
    IF p_src_type = 'money' THEN
        RETURN (p_src #>> '{}')::money::numeric = (p_dst #>> '{}')::numeric;
    ELSIF p_src_type LIKE 'time% with time zone' AND p_src_type NOT LIKE 'timestamp%' THEN
        RETURN ((p_src #>> '{}')::timetz AT TIME ZONE 'UTC') = ((p_dst #>> '{}')::timetz AT TIME ZONE 'UTC');
    END IF;
    RETURN false;
EXCEPTION WHEN others THEN
    RETURN false;
END
$f$;

-- p_table   : tên table (giống nhau ở nguồn và đích)
-- p_where   : điều kiện SQL chạy trên nguồn, mô phỏng where trong sync_config
-- p_include : danh sách cột được sync (NULL = tất cả)
-- p_exclude : danh sách cột không sync
-- p_key     : cột khóa để ghép dòng
CREATE OR REPLACE FUNCTION e2e_check(
    p_table   text,
    p_where   text   DEFAULT 'true',
    p_include text[] DEFAULT NULL,
    p_exclude text[] DEFAULT '{}',
    p_key     text   DEFAULT 'id'
) RETURNS void LANGUAGE plpgsql AS $f$
DECLARE
    v_rel      text := 'public.' || quote_ident(p_table);
    v_expected text[];
BEGIN
    DROP TABLE IF EXISTS _cols, _src, _dst;

    CREATE TEMP TABLE _cols AS
    SELECT s.col, s.typ AS src_typ, d.typ AS dst_typ
    FROM e2e_src_cols(p_table) s
    LEFT JOIN (
        SELECT attname::text AS col, format_type(atttypid, atttypmod) AS typ
        FROM pg_attribute
        WHERE attrelid = to_regclass(v_rel) AND attnum > 0 AND NOT attisdropped
    ) d ON d.col = s.col
    WHERE (p_include IS NULL OR s.col = ANY (p_include) OR s.col = p_key)
      AND NOT s.col = ANY (p_exclude);

    SELECT array_agg(col) INTO v_expected FROM _cols;
    IF v_expected IS NULL THEN
        INSERT INTO e2e_result VALUES ('ERROR', p_table, 'SOURCE_TABLE_NOT_FOUND', NULL, NULL, NULL, NULL);
        RETURN;
    END IF;

    CREATE TEMP TABLE _src AS
    SELECT k, r FROM dblink(
        'host=pg-source dbname=source_db user=postgres password=postgres',
        format('SELECT %I::text, to_jsonb(t) FROM %s t WHERE %s', p_key, v_rel, p_where)
    ) AS x(k text, r jsonb);
    UPDATE _src SET r = (SELECT coalesce(jsonb_object_agg(key, value), '{}'::jsonb)
                         FROM jsonb_each(r) WHERE key = ANY (v_expected));

    IF to_regclass(v_rel) IS NULL THEN
        INSERT INTO e2e_result
        SELECT 'ERROR', p_table, 'TABLE_MISSING', NULL, NULL, count(*) || ' rows expected', NULL FROM _src;
        RETURN;
    END IF;

    INSERT INTO e2e_result
    SELECT 'ERROR', p_table, 'COLUMN_EXTRA', attname, NULL, NULL, format_type(atttypid, atttypmod)
    FROM pg_attribute
    WHERE attrelid = to_regclass(v_rel) AND attnum > 0 AND NOT attisdropped
      AND NOT attname = ANY (v_expected);

    INSERT INTO e2e_result
    SELECT 'ERROR', p_table, 'COLUMN_MISSING', col, NULL, src_typ, NULL FROM _cols WHERE dst_typ IS NULL;

    -- Bỏ qua khác biệt vô hại: varchar/char -> text, numeric(p,s) -> numeric, timestamp(0) -> timestamp
    INSERT INTO e2e_result
    SELECT 'WARN', p_table, 'TYPE_DIFF', col, NULL, src_typ, dst_typ FROM _cols
    WHERE dst_typ IS NOT NULL
      AND regexp_replace(src_typ, '\(.*?\)', '') <> regexp_replace(dst_typ, '\(.*?\)', '')
      AND NOT (src_typ ~ '^(character|text)' AND dst_typ = 'text');

    BEGIN
        EXECUTE format('CREATE TEMP TABLE _dst AS SELECT %I::text AS k, to_jsonb(t) AS r FROM %s t', p_key, v_rel);
    EXCEPTION WHEN undefined_column THEN
        INSERT INTO e2e_result VALUES ('ERROR', p_table, 'KEY_COLUMN_MISSING', p_key, NULL, NULL, NULL);
        RETURN;
    END;

    INSERT INTO e2e_result
    SELECT 'ERROR', p_table, 'ROW_MISSING', NULL, s.k, s.r::text, NULL
    FROM _src s WHERE NOT EXISTS (SELECT 1 FROM _dst d WHERE d.k = s.k);

    INSERT INTO e2e_result
    SELECT 'ERROR', p_table, 'ROW_EXTRA', NULL, d.k, NULL, d.r::text
    FROM _dst d WHERE NOT EXISTS (SELECT 1 FROM _src s WHERE s.k = d.k);

    INSERT INTO e2e_result
    SELECT 'ERROR', p_table, 'VALUE_DIFF', e.key, s.k, e.value::text, (d.r -> e.key)::text
    FROM _src s
    JOIN _dst d ON d.k = s.k
    CROSS JOIN LATERAL jsonb_each(s.r) e
    JOIN _cols c ON c.col = e.key
    WHERE d.r ? e.key AND (d.r -> e.key) IS DISTINCT FROM e.value
      AND NOT e2e_equivalent(c.src_typ, e.value, d.r -> e.key);
END
$f$;
