# Thiết kế: cấu hình cột và bộ lọc dòng theo từng table

- Ngày: 2026-09-29
- Trạng thái: chờ review
- Phạm vi: `cdcsink` (NATS JetStream → PostgreSQL)

## 1. Mục tiêu

Cho phép cấu hình theo từng table nhận được từ CDC:

1. **Chọn cột được sync**, bằng `include` hoặc `exclude`.
2. **Lọc dòng theo điều kiện** (`where`). Dòng không khớp sẽ không có ở đích; nếu trước đó đã có thì bị xóa.

Table không khai báo trong config được sync toàn bộ cột và toàn bộ dòng, giống hành vi hiện tại. Không đặt biến `SYNC_CONFIG_PATH` thì hệ thống chạy y như trước khi có tính năng này, và in một dòng log khi khởi động để tránh trường hợp quên cấu hình trên production:

```
SYNC_CONFIG_PATH not set: syncing ALL tables/columns/rows (no filter)
```

## 2. Cấu hình

Đường dẫn file đọc từ env `SYNC_CONFIG_PATH`, định dạng YAML. File chỉ được nạp **một lần khi khởi động**; đổi config thì phải restart.

```yaml
tables:
  orders:
    include: [id, total, status]        # hoặc exclude, không dùng cả hai
    where:                              # tất cả điều kiện phải đúng (AND)
      - { column: tenant_id,  op: eq, value: 5 }
      - { column: status,     op: in, value: [paid, shipped] }
      - { column: deleted_at, op: is_null }
  users:
    exclude: [password_hash, secret]
```

### 2.1 Chọn cột
- `include`: chỉ sync các cột được liệt kê. `exclude`: sync mọi cột trừ các cột được liệt kê.
- Một table không được khai báo cả `include` lẫn `exclude`. `include` không được là danh sách rỗng.
- Cột `id` **luôn được giữ**, vì upsert (`ON CONFLICT (id)`) và delete đều dựa vào nó.
- Cột đã có ở đích rồi sau đó bị loại khỏi config thì **không bị drop**, chỉ ngừng được cập nhật. Nếu cột đó là `NOT NULL`, lệnh insert dòng mới sẽ lỗi; file mẫu phải ghi rõ cảnh báo này.

### 2.2 Bộ lọc dòng (`where`)
- Là danh sách điều kiện, kết hợp bằng AND. Mỗi điều kiện có dạng `{ column, op, value? }`.
- Toán tử: `eq`, `ne`, `gt`, `gte`, `lt`, `lte`, `in`, `not_in`, `is_null`, `not_null`.
  - `is_null` và `not_null` **không** được có `value`.
  - `in` và `not_in` bắt buộc `value` là mảng.
  - Các toán tử còn lại bắt buộc `value` là một giá trị đơn (số, chuỗi, boolean).
- `where` được phép tham chiếu cột bị loại bởi `include`/`exclude`. Điều kiện được xét **trước** khi lọc cột.
- **So sánh trên giá trị đã chuyển kiểu**, tức là giống giá trị sẽ ghi vào đích:
  - Số: so theo giá trị số (`f64`). Decimal Debezium dạng `{scale, value(base64)}` được giải mã trước khi so.
  - Chuỗi: so theo thứ tự từ điển. Date và timestamp đã được đổi sang dạng `YYYY-MM-DD[ HH:MM:SS]`, nên `{column: created_at, op: gte, value: "2025-01-01"}` hoạt động đúng.
  - Boolean: chỉ dùng được với `eq`, `ne`, `in`, `not_in`.
- **Với NULL hoặc khi thiếu cột:** mọi toán tử trừ `is_null` đều cho kết quả false, giống cách SQL xử lý NULL. `is_null` cho kết quả true khi giá trị là NULL hoặc cột không có.
- **Khi kiểu không khớp** (ví dụ `gt` giữa chuỗi và số, hoặc `gt` với boolean): điều kiện được coi là false và hệ thống ghi nhận lỗi kiểu (xem mục 5.2).

### 2.3 So khớp tên
- Tên table được so **sau khi bỏ hậu tố `_resync`**.
- Tên table, tên cột trong `include`/`exclude` và `where.column` đều được so khớp chính xác trước; nếu không có thì so khớp không phân biệt hoa thường.
- Nếu config có hai key table chỉ khác nhau về hoa thường (ví dụ `Orders` và `orders`), hệ thống báo lỗi khi khởi động.
- Hệ thống không tự đổi giữa snake_case và CamelCase: `order_items` không khớp với `OrderItems`.
- Key là tên table không kèm schema (`OrderItems`, không phải `public.OrderItems`) và không kèm `_resync`. Schema đích lấy từ `DATABASE_SCHEMA_EXPECT`.
- Quy tắc không phân biệt hoa thường chỉ áp dụng cho **tên** table và cột. **Giá trị** trong `where` được so chính xác: `value: Paid` không khớp với dữ liệu `paid`.

Ví dụ với table CamelCase:

```yaml
tables:
  OrderItems:
    include: [id, OrderId, ProductId, Quantity, UnitPrice]
    where:
      - { column: TenantId, op: eq, value: 5 }
      - { column: Status,   op: in, value: [Paid, Shipped] }
  UserAccounts:
    exclude: [PasswordHash, SecurityStamp]
```

## 3. Hành vi theo từng message

| Tình huống | Hành động |
|---|---|
| Có cờ `_PEERDB_IS_DELETED = true` | Delete |
| `where` không khớp | Delete |
| Còn lại | Upsert |

- Delete được thực hiện bằng câu lệnh có sẵn `DELETE ... WHERE id = ANY($1)`. Lệnh này an toàn nếu dòng chưa từng có ở đích.
- Message có hành động Delete **không kích hoạt DDL** (không tạo table, không thêm cột). Nếu table chưa có trong `schema_cache`, message đó bị bỏ qua nhưng vẫn được ack.
- Loại trùng theo `id` trong batch (giữ message mới nhất) được làm **trước** khi phân nhóm Upsert/Delete. Vì vậy trạng thái cuối cùng của một dòng trong batch quyết định hành động.
- Cờ `_PEERDB_IS_DELETED` được đọc **trước** khi lọc cột, nên delete vẫn đúng kể cả khi cột cờ bị `exclude`.

## 4. Thay đổi trong code

### 4.1 Module mới `src/models/sync_config.rs`
Không đụng I/O, ngoại trừ `load`.
- `SyncConfig::load(path) -> Result<SyncConfig, String>`: đọc và validate, **gom tất cả lỗi** rồi trả về một lượt.
- `SyncConfig::table(name) -> Option<&TableConfig>`: so khớp chính xác, sau đó không phân biệt hoa thường.
- `TableConfig::matches(&HashMap<String, DataModel>) -> MatchOutcome { matched: bool, type_errors: Vec<TypeError> }`.
- `TableConfig::retain_columns(&mut HashMap<String, DataModel>)`: luôn giữ `id`.
- `RowAction { Upsert, Delete }`.
- Hàm thuần `classify(table_value, Option<&TableConfig>) -> (RowAction, MatchOutcome)`, kết hợp cờ `_PEERDB_IS_DELETED` với `matches`, để test được mà không cần message NATS thật.

### 4.2 Hàm giải mã decimal dùng chung
`convert_base64_to_decimal` được chuyển từ `postgres_destination.rs` sang một module helper để bộ lọc dùng lại. Logic giữ nguyên, kể cả giới hạn: đi qua `f64` và `i64`.

### 4.3 `nats_receive.rs`
- `NatMessageReceive` thêm trường `action: RowAction`.
- `receive_messages` nhận thêm `Option<&SyncConfig>` và `&mut HashSet` để khử trùng log lỗi kiểu. Với mỗi message:
  1. Parse, lấy tên table, bỏ hậu tố `_resync`. Bước này chuyển từ `main.rs` sang.
  2. Gọi `get_table_structure()`. Message không có `id` vẫn bị bỏ qua như hiện tại.
  3. Gọi `classify(...)` để có `action`, rồi log lỗi kiểu nếu có.
  4. Gọi `retain_columns(...)`.
- Sau mỗi batch: nếu một table có mọi message bị loại vì lỗi kiểu, in `WARNING: all N rows of table X rejected due to type mismatch in where`.

### 4.4 `main.rs`
- Nạp `SyncConfig` nếu có `SYNC_CONFIG_PATH`, rồi log danh sách table đã cấu hình. Config lỗi thì thoát ngay. Không có biến thì in dòng log "not set" ở mục 1.
- Bỏ đoạn strip `_resync` vì đã chuyển sang 4.3.
- Message có `action == Delete` bỏ qua phần kiểm tra và tạo table/cột. Nếu table không có trong `schema_cache` thì không đưa vào `message_active`.

### 4.5 `postgres_destination.rs`
- `insert_value` phân nhóm theo `msg.action` thay vì đọc `_PEERDB_IS_DELETED` trong `table_value`.
- Câu INSERT và DELETE dùng `quote_identifier(schema_expect)`, sửa lỗi schema CamelCase.

### 4.6 Dependency và file phụ
- `Cargo.toml`: thêm `serde_yaml_ng`.
- `.env.example`: thêm `SYNC_CONFIG_PATH`.
- Thêm `sync_config.example.yaml`, có ghi chú cảnh báo về cột `NOT NULL`, kèm ví dụ table snake_case và CamelCase như ở mục 2.3.

## 5. Xử lý lỗi

### 5.1 Khi khởi động (fail-fast)
Các lỗi sau làm ứng dụng dừng, và mọi lỗi được liệt kê trong một thông báo:
- Không đọc được file, hoặc YAML sai cú pháp.
- Khai báo cả `include` lẫn `exclude`; `include` rỗng.
- Toán tử lạ; `value` sai dạng so với toán tử; `is_null`/`not_null` có kèm `value`.
- Key table trùng khi bỏ qua hoa thường.

Ví dụ:
```
Invalid sync config:
  - tables.orders: cannot set both include and exclude
  - tables.orders.where[1]: op 'in' requires array value
  - tables: 'Orders' and 'orders' collide (case-insensitive)
```

### 5.2 Khi chạy
- `matches` không panic. Lỗi kiểu được log **một lần cho mỗi bộ (table, cột, toán tử)** trong suốt vòng đời process.
- Các chỗ khác giữ nguyên cách xử lý lỗi hiện tại.

## 6. Kiểm thử

Unit test viết trước theo TDD:
- Parse config hợp lệ; từng trường hợp lỗi ở mục 5.1; kiểm tra lỗi được gom đủ.
- Tra table: khớp chính xác, khớp không phân biệt hoa thường, không khớp.
- `retain_columns`: include, exclude, `id` luôn được giữ, tên cột khác hoa thường.
- `matches`: từng toán tử với số, chuỗi, boolean; decimal base64 (gồm số âm); so sánh date/timestamp dạng chuỗi; NULL và thiếu cột; lỗi kiểu cho `matched=false` kèm `type_errors`.
- `classify`: khớp thì Upsert; không khớp thì Delete; có cờ `_PEERDB_IS_DELETED` thì Delete; `where` xét được cột bị exclude; cột cờ bị exclude mà delete vẫn đúng.

Xác minh bằng `cargo test` và `cargo build --release`.

### Checklist test thủ công (NATS + Postgres)
1. Chạy cdcsink với `SYNC_CONFIG_PATH=sync_config.example.yaml`.
2. Dùng `nats pub` đẩy message Debezium mẫu vào subject `debezium.*`: một dòng khớp `where`, một dòng không khớp, một update làm dòng đang khớp thành không khớp, và một message có `_PEERDB_IS_DELETED=true`.
3. Kiểm tra: bảng đích chỉ có cột theo config; dòng không khớp không có; dòng chuyển sang không khớp bị xóa; log có danh sách table đã cấu hình.
4. Chạy lại với config sai cú pháp: ứng dụng phải dừng và liệt kê đủ lỗi.

## 7. Ngoài phạm vi

- Delete theo Debezium `op = "d"` (hiện đang upsert lại dòng cũ).
- Message không có `id` không bao giờ được ack.
- Đổi config không tác động lên dữ liệu đã có ở đích. Ví dụ thêm `where` sau khi đã sync thì các dòng cũ không khớp vẫn còn cho đến khi được update; muốn dọn thì phải resync.
- Reload config khi đang chạy; biểu thức OR hoặc điều kiện lồng nhau.
