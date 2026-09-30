-- Chạm vào mọi dòng để đích có lại đủ dữ liệu sau khi table/cột được tạo lại
UPDATE "Branchs" SET "BranchName" = "BranchName" || ' (sửa)';
INSERT INTO "Branchs" VALUES (3, 'DN', false);
UPDATE regions SET "Code" = "Code" || '1';
