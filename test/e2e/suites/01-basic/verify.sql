SELECT e2e_check('orders',
    $$tenant_id = 5 AND status IN ('paid', 'shipped') AND deleted_at IS NULL$$,
    ARRAY['id', 'customer_id', 'total', 'status', 'created_at']);
SELECT e2e_check('users', 'true', NULL, ARRAY['password_hash', 'secret_token']);
SELECT e2e_check('OrderItems',
    $$"TenantId" = 5 AND "Status" IN ('Paid', 'Shipped')$$,
    ARRAY['id', 'OrderId', 'ProductId', 'Quantity', 'UnitPrice']);
SELECT e2e_check('UserAccounts', 'true', NULL, ARRAY['PasswordHash', 'SecurityStamp']);
SELECT e2e_check('products');
