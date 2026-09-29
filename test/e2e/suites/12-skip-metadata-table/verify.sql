SELECT e2e_check('normal_after');

-- Metadata của đích chỉ được chứa table mà chính cdcsink tạo, không có dòng nào từ nguồn
INSERT INTO e2e_result
SELECT 'ERROR', '_cdc_schema_metadata', 'SYNCED_FROM_SOURCE', column_name, table_name, NULL, data_type
FROM _cdc_schema_metadata WHERE table_name LIKE 'fake%';

INSERT INTO e2e_result
SELECT 'ERROR', '_cdc_schema_metadata', 'COLUMN_EXTRA', attname, NULL, NULL, NULL
FROM pg_attribute WHERE attrelid = '_cdc_schema_metadata'::regclass AND attnum > 0 AND NOT attisdropped
  AND attname NOT IN ('schema_name', 'table_name', 'column_name', 'data_type', 'nullable', 'last_updated');

INSERT INTO e2e_result
SELECT 'ERROR', relname, 'TABLE_CREATED', NULL, NULL, NULL, NULL
FROM pg_class WHERE relkind = 'r' AND relname ILIKE '\_cdc\_schema\_metadata\_%';
