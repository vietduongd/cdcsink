# Thiết kế: xử lý dữ liệu lỗi (dead-letter)

- Ngày: 2026-09-30
- Trạng thái: chờ review
- Phạm vi: `cdcsink` (NATS JetStream → PostgreSQL)

## 1. Mục tiêu

Hiện có hai loại dữ liệu lỗi:

- **Message hỏng** (payload không parse được, thiếu tên table hoặc cấu trúc): bị `Term` và chỉ còn trong log.
- **Dòng bị DB đích từ chối** (vi phạm constraint, sai kiểu, cast lỗi): cả batch retry rồi service thoát, nên **cả pipeline bị chặn** ở dòng đó.

Sau thay đổi:

1. Dữ liệu lỗi **không làm tắc** các dòng khác: chỉ dòng lỗi bị tách ra, phần còn lại của batch ghi bình thường.
2. **Không mất dữ liệu**: dữ liệu lỗi được lưu vào table `_cdc_dead_letter` ở DB đích, kèm lý do.
3. **Replay được**: sau khi sửa nguyên nhân, vận hành đánh dấu bằng SQL, cdcsink tự ghi lại.
4. Bản lỗi cũ **không bao giờ ghi đè** bản mới hơn của cùng dòng.

Ngoài phạm vi: DDL lỗi (`CREATE TABLE`, `ADD COLUMN`) vẫn retry rồi thoát như hiện tại, vì thường cần người sửa schema và tách dòng không giúp gì. Không có subject NATS dead-letter, không có CLI riêng.

## 2. Phân loại lỗi

Phân loại theo SQLSTATE của lỗi Postgres:

| Loại | SQLSTATE | Xử lý |
|------|----------|-------|
| Lỗi dữ liệu | `22xxx` (data exception: sai giá trị, cast lỗi, tràn số…), `23xxx` (integrity: NOT NULL, UNIQUE, CHECK, FK) | Bisection để tìm dòng lỗi, đưa vào dead-letter |
| Lỗi tạm thời | Mọi lỗi khác: mất kết nối, deadlock, table bị drop, lỗi không phải từ DB | Giữ cơ chế hiện có: retry cả batch có backoff, hết lượt thì thoát mã 1, không ack |

Kiểu lỗi trong code:

```rust
pub enum WriteError {
    Data { code: String, message: String },
    Transient(String),
}
```

`WriteError::from(sqlx::Error)` đọc `DatabaseError::code()`; hai ký tự đầu là `22` hoặc `23` thì là `Data`, còn lại là `Transient`.

## 3. Table `_cdc_dead_letter`

Tạo khi khởi động (cùng lúc với `_cdc_schema_metadata`), trong schema `DATABASE_SCHEMA_EXPECT`:

```sql
CREATE TABLE IF NOT EXISTS _cdc_dead_letter (
  id              BIGSERIAL PRIMARY KEY,
  kind            TEXT NOT NULL,          -- 'poison' | 'rejected'
  subject         TEXT NOT NULL,          -- subject NATS gốc
  stream_sequence BIGINT,
  table_name      TEXT,                   -- NULL nếu poison không đọc được tên table
  primary_key     TEXT,
  payload         TEXT NOT NULL,          -- payload gốc Debezium, sửa được bằng UPDATE
  error_code      TEXT,                   -- SQLSTATE, vd 23514; NULL với poison
  error_message   TEXT NOT NULL,
  status          TEXT NOT NULL DEFAULT 'pending',
  attempts        INT  NOT NULL DEFAULT 1,
  created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS _cdc_dead_letter_open_idx
  ON _cdc_dead_letter (table_name, primary_key) WHERE status IN ('pending', 'retry');
CREATE INDEX IF NOT EXISTS _cdc_dead_letter_retry_idx
  ON _cdc_dead_letter (id) WHERE status = 'retry';
```

- Lưu **payload gốc** chứ không lưu dòng đã xử lý: khi replay, dữ liệu đi lại đúng đường như message mới (parse, áp `sync_config`, tạo table/cột nếu cần). Payload sai thì vận hành sửa bằng `UPDATE … SET payload = …`.
- Payload không phải UTF-8 hợp lệ được lưu bằng `String::from_utf8_lossy`.
- `_cdc_dead_letter` được thêm vào danh sách table không bao giờ sync (`is_ignored_table`), giống `_cdc_schema_metadata`, để tránh vòng lặp khi cdcsink nối tiếp cdcsink.

### 3.1 Trạng thái

| status | Ai đặt | Ý nghĩa |
|--------|--------|---------|
| `pending` | cdcsink | Chờ xử lý |
| `retry` | Vận hành | Yêu cầu replay |
| `resolved` | cdcsink | Replay thành công |
| `superseded` | cdcsink | Đã có bản mới hơn của cùng `(table_name, primary_key)` |
| `discarded` | Vận hành | Bỏ qua, cdcsink không động tới |

cdcsink chỉ đọc các dòng `retry`, và chỉ đổi trạng thái của dòng `pending`/`retry`.

### 3.2 Không để bản cũ ghi đè bản mới

Sau mỗi batch, với mỗi table, mọi khóa chính **được ghi thành công hoặc vừa bị đưa vào dead-letter** sẽ làm các dòng dead-letter `pending`/`retry` cũ hơn của cùng `(table_name, primary_key)` chuyển sang `superseded`:

```sql
UPDATE _cdc_dead_letter
   SET status = 'superseded', updated_at = now()
 WHERE status IN ('pending', 'retry')
   AND table_name = $1 AND primary_key = ANY($2)
   AND id <> ALL($3);   -- trừ các dòng dead-letter vừa ghi / đang replay trong batch này
```

Nhờ vậy mỗi dòng chỉ còn tối đa một bản dead-letter mở, và đó luôn là bản mới nhất. Batch và replay chạy tuần tự trong một vòng lặp nên không có race condition.

Message `poison` không có khóa chính nên không bao giờ bị `superseded`.

## 4. Luồng xử lý

### 4.1 Nhận message (`nats_receive.rs`)

`receive_messages` trả về:

```rust
pub struct ReceivedBatch {
    pub rows: Vec<ChangeRow>,          // dòng hợp lệ, theo thứ tự nhận
    pub poison: Vec<PoisonMessage>,    // message hỏng: subject, sequence, payload, lý do
    pub messages: Vec<Message>,        // mọi message cần ack sau khi batch xong
}
```

- Message hỏng **không còn bị `Term` ngay**: `main` ghi chúng vào dead-letter (`kind = 'poison'`) rồi mới ack cùng batch.
- Message bị bỏ qua có chủ đích (tombstone, table không bao giờ sync, table không có `id`) vẫn được ack ngay như hiện tại, không vào dead-letter.

### 4.2 Parse dùng chung (`change.rs`)

```rust
pub struct ChangeRow {
    pub table_name: String,
    pub table_value: HashMap<String, DataModel>,
    pub primary_key: Option<String>,
    pub action: RowAction,
    pub index: i64,
    pub source: RowSource,
}

pub struct RowSource {
    pub subject: String,
    pub stream_sequence: Option<u64>,
    pub payload: Arc<str>,
    pub dead_letter_id: Option<i64>,   // Some khi dòng đến từ replay
}

pub enum Parsed {
    Row(ChangeRow),
    Skip(SkipReason),       // tombstone, table bị bỏ qua, thiếu id
    Poison(String),         // lý do
}

pub fn parse_change(payload: &[u8], subject: &str, sync_config: Option<&SyncConfig>) -> Parsed;
```

`NatMessageReceive` được thay bằng `ChangeRow`. Log cảnh báo lỗi kiểu của bộ lọc (`logged_type_errors`) vẫn nằm ở `nats_receive.rs`; `parse_change` trả kèm `MatchOutcome` để bên gọi log.

### 4.3 Ghi batch với bisection (`isolate.rs`)

Khi upsert hoặc delete batch của một table trả `WriteError::Data`:

1. Chia các dòng làm hai nửa, ghi từng nửa. Nửa nào thành công thì xong.
2. Nửa nào vẫn `Data` thì chia đôi tiếp. Còn đúng 1 dòng mà vẫn `Data` thì dòng đó bị từ chối, kèm `code` và `message`.
3. Gặp `Transient` ở bất kỳ bước nào: dừng ngay và trả lỗi lên, cả batch retry như hiện tại. Chạy lại là an toàn vì mọi thao tác đều idempotent.

```rust
pub async fn write_isolating<'a, T, F, Fut>(
    items: Vec<&'a T>,
    write: F,
) -> Result<Vec<(&'a T, String /*code*/, String /*message*/)>, String>
where
    F: FnMut(Vec<&'a T>) -> Fut,
    Fut: Future<Output = Result<(), WriteError>>;
```

- Viết bằng vòng lặp với stack, không đệ quy async.
- Lần gọi đầu là cả batch, nên khi không có lỗi chi phí giống hệt hiện tại.
- Với k dòng lỗi trong n dòng, số lần gọi `write` không vượt quá `1 + 2·k·⌈log₂n⌉`.
- Mỗi table chỉ còn một bản cho mỗi khóa chính (`remove_duplicate_data`), nên thứ tự giữa hai nửa không ảnh hưởng kết quả.
- Delete và upsert của cùng table được bisection riêng.

### 4.4 Sau khi ghi (`main.rs`)

`process_batch` trả về danh sách dòng bị từ chối. Sau đó, trong cùng lượt xử lý batch:

1. Ghi các dòng bị từ chối vào dead-letter (`kind = 'rejected'`):
   - `dead_letter_id` là `None`: `INSERT` dòng mới.
   - `dead_letter_id` là `Some(id)` (đang replay): `UPDATE` dòng đó về `pending`, cập nhật `error_code`, `error_message`, `attempts = attempts + 1`.
2. Ghi các message hỏng vào dead-letter (`kind = 'poison'`).
3. Chạy `superseded` (mục 3.2) cho từng table.
4. Ack toàn bộ message của batch.

Lỗi khi ghi dead-letter được coi là `Transient`: cả batch retry, không ack. Không bao giờ ack một message mà dữ liệu của nó chưa được ghi vào đích hoặc vào dead-letter.

Mỗi dòng đưa vào dead-letter ghi một log `WARN` gồm `table`, `primary_key`, `error_code`, `dead_letter_id`.

### 4.5 Replay

- Chu kỳ `DEAD_LETTER_REPLAY_SECS` (mặc định 30, `0` để tắt). Kiểm tra sau mỗi vòng fetch, tức là ở giữa hai batch, nên không chạy song song với batch.
- Mỗi lần lấy tối đa 500 dòng `status = 'retry'`, `ORDER BY id`.
- Mỗi dòng được parse lại bằng `parse_change` với `dead_letter_id = Some(id)`:
  - `Poison`: `UPDATE` về `pending` với lý do mới, `attempts + 1`.
  - `Skip`: đánh dấu `resolved` (ví dụ table đã bị đưa vào danh sách bỏ qua), log `INFO`.
  - `Row`: gom tất cả vào một lần `process_batch` (có tạo table/cột và bisection như batch thường).
- Kết quả:
  - Dòng ghi thành công: `resolved`.
  - Dòng bị từ chối: về `pending` với lỗi mới (mục 4.4).
  - Lỗi `Transient`: giữ `retry`, thử lại ở chu kỳ sau, log `WARN`. Replay không làm service thoát.
- Các dòng dead-letter được replay trong cùng lần gom: nếu hai dòng cùng khóa chính thì `remove_duplicate_data` giữ bản có `index` lớn hơn; `index` của dòng replay lấy theo thứ tự `id` nên bản mới hơn thắng, bản còn lại bị `superseded`.

Cách dùng của vận hành:

```sql
-- xem dữ liệu lỗi
SELECT id, kind, table_name, primary_key, error_code, error_message, attempts, created_at
  FROM _cdc_dead_letter WHERE status = 'pending' ORDER BY id;

-- sửa nguyên nhân (gỡ constraint, sửa kiểu cột, hoặc sửa payload) rồi:
UPDATE _cdc_dead_letter SET status = 'retry', updated_at = now()
 WHERE table_name = 'users' AND status = 'pending';

-- bỏ qua hẳn
UPDATE _cdc_dead_letter SET status = 'discarded', updated_at = now() WHERE id = 42;
```

## 5. Cấu hình mới

| Biến env | Mặc định | Ý nghĩa |
|----------|----------|---------|
| `DEAD_LETTER_REPLAY_SECS` | `30` | Chu kỳ replay; `0` để tắt |

Ghi chú thêm vào `.env.example`.

## 6. Thay đổi file

| File | Thay đổi |
|------|----------|
| `src/models/write_error.rs` (mới) | `WriteError`, phân loại SQLSTATE |
| `src/models/change.rs` (mới) | `ChangeRow`, `RowSource`, `parse_change` |
| `src/models/isolate.rs` (mới) | `write_isolating` (bisection) |
| `src/models/dead_letter.rs` (mới) | Tạo table, ghi poison/rejected, superseded, lấy `retry`, đánh dấu `resolved` |
| `src/models/nats_receive.rs` | Trả `ReceivedBatch`, dùng `parse_change`, bỏ `Term` ngay; `_cdc_dead_letter` vào `is_ignored_table` |
| `src/models/postgres_destination.rs` | Hàm ghi trả `WriteError`; upsert/delete nhận `&[&ChangeRow]` |
| `src/main.rs` | `process_batch` dùng bisection và trả dòng bị từ chối; ghi dead-letter; vòng replay |
| `.env.example` | `DEAD_LETTER_REPLAY_SECS` |
| `test/e2e/run.sh` | Hook tùy chọn `after-changes.sh` |
| `test/e2e/suites/14-dead-letter/` (mới) | Suite e2e |

## 7. Kiểm thử

### 7.1 Unit test
- `WriteError`: `23514` → `Data`, `22P02` → `Data`, `08006` → `Transient`, `40P01` → `Transient`, lỗi không phải DB → `Transient`.
- `parse_change`: payload hợp lệ → `Row`; JSON hỏng → `Poison`; payload rỗng → `Skip`; `_cdc_dead_letter` / `_cdc_schema_metadata` → `Skip`; thiếu `id` → `Skip`.
- `write_isolating` với writer giả:
  - 0 dòng lỗi → gọi `write` đúng 1 lần.
  - 1 dòng lỗi trong 1000 → trả đúng dòng đó, số lần gọi ≤ `1 + 2·⌈log₂1000⌉`.
  - Nhiều dòng lỗi, kể cả mọi dòng đều lỗi → trả đúng tập dòng lỗi, mọi dòng tốt đều được ghi.
  - `Transient` giữa chừng → trả `Err`, không trả danh sách từ chối.

### 7.2 E2E suite `14-dead-letter`
- `setup.sql`: table `accounts` có `id`, `email`, `balance`.
- `sink-before-changes.sql`: thêm `CHECK (balance >= 0)` ở đích.
- `changes.sql`: ghi lẫn dòng tốt và dòng vi phạm (`balance < 0`); update một dòng lỗi thành giá trị tốt (phải thành `superseded`); update một dòng lỗi thành giá trị lỗi khác (chỉ còn bản mới nhất `pending`).
- `after-changes.sh` (hook mới, chạy sau khi chờ marker `changes`, có sẵn `$DC`, `src_psql`, `sink_psql`):
  1. Kiểm tra dead-letter có đúng các dòng `pending` mong đợi.
  2. Publish 1 message hỏng lên `debezium.public.garbage`.
  3. Gỡ constraint, `UPDATE … SET status = 'retry'` cho các dòng `rejected`.
  4. Chờ đến khi không còn dòng `retry` (timeout theo `WAIT_TIMEOUT`).
- `verify.sql`: nguồn và đích khớp 100%; `_cdc_dead_letter` có đúng số dòng `resolved`, `superseded`, và 1 dòng `poison` còn `pending`.
- Chạy lại toàn bộ 13 suite cũ để bảo đảm không hồi quy.
