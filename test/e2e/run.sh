#!/usr/bin/env bash
# Chạy các suite e2e, mỗi suite trên một stack sạch (down -v rồi up lại).
#
#   test/e2e/run.sh                 # chạy tất cả suite
#   test/e2e/run.sh 02-types        # chạy một suite
#   KEEP=1 test/e2e/run.sh 03-camelcase   # giữ stack sau khi chạy để tự xem DB
#
# Mỗi suite gồm:
#   setup.sql   : tạo table + dữ liệu ban đầu ở nguồn (đi qua snapshot, op = "r")
#   changes.sql : thay đổi sau snapshot (đi qua streaming, op = c/u/d) — tùy chọn
#   verify.sql  : các lệnh SELECT e2e_check(...) chạy trên DB đích
#
# Biến môi trường: WAIT_TIMEOUT (giây, mặc định 120), KEEP=1.

set -uo pipefail
cd "$(dirname "$0")/../.."
export MSYS_NO_PATHCONV=1

DC="docker compose -f docker-compose.test.yml"
E2E=test/e2e
WAIT_TIMEOUT=${WAIT_TIMEOUT:-120}

src_psql()  { $DC exec -T pg-source psql -U postgres -d source_db -v ON_ERROR_STOP=1 -q "$@"; }
sink_psql() { $DC exec -T pg-sink psql -U postgres -d receive_sync -v ON_ERROR_STOP=1 -q -P pager=off "$@"; }

wait_tcp() {
    local svc=$1 db=$2
    for _ in $(seq 1 60); do
        $DC exec -T "$svc" psql -h 127.0.0.1 -U postgres -d "$db" -tAc 'select 1' >/dev/null 2>&1 && return 0
        sleep 1
    done
    return 1
}

restart_count() {
    docker inspect -f '{{.RestartCount}}' "$($DC ps -a -q cdcsink)" 2>/dev/null || echo 0
}

# Chờ token marker xuất hiện ở đích. Trả về 1 nếu hết giờ hoặc cdcsink crash liên tục.
wait_marker() {
    local token=$1 start=$SECONDS
    while (( SECONDS - start < WAIT_TIMEOUT )); do
        if [[ "$(sink_psql -tAc "select 1 from e2e_marker where id = '$token'" 2>/dev/null)" == "1" ]]; then
            return 0
        fi
        if (( $(restart_count) >= 3 )); then
            echo "   cdcsink restart $(restart_count) lần -> dừng chờ"
            return 1
        fi
        sleep 2
    done
    echo "   hết ${WAIT_TIMEOUT}s mà chưa thấy marker '$token' ở đích"
    return 1
}

run_suite() {
    local name=$1 dir=$E2E/suites/$1 stalled=0
    echo
    echo "================================================================"
    echo " SUITE $name"
    echo "================================================================"

    $DC down -v --remove-orphans >/dev/null 2>&1
    $DC up -d pg-source pg-sink nats >/dev/null 2>&1
    wait_tcp pg-source source_db && wait_tcp pg-sink receive_sync || { echo "   Postgres không lên"; return 1; }

    src_psql < $E2E/lib/common-setup.sql
    if ! src_psql < "$dir/setup.sql"; then
        echo "   setup.sql lỗi"; RESULTS+=("$name|SETUP_FAILED"); return
    fi
    src_psql -c "insert into e2e_marker(id) values ('snapshot')"

    $DC up -d >/dev/null 2>&1
    echo " - chờ snapshot đi hết pipeline..."
    wait_marker snapshot || stalled=1

    if [[ $stalled == 0 && -f "$dir/changes.sql" ]]; then
        echo " - chạy changes.sql..."
        if ! src_psql < "$dir/changes.sql"; then
            echo "   changes.sql lỗi"; RESULTS+=("$name|CHANGES_FAILED"); return
        fi
        src_psql -c "insert into e2e_marker(id) values ('changes')"
        wait_marker changes || stalled=1
    fi

    # Marker có thể đã được ghi trước khi cdcsink panic ở một message khác cùng batch -> kiểm tra panic riêng
    sleep 5
    local panics
    panics=$($DC logs --no-log-prefix cdcsink 2>&1 | grep -c 'panicked at')
    if (( panics > 0 )); then
        stalled=1
        echo "   cdcsink panic $panics lần:"
        $DC logs --no-log-prefix cdcsink 2>&1 | grep -A1 'panicked at' | grep -v '^--' | sort -u | head -4 \
            | cut -c1-240 | sed 's/^/   | /'
    fi

    echo " - so sánh nguồn và đích:"
    local out
    out=$( { echo "SET client_min_messages = warning;"; cat $E2E/lib/verify.sql
            echo '\o /dev/null'; cat "$dir/verify.sql"; echo '\o'
            cat $E2E/lib/report.sql; } | sink_psql 2>&1)
    echo "$out" | grep -v '^E2E_' | grep -v '^\s*?column?' | sed 's/^/   /'
    local counts
    counts=$(echo "$out" | grep -o 'E2E_ERRORS=[0-9]* E2E_WARNS=[0-9]*')

    local dbz_errors
    dbz_errors=$($DC logs --no-log-prefix debezium 2>&1 | grep ' ERROR ' | grep -v 'already shut down' \
        | cut -c1-240 | sort -u | head -3)
    if [[ -n "$dbz_errors" ]]; then
        echo " - Debezium báo lỗi:"
        echo "$dbz_errors" | sed 's/^/   | /'
    fi

    if [[ $stalled == 1 ]]; then
        RESULTS+=("$name|CRASH/STALLED (panic=$panics) $counts")
    else
        RESULTS+=("$name|$counts")
    fi
}

RESULTS=()
if (( $# > 0 )); then suites=("$@"); else suites=($(ls $E2E/suites)); fi

echo "Build cdcsink..."
$DC build cdcsink >/dev/null 2>&1 || { echo "Build lỗi"; exit 1; }

for s in "${suites[@]}"; do run_suite "$s"; done

[[ "${KEEP:-0}" == "1" ]] || $DC down -v --remove-orphans >/dev/null 2>&1

echo
echo "================================ TỔNG KẾT ================================"
failed=0
for r in "${RESULTS[@]}"; do
    name=${r%%|*}; res=${r#*|}
    # known-issue.txt: lỗi đã biết ngoài cdcsink (vd giới hạn của Debezium), không tính là FAIL
    if [[ "$res" == "E2E_ERRORS=0 "* ]]; then mark=PASS
    elif [[ -f "$E2E/suites/$name/known-issue.txt" ]]; then mark=KNOWN; res="$res — $(head -1 "$E2E/suites/$name/known-issue.txt")"
    else mark=FAIL; failed=1; fi
    printf ' %-5s  %-38s %s\n' "$mark" "$name" "$res"
done
exit $failed
