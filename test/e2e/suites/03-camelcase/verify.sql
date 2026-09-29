SELECT e2e_check('CustomerProfiles');
SELECT e2e_check('SalesOrders',
    $$"tenantId" = 5 AND "orderStatus" IN ('Paid', 'Shipped') AND "deletedAt" IS NULL
      AND "totalAmount" >= 100 AND "isActive"$$,
    ARRAY['id', 'orderNo', 'customerId', 'totalAmount', 'orderStatus', 'isActive', 'createdAt']);
SELECT e2e_check('SalesOrderLines', 'true', NULL, ARRAY['internalNote', 'costPrice']);
SELECT e2e_check('Invoices', 'true', NULL, '{}', 'Id');
SELECT e2e_check('Tags');
SELECT e2e_check('tags');
SELECT e2e_check('AUDIT_LOG');
