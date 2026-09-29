-- Kịch bản thay đổi, chạy SAU khi snapshot xong. Kết quả mong đợi ở DB đích ghi trong comment.

-- 1. Insert mới khớp filter -> xuất hiện ở đích
INSERT INTO orders (tenant_id, customer_id, total, status) VALUES (5, 201, 10.00, 'paid');

-- 2. Update làm dòng rơi khỏi filter -> bị XÓA ở đích (orders id 1)
UPDATE orders SET status = 'cancelled' WHERE id = 1;

-- 3. Update làm dòng lọt vào filter -> xuất hiện ở đích (orders id 3)
UPDATE orders SET status = 'paid' WHERE id = 3;

-- 4. Update cột không sync -> đích không có cột note
UPDATE orders SET note = 'khong duoc sync' WHERE id = 2;

-- 5. Soft delete -> xóa ở đích (orders id 2)
UPDATE orders SET deleted_at = now() WHERE id = 2;

-- 6. Hard delete (op = "d") trên table không có config -> mong đợi products id 3 biến mất ở đích
DELETE FROM products WHERE id = 3;

-- 7. Thêm cột mới ở nguồn -> cdcsink tự ADD COLUMN ở đích
ALTER TABLE products ADD COLUMN weight_kg NUMERIC(6,2);
UPDATE products SET weight_kg = 1.25 WHERE id = 1;

-- 8. Cột bị exclude không sync, cột khác vẫn cập nhật
UPDATE users SET password_hash = 'new-hash', full_name = 'Alice Nguyen' WHERE id = 1;

-- 9. Table CamelCase
UPDATE "OrderItems" SET "Status" = 'Shipped', "Quantity" = 3 WHERE id = 1;
UPDATE "OrderItems" SET "TenantId" = 9 WHERE id = 2;   -- rơi khỏi filter -> xóa ở đích

-- 10. Batch lớn để test pull nhiều message
INSERT INTO products (sku, name, price)
SELECT 'BULK-' || g, 'Bulk ' || g, (g % 100) + 0.99 FROM generate_series(1, 5000) g;
