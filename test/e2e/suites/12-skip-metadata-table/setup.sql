-- DB nguồn là DB đích của một cdcsink khác (cdcsink nối tiếp): có sẵn _cdc_schema_metadata.
-- Table này không bao giờ được sync, kể cả bản _resync. Các table khác vẫn sync bình thường.
CREATE TABLE _cdc_schema_metadata (
    schema_name  text NOT NULL,
    table_name   text NOT NULL,
    column_name  text NOT NULL,
    data_type    text NOT NULL,
    nullable     boolean NOT NULL,
    last_updated timestamp NOT NULL DEFAULT now(),
    PRIMARY KEY (schema_name, table_name, column_name)
);
ALTER TABLE _cdc_schema_metadata REPLICA IDENTITY FULL;

-- Bản có cột id: nếu không bị chặn theo tên thì sẽ ghi vào metadata của đích
CREATE TABLE _cdc_schema_metadata_resync (
    id          integer PRIMARY KEY,
    schema_name text,
    table_name  text,
    column_name text,
    data_type   text,
    nullable    boolean
);
ALTER TABLE _cdc_schema_metadata_resync REPLICA IDENTITY FULL;

CREATE TABLE normal_after (id integer PRIMARY KEY, note text);
ALTER TABLE normal_after REPLICA IDENTITY FULL;

INSERT INTO _cdc_schema_metadata (schema_name, table_name, column_name, data_type, nullable)
VALUES ('public', 'fake_table', 'id', 'INTEGER', false);
INSERT INTO _cdc_schema_metadata_resync VALUES (1, 'public', 'fake_resync', 'id', 'INTEGER', false);
INSERT INTO normal_after VALUES (1, 'snapshot');
