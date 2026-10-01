-- Constraint chỉ có ở đích: dòng balance < 0 bị từ chối (SQLSTATE 23514) và phải vào dead-letter
-- thay vì chặn cả pipeline.
ALTER TABLE dl_accounts ADD CONSTRAINT dl_balance_non_negative CHECK (balance >= 0);
