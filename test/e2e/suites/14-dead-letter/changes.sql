INSERT INTO dl_accounts VALUES (5, 'e@x', 50), (6, 'f@x', -60), (7, 'g@x', 70);  -- 6 bị từ chối
UPDATE dl_accounts SET balance = -11 WHERE id = 2;                                  -- 2 bị từ chối
UPDATE dl_accounts SET balance = -22 WHERE id = 2;  -- chỉ còn bản mới nhất của 2 ở trạng thái pending
UPDATE dl_accounts SET email = 'c2@x' WHERE id = 3;
DELETE FROM dl_accounts WHERE id = 4;
