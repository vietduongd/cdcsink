# Dead-letter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Dòng bị DB đích từ chối vì lỗi dữ liệu và message hỏng được tách ra, lưu vào `_cdc_dead_letter` ở DB đích, pipeline chạy tiếp, và vận hành replay được bằng SQL.

**Architecture:** Lỗi ghi được phân loại theo SQLSTATE (`WriteError`). Khi một table gặp lỗi dữ liệu, `write_isolating` chia đôi batch để tìm đúng dòng lỗi; lỗi tạm thời vẫn retry cả batch như hiện tại. Dòng dữ liệu được tách khỏi message NATS (`ChangeRow` + `parse_change`) để luồng nhận và luồng replay dùng chung một đường ghi. Mọi thao tác trên `_cdc_dead_letter` của một batch chạy trong một transaction, trước khi ack.

**Tech Stack:** Rust 2024, tokio, sqlx 0.8 (Postgres), async-nats 0.46, tracing. E2E: docker compose + Debezium Server + bash.

**Spec:** `docs/superpowers/specs/2026-09-30-dead-letter-design.md`

## Global Constraints

- Repo **không dùng git**: bỏ qua mọi bước commit; mỗi task kết thúc bằng `cargo test` và `cargo build` xanh.
- Chạy lệnh từ thư mục `cdcsink/` (nơi có `Cargo.toml`).
- Lỗi dữ liệu = SQLSTATE bắt đầu bằng `22` hoặc `23`; mọi lỗi khác (kể cả lỗi không phải từ DB) là lỗi tạm thời.
- Table dead-letter tên `_cdc_dead_letter`, nằm trong schema `DATABASE_SCHEMA_EXPECT`.
- Trạng thái: `pending`, `retry`, `resolved`, `superseded`, `discarded`. cdcsink chỉ đọc `retry` và chỉ đổi trạng thái của dòng `pending`/`retry`.
- `kind`: `poison` | `rejected`.
- Biến env mới: `DEAD_LETTER_REPLAY_SECS`, mặc định `30`, `0` để tắt.
- Replay lấy tối đa 500 dòng mỗi lần, `ORDER BY id`.
- Không bao giờ ack message khi dữ liệu của nó chưa nằm ở đích hoặc ở `_cdc_dead_letter`.
- DDL lỗi (`CREATE TABLE`, `ADD COLUMN`) vẫn là lỗi tạm thời (retry rồi thoát), không vào dead-letter.
- Code mới theo phong cách hiện có: comment tiếng Việt ngắn, log bằng `tracing` với field có cấu trúc, lỗi nội bộ dạng `Result<_, String>`.
- Warning `dead_code`/`unused` của code mới được phép tồn tại giữa các task; sau Task 7 `cargo build` chỉ còn 2 warning cũ trong `data_record.rs`.

## Review Focus

1. **Khóa chính dạng chuỗi** (`id` là text/uuid): `primary_key` lưu trong dead-letter phải là `abc`, không phải `"abc"` có dấu nháy JSON, để vận hành lọc bằng `primary_key = 'abc'` → test `string_primary_key_has_no_json_quotes` (Task 3).
2. **Nhiều bản của cùng khóa chính trong một batch**: chỉ bản có `index` lớn nhất được ghi, kết quả theo thứ tự `index` ổn định → test `latest_per_key_keeps_newest_in_index_order` (Task 3).
3. **Mọi dòng trong batch đều lỗi**: bisection trả về đủ tất cả các dòng, không lặp vô hạn, không ghi dòng nào → test `every_row_bad_returns_every_row` (Task 2).
4. **Replay một payload vẫn hỏng**: dòng quay về `pending`, `attempts` tăng, service không thoát → e2e: poison được đặt `retry` rồi phải về `pending` với `attempts = 2` (Task 8).
5. **Payload không phải UTF-8**: không panic, trở thành poison với payload đã thay ký tự lỗi bằng U+FFFD → test `non_utf8_payload_is_poison_not_panic` (Task 3).

---

## File Structure

| File | Trách nhiệm |
|------|-------------|
| `src/models/write_error.rs` (mới) | `WriteError`: phân loại lỗi ghi theo SQLSTATE |
| `src/models/isolate.rs` (mới) | `write_isolating`: bisection tìm dòng lỗi, generic, không phụ thuộc DB |
| `src/models/change.rs` (mới) | `RowSource`, `ChangeRow`, `PoisonMessage`, `SkipReason`, `Parsed`, `parse_change`, `latest_per_key`, `normalize_table_name`, `is_ignored_table` |
| `src/models/dead_letter.rs` (mới) | `DeadLetterStore`: tạo table, ghi, supersede, lấy `retry`, đánh dấu `resolved` |
| `src/models/nats_receive.rs` | Fetch từ NATS → `ReceivedBatch` qua `parse_change`; ack |
| `src/models/postgres_destination.rs` | DDL (giữ nguyên), `upsert_rows` / `delete_rows` trả `WriteError` |
| `src/models/mod.rs` | Khai báo module và re-export |
| `src/main.rs` | Vòng lặp chính, `Sink`, `persist_batch`, `process_batch`, `flush_table`, replay |
| `.env.example`, `docker-compose.test.yml` | `DEAD_LETTER_REPLAY_SECS` |
| `test/e2e/run.sh` | Hook tùy chọn `after-changes.sh` |
| `test/e2e/suites/14-dead-letter/*` (mới) | Suite e2e |

---

### Task 1: `WriteError` — phân loại lỗi ghi theo SQLSTATE

**Files:**
- Create: `src/models/write_error.rs`
- Modify: `src/models/mod.rs`

**Interfaces:**
- Consumes: không có.
- Produces:
  - `pub enum WriteError { Data { code: String, message: String }, Transient(String) }` (derive `Debug, Clone, PartialEq`)
  - `WriteError::from_sqlstate(code: Option<&str>, message: String) -> WriteError`
  - `WriteError::context(self, context: &str) -> WriteError` — thêm `"{context}: "` vào đầu `message`, giữ nguyên loại.
  - `impl From<sqlx::Error> for WriteError`
  - `impl Display`: `Data` → `"{message} (SQLSTATE {code})"`, `Transient` → `"{message}"`.
  - Re-export `crate::models::WriteError`.

- [ ] **Step 1: Viết test (file mới, chỉ có test và khai báo tối thiểu để thấy fail)**

Tạo `src/models/write_error.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn classify(code: Option<&str>) -> WriteError {
        WriteError::from_sqlstate(code, "boom".to_string())
    }

    fn data(code: &str) -> WriteError {
        WriteError::Data {
            code: code.to_string(),
            message: "boom".to_string(),
        }
    }

    #[test]
    fn integrity_and_data_exceptions_are_data_errors() {
        assert_eq!(classify(Some("23514")), data("23514")); // check_violation
        assert_eq!(classify(Some("23502")), data("23502")); // not_null_violation
        assert_eq!(classify(Some("23505")), data("23505")); // unique_violation
        assert_eq!(classify(Some("22P02")), data("22P02")); // invalid_text_representation
        assert_eq!(classify(Some("22003")), data("22003")); // numeric_value_out_of_range
    }

    #[test]
    fn other_errors_are_transient() {
        // mất kết nối, deadlock, table không tồn tại, admin shutdown, lỗi không phải từ DB
        for code in [Some("08006"), Some("40P01"), Some("42P01"), Some("57P01"), None] {
            assert_eq!(classify(code), WriteError::Transient("boom".to_string()));
        }
    }

    #[test]
    fn non_database_sqlx_error_is_transient() {
        assert!(matches!(
            WriteError::from(sqlx::Error::PoolTimedOut),
            WriteError::Transient(_)
        ));
    }

    #[test]
    fn context_prefixes_message_and_keeps_kind() {
        let err = classify(Some("23514")).context("Failed to upsert into table users");
        assert_eq!(
            err,
            WriteError::Data {
                code: "23514".to_string(),
                message: "Failed to upsert into table users: boom".to_string(),
            }
        );
        assert_eq!(
            err.to_string(),
            "Failed to upsert into table users: boom (SQLSTATE 23514)"
        );
        assert_eq!(
            classify(None).context("Failed to delete").to_string(),
            "Failed to delete: boom"
        );
    }
}
```

Thêm vào `src/models/mod.rs` (sau dòng `mod sync_config;`):

```rust
mod write_error;
```

- [ ] **Step 2: Chạy test, xác nhận fail**

Run: `cargo test write_error`
Expected: lỗi biên dịch `cannot find type WriteError`.

- [ ] **Step 3: Viết implementation**

Thêm vào **đầu** `src/models/write_error.rs` (trước `#[cfg(test)]`):

```rust
use std::fmt;

/// Lỗi khi ghi vào DB đích. Phân loại theo SQLSTATE để quyết định tách dòng ra dead-letter
/// (lỗi do dữ liệu) hay retry cả batch (lỗi tạm thời).
#[derive(Debug, Clone, PartialEq)]
pub enum WriteError {
    /// SQLSTATE 22xxx (sai giá trị, cast lỗi, tràn số) hoặc 23xxx (NOT NULL, UNIQUE, CHECK, FK):
    /// ghi lại dòng đó bao nhiêu lần cũng lỗi.
    Data { code: String, message: String },
    /// Mọi lỗi khác: mất kết nối, deadlock, table bị drop...
    Transient(String),
}

impl WriteError {
    pub fn from_sqlstate(code: Option<&str>, message: String) -> Self {
        match code {
            Some(code) if code.starts_with("22") || code.starts_with("23") => WriteError::Data {
                code: code.to_string(),
                message,
            },
            _ => WriteError::Transient(message),
        }
    }

    /// Thêm ngữ cảnh (thao tác, table) vào đầu thông báo lỗi.
    pub fn context(self, context: &str) -> Self {
        match self {
            WriteError::Data { code, message } => WriteError::Data {
                code,
                message: format!("{}: {}", context, message),
            },
            WriteError::Transient(message) => {
                WriteError::Transient(format!("{}: {}", context, message))
            }
        }
    }
}

impl From<sqlx::Error> for WriteError {
    fn from(e: sqlx::Error) -> Self {
        let code = e
            .as_database_error()
            .and_then(|db| db.code())
            .map(|code| code.into_owned());
        WriteError::from_sqlstate(code.as_deref(), e.to_string())
    }
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WriteError::Data { code, message } => write!(f, "{} (SQLSTATE {})", message, code),
            WriteError::Transient(message) => f.write_str(message),
        }
    }
}

```

Thêm re-export vào `src/models/mod.rs` (sau các `pub use` hiện có):

```rust
pub use write_error::WriteError;
```

- [ ] **Step 4: Chạy test, xác nhận pass**

Run: `cargo test write_error`
Expected: 4 test PASS. `cargo test` toàn bộ vẫn PASS.

---

### Task 2: `write_isolating` — bisection tìm dòng lỗi

**Files:**
- Create: `src/models/isolate.rs`
- Modify: `src/models/mod.rs`

**Interfaces:**
- Consumes: `WriteError` (Task 1).
- Produces:
  - `pub struct Rejected<'a, T> { pub item: &'a T, pub code: String, pub message: String }` (derive `Debug`)
  - `pub async fn write_isolating<'a, T, F, Fut>(items: Vec<&'a T>, write: F) -> Result<Vec<Rejected<'a, T>>, String> where F: FnMut(Vec<&'a T>) -> Fut, Fut: Future<Output = Result<(), WriteError>>`
    - `Ok(rejected)`: mọi dòng không bị từ chối đã được ghi.
    - `Err(message)`: gặp `WriteError::Transient`, dừng ngay.
    - Không gọi `write` với vec rỗng.
  - Re-export `crate::models::{Rejected, write_isolating}`.

- [ ] **Step 1: Viết test**

Tạo `src/models/isolate.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, collections::HashSet};

    /// DB giả: chunk chứa phần tử thuộc `bad` thì lỗi dữ liệu, ngược lại ghi thành công.
    /// `transient_on_call`: lần gọi thứ n (tính từ 1) trả lỗi tạm thời.
    struct FakeDb {
        bad: HashSet<i32>,
        transient_on_call: Option<usize>,
        calls: RefCell<usize>,
        written: RefCell<Vec<i32>>,
    }

    impl FakeDb {
        fn new(bad: &[i32]) -> Self {
            FakeDb {
                bad: bad.iter().copied().collect(),
                transient_on_call: None,
                calls: RefCell::new(0),
                written: RefCell::new(Vec::new()),
            }
        }

        fn write(&self, chunk: Vec<&i32>) -> std::future::Ready<Result<(), WriteError>> {
            let call = {
                let mut calls = self.calls.borrow_mut();
                *calls += 1;
                *calls
            };
            let result = if Some(call) == self.transient_on_call {
                Err(WriteError::Transient("connection reset".to_string()))
            } else if let Some(bad) = chunk.iter().find(|item| self.bad.contains(item)) {
                Err(WriteError::Data {
                    code: "23514".to_string(),
                    message: format!("row {} violates check", bad),
                })
            } else {
                self.written.borrow_mut().extend(chunk.iter().copied());
                Ok(())
            };
            std::future::ready(result)
        }

        fn calls(&self) -> usize {
            *self.calls.borrow()
        }

        fn written_sorted(&self) -> Vec<i32> {
            let mut written = self.written.borrow().clone();
            written.sort();
            written
        }
    }

    fn rejected_sorted(rejected: &[Rejected<'_, i32>]) -> Vec<i32> {
        let mut items: Vec<i32> = rejected.iter().map(|r| *r.item).collect();
        items.sort();
        items
    }

    #[tokio::test]
    async fn no_errors_writes_whole_batch_once() {
        let db = FakeDb::new(&[]);
        let data: Vec<i32> = (1..=100).collect();
        let rejected = write_isolating(data.iter().collect(), |c| db.write(c))
            .await
            .unwrap();
        assert!(rejected.is_empty());
        assert_eq!(db.calls(), 1);
        assert_eq!(db.written_sorted(), data);
    }

    #[tokio::test]
    async fn empty_input_does_not_call_writer() {
        let db = FakeDb::new(&[]);
        let rejected = write_isolating(Vec::<&i32>::new(), |c| db.write(c))
            .await
            .unwrap();
        assert!(rejected.is_empty());
        assert_eq!(db.calls(), 0);
    }

    #[tokio::test]
    async fn single_bad_row_in_1000_is_found_with_few_statements() {
        let db = FakeDb::new(&[537]);
        let data: Vec<i32> = (1..=1000).collect();
        let rejected = write_isolating(data.iter().collect(), |c| db.write(c))
            .await
            .unwrap();
        assert_eq!(rejected_sorted(&rejected), vec![537]);
        assert_eq!(rejected[0].code, "23514");
        assert_eq!(rejected[0].message, "row 537 violates check");
        // 1 + 2·k·⌈log₂n⌉ với k = 1, n = 1000
        assert!(db.calls() <= 21, "calls = {}", db.calls());
        let expected: Vec<i32> = data.into_iter().filter(|x| *x != 537).collect();
        assert_eq!(db.written_sorted(), expected);
    }

    #[tokio::test]
    async fn several_bad_rows_are_all_isolated() {
        let db = FakeDb::new(&[1, 50, 51, 100]);
        let data: Vec<i32> = (1..=100).collect();
        let rejected = write_isolating(data.iter().collect(), |c| db.write(c))
            .await
            .unwrap();
        assert_eq!(rejected_sorted(&rejected), vec![1, 50, 51, 100]);
        let expected: Vec<i32> = data
            .into_iter()
            .filter(|x| ![1, 50, 51, 100].contains(x))
            .collect();
        assert_eq!(db.written_sorted(), expected);
    }

    // Review Focus #3: mọi dòng đều lỗi
    #[tokio::test]
    async fn every_row_bad_returns_every_row() {
        let data: Vec<i32> = (1..=8).collect();
        let db = FakeDb::new(&data);
        let rejected = write_isolating(data.iter().collect(), |c| db.write(c))
            .await
            .unwrap();
        assert_eq!(rejected_sorted(&rejected), data);
        assert!(db.written_sorted().is_empty());
    }

    #[tokio::test]
    async fn transient_error_mid_bisection_aborts() {
        let mut db = FakeDb::new(&[7]);
        db.transient_on_call = Some(2);
        let data: Vec<i32> = (1..=16).collect();
        let result = write_isolating(data.iter().collect(), |c| db.write(c)).await;
        assert_eq!(result.unwrap_err(), "connection reset");
        assert_eq!(db.calls(), 2);
    }
}
```

Thêm vào `src/models/mod.rs` (sau `mod decimal;`):

```rust
mod isolate;
```

- [ ] **Step 2: Chạy test, xác nhận fail**

Run: `cargo test isolate`
Expected: lỗi biên dịch `cannot find function write_isolating` / `cannot find type Rejected`.

- [ ] **Step 3: Viết implementation**

Thêm vào **đầu** `src/models/isolate.rs`:

```rust
use std::future::Future;

use crate::models::WriteError;

/// Một dòng bị DB đích từ chối vì lỗi dữ liệu.
#[derive(Debug)]
pub struct Rejected<'a, T> {
    pub item: &'a T,
    pub code: String,
    pub message: String,
}

/// Ghi `items` bằng `write`; gặp lỗi dữ liệu thì chia đôi và ghi lại từng nửa cho đến khi
/// còn đúng dòng lỗi. Với k dòng lỗi trong n dòng, số lần gọi `write` ≤ 1 + 2·k·⌈log₂n⌉.
///
/// Trả về các dòng bị từ chối; mọi dòng khác đã được ghi. Gặp lỗi tạm thời thì dừng ngay và
/// trả `Err` để cả batch được retry (mọi thao tác ghi đều idempotent nên chạy lại là an toàn).
/// Thứ tự giữa hai nửa không quan trọng vì mỗi khóa chính chỉ còn một bản.
pub async fn write_isolating<'a, T, F, Fut>(
    items: Vec<&'a T>,
    mut write: F,
) -> Result<Vec<Rejected<'a, T>>, String>
where
    F: FnMut(Vec<&'a T>) -> Fut,
    Fut: Future<Output = Result<(), WriteError>>,
{
    let mut rejected = Vec::new();
    let mut pending = vec![items];
    while let Some(chunk) = pending.pop() {
        if chunk.is_empty() {
            continue;
        }
        match write(chunk.clone()).await {
            Ok(()) => {}
            Err(WriteError::Transient(message)) => return Err(message),
            Err(WriteError::Data { code, message }) if chunk.len() == 1 => {
                rejected.push(Rejected {
                    item: chunk[0],
                    code,
                    message,
                });
            }
            Err(WriteError::Data { .. }) => {
                let mut left = chunk;
                let right = left.split_off(left.len() / 2);
                pending.push(right);
                pending.push(left);
            }
        }
    }
    Ok(rejected)
}

```

Thêm re-export vào `src/models/mod.rs`:

```rust
pub use isolate::{Rejected, write_isolating};
```

- [ ] **Step 4: Chạy test, xác nhận pass**

Run: `cargo test isolate`
Expected: 6 test PASS. `cargo test` toàn bộ PASS.

---

### Task 3: `change.rs` — dòng dữ liệu tách khỏi message NATS, `parse_change`

**Files:**
- Create: `src/models/change.rs`
- Modify: `src/models/nats_receive.rs` (xóa `normalize_table_name`, `is_ignored_table` và module `tests`, import từ `change`)
- Modify: `src/models/mod.rs`

**Interfaces:**
- Consumes: `DataRecord`, `DataModel`, `RowAction`, `SyncConfig`, `sync_config::{MatchOutcome, classify, primary_key_column}`, `postgres_destination::SCHEMA_METADATA_TABLE` (đều đã có).
- Produces (tất cả `pub` trong `crate::models::change`):
  - `RowSource { subject: String, stream_sequence: Option<u64>, payload: Arc<str>, dead_letter_id: Option<i64> }` (derive `Debug, Clone`); `RowSource::new(subject: &str, stream_sequence: Option<u64>, payload: &[u8]) -> RowSource` (UTF-8 lossy, `dead_letter_id: None`).
  - `ChangeRow { table_name: String, table_value: HashMap<String, DataModel>, primary_key: String, action: RowAction, index: i64, source: RowSource }` (derive `Debug`).
  - `PoisonMessage { source: RowSource, reason: String }` (derive `Debug`).
  - `SkipReason { Tombstone, IgnoredTable(String), NoPrimaryKey(String) }` (derive `Debug, PartialEq`).
  - `Parsed { Row { row: ChangeRow, outcome: MatchOutcome }, Skip(SkipReason), Poison(PoisonMessage) }` (derive `Debug`).
  - `parse_change(source: RowSource, index: i64, sync_config: Option<&SyncConfig>) -> Parsed`
  - `latest_per_key<'a>(rows: &[&'a ChangeRow]) -> Vec<&'a ChangeRow>` — một bản mới nhất mỗi `primary_key`, sắp theo `index` tăng dần.
  - `normalize_table_name(name: &str) -> String`, `is_ignored_table(name: &str) -> bool` (chuyển từ `nats_receive.rs`, giữ nguyên hành vi).
  - Re-export `crate::models::{ChangeRow, Parsed, PoisonMessage, RowSource, latest_per_key, parse_change}`.

- [ ] **Step 1: Viết test**

Tạo `src/models/change.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Payload Debezium tối thiểu: cột khóa `key_field` kiểu `key_type` và cột `name`.
    fn payload(table: &str, op: &str, key_field: &str, key_type: &str, row: Value) -> String {
        let (before, after) = if op == "d" {
            (row, Value::Null)
        } else {
            (Value::Null, row)
        };
        json!({
            "schema": {
                "type": "struct", "optional": false, "name": "t.Envelope", "version": 1,
                "fields": [
                    { "type": "struct", "optional": true, "field": "after", "fields": [
                        { "type": key_type, "optional": false, "field": key_field },
                        { "type": "string", "optional": true, "field": "name" }
                    ]}
                ]
            },
            "payload": {
                "before": before,
                "after": after,
                "source": {
                    "version": "2", "connector": "postgresql", "name": "s", "ts_ms": 0,
                    "snapshot": "false", "db": "db", "schema": "public", "table": table
                },
                "transaction": null,
                "op": op
            }
        })
        .to_string()
    }

    fn parse(payload: &str) -> Parsed {
        let source = RowSource::new("debezium.public.accounts", Some(7), payload.as_bytes());
        parse_change(source, 3, None)
    }

    fn expect_row(parsed: Parsed) -> ChangeRow {
        match parsed {
            Parsed::Row { row, .. } => row,
            other => panic!("expected Row, got {:?}", other),
        }
    }

    fn expect_poison(parsed: Parsed) -> PoisonMessage {
        match parsed {
            Parsed::Poison(message) => message,
            other => panic!("expected Poison, got {:?}", other),
        }
    }

    fn expect_skip(parsed: Parsed) -> SkipReason {
        match parsed {
            Parsed::Skip(reason) => reason,
            other => panic!("expected Skip, got {:?}", other),
        }
    }

    #[test]
    fn valid_payload_becomes_row() {
        let text = payload("accounts", "c", "id", "int32", json!({ "id": 5, "name": "a" }));
        let row = expect_row(parse(&text));
        assert_eq!(row.table_name, "accounts");
        assert_eq!(row.primary_key, "5");
        assert_eq!(row.action, RowAction::Upsert);
        assert_eq!(row.index, 3);
        assert_eq!(row.source.subject, "debezium.public.accounts");
        assert_eq!(row.source.stream_sequence, Some(7));
        assert_eq!(&*row.source.payload, text.as_str());
        assert_eq!(row.source.dead_letter_id, None);
        assert_eq!(row.table_value["name"].value, json!("a"));
    }

    // Review Focus #1: khóa chuỗi không kèm dấu nháy JSON
    #[test]
    fn string_primary_key_has_no_json_quotes() {
        let text = payload("accounts", "c", "id", "string", json!({ "id": "abc", "name": "a" }));
        assert_eq!(expect_row(parse(&text)).primary_key, "abc");
    }

    #[test]
    fn delete_op_is_delete() {
        let text = payload("accounts", "d", "id", "int32", json!({ "id": 5, "name": "a" }));
        let row = expect_row(parse(&text));
        assert_eq!(row.action, RowAction::Delete);
        assert_eq!(row.primary_key, "5");
    }

    #[test]
    fn resync_suffix_is_stripped() {
        let text = payload("accounts_resync", "r", "id", "int32", json!({ "id": 1, "name": "a" }));
        assert_eq!(expect_row(parse(&text)).table_name, "accounts");
    }

    #[test]
    fn invalid_json_is_poison() {
        let poison = expect_poison(parse("not json at all"));
        assert!(poison.reason.starts_with("invalid payload:"), "{}", poison.reason);
        assert_eq!(&*poison.source.payload, "not json at all");
        assert_eq!(poison.source.stream_sequence, Some(7));
    }

    // Review Focus #5: payload không phải UTF-8
    #[test]
    fn non_utf8_payload_is_poison_not_panic() {
        let source = RowSource::new("debezium.public.x", None, &[0xff, 0xfe, b'{']);
        assert!(source.payload.contains('\u{FFFD}'));
        let poison = expect_poison(parse_change(source, 0, None));
        assert!(poison.reason.starts_with("invalid payload:"));
    }

    #[test]
    fn missing_after_schema_is_poison() {
        let text = json!({
            "schema": { "type": "struct", "optional": false, "name": "t", "version": 1, "fields": [] },
            "payload": {
                "before": null, "after": { "id": 1 },
                "source": {
                    "version": "2", "connector": "postgresql", "name": "s", "ts_ms": 0,
                    "snapshot": "false", "db": "db", "schema": "public", "table": "accounts"
                },
                "transaction": null, "op": "c"
            }
        })
        .to_string();
        assert_eq!(expect_poison(parse(&text)).reason, "missing table structure");
    }

    #[test]
    fn empty_payload_is_tombstone() {
        assert_eq!(expect_skip(parse("")), SkipReason::Tombstone);
        assert_eq!(expect_skip(parse("  \n")), SkipReason::Tombstone);
    }

    #[test]
    fn metadata_table_is_skipped() {
        let text = payload("_cdc_schema_metadata", "c", "id", "int32", json!({ "id": 1, "name": "a" }));
        assert_eq!(
            expect_skip(parse(&text)),
            SkipReason::IgnoredTable("_cdc_schema_metadata".to_string())
        );
    }

    #[test]
    fn table_without_id_is_skipped() {
        let text = payload("accounts", "c", "code", "int32", json!({ "code": 1, "name": "a" }));
        assert_eq!(
            expect_skip(parse(&text)),
            SkipReason::NoPrimaryKey("accounts".to_string())
        );
    }

    fn row(primary_key: &str, index: i64) -> ChangeRow {
        ChangeRow {
            table_name: "accounts".to_string(),
            table_value: HashMap::new(),
            primary_key: primary_key.to_string(),
            action: RowAction::Upsert,
            index,
            source: RowSource::new("s", None, b"{}"),
        }
    }

    // Review Focus #2: nhiều bản cùng khóa chính trong một batch
    #[test]
    fn latest_per_key_keeps_newest_in_index_order() {
        let rows = [row("1", 0), row("2", 1), row("1", 2), row("3", 3), row("2", 4)];
        let refs: Vec<&ChangeRow> = rows.iter().collect();
        let latest: Vec<(&str, i64)> = latest_per_key(&refs)
            .into_iter()
            .map(|r| (r.primary_key.as_str(), r.index))
            .collect();
        assert_eq!(latest, vec![("1", 2), ("3", 3), ("2", 4)]);
    }

    // Chuyển từ nats_receive.rs
    #[test]
    fn strips_resync_suffix() {
        assert_eq!(normalize_table_name("orders_resync"), "orders");
        assert_eq!(normalize_table_name("OrderItems_resync"), "OrderItems");
        assert_eq!(normalize_table_name("orders"), "orders");
        assert_eq!(normalize_table_name("resync_log"), "resync_log");
    }

    #[test]
    fn ignores_cdc_internal_tables() {
        assert!(is_ignored_table("_cdc_schema_metadata"));
        assert!(is_ignored_table("_CDC_Schema_Metadata"));
        // bản _resync được chuẩn hóa trước khi kiểm tra
        assert!(is_ignored_table(&normalize_table_name(
            "_cdc_schema_metadata_resync"
        )));
        assert!(!is_ignored_table("cdc_schema_metadata"));
        assert!(!is_ignored_table("orders"));
    }
}
```

Thêm vào `src/models/mod.rs` (dòng đầu tiên, trước `mod data_record;`):

```rust
mod change;
```

- [ ] **Step 2: Chạy test, xác nhận fail**

Run: `cargo test change`
Expected: lỗi biên dịch `cannot find type RowSource` (và các tên khác).

- [ ] **Step 3: Viết implementation**

Thêm vào **đầu** `src/models/change.rs`:

```rust
use std::{collections::HashMap, sync::Arc};

use serde_json::Value;

use crate::models::{
    DataModel, DataRecord, RowAction, SyncConfig,
    postgres_destination::SCHEMA_METADATA_TABLE,
    sync_config::{MatchOutcome, classify, primary_key_column},
};

/// Nguồn gốc của một dòng: message NATS, hoặc một dòng `_cdc_dead_letter` khi replay.
#[derive(Debug, Clone)]
pub struct RowSource {
    pub subject: String,
    pub stream_sequence: Option<u64>,
    /// Payload gốc Debezium. Byte không phải UTF-8 được thay bằng U+FFFD.
    pub payload: Arc<str>,
    /// Some khi dòng đến từ replay dead-letter.
    pub dead_letter_id: Option<i64>,
}

impl RowSource {
    pub fn new(subject: &str, stream_sequence: Option<u64>, payload: &[u8]) -> Self {
        RowSource {
            subject: subject.to_string(),
            stream_sequence,
            payload: String::from_utf8_lossy(payload).into(),
            dead_letter_id: None,
        }
    }
}

/// Thay đổi của một dòng, đã áp sync_config, sẵn sàng ghi vào đích.
#[derive(Debug)]
pub struct ChangeRow {
    pub table_name: String,
    pub table_value: HashMap<String, DataModel>,
    /// Khóa chính dạng text: chuỗi không kèm dấu nháy, kiểu khác theo JSON (vd `42`).
    pub primary_key: String,
    pub action: RowAction,
    /// Thứ tự trong batch: cùng khóa chính thì index lớn hơn là bản mới hơn.
    pub index: i64,
    pub source: RowSource,
}

/// Message không parse được, sẽ được ghi vào dead-letter.
#[derive(Debug)]
pub struct PoisonMessage {
    pub source: RowSource,
    pub reason: String,
}

/// Message bỏ qua có chủ đích: ack luôn, không vào dead-letter.
#[derive(Debug, PartialEq)]
pub enum SkipReason {
    /// Tombstone của Debezium (tombstones.on.delete=true): không mang dữ liệu
    Tombstone,
    IgnoredTable(String),
    NoPrimaryKey(String),
}

#[derive(Debug)]
pub enum Parsed {
    Row { row: ChangeRow, outcome: MatchOutcome },
    Skip(SkipReason),
    Poison(PoisonMessage),
}

/// Bỏ hậu tố `_resync` để message resync dùng chung table và config với table gốc.
pub fn normalize_table_name(name: &str) -> String {
    name.strip_suffix("_resync").unwrap_or(name).to_string()
}

/// Table không bao giờ sync. `_cdc_schema_metadata` là metadata cdcsink tự tạo ở DB đích:
/// khi DB đích lại là nguồn của một tầng CDC khác (cdcsink nối tiếp cdcsink),
/// sync table này sẽ ghi đè metadata của tầng sau.
pub fn is_ignored_table(name: &str) -> bool {
    name.eq_ignore_ascii_case(SCHEMA_METADATA_TABLE)
}

/// Parse payload Debezium thành `ChangeRow`. Dùng chung cho message NATS và replay dead-letter.
pub fn parse_change(source: RowSource, index: i64, sync_config: Option<&SyncConfig>) -> Parsed {
    if source.payload.trim().is_empty() {
        return Parsed::Skip(SkipReason::Tombstone);
    }
    let data_record: DataRecord = match serde_json::from_str(&source.payload) {
        Ok(record) => record,
        Err(e) => return poison(source, format!("invalid payload: {}", e)),
    };
    let Some(raw_table_name) = data_record.get_table_name() else {
        return poison(source, "missing table name".to_string());
    };
    let table_name = normalize_table_name(&raw_table_name);
    if is_ignored_table(&table_name) {
        return Parsed::Skip(SkipReason::IgnoredTable(table_name));
    }
    let Some(mut table_value) = data_record.get_table_structure() else {
        return poison(source, "missing table structure".to_string());
    };
    let Some(primary_key) =
        primary_key_column(&table_value).map(|column| key_text(&table_value[column].value))
    else {
        return Parsed::Skip(SkipReason::NoPrimaryKey(table_name));
    };

    let table_config = sync_config.and_then(|config| config.table(&table_name));
    // Debezium xóa dòng bằng op "d"; "before" có thể chỉ chứa khóa chính nên không xét where
    let (action, outcome) = if data_record.payload.op == "d" {
        (RowAction::Delete, MatchOutcome::default())
    } else {
        classify(&table_value, table_config)
    };
    if let Some(config) = table_config {
        config.retain_columns(&mut table_value);
    }
    Parsed::Row {
        row: ChangeRow {
            table_name,
            table_value,
            primary_key,
            action,
            index,
            source,
        },
        outcome,
    }
}

/// Mỗi khóa chính chỉ giữ bản mới nhất (index lớn nhất), sắp theo index để kết quả ổn định.
pub fn latest_per_key<'a>(rows: &[&'a ChangeRow]) -> Vec<&'a ChangeRow> {
    let mut latest: HashMap<&'a str, &'a ChangeRow> = HashMap::new();
    for &row in rows {
        let key = row.primary_key.as_str();
        if latest.get(key).is_none_or(|existing| existing.index < row.index) {
            latest.insert(key, row);
        }
    }
    let mut result: Vec<&'a ChangeRow> = latest.into_values().collect();
    result.sort_by_key(|row| row.index);
    result
}

fn poison(source: RowSource, reason: String) -> Parsed {
    Parsed::Poison(PoisonMessage { source, reason })
}

fn key_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

```

Trong `src/models/nats_receive.rs`:
- Xóa hai hàm `normalize_table_name` và `is_ignored_table` (kèm doc comment của chúng).
- Xóa toàn bộ `#[cfg(test)] mod tests { ... }` ở cuối file (hai test đã chuyển sang `change.rs`).
- Trong khối `use crate::models::{ ... }`, bỏ `postgres_destination::SCHEMA_METADATA_TABLE,` và thêm `change::{is_ignored_table, normalize_table_name},`.

Thêm re-export vào `src/models/mod.rs`:

```rust
pub use change::{ChangeRow, Parsed, PoisonMessage, RowSource, latest_per_key, parse_change};
```

- [ ] **Step 4: Chạy test, xác nhận pass**

Run: `cargo test`
Expected: toàn bộ PASS (test `change::tests::*` mới + test cũ; hai test `nats_receive::tests::*` giờ nằm ở `change::tests`).

---

### Task 4: `DeadLetterStore` — table `_cdc_dead_letter`

**Files:**
- Create: `src/models/dead_letter.rs`
- Modify: `src/models/change.rs` (`is_ignored_table` bỏ qua cả `_cdc_dead_letter`, cập nhật test)
- Modify: `src/models/postgres_destination.rs:489` (`fn quote_identifier` → `pub(crate) fn quote_identifier`)
- Modify: `src/models/mod.rs`

**Interfaces:**
- Consumes: `ChangeRow`, `PoisonMessage` (Task 3), `PostgresDestination::quote_identifier`.
- Produces:
  - `pub const DEAD_LETTER_TABLE: &str = "_cdc_dead_letter";`
  - `pub enum DeadLetterKind { Poison, Rejected }` với `as_str() -> &'static str` (`"poison"`, `"rejected"`).
  - `pub struct NewDeadLetter<'a> { kind, subject: &'a str, stream_sequence: Option<u64>, table_name: Option<&'a str>, primary_key: Option<&'a str>, payload: &'a str, error_code: Option<&'a str>, error_message: &'a str, existing_id: Option<i64> }`
    - `NewDeadLetter::rejected(row: &'a ChangeRow, code: &'a str, message: &'a str) -> Self`
    - `NewDeadLetter::poison(message: &'a PoisonMessage) -> Self`
  - `pub struct RetryEntry { pub id: i64, pub subject: String, pub stream_sequence: Option<i64>, pub payload: String }`
  - `pub struct DeadLetterStore` với:
    - `new(schema: &str) -> Self`
    - `async ensure_table(&self, pool: &PgPool) -> Result<(), String>`
    - `async record(&self, entry: &NewDeadLetter<'_>, conn: &mut PgConnection) -> Result<i64, String>`
    - `async supersede(&self, table_name: &str, primary_keys: &[String], keep_ids: &[i64], conn: &mut PgConnection) -> Result<u64, String>`
    - `async fetch_retry(&self, limit: i64, pool: &PgPool) -> Result<Vec<RetryEntry>, String>`
    - `async mark_resolved(&self, ids: &[i64], conn: &mut PgConnection) -> Result<(), String>`
  - Re-export `crate::models::{DeadLetterStore, NewDeadLetter}`.

- [ ] **Step 1: Viết test**

Tạo `src/models/dead_letter.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{RowAction, RowSource};
    use std::collections::HashMap;

    fn replayed_source() -> RowSource {
        let mut source = RowSource::new("debezium.public.accounts", Some(42), b"{\"x\":1}");
        source.dead_letter_id = Some(9);
        source
    }

    #[test]
    fn kind_names() {
        assert_eq!(DeadLetterKind::Poison.as_str(), "poison");
        assert_eq!(DeadLetterKind::Rejected.as_str(), "rejected");
    }

    #[test]
    fn rejected_entry_copies_row_fields() {
        let row = ChangeRow {
            table_name: "accounts".to_string(),
            table_value: HashMap::new(),
            primary_key: "5".to_string(),
            action: RowAction::Upsert,
            index: 0,
            source: replayed_source(),
        };
        let entry = NewDeadLetter::rejected(&row, "23514", "violates check");
        assert_eq!(entry.kind, DeadLetterKind::Rejected);
        assert_eq!(entry.subject, "debezium.public.accounts");
        assert_eq!(entry.stream_sequence, Some(42));
        assert_eq!(entry.table_name, Some("accounts"));
        assert_eq!(entry.primary_key, Some("5"));
        assert_eq!(entry.payload, "{\"x\":1}");
        assert_eq!(entry.error_code, Some("23514"));
        assert_eq!(entry.error_message, "violates check");
        assert_eq!(entry.existing_id, Some(9));
    }

    #[test]
    fn poison_entry_has_no_table_or_key() {
        let message = PoisonMessage {
            source: RowSource::new("debezium.public.x", None, b"not json"),
            reason: "invalid payload: boom".to_string(),
        };
        let entry = NewDeadLetter::poison(&message);
        assert_eq!(entry.kind, DeadLetterKind::Poison);
        assert_eq!(entry.table_name, None);
        assert_eq!(entry.primary_key, None);
        assert_eq!(entry.error_code, None);
        assert_eq!(entry.error_message, "invalid payload: boom");
        assert_eq!(entry.payload, "not json");
        assert_eq!(entry.existing_id, None);
    }

    #[test]
    fn table_is_schema_qualified_and_quoted() {
        assert_eq!(
            DeadLetterStore::new("public").table,
            "\"public\".\"_cdc_dead_letter\""
        );
    }
}
```

Trong `src/models/change.rs`, sửa test `ignores_cdc_internal_tables`: thêm hai dòng trước `assert!(!is_ignored_table("cdc_schema_metadata"));`:

```rust
        assert!(is_ignored_table("_cdc_dead_letter"));
        assert!(is_ignored_table(&normalize_table_name("_cdc_dead_letter_resync")));
```

Thêm vào `src/models/mod.rs` (sau `mod data_record;`):

```rust
mod dead_letter;
```

- [ ] **Step 2: Chạy test, xác nhận fail**

Run: `cargo test dead_letter` rồi `cargo test ignores_cdc_internal_tables`
Expected: lỗi biên dịch (`DeadLetterKind` chưa có); sau khi có Step 3 phần struct thì `ignores_cdc_internal_tables` phải fail trước khi sửa `is_ignored_table`.

- [ ] **Step 3: Viết implementation**

Trong `src/models/postgres_destination.rs` đổi `fn quote_identifier(identifier: &str) -> String {` thành `pub(crate) fn quote_identifier(identifier: &str) -> String {`.

Thêm vào **đầu** `src/models/dead_letter.rs`:

```rust
use sqlx::{PgConnection, PgPool, Row};

use crate::models::{ChangeRow, PoisonMessage, PostgresDestination};

/// Table lưu dữ liệu lỗi ở DB đích.
pub const DEAD_LETTER_TABLE: &str = "_cdc_dead_letter";

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeadLetterKind {
    /// Message không parse được
    Poison,
    /// Dòng bị DB đích từ chối vì lỗi dữ liệu
    Rejected,
}

impl DeadLetterKind {
    pub fn as_str(self) -> &'static str {
        match self {
            DeadLetterKind::Poison => "poison",
            DeadLetterKind::Rejected => "rejected",
        }
    }
}

/// Một dòng cần ghi vào `_cdc_dead_letter`.
pub struct NewDeadLetter<'a> {
    pub kind: DeadLetterKind,
    pub subject: &'a str,
    pub stream_sequence: Option<u64>,
    pub table_name: Option<&'a str>,
    pub primary_key: Option<&'a str>,
    pub payload: &'a str,
    pub error_code: Option<&'a str>,
    pub error_message: &'a str,
    /// Some khi dòng đến từ replay: cập nhật dòng cũ thay vì thêm mới.
    pub existing_id: Option<i64>,
}

impl<'a> NewDeadLetter<'a> {
    pub fn rejected(row: &'a ChangeRow, code: &'a str, message: &'a str) -> Self {
        NewDeadLetter {
            kind: DeadLetterKind::Rejected,
            subject: &row.source.subject,
            stream_sequence: row.source.stream_sequence,
            table_name: Some(&row.table_name),
            primary_key: Some(&row.primary_key),
            payload: &row.source.payload,
            error_code: Some(code),
            error_message: message,
            existing_id: row.source.dead_letter_id,
        }
    }

    pub fn poison(message: &'a PoisonMessage) -> Self {
        NewDeadLetter {
            kind: DeadLetterKind::Poison,
            subject: &message.source.subject,
            stream_sequence: message.source.stream_sequence,
            table_name: None,
            primary_key: None,
            payload: &message.source.payload,
            error_code: None,
            error_message: &message.reason,
            existing_id: message.source.dead_letter_id,
        }
    }
}

/// Dòng `retry` lấy ra để replay.
pub struct RetryEntry {
    pub id: i64,
    pub subject: String,
    pub stream_sequence: Option<i64>,
    pub payload: String,
}

pub struct DeadLetterStore {
    /// Tên table đã kèm schema và quote
    table: String,
}

impl DeadLetterStore {
    pub fn new(schema: &str) -> Self {
        DeadLetterStore {
            table: format!(
                "{}.{}",
                PostgresDestination::quote_identifier(schema),
                PostgresDestination::quote_identifier(DEAD_LETTER_TABLE)
            ),
        }
    }

    pub async fn ensure_table(&self, pool: &PgPool) -> Result<(), String> {
        let statements = [
            format!(
                "CREATE TABLE IF NOT EXISTS {} (
                    id              BIGSERIAL PRIMARY KEY,
                    kind            TEXT NOT NULL,
                    subject         TEXT NOT NULL,
                    stream_sequence BIGINT,
                    table_name      TEXT,
                    primary_key     TEXT,
                    payload         TEXT NOT NULL,
                    error_code      TEXT,
                    error_message   TEXT NOT NULL,
                    status          TEXT NOT NULL DEFAULT 'pending',
                    attempts        INT  NOT NULL DEFAULT 1,
                    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
                    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
                )",
                self.table
            ),
            format!(
                "CREATE INDEX IF NOT EXISTS {}_open_idx ON {} (table_name, primary_key) \
                 WHERE status IN ('pending', 'retry')",
                DEAD_LETTER_TABLE, self.table
            ),
            format!(
                "CREATE INDEX IF NOT EXISTS {}_retry_idx ON {} (id) WHERE status = 'retry'",
                DEAD_LETTER_TABLE, self.table
            ),
        ];
        for statement in &statements {
            sqlx::query(statement)
                .execute(pool)
                .await
                .map_err(|e| format!("Can not create dead letter table: {}", e))?;
        }
        Ok(())
    }

    /// Ghi một dòng lỗi, trả về id. Khi replay (`existing_id`) thì đưa dòng cũ về `pending` với
    /// lỗi mới; nếu dòng cũ đã bị xóa bằng tay thì thêm dòng mới.
    pub async fn record(
        &self,
        entry: &NewDeadLetter<'_>,
        conn: &mut PgConnection,
    ) -> Result<i64, String> {
        if let Some(id) = entry.existing_id {
            let updated = sqlx::query(&format!(
                "UPDATE {} SET status = 'pending', error_code = $2, error_message = $3, \
                 attempts = attempts + 1, updated_at = now() WHERE id = $1 RETURNING id",
                self.table
            ))
            .bind(id)
            .bind(entry.error_code)
            .bind(entry.error_message)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|e| format!("Failed to update dead letter {}: {}", id, e))?;
            if let Some(row) = updated {
                return Ok(row.get::<i64, _>("id"));
            }
        }
        let row = sqlx::query(&format!(
            "INSERT INTO {} (kind, subject, stream_sequence, table_name, primary_key, payload, \
             error_code, error_message) VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING id",
            self.table
        ))
        .bind(entry.kind.as_str())
        .bind(entry.subject)
        .bind(entry.stream_sequence.map(|sequence| sequence as i64))
        .bind(entry.table_name)
        .bind(entry.primary_key)
        .bind(entry.payload)
        .bind(entry.error_code)
        .bind(entry.error_message)
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| format!("Failed to record dead letter: {}", e))?;
        Ok(row.get::<i64, _>("id"))
    }

    /// Các khóa chính vừa có bản mới hơn (ghi được hoặc lại bị từ chối): dòng lỗi cũ của chúng
    /// chuyển sang `superseded`, trừ các id trong `keep_ids`.
    pub async fn supersede(
        &self,
        table_name: &str,
        primary_keys: &[String],
        keep_ids: &[i64],
        conn: &mut PgConnection,
    ) -> Result<u64, String> {
        if primary_keys.is_empty() {
            return Ok(0);
        }
        sqlx::query(&format!(
            "UPDATE {} SET status = 'superseded', updated_at = now() \
             WHERE status IN ('pending', 'retry') AND table_name = $1 \
               AND primary_key = ANY($2) AND id <> ALL($3)",
            self.table
        ))
        .bind(table_name)
        .bind(primary_keys)
        .bind(keep_ids)
        .execute(&mut *conn)
        .await
        .map(|result| result.rows_affected())
        .map_err(|e| format!("Failed to supersede dead letters: {}", e))
    }

    pub async fn fetch_retry(&self, limit: i64, pool: &PgPool) -> Result<Vec<RetryEntry>, String> {
        let rows = sqlx::query(&format!(
            "SELECT id, subject, stream_sequence, payload FROM {} \
             WHERE status = 'retry' ORDER BY id LIMIT $1",
            self.table
        ))
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to fetch dead letters to retry: {}", e))?;
        Ok(rows
            .into_iter()
            .map(|row| RetryEntry {
                id: row.get("id"),
                subject: row.get("subject"),
                stream_sequence: row.get("stream_sequence"),
                payload: row.get("payload"),
            })
            .collect())
    }

    pub async fn mark_resolved(&self, ids: &[i64], conn: &mut PgConnection) -> Result<(), String> {
        if ids.is_empty() {
            return Ok(());
        }
        sqlx::query(&format!(
            "UPDATE {} SET status = 'resolved', updated_at = now() \
             WHERE id = ANY($1) AND status IN ('pending', 'retry')",
            self.table
        ))
        .bind(ids)
        .execute(&mut *conn)
        .await
        .map_err(|e| format!("Failed to mark dead letters resolved: {}", e))?;
        Ok(())
    }
}

```

Trong `src/models/change.rs`:
- Thêm `dead_letter::DEAD_LETTER_TABLE,` vào khối `use crate::models::{ ... }`.
- Sửa `is_ignored_table` và doc comment:

```rust
/// Table không bao giờ sync: table nội bộ cdcsink tự tạo ở DB đích (`_cdc_schema_metadata`,
/// `_cdc_dead_letter`). Khi DB đích lại là nguồn của một tầng CDC khác (cdcsink nối tiếp cdcsink),
/// sync các table này sẽ ghi đè dữ liệu nội bộ của tầng sau.
pub fn is_ignored_table(name: &str) -> bool {
    name.eq_ignore_ascii_case(SCHEMA_METADATA_TABLE) || name.eq_ignore_ascii_case(DEAD_LETTER_TABLE)
}
```

Thêm re-export vào `src/models/mod.rs`:

```rust
pub use dead_letter::{DeadLetterStore, NewDeadLetter};
```

- [ ] **Step 4: Chạy test, xác nhận pass**

Run: `cargo test`
Expected: toàn bộ PASS, gồm 4 test `dead_letter::tests::*` và `ignores_cdc_internal_tables` mới.

---

### Task 5: Pipeline dùng `ChangeRow`/`WriteError`; message hỏng vào dead-letter

Sau task này: message hỏng được ghi vào `_cdc_dead_letter` (không còn `Term`); lỗi DB vẫn retry cả batch như trước (chưa bisection).

**Files:**
- Modify: `src/models/nats_receive.rs` (viết lại phần sau `connected`)
- Modify: `src/models/postgres_destination.rs` (thay `remove_duplicate_data`, `insert_value`, `delete_rows` bằng `upsert_rows`, `delete_rows`)
- Modify: `src/main.rs` (viết lại)
- Modify: `src/models/mod.rs`

**Interfaces:**
- Consumes: `ChangeRow`, `Parsed`, `PoisonMessage`, `RowSource`, `SkipReason`, `parse_change`, `latest_per_key` (Task 3); `DeadLetterStore`, `NewDeadLetter` (Task 4); `WriteError` (Task 1).
- Produces:
  - `pub struct ReceivedBatch { pub rows: Vec<ChangeRow>, pub poison: Vec<PoisonMessage>, pub messages: Vec<Message> }`
  - `NatsReceive::receive_messages(&self, consumer: &mut PullConsumer, sync_config: Option<&SyncConfig>, logged_type_errors: &mut HashSet<String>) -> Result<ReceivedBatch, String>`
  - `NatsReceive::extend_ack_deadline(&self, messages: &[Message])`
  - `NatsReceive::ack_messages(&self, messages: &[Message]) -> Result<(), String>`
  - `PostgresDestination::upsert_rows(&self, table_name: &str, rows: Vec<&ChangeRow>, pool: &PgPool) -> Result<(), WriteError>` (rows rỗng → `Ok`)
  - `PostgresDestination::delete_rows(&self, table_name: &str, rows: Vec<&ChangeRow>, pool: &PgPool) -> Result<(), WriteError>` (rows rỗng → `Ok`)
  - Trong `main.rs`: `struct Sink { destination, dead_letters, pool, schema }`, `persist_batch`, `process_batch`, `flush_table` (Task 6 và 7 sửa tiếp).
  - `mod.rs`: bỏ export `NatMessageReceive`, thêm `ReceivedBatch`.

Task này là refactor giữ hành vi; kiểm chứng bằng build + unit test hiện có + e2e ở Task 8. Không thêm unit test mới.

- [ ] **Step 1: Sửa `postgres_destination.rs`**

Đổi import:

```rust
use crate::models::{
    ChangeRow, DataModel, WriteError, decimal::decimal_text, sync_config::primary_key_column,
};
```

Xóa `remove_duplicate_data`, `insert_value`, `delete_rows` hiện có (từ `fn remove_duplicate_data<'a>(` đến hết `async fn delete_rows(...) { ... }`), thay bằng:

```rust
    fn qualified_table(&self, table_name: &str) -> String {
        format!(
            "{}.{}",
            Self::quote_identifier(&self.schema_expect),
            Self::quote_identifier(table_name)
        )
    }

    /// Upsert các dòng của một table bằng một câu lệnh. Mỗi khóa chính chỉ được có một dòng
    /// (xem `latest_per_key`); dòng thiếu cột nào thì cột đó nhận NULL.
    pub async fn upsert_rows(
        &self,
        table_name: &str,
        rows: Vec<&ChangeRow>,
        pool: &PgPool,
    ) -> Result<(), WriteError> {
        if rows.is_empty() {
            return Ok(());
        }
        let column_active = &rows[0].table_value;
        let primary_key = match primary_key_column(column_active) {
            Some(pk) => pk.clone(),
            None => {
                return Err(WriteError::Transient(format!(
                    "Missing id column for upsert into table {}",
                    table_name
                )));
            }
        };
        let mut colum_keys = column_active.keys().cloned().collect::<Vec<String>>();
        colum_keys.sort();

        // Mọi cột bind dạng TEXT[] rồi cast sang kiểu cột trong SELECT: một đường xử lý cho mọi kiểu
        // (NaN/Infinity, infinity, mảng, bytea, numeric không giới hạn chữ số...)
        let column_list = colum_keys
            .iter()
            .map(|column| Self::quote_identifier(column))
            .collect::<Vec<String>>()
            .join(", ");
        let select_list = colum_keys
            .iter()
            .enumerate()
            .map(|(i, column)| format!("u.c{}::{}", i, column_active[column].data_type))
            .collect::<Vec<String>>()
            .join(", ");
        let params = (1..=colum_keys.len())
            .map(|i| format!("${}::TEXT[]", i))
            .collect::<Vec<String>>()
            .join(", ");
        let aliases = (0..colum_keys.len())
            .map(|i| format!("c{}", i))
            .collect::<Vec<String>>()
            .join(", ");
        let updates = colum_keys
            .iter()
            .filter(|column| **column != primary_key)
            .map(|column| {
                let quoted = Self::quote_identifier(column);
                format!("{} = EXCLUDED.{}", quoted, quoted)
            })
            .collect::<Vec<String>>();
        let on_conflict = if updates.is_empty() {
            "DO NOTHING".to_string()
        } else {
            format!("DO UPDATE SET {}", updates.join(", "))
        };
        let s = format!(
            "INSERT INTO {} ({}) SELECT {} FROM unnest({}) AS u({}) ON CONFLICT ({}) {}",
            self.qualified_table(table_name),
            column_list,
            select_list,
            params,
            aliases,
            Self::quote_identifier(&primary_key),
            on_conflict
        );

        let mut query = sqlx::query(&s);
        for column in &colum_keys {
            // Record thiếu cột này thì dùng NULL
            let values: Vec<Option<String>> = rows
                .iter()
                .map(|row| row.table_value.get(column).and_then(Self::to_pg_text))
                .collect();
            query = query.bind(values);
        }
        query.execute(pool).await.map_err(|e| {
            WriteError::from(e).context(&format!("Failed to upsert into table {}", table_name))
        })?;
        Ok(())
    }

    pub async fn delete_rows(
        &self,
        table_name: &str,
        rows: Vec<&ChangeRow>,
        pool: &PgPool,
    ) -> Result<(), WriteError> {
        if rows.is_empty() {
            return Ok(());
        }
        let primary_key = match primary_key_column(&rows[0].table_value) {
            Some(pk) => pk.clone(),
            None => {
                return Err(WriteError::Transient(format!(
                    "Missing id column for delete in table {}",
                    table_name
                )));
            }
        };
        let key_type = &rows[0].table_value[&primary_key].data_type;
        let delete_query_str = format!(
            "DELETE FROM {} WHERE {} = ANY($1::TEXT[]::{}[]);",
            self.qualified_table(table_name),
            Self::quote_identifier(&primary_key),
            key_type
        );
        let ids: Vec<Option<String>> = rows
            .iter()
            .map(|row| row.table_value.get(&primary_key).and_then(Self::to_pg_text))
            .collect();
        sqlx::query(&delete_query_str)
            .bind(ids)
            .execute(pool)
            .await
            .map_err(|e| {
                WriteError::from(e).context(&format!("Failed to delete from table {}", table_name))
            })?;
        Ok(())
    }
```

Nếu import `RowAction` hoặc `NatMessageReceive` không còn dùng trong file thì xóa khỏi dòng `use`.

- [ ] **Step 2: Viết lại phần xử lý message trong `nats_receive.rs`**

Khối `use` đầu file thành:

```rust
use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use async_nats::{
    connect,
    jetstream::{self, AckKind, Message},
};

use async_nats::jetstream::consumer::PullConsumer;
use futures_util::StreamExt;
use tracing::{debug, warn};

use crate::models::{
    ChangeRow, Parsed, PoisonMessage, RowSource, SyncConfig,
    change::SkipReason,
    sync_config::MatchOutcome,
};
```

Thay `pub struct NatMessageReceive { ... }` bằng:

```rust
/// Kết quả một lần fetch.
pub struct ReceivedBatch {
    /// Dòng hợp lệ, theo thứ tự nhận.
    pub rows: Vec<ChangeRow>,
    /// Message không parse được, được ghi vào dead-letter trước khi ack.
    pub poison: Vec<PoisonMessage>,
    /// Message của `rows` và `poison`, chỉ ack sau khi batch được lưu xong.
    pub messages: Vec<Message>,
}
```

Giữ nguyên `NatsReceive`, `new`, `connected`. Thay toàn bộ từ `pub async fn receive_messages(` đến hết `impl NatsReceive` bằng:

```rust
    pub async fn receive_messages(
        &self,
        consumer: &mut PullConsumer,
        sync_config: Option<&SyncConfig>,
        logged_type_errors: &mut HashSet<String>,
    ) -> Result<ReceivedBatch, String> {
        let mut stream = consumer
            .fetch()
            .max_messages(self.number_pull_object)
            .expires(Duration::from_secs(5)) // 👈 MaxWait
            .messages()
            .await
            .map_err(|e| format!("Failed to receive messages: {}", e))?;

        let mut batch = ReceivedBatch {
            rows: Vec::new(),
            poison: Vec::new(),
            messages: Vec::new(),
        };
        // table -> (tổng số message, số message bị loại vì lỗi kiểu hoặc thiếu cột)
        let mut type_rejections: HashMap<String, (usize, usize)> = HashMap::new();
        let mut counter = 0;
        while let Some(item) = stream.next().await {
            let message = match item {
                Ok(message) => message,
                Err(e) => {
                    // Phần còn lại của batch chưa nhận sẽ được NATS gửi lại ở lần fetch sau
                    warn!(error = %e, "Failed to read message from batch");
                    break;
                }
            };
            let stream_sequence = message.info().ok().map(|info| info.stream_sequence);
            let source = RowSource::new(&message.subject, stream_sequence, &message.payload);
            match crate::models::parse_change(source, counter, sync_config) {
                Parsed::Row { row, outcome } => {
                    log_filter_problems(&row.table_name, &outcome, logged_type_errors);
                    let stats = type_rejections
                        .entry(row.table_name.clone())
                        .or_insert((0, 0));
                    stats.0 += 1;
                    if !outcome.matched
                        && (!outcome.type_errors.is_empty() || !outcome.missing_columns.is_empty())
                    {
                        stats.1 += 1;
                    }
                    batch.rows.push(row);
                    batch.messages.push(message);
                    counter += 1;
                }
                Parsed::Poison(poison) => {
                    batch.poison.push(poison);
                    batch.messages.push(message);
                }
                Parsed::Skip(reason) => {
                    log_skip(&reason, logged_type_errors);
                    Self::ack_skipped(&message).await?;
                }
            }
        }

        for (table_name, (total, rejected)) in &type_rejections {
            if *total > 0 && total == rejected {
                warn!(
                    table = %table_name,
                    total,
                    "All rows rejected due to type mismatch or missing column in where"
                );
            }
        }

        Ok(batch)
    }

    /// Gia hạn ack_wait cho cả batch khi đang retry, để NATS không gửi lại giữa chừng.
    pub async fn extend_ack_deadline(&self, messages: &[Message]) {
        for message in messages {
            if let Err(e) = message.ack_with(AckKind::Progress).await {
                warn!(error = %e, "Failed to extend ack deadline");
                return;
            }
        }
    }

    /// Message bị bỏ qua vẫn phải ack, nếu không NATS gửi lại sau mỗi ack_wait mãi mãi.
    async fn ack_skipped(message: &Message) -> Result<(), String> {
        message
            .ack()
            .await
            .map_err(|e| format!("Failed to acknowledge skipped message: {}", e))
    }

    pub async fn ack_messages(&self, messages: &[Message]) -> Result<(), String> {
        for message in messages {
            message
                .ack()
                .await
                .map_err(|e| format!("Failed to acknowledge message: {}", e))?;
        }
        Ok(())
    }
}

/// Log một lần cho mỗi (table, lý do) để không lặp log mỗi batch.
fn log_skip(reason: &SkipReason, logged: &mut HashSet<String>) {
    match reason {
        SkipReason::Tombstone => debug!("Skipping tombstone message"),
        SkipReason::IgnoredTable(table) => {
            if logged.insert(format!("{}|ignored", table)) {
                warn!(table = %table, "Table is never synced, messages are skipped");
            }
        }
        SkipReason::NoPrimaryKey(table) => {
            if logged.insert(format!("{}|no-primary-key", table)) {
                warn!(table = %table, "Table has no \"id\" column, messages are skipped");
            }
        }
    }
}

fn log_filter_problems(table_name: &str, outcome: &MatchOutcome, logged: &mut HashSet<String>) {
    for error in &outcome.type_errors {
        let key = format!("{}|{}|{:?}", table_name, error.column, error.op);
        if logged.insert(key) {
            warn!(
                table = %table_name,
                column = %error.column,
                op = ?error.op,
                detail = %error.detail,
                "Sync filter type mismatch"
            );
        }
    }
    for column in &outcome.missing_columns {
        let key = format!("{}|{}|missing", table_name, column);
        if logged.insert(key) {
            warn!(
                table = %table_name,
                column = %column,
                "Sync filter column not found (check the name in where; rows are deleted)"
            );
        }
    }
}
```

Trong `src/models/mod.rs`, đổi `pub use nats_receive::{NatMessageReceive, NatsReceive};` thành:

```rust
pub use nats_receive::{NatsReceive, ReceivedBatch};
```

Và đổi `mod change;` thành `pub(crate) mod change;` (để `nats_receive.rs` dùng `change::SkipReason` và `main.rs` dùng được ở Task 7).

- [ ] **Step 3: Viết lại `src/main.rs`**

Toàn bộ nội dung mới:

```rust
use std::{
    collections::{HashMap, HashSet},
    env,
    error::Error,
    time::Duration,
};

use async_nats::jetstream::Message;
use dotenvy::dotenv;
use sqlx::PgPool;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use crate::models::{
    ChangeRow, DataModel, DeadLetterStore, NatsReceive, NewDeadLetter, PoisonMessage,
    PostgresDestination, RowAction, SyncConfig, latest_per_key,
};

mod models;

type SchemaCache = HashMap<String, HashSet<String>>;

/// Chờ giữa hai lần fetch khi NATS lỗi.
const FETCH_RETRY_DELAY: Duration = Duration::from_secs(1);
/// Backoff khi ghi batch lỗi: 1s, 2s, 4s... tối đa 60s.
const MAX_RETRY_DELAY: Duration = Duration::from_secs(60);
/// Chu kỳ gia hạn ack_wait khi đang chờ retry, phải nhỏ hơn ack_wait của consumer (10s).
const ACK_PROGRESS_INTERVAL: Duration = Duration::from_secs(5);

/// Phía DB đích, dùng chung cho batch thường và replay dead-letter.
struct Sink {
    destination: PostgresDestination,
    dead_letters: DeadLetterStore,
    pool: PgPool,
    schema: String,
}

/// Mức log lấy từ RUST_LOG (mặc định info), LOG_FORMAT=json để xuất JSON cho hệ thống gom log.
fn init_logging() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn,async_nats=warn"));
    let builder = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true);
    if env::var("LOG_FORMAT").is_ok_and(|v| v.eq_ignore_ascii_case("json")) {
        builder.json().flatten_event(true).init();
    } else {
        builder.init();
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();
    init_logging();

    info!(version = env!("CARGO_PKG_VERSION"), "Starting CDC Sink");
    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL not set");
    let nats_url = env::var("NATS_URL").expect("NATS_URL not set");
    let topic_name = env::var("TOPIC_NAME").expect("TOPIC_NAME not set");
    let database_schema_expected =
        env::var("DATABASE_SCHEMA_EXPECT").unwrap_or("public".to_string());
    let number_pull_object = env::var("NATS_PULL_NUMBER_OBJECT")
        .unwrap_or("100".to_string())
        .parse::<usize>()?;

    let nats_consumer_name =
        env::var("NATS_CONSUMER_NAME").unwrap_or("cdcsink_consumer".to_string());
    let nats_stream_name = env::var("NATS_STREAM_NAME").expect("NATS_STREAM_NAME not set");
    let max_batch_retries = env::var("MAX_BATCH_RETRIES")
        .unwrap_or("5".to_string())
        .parse::<u32>()?;

    let sync_config = match SyncConfig::from_env_value(env::var("SYNC_CONFIG_PATH").ok()) {
        Ok(Some(config)) => {
            info!(tables = %config.table_names().join(", "), "Sync config loaded");
            Some(config)
        }
        Ok(None) => {
            info!("SYNC_CONFIG_PATH not set: syncing all tables/columns/rows (no filter)");
            None
        }
        Err(e) => {
            error!(error = %e, "Failed to load sync config");
            std::process::exit(1);
        }
    };

    info!(
        nats_url = %nats_url,
        topic = %topic_name,
        stream = %nats_stream_name,
        consumer = %nats_consumer_name,
        schema = %database_schema_expected,
        batch_size = number_pull_object,
        max_batch_retries,
        "Configuration loaded"
    );

    let nats_info = NatsReceive::new(
        nats_url,
        nats_consumer_name,
        topic_name,
        nats_stream_name,
        number_pull_object,
    );

    let mut consumer = nats_info.connected().await?;

    let destination = PostgresDestination::new(db_url, database_schema_expected.clone());
    let pool = destination.connect().await.map_err(Box::<dyn Error>::from)?;
    destination
        .ensure_schema_metadata_table(&pool)
        .await
        .map_err(Box::<dyn Error>::from)?;
    let dead_letters = DeadLetterStore::new(&database_schema_expected);
    dead_letters
        .ensure_table(&pool)
        .await
        .map_err(Box::<dyn Error>::from)?;
    let mut schema_cache = destination
        .get_schema_info(&pool)
        .await
        .map_err(Box::<dyn Error>::from)?;
    let sink = Sink {
        destination,
        dead_letters,
        pool,
        schema: database_schema_expected,
    };

    let mut logged_type_errors: HashSet<String> = HashSet::new();
    loop {
        let batch = match nats_info
            .receive_messages(&mut consumer, sync_config.as_ref(), &mut logged_type_errors)
            .await
        {
            Ok(batch) => batch,
            Err(e) => {
                // Message chưa ack sẽ được NATS gửi lại, chỉ cần thử fetch lại
                warn!(error = %e, "Failed to receive messages, retrying");
                tokio::time::sleep(FETCH_RETRY_DELAY).await;
                continue;
            }
        };
        if batch.messages.is_empty() {
            continue;
        }
        info!(
            rows = batch.rows.len(),
            poison = batch.poison.len(),
            "Received messages"
        );

        // Giữ batch và retry tại chỗ thay vì nak: nak để message mới đi trước message cũ,
        // bản cũ gửi lại sau sẽ ghi đè bản mới ở đích. Mọi thao tác đều idempotent
        // (IF NOT EXISTS, upsert, delete theo id) nên chạy lại cả batch là an toàn.
        let mut attempt: u32 = 0;
        loop {
            match persist_batch(&sink, &batch.rows, &batch.poison, &mut schema_cache).await {
                Ok(()) => break,
                Err(e) if attempt >= max_batch_retries => {
                    // Không ack: NATS gửi lại batch sau khi service khởi động lại
                    error!(
                        error = %e,
                        attempts = attempt + 1,
                        "Batch failed after all retries, exiting"
                    );
                    std::process::exit(1);
                }
                Err(e) => {
                    let delay = MAX_RETRY_DELAY.min(Duration::from_secs(1 << attempt.min(6)));
                    attempt += 1;
                    warn!(
                        error = %e,
                        attempt,
                        max_retries = max_batch_retries,
                        delay_secs = delay.as_secs(),
                        "Batch failed, retrying"
                    );
                    wait_keeping_messages(&nats_info, &batch.messages, delay).await;
                    // DDL có thể đã chạy một phần: đọc lại schema thật ở đích
                    match sink.destination.get_schema_info(&sink.pool).await {
                        Ok(fresh) => schema_cache = fresh,
                        Err(e) => warn!(error = %e, "Failed to reload schema info"),
                    }
                }
            }
        }

        if let Err(e) = nats_info.ack_messages(&batch.messages).await {
            // Dữ liệu đã lưu xong; message chưa ack sẽ được gửi lại và ghi lại, không mất dữ liệu
            warn!(error = %e, "Failed to acknowledge batch");
        }
    }
}

/// Chờ `delay`, gia hạn ack_wait định kỳ để NATS không gửi lại batch đang giữ.
async fn wait_keeping_messages(nats_info: &NatsReceive, messages: &[Message], delay: Duration) {
    let mut remaining = delay;
    while !remaining.is_zero() {
        nats_info.extend_ack_deadline(messages).await;
        let step = remaining.min(ACK_PROGRESS_INTERVAL);
        tokio::time::sleep(step).await;
        remaining -= step;
    }
    nats_info.extend_ack_deadline(messages).await;
}

/// Ghi batch vào đích rồi lưu dữ liệu lỗi vào dead-letter trong một transaction.
/// `Err` là lỗi tạm thời: batch chưa được lưu trọn vẹn nên không được ack.
async fn persist_batch(
    sink: &Sink,
    rows: &[ChangeRow],
    poison: &[PoisonMessage],
    schema_cache: &mut SchemaCache,
) -> Result<(), String> {
    process_batch(sink, rows, schema_cache).await?;

    let mut tx = sink
        .pool
        .begin()
        .await
        .map_err(|e| format!("Failed to begin dead letter transaction: {}", e))?;
    for message in poison {
        let id = sink
            .dead_letters
            .record(&NewDeadLetter::poison(message), &mut tx)
            .await?;
        warn!(
            dead_letter_id = id,
            subject = %message.source.subject,
            reason = %message.reason,
            "Unprocessable message sent to dead letter"
        );
    }
    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit dead letters: {}", e))
}

/// Ghi các dòng vào đích: tạo table/cột khi cần, rồi upsert/delete theo từng table.
/// Chỉ cập nhật `schema_cache` sau khi DDL thành công.
async fn process_batch(
    sink: &Sink,
    rows: &[ChangeRow],
    schema_cache: &mut SchemaCache,
) -> Result<(), String> {
    let mut message_active: HashMap<String, Vec<&ChangeRow>> = HashMap::new();
    for msg in rows {
        let table_name = &msg.table_name;
        if msg.action == RowAction::Delete {
            // Delete không kích hoạt DDL; table chưa có ở đích thì không có gì để xóa
            if schema_cache.contains_key(table_name) {
                message_active
                    .entry(table_name.clone())
                    .or_insert(Vec::new())
                    .push(msg);
            }
            continue;
        }
        if !schema_cache.contains_key(table_name) {
            // Table chưa tồn tại: nếu message_active đang có dữ liệu thì insert trước
            if let Some(buffered) = message_active.remove(table_name) {
                flush_table(sink, table_name, &buffered).await?;
            }
            sink.destination
                .create_table_if_not_exists_query(
                    &sink.schema,
                    table_name,
                    &msg.table_value,
                    &sink.pool,
                )
                .await?;
            schema_cache.insert(
                table_name.clone(),
                msg.table_value.keys().cloned().collect(),
            );
        } else {
            // Table đã tồn tại: kiểm tra xem có column mới không
            let cached_columns = schema_cache.get_mut(table_name).unwrap();
            let new_columns: Vec<(&String, &DataModel)> = msg
                .table_value
                .iter()
                .filter(|(col_name, _)| !cached_columns.contains(*col_name))
                .collect();

            if !new_columns.is_empty() {
                // Có column mới: nếu message_active đang có dữ liệu thì insert trước
                if let Some(buffered) = message_active.remove(table_name) {
                    flush_table(sink, table_name, &buffered).await?;
                }
                // Tạo các column mới
                for (col_name, col_type) in &new_columns {
                    sink.destination
                        .add_column_if_not_exists(
                            &sink.schema,
                            table_name,
                            col_name,
                            col_type,
                            &sink.pool,
                        )
                        .await?;
                    cached_columns.insert(col_name.to_string());
                }
            }
        }
        message_active
            .entry(table_name.clone())
            .or_insert(Vec::new())
            .push(msg);
    }
    for (table_name, table_rows) in &message_active {
        flush_table(sink, table_name, table_rows).await?;
    }
    Ok(())
}

/// Ghi các dòng của một table: mỗi khóa chính một bản mới nhất, delete trước rồi upsert.
async fn flush_table(sink: &Sink, table_name: &str, rows: &[&ChangeRow]) -> Result<(), String> {
    let (to_delete, to_upsert): (Vec<&ChangeRow>, Vec<&ChangeRow>) = latest_per_key(rows)
        .into_iter()
        .partition(|row| row.action == RowAction::Delete);
    sink.destination
        .delete_rows(table_name, to_delete, &sink.pool)
        .await
        .map_err(|e| e.to_string())?;
    sink.destination
        .upsert_rows(table_name, to_upsert, &sink.pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}
```

Nếu `DataModel` chưa được re-export ở `crate::models` thì đã có sẵn (`pub use models_info::DataModel;`). `create_table_if_not_exists_query` nhận `&String`: truyền `&sink.schema` (kiểu `String`) và `table_name` (kiểu `&String`) như trên.

- [ ] **Step 4: Build, format và chạy test**

Run: `cargo fmt && cargo build && cargo test`
Expected: build thành công (warning `unused`/`dead_code` cho `Rejected`, `write_isolating`, `supersede`, `fetch_retry`, `mark_resolved`... được chấp nhận); toàn bộ test PASS.

---

### Task 6: Bisection trong `flush_table`; dòng bị từ chối vào dead-letter; supersede

**Files:**
- Modify: `src/main.rs` (`persist_batch`, `process_batch`, `flush_table`, thêm `keys_by_table`, `latest_replay_ids`, test)

**Interfaces:**
- Consumes: `write_isolating`, `Rejected` (Task 2); `DeadLetterStore::{record, supersede, mark_resolved}`, `NewDeadLetter::rejected` (Task 4); `upsert_rows`, `delete_rows` (Task 5).
- Produces (dùng trong Task 7):
  - `async fn persist_batch(sink: &Sink, rows: &[ChangeRow], poison: &[PoisonMessage], resolved_ids: &[i64], schema_cache: &mut SchemaCache) -> Result<(), String>`
    - `resolved_ids`: id dead-letter cần đánh dấu `resolved` mà không có dòng tương ứng (replay bị `Skip`); batch thường truyền `&[]`.
  - `async fn process_batch<'a>(sink: &Sink, rows: &'a [ChangeRow], schema_cache: &mut SchemaCache) -> Result<Vec<Rejected<'a, ChangeRow>>, String>`
  - `fn keys_by_table(rows: &[ChangeRow]) -> HashMap<&str, Vec<String>>`
  - `fn latest_replay_ids(rows: &[ChangeRow]) -> Vec<i64>`

- [ ] **Step 1: Viết test cho hai hàm thuần**

Thêm vào cuối `src/main.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::RowSource;

    fn row(table: &str, key: &str, index: i64, dead_letter_id: Option<i64>) -> ChangeRow {
        let mut source = RowSource::new("s", None, b"{}");
        source.dead_letter_id = dead_letter_id;
        ChangeRow {
            table_name: table.to_string(),
            table_value: HashMap::new(),
            primary_key: key.to_string(),
            action: RowAction::Upsert,
            index,
            source,
        }
    }

    #[test]
    fn keys_are_grouped_by_table() {
        let rows = [row("a", "1", 0, None), row("b", "1", 1, None), row("a", "2", 2, None)];
        let keys = keys_by_table(&rows);
        assert_eq!(keys["a"], vec!["1".to_string(), "2".to_string()]);
        assert_eq!(keys["b"], vec!["1".to_string()]);
    }

    #[test]
    fn only_newest_replayed_row_per_key_counts() {
        let rows = [
            row("a", "1", 0, Some(10)), // bị bản id 12 thay thế
            row("a", "2", 1, Some(11)),
            row("a", "1", 2, Some(12)),
            row("b", "1", 3, Some(13)), // khác table nên không đụng "a"/"1"
            row("a", "3", 4, None),     // không phải replay
        ];
        let mut ids = latest_replay_ids(&rows);
        ids.sort();
        assert_eq!(ids, vec![11, 12, 13]);
    }
}
```

- [ ] **Step 2: Chạy test, xác nhận fail**

Run: `cargo test --bin cdcsink tests::`
Expected: lỗi biên dịch `cannot find function keys_by_table` / `latest_replay_ids`.

- [ ] **Step 3: Viết implementation**

Sửa import trong `src/main.rs`:

```rust
use crate::models::{
    ChangeRow, DataModel, DeadLetterStore, NatsReceive, NewDeadLetter, PoisonMessage,
    PostgresDestination, Rejected, RowAction, SyncConfig, latest_per_key, write_isolating,
};
```

Trong vòng lặp chính, đổi lời gọi thành:

```rust
            match persist_batch(&sink, &batch.rows, &batch.poison, &[], &mut schema_cache).await {
```

Thay `persist_batch` bằng:

```rust
/// Ghi batch vào đích rồi lưu dữ liệu lỗi vào dead-letter trong một transaction.
/// `Err` là lỗi tạm thời: batch chưa được lưu trọn vẹn nên không được ack.
///
/// `resolved_ids`: id dead-letter replay xong mà không có dòng tương ứng (vd table giờ bị bỏ qua).
async fn persist_batch(
    sink: &Sink,
    rows: &[ChangeRow],
    poison: &[PoisonMessage],
    resolved_ids: &[i64],
    schema_cache: &mut SchemaCache,
) -> Result<(), String> {
    let rejected = process_batch(sink, rows, schema_cache).await?;

    let mut tx = sink
        .pool
        .begin()
        .await
        .map_err(|e| format!("Failed to begin dead letter transaction: {}", e))?;
    let mut recorded_ids: Vec<i64> = Vec::new();
    for item in &rejected {
        let row = item.item;
        let id = sink
            .dead_letters
            .record(&NewDeadLetter::rejected(row, &item.code, &item.message), &mut tx)
            .await?;
        warn!(
            dead_letter_id = id,
            table = %row.table_name,
            primary_key = %row.primary_key,
            error_code = %item.code,
            error = %item.message,
            "Row rejected by destination, sent to dead letter"
        );
        recorded_ids.push(id);
    }
    for message in poison {
        let id = sink
            .dead_letters
            .record(&NewDeadLetter::poison(message), &mut tx)
            .await?;
        warn!(
            dead_letter_id = id,
            subject = %message.source.subject,
            reason = %message.reason,
            "Unprocessable message sent to dead letter"
        );
        recorded_ids.push(id);
    }

    // Bản mới nhất của các dòng replay đã ghi được; bản cũ hơn của cùng khóa bị superseded
    let replayed: Vec<i64> = latest_replay_ids(rows)
        .into_iter()
        .filter(|id| !recorded_ids.contains(id))
        .collect();
    let keep_ids: Vec<i64> = recorded_ids.iter().chain(&replayed).copied().collect();
    for (table_name, keys) in keys_by_table(rows) {
        sink.dead_letters
            .supersede(table_name, &keys, &keep_ids, &mut tx)
            .await?;
    }
    let resolved: Vec<i64> = replayed.iter().chain(resolved_ids).copied().collect();
    sink.dead_letters.mark_resolved(&resolved, &mut tx).await?;

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit dead letters: {}", e))
}

/// Khóa chính của các dòng trong batch, gom theo table.
fn keys_by_table(rows: &[ChangeRow]) -> HashMap<&str, Vec<String>> {
    let mut keys: HashMap<&str, Vec<String>> = HashMap::new();
    for row in rows {
        keys.entry(row.table_name.as_str())
            .or_default()
            .push(row.primary_key.clone());
    }
    keys
}

/// id dead-letter của các dòng replay là bản mới nhất của (table, khóa chính) trong batch.
fn latest_replay_ids(rows: &[ChangeRow]) -> Vec<i64> {
    let mut latest: HashMap<(&str, &str), &ChangeRow> = HashMap::new();
    for row in rows {
        let key = (row.table_name.as_str(), row.primary_key.as_str());
        if latest.get(&key).is_none_or(|existing| existing.index < row.index) {
            latest.insert(key, row);
        }
    }
    latest
        .values()
        .filter_map(|row| row.source.dead_letter_id)
        .collect()
}
```

Trong `process_batch`:
- Đổi chữ ký thành:

```rust
/// Ghi các dòng vào đích: tạo table/cột khi cần, rồi upsert/delete theo từng table.
/// Chỉ cập nhật `schema_cache` sau khi DDL thành công. Trả về các dòng bị DB từ chối vì lỗi dữ liệu.
async fn process_batch<'a>(
    sink: &Sink,
    rows: &'a [ChangeRow],
    schema_cache: &mut SchemaCache,
) -> Result<Vec<Rejected<'a, ChangeRow>>, String> {
    let mut rejected: Vec<Rejected<'a, ChangeRow>> = Vec::new();
    let mut message_active: HashMap<String, Vec<&'a ChangeRow>> = HashMap::new();
```

- Ba lời gọi `flush_table(sink, table_name, &buffered).await?;` / `flush_table(sink, table_name, table_rows).await?;` thành `flush_table(sink, table_name, &buffered, &mut rejected).await?;` và `flush_table(sink, table_name, table_rows, &mut rejected).await?;`.
- Cuối hàm: `Ok(rejected)` thay cho `Ok(())`.

Thay `flush_table` bằng:

```rust
/// Ghi các dòng của một table: mỗi khóa chính một bản mới nhất, delete trước rồi upsert.
/// Lỗi dữ liệu thì chia đôi để tìm đúng dòng lỗi; dòng lỗi được thêm vào `rejected`.
async fn flush_table<'a>(
    sink: &Sink,
    table_name: &str,
    rows: &[&'a ChangeRow],
    rejected: &mut Vec<Rejected<'a, ChangeRow>>,
) -> Result<(), String> {
    let (to_delete, to_upsert): (Vec<&'a ChangeRow>, Vec<&'a ChangeRow>) = latest_per_key(rows)
        .into_iter()
        .partition(|row| row.action == RowAction::Delete);
    rejected.extend(
        write_isolating(to_delete, |chunk| {
            sink.destination.delete_rows(table_name, chunk, &sink.pool)
        })
        .await?,
    );
    rejected.extend(
        write_isolating(to_upsert, |chunk| {
            sink.destination.upsert_rows(table_name, chunk, &sink.pool)
        })
        .await?,
    );
    Ok(())
}
```

- [ ] **Step 4: Build, format và chạy test**

Run: `cargo fmt && cargo build && cargo test`
Expected: build thành công; toàn bộ test PASS, gồm `tests::keys_are_grouped_by_table` và `tests::only_newest_replayed_row_per_key_counts`.

---

### Task 7: Replay các dòng `retry`

**Files:**
- Modify: `src/main.rs`
- Modify: `.env.example`

**Interfaces:**
- Consumes: `persist_batch` (Task 6); `DeadLetterStore::fetch_retry`, `RetryEntry` (Task 4); `parse_change`, `Parsed`, `RowSource` (Task 3).
- Produces: `async fn replay_dead_letters(sink: &Sink, sync_config: Option<&SyncConfig>, schema_cache: &mut SchemaCache) -> Result<usize, String>`; biến env `DEAD_LETTER_REPLAY_SECS`.

Hàm này chỉ ghép các phần đã có unit test; kiểm chứng bằng e2e ở Task 8.

- [ ] **Step 1: Thêm hằng số, import và hàm replay**

Sửa import trong `src/main.rs`:

```rust
use std::{
    collections::{HashMap, HashSet},
    env,
    error::Error,
    time::{Duration, Instant},
};
```

```rust
use crate::models::{
    ChangeRow, DataModel, DeadLetterStore, NatsReceive, NewDeadLetter, Parsed, PoisonMessage,
    PostgresDestination, Rejected, RowAction, RowSource, SyncConfig, latest_per_key,
    parse_change, write_isolating,
};
```

Thêm hằng số sau `ACK_PROGRESS_INTERVAL`:

```rust
/// Số dòng dead-letter tối đa mỗi lần replay.
const REPLAY_BATCH_LIMIT: i64 = 500;
```

Thêm hàm (sau `wait_keeping_messages`):

```rust
/// Replay các dòng `retry` của `_cdc_dead_letter` qua đúng luồng ghi của batch thường.
/// Ghi được thì `resolved`, lại lỗi dữ liệu thì về `pending`; `Err` (lỗi tạm thời) thì giữ `retry`.
async fn replay_dead_letters(
    sink: &Sink,
    sync_config: Option<&SyncConfig>,
    schema_cache: &mut SchemaCache,
) -> Result<usize, String> {
    let entries = sink
        .dead_letters
        .fetch_retry(REPLAY_BATCH_LIMIT, &sink.pool)
        .await?;
    let count = entries.len();
    if count == 0 {
        return Ok(0);
    }
    let mut rows = Vec::new();
    let mut poison = Vec::new();
    let mut skipped = Vec::new();
    // index theo thứ tự id: cùng khóa chính thì dòng dead-letter mới hơn thắng
    for (index, entry) in entries.into_iter().enumerate() {
        let id = entry.id;
        let source = RowSource {
            subject: entry.subject,
            stream_sequence: entry.stream_sequence.map(|sequence| sequence as u64),
            payload: entry.payload.into(),
            dead_letter_id: Some(id),
        };
        match parse_change(source, index as i64, sync_config) {
            Parsed::Row { row, .. } => rows.push(row),
            Parsed::Poison(message) => poison.push(message),
            Parsed::Skip(reason) => {
                info!(
                    dead_letter_id = id,
                    reason = ?reason,
                    "Dead letter no longer needs syncing, marked resolved"
                );
                skipped.push(id);
            }
        }
    }
    persist_batch(sink, &rows, &poison, &skipped, schema_cache).await?;
    Ok(count)
}
```

- [ ] **Step 2: Gọi replay trong vòng lặp chính**

Sau khối đọc `max_batch_retries`, thêm:

```rust
    let replay_interval = Duration::from_secs(
        env::var("DEAD_LETTER_REPLAY_SECS")
            .unwrap_or("30".to_string())
            .parse::<u64>()?,
    );
```

Thêm vào `info!(... "Configuration loaded")` field `replay_secs = replay_interval.as_secs(),` (sau `max_batch_retries,`).

Ngay trước `let mut logged_type_errors`, thêm `let mut last_replay = Instant::now();`. Ở **đầu** thân `loop {` của vòng lặp chính (trước `let batch = match ...`), thêm:

```rust
        // Replay chạy giữa hai batch nên không bao giờ song song với ghi batch thường
        if !replay_interval.is_zero() && last_replay.elapsed() >= replay_interval {
            last_replay = Instant::now();
            match replay_dead_letters(&sink, sync_config.as_ref(), &mut schema_cache).await {
                Ok(0) => {}
                Ok(count) => info!(count, "Replayed dead letters"),
                Err(e) => {
                    // Dòng vẫn ở trạng thái retry, thử lại ở chu kỳ sau
                    warn!(error = %e, "Dead letter replay failed, will retry next cycle");
                    match sink.destination.get_schema_info(&sink.pool).await {
                        Ok(fresh) => schema_cache = fresh,
                        Err(e) => warn!(error = %e, "Failed to reload schema info"),
                    }
                }
            }
        }
```

- [ ] **Step 3: Ghi chú biến env**

Thêm vào cuối `.env.example`:

```
# Chu kỳ (giây) replay dữ liệu lỗi được đánh dấu retry trong _cdc_dead_letter; 0 để tắt.
#   UPDATE _cdc_dead_letter SET status = 'retry' WHERE status = 'pending' AND table_name = 'users';
# DEAD_LETTER_REPLAY_SECS=30
```

- [ ] **Step 4: Build, format và chạy test**

Run: `cargo fmt && cargo build 2>&1 | grep -E '^(warning|error)' ; cargo test`
Expected: chỉ còn 2 warning cũ (`Operation`, `parse_record`… trong `data_record.rs`); toàn bộ test PASS.

---

### Task 8: E2E suite `14-dead-letter`

**Files:**
- Modify: `test/e2e/run.sh` (hook `after-changes.sh`)
- Modify: `docker-compose.test.yml` (env `DEAD_LETTER_REPLAY_SECS: "2"` cho service `cdcsink`)
- Create: `test/e2e/suites/14-dead-letter/setup.sql`
- Create: `test/e2e/suites/14-dead-letter/sink-before-changes.sql`
- Create: `test/e2e/suites/14-dead-letter/changes.sql`
- Create: `test/e2e/suites/14-dead-letter/after-changes.sh`
- Create: `test/e2e/suites/14-dead-letter/verify.sql`

**Interfaces:**
- Consumes: toàn bộ tính năng Task 1–7; helper trong `run.sh`: `$DC`, `src_psql`, `sink_psql`, `wait_marker`, biến `stalled`, `WAIT_TIMEOUT`; `e2e_check`, bảng `e2e_result` từ `test/e2e/lib/verify.sql`.
- Produces: hook tùy chọn `after-changes.sh` cho mọi suite.

- [ ] **Step 1: Thêm hook vào `run.sh`**

Trong `run_suite`, ngay sau khối `if [[ $stalled == 0 && -f "$dir/changes.sql" ]]; then ... fi`, thêm:

```bash
    # Tùy chọn: kịch bản riêng của suite sau khi changes đã tới đích. Chạy bằng source nên dùng được
    # src_psql, sink_psql, wait_marker, $DC, WAIT_TIMEOUT và đặt stalled=1 khi thấy sai.
    if [[ $stalled == 0 && -f "$dir/after-changes.sh" ]]; then
        echo " - chạy after-changes.sh..."
        source "$dir/after-changes.sh"
    fi
```

Cập nhật comment đầu file (khối "Mỗi suite gồm:"), thêm dòng:

```bash
#   after-changes.sh : (tùy chọn) bash chạy sau khi changes tới đích, vd thao tác dead-letter rồi chờ replay
```

- [ ] **Step 2: Rút ngắn chu kỳ replay trong e2e**

Trong `docker-compose.test.yml`, service `cdcsink`, `environment:` thêm (sau `SYNC_CONFIG_PATH`):

```yaml
      DEAD_LETTER_REPLAY_SECS: "2"
```

- [ ] **Step 3: Tạo dữ liệu suite**

`test/e2e/suites/14-dead-letter/setup.sql`:

```sql
-- Table không có trong sync_config: sync toàn bộ cột, toàn bộ dòng.
CREATE TABLE dl_accounts (
    id      integer PRIMARY KEY,
    email   text NOT NULL,
    balance numeric(12,2) NOT NULL
);
ALTER TABLE dl_accounts REPLICA IDENTITY FULL;

INSERT INTO dl_accounts VALUES (1, 'a@x', 10), (2, 'b@x', 20), (3, 'c@x', 30), (4, 'd@x', 40);
```

`test/e2e/suites/14-dead-letter/sink-before-changes.sql`:

```sql
-- Constraint chỉ có ở đích: dòng balance < 0 bị từ chối (SQLSTATE 23514) và phải vào dead-letter
-- thay vì chặn cả pipeline.
ALTER TABLE dl_accounts ADD CONSTRAINT dl_balance_non_negative CHECK (balance >= 0);
```

`test/e2e/suites/14-dead-letter/changes.sql`:

```sql
INSERT INTO dl_accounts VALUES (5, 'e@x', 50), (6, 'f@x', -60), (7, 'g@x', 70);  -- 6 bị từ chối
UPDATE dl_accounts SET balance = -11 WHERE id = 2;                                  -- 2 bị từ chối
UPDATE dl_accounts SET balance = -22 WHERE id = 2;  -- chỉ còn bản mới nhất của 2 ở trạng thái pending
UPDATE dl_accounts SET email = 'c2@x' WHERE id = 3;
DELETE FROM dl_accounts WHERE id = 4;
```

- [ ] **Step 4: Tạo kịch bản `after-changes.sh`**

`test/e2e/suites/14-dead-letter/after-changes.sh`:

```bash
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
```

- [ ] **Step 5: Tạo `verify.sql`**

`test/e2e/suites/14-dead-letter/verify.sql`:

```sql
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
```

- [ ] **Step 6: Chạy suite mới**

Run: `bash test/e2e/run.sh 14-dead-letter`
Expected: `PASS   14-dead-letter   E2E_ERRORS=0 E2E_WARNS=0`. Nếu FAIL, chạy lại với `KEEP=1` và xem `docker compose -f docker-compose.test.yml logs cdcsink`.

- [ ] **Step 7: Chạy toàn bộ e2e (kiểm tra hồi quy)**

Run (chạy nền, có thể mất 15–25 phút): `bash test/e2e/run.sh`
Expected: mọi suite `PASS`, trừ `09-columns-differ-by-case` và `11-numeric-nan` là `KNOWN` như trước.

- [ ] **Step 8: Dọn dẹp**

Run: `docker compose -f docker-compose.test.yml down -v`
