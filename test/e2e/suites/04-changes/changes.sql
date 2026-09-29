-- 1. 200 update liên tiếp cùng một dòng -> đích phải giữ bản cuối (version 201, balance 300)
DO $$ BEGIN
    FOR i IN 1..200 LOOP
        UPDATE accounts SET version = version + 1, balance = balance + 1 WHERE id = 1;
    END LOOP;
END $$;

-- 2. Nhiều update, mỗi update một transaction riêng
UPDATE accounts SET version = version + 1 WHERE id = 5;
UPDATE accounts SET version = version + 1 WHERE id = 5;
UPDATE accounts SET version = version + 1 WHERE id = 5;
UPDATE accounts SET status = 'frozen' WHERE id = 5;

-- 3. Insert -> update -> delete trong cùng transaction -> đích không được có id 50
BEGIN;
INSERT INTO accounts VALUES (50, 1.00, 'temp', 1);
UPDATE accounts SET balance = 2.00 WHERE id = 50;
DELETE FROM accounts WHERE id = 50;
COMMIT;

-- 4. Hard delete
DELETE FROM accounts WHERE id = 2;

-- 5. Delete rồi insert lại cùng id với giá trị khác
DELETE FROM accounts WHERE id = 3;
INSERT INTO accounts VALUES (3, 999.99, 'reborn', 1);

-- 6. Đổi khóa chính: id 4 -> 1000 (đích phải mất id 4, có id 1000)
UPDATE accounts SET id = 1000 WHERE id = 4;

-- 7. Transaction nhiều table
BEGIN;
UPDATE accounts SET balance = balance - 10 WHERE id = 6;
INSERT INTO events VALUES (1, 'withdraw', 10.00);
COMMIT;

-- 8. Khối lượng lớn: insert 20k, update 20k, delete 5k
INSERT INTO events (id, kind, amount) SELECT g, 'bulk', g % 1000 + 0.5 FROM generate_series(2, 20001) g;
UPDATE events SET amount = amount * 2, kind = 'bulk-updated' WHERE kind = 'bulk';
DELETE FROM events WHERE id BETWEEN 15002 AND 20001;

-- 9. Khóa text đặc biệt
INSERT INTO text_pk VALUES ('it''s', 'nháy đơn'), ('', 'rỗng'), ('"quoted"', 'nháy kép');
DELETE FROM text_pk WHERE id = 'b';
UPDATE text_pk SET val = 'sửa' WHERE id = 'with space';
