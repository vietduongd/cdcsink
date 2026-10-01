SELECT e2e_check('dl_accounts');

-- Chỉ dòng 2 được replay thành công
INSERT INTO e2e_result (severity, tbl, problem, sink_val)
SELECT 'ERROR', '_cdc_dead_letter', 'rejected resolved phải đúng là dòng 2',
       coalesce(string_agg(primary_key, ',' ORDER BY primary_key), '(không có)')
FROM _cdc_dead_letter WHERE kind = 'rejected' AND status = 'resolved'
HAVING coalesce(string_agg(primary_key, ',' ORDER BY primary_key), '') <> '2';

-- Không còn dòng rejected nào mở
INSERT INTO e2e_result (severity, tbl, problem, sink_val)
SELECT 'ERROR', '_cdc_dead_letter', 'còn dòng rejected pending/retry', count(*)::text
FROM _cdc_dead_letter WHERE kind = 'rejected' AND status IN ('pending', 'retry')
HAVING count(*) > 0;

-- Bản lỗi cũ của 6 bị thay thế
INSERT INTO e2e_result (severity, tbl, problem)
SELECT 'ERROR', '_cdc_dead_letter', 'dòng 6 không có bản superseded'
WHERE NOT EXISTS (SELECT 1 FROM _cdc_dead_letter
                  WHERE kind = 'rejected' AND primary_key = '6' AND status = 'superseded');

-- Review Focus #4: poison replay vẫn hỏng -> về pending, attempts tăng lên 2
INSERT INTO e2e_result (severity, tbl, problem, sink_val)
SELECT 'ERROR', '_cdc_dead_letter', 'poison phải còn đúng 1 dòng pending với attempts = 2',
       coalesce(string_agg(status || '/' || attempts, ','), '(không có)')
FROM _cdc_dead_letter WHERE kind = 'poison'
HAVING count(*) <> 1 OR bool_or(status <> 'pending' OR attempts <> 2);
