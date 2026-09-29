UPDATE _cdc_schema_metadata SET data_type = 'BIGINT' WHERE table_name = 'fake_table';
INSERT INTO _cdc_schema_metadata (schema_name, table_name, column_name, data_type, nullable)
VALUES ('public', 'fake_table_2', 'id', 'TEXT', false);
DELETE FROM _cdc_schema_metadata WHERE table_name = 'fake_table';
INSERT INTO _cdc_schema_metadata_resync VALUES (2, 'public', 'fake_resync_2', 'id', 'TEXT', false);
INSERT INTO normal_after VALUES (2, 'streaming');
