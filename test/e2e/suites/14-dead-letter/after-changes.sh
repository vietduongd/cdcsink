# Chạy bằng source trong run_suite (xem run.sh).

dl_count() { sink_psql -tAc "select count(*) from _cdc_dead_letter where $1"; }

# Chờ tới khi điều kiện SQL trên đích đúng, hết WAIT_TIMEOUT thì báo lỗi.
dl_wait() {
    local condition=$1 label=$2 start=$SECONDS
    while (( SECONDS - start < WAIT_TIMEOUT )); do
        [[ "$(sink_psql -tAc "select ($condition)::int")" == "1" ]] && return 0
        sleep 1
    done
    echo "   hết ${WAIT_TIMEOUT}s mà chưa thấy: $label"
    stalled=1
    return 1
}

dl_expect() {
    local condition=$1 label=$2
    if [[ "$(sink_psql -tAc "select ($condition)::int")" != "1" ]]; then
        echo "   dead-letter sai: $label"
        sink_psql -c "select id, kind, primary_key, status, attempts, error_code from _cdc_dead_letter order by id"
        stalled=1
    fi
}

# 1. Dòng 2 và 6 bị từ chối, pipeline không bị chặn (dòng 3, 5, 7 đã tới đích nhờ marker changes)
dl_expect "(select string_agg(primary_key, ',' order by primary_key) from _cdc_dead_letter
            where kind = 'rejected' and status = 'pending') = '2,6'" \
          "phải có đúng dòng 2 và 6 ở trạng thái pending"
dl_expect "(select error_code from _cdc_dead_letter where primary_key = '6' and status = 'pending') = '23514'" \
          "dòng 6 phải có error_code 23514"

# 2. Bản mới hơn ghi được (constraint vẫn còn) -> bản lỗi cũ của 6 thành superseded
src_psql -c "UPDATE dl_accounts SET balance = 66 WHERE id = 6"
src_psql -c "insert into e2e_marker(id) values ('dl-supersede')"
wait_marker dl-supersede || stalled=1
dl_expect "exists (select 1 from _cdc_dead_letter where kind = 'rejected' and primary_key = '6'
                   and status = 'superseded')" \
          "dòng 6 phải thành superseded sau khi có bản hợp lệ"

# 3. Message hỏng vào dead-letter dạng poison
docker run --rm --network cdcsink-e2e_default natsio/nats-box:latest \
    nats -s nats://nats:4222 pub debezium.public.dl_garbage 'not json at all' >/dev/null 2>&1
dl_wait "exists (select 1 from _cdc_dead_letter where kind = 'poison' and status = 'pending')" \
        "message hỏng xuất hiện trong _cdc_dead_letter"

# 4. Sửa nguyên nhân rồi replay: dòng 2 phải resolved, poison replay vẫn hỏng -> về pending, attempts = 2
sink_psql -c "ALTER TABLE dl_accounts DROP CONSTRAINT dl_balance_non_negative"
sink_psql -c "UPDATE _cdc_dead_letter SET status = 'retry' WHERE status = 'pending'"
dl_wait "not exists (select 1 from _cdc_dead_letter where status = 'retry')" \
        "mọi dòng retry đã được replay"
