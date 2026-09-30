-- Table/cột ở đích bị xóa bằng tay trong khi _cdc_schema_metadata vẫn còn ghi nhận.
-- cdcsink phải tạo lại table/cột thay vì panic "relation does not exist" và crash liên tục.
CREATE TABLE "Branchs" (id integer PRIMARY KEY, "BranchName" text NOT NULL, "IsActive" boolean NOT NULL);
ALTER TABLE "Branchs" REPLICA IDENTITY FULL;
CREATE TABLE regions (id integer PRIMARY KEY, name text NOT NULL, "Code" text);
ALTER TABLE regions REPLICA IDENTITY FULL;

INSERT INTO "Branchs" VALUES (1, 'HN', true), (2, 'HCM', true);
INSERT INTO regions VALUES (1, 'Bắc', 'N'), (2, 'Nam', 'S');
