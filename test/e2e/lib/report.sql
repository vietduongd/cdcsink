-- Gom kết quả: mỗi (table, lỗi, cột) một dòng, kèm số dòng bị ảnh hưởng và một ví dụ.
SELECT severity,
       tbl                                                        AS "table",
       problem,
       coalesce(col, '')                                          AS "column",
       count(*)                                                   AS n,
       (array_agg(row_id ORDER BY row_id))[1]                     AS sample_id,
       left((array_agg(src_val ORDER BY row_id))[1], 60)          AS source,
       left((array_agg(sink_val ORDER BY row_id))[1], 60)         AS sink
FROM e2e_result
GROUP BY severity, tbl, problem, col
ORDER BY severity, tbl, problem, col;

SELECT 'E2E_ERRORS=' || count(*) FILTER (WHERE severity = 'ERROR')
    || ' E2E_WARNS=' || count(*) FILTER (WHERE severity = 'WARN')
FROM e2e_result;
