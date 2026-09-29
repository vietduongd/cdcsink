# Table Column & Row Filter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cho phép cấu hình theo từng table (file YAML qua `SYNC_CONFIG_PATH`) các cột được sync (`include`/`exclude`) và bộ lọc dòng (`where`, AND); dòng không khớp bị xóa ở đích.

**Architecture:** Module thuần `src/models/sync_config.rs` lo parse/validate config, chọn cột, xét điều kiện và phân loại message thành `RowAction::{Upsert, Delete}`. `receive_messages` gọi module này cho từng message (sau khi bỏ `_resync`), `main.rs` bỏ DDL cho message Delete, `insert_value` phân nhóm theo `action`. Hàm giải mã decimal base64 được tách ra `src/models/decimal.rs` để bộ lọc dùng lại.

**Tech Stack:** Rust 2024, tokio, async-nats, sqlx (Postgres), serde/serde_json, thêm `serde_yaml_ng = "0.10"`.

**Spec:** `docs/superpowers/specs/2026-09-29-table-column-row-filter-design.md`

## Global Constraints

- Không đặt `SYNC_CONFIG_PATH` (hoặc để rỗng) → hành vi y như hiện tại + log `SYNC_CONFIG_PATH not set: syncing ALL tables/columns/rows (no filter)`.
- Có `SYNC_CONFIG_PATH` nhưng file thiếu/sai → dừng khi khởi động, liệt kê tất cả lỗi sau dòng `Invalid sync config:`.
- Config chỉ nạp một lần khi khởi động.
- Cột `id` luôn được giữ.
- Tên table/cột: khớp chính xác trước, sau đó không phân biệt hoa thường. Giá trị trong `where`: so chính xác.
- Tên table được so sau khi bỏ hậu tố `_resync`.
- NULL/thiếu cột: mọi toán tử trừ `is_null` → false. Lỗi kiểu → false + ghi nhận type error.
- Message Delete không kích hoạt DDL; table chưa có ở đích → bỏ qua, vẫn ack.
- Lệnh chạy từ thư mục `cdcsink/`. Mỗi commit kết thúc bằng dòng `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. `SYNC_CONFIG_PATH=` (chuỗi rỗng) trong `.env` → phải coi như không đặt, không crash vì "cannot read file ''" — test ở Task 5.
2. Gõ sai key trong YAML (`exlude`, `wher`, `colum`) → phải báo lỗi khi khởi động, không âm thầm bỏ qua (nếu bỏ qua, cột nhạy cảm bị sync) — test ở Task 2.
3. `value: "5"` (chuỗi) so với cột số → không được âm thầm coi là khớp/không khớp; phải trả về type error để được log — test ở Task 4.
4. Cột decimal Debezium (`{"scale":2,"value":"AeI="}`) so với số trong `where` (`gt: 4`) → phải so theo giá trị số — test ở Task 4.
5. `include` không liệt kê `id`, và message đến từ table `X_resync` → `id` vẫn được giữ, config của `X` vẫn áp dụng — test ở Task 3 và Task 6.

---

## File Structure

| File | Trách nhiệm |
|---|---|
| Create `src/models/decimal.rs` | Giải mã decimal base64 của Debezium; lấy giá trị số từ `serde_json::Value` |
| Create `src/models/sync_config.rs` | Parse/validate YAML, tra table, chọn cột, xét `where`, `classify` |
| Modify `src/models/mod.rs` | Khai báo module, re-export |
| Modify `src/models/nats_receive.rs` | `normalize_table_name`, gọi `classify` + `retain_columns`, trường `action`, log lỗi kiểu |
| Modify `src/models/postgres_destination.rs` | Dùng `decode_base64_decimal`; phân nhóm theo `action`; quote schema |
| Modify `src/main.rs` | Nạp config; bỏ strip `_resync`; bỏ DDL cho Delete |
| Modify `Cargo.toml`, `.env.example`; Create `sync_config.example.yaml` | Dependency, cấu hình mẫu |

---

### Task 1: Tách helper decimal

**Files:**
- Create: `src/models/decimal.rs`
- Modify: `src/models/mod.rs`
- Modify: `src/models/postgres_destination.rs` (xóa `convert_base64_to_decimal`, import base64; đổi chỗ gọi)

**Interfaces:**
- Produces: `crate::models::decimal::decode_base64_decimal(base64_value: &str, scale: i32) -> Option<f64>` (base64 sai → `None`, không panic); `crate::models::decimal::numeric_value(v: &serde_json::Value) -> Option<f64>` (số JSON hoặc object `{scale, value}`).

- [ ] **Step 1: Viết test trước** — tạo `src/models/decimal.rs`:

```rust
use base64::{Engine, engine::general_purpose};
use serde_json::Value;

use crate::models::models_info::DecimalModel;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn decodes_positive_decimal() {
        // 0x01E2 = 482, scale 2
        assert_eq!(decode_base64_decimal("AeI=", 2), Some(4.82));
    }

    #[test]
    fn decodes_negative_decimal() {
        // 0xFE1E = -482 (two's complement), scale 2
        assert_eq!(decode_base64_decimal("/h4=", 2), Some(-4.82));
    }

    #[test]
    fn empty_bytes_is_zero() {
        assert_eq!(decode_base64_decimal("", 3), Some(0.0));
    }

    #[test]
    fn invalid_base64_is_none() {
        assert_eq!(decode_base64_decimal("!!!", 2), None);
    }

    #[test]
    fn numeric_value_reads_json_numbers() {
        assert_eq!(numeric_value(&json!(5)), Some(5.0));
        assert_eq!(numeric_value(&json!(-1.5)), Some(-1.5));
    }

    #[test]
    fn numeric_value_reads_debezium_decimal_object() {
        assert_eq!(numeric_value(&json!({"scale": 2, "value": "AeI="})), Some(4.82));
    }

    #[test]
    fn numeric_value_rejects_non_numbers() {
        assert_eq!(numeric_value(&json!("5")), None);
        assert_eq!(numeric_value(&json!(true)), None);
        assert_eq!(numeric_value(&json!({"a": 1})), None);
        assert_eq!(numeric_value(&Value::Null), None);
    }
}
```

Thêm vào `src/models/mod.rs` (sau `mod data_record;`):

```rust
mod decimal;
```

- [ ] **Step 2: Chạy test, xác nhận FAIL**

Run: `cargo test decimal`
Expected: lỗi biên dịch `cannot find function decode_base64_decimal` / `numeric_value`.

- [ ] **Step 3: Viết code** — thêm vào `src/models/decimal.rs`, giữa phần `use` và `#[cfg(test)]`:

```rust
/// Giải mã decimal Debezium: big-endian two's complement dạng base64, chia 10^scale.
/// Base64 sai trả về `None`.
pub fn decode_base64_decimal(base64_value: &str, scale: i32) -> Option<f64> {
    let bytes = general_purpose::STANDARD.decode(base64_value).ok()?;

    if bytes.is_empty() {
        return Some(0.0);
    }

    let num_bytes = bytes.len();
    let mut raw: i64 = 0;
    for b in &bytes {
        raw = (raw << 8) | (*b as i64);
    }

    // Sign-extend: nếu bit cao nhất của byte đầu là 1 thì đây là số âm
    if bytes[0] & 0x80 != 0 {
        let bits = num_bytes * 8;
        if bits < 64 {
            raw |= !((1i64 << bits) - 1);
        }
    }

    Some(raw as f64 / 10_f64.powi(scale))
}

/// Giá trị số của một ô dữ liệu: số JSON hoặc decimal Debezium `{scale, value}`.
pub fn numeric_value(v: &Value) -> Option<f64> {
    if let Some(n) = v.as_f64() {
        return Some(n);
    }
    if v.is_object() {
        if let Ok(decimal) = serde_json::from_value::<DecimalModel>(v.clone()) {
            return decode_base64_decimal(&decimal.value, decimal.scale);
        }
    }
    None
}
```

- [ ] **Step 4: Chuyển `postgres_destination.rs` sang dùng helper**

Trong `src/models/postgres_destination.rs`:
1. Xóa dòng `use base64::{Engine, engine::general_purpose};`.
2. Thêm import: `use crate::models::decimal::decode_base64_decimal;`
3. Xóa toàn bộ hàm `fn convert_base64_to_decimal(...)`.
4. Trong nhánh `"NUMERIC" | "DECIMAL"` của `insert_value`, thay:

```rust
                                if let Some(f64_value) = Self::convert_base64_to_decimal(
                                    &decimal_model.value,
                                    decimal_model.scale,
                                ) {
```
bằng:
```rust
                                if let Some(f64_value) = decode_base64_decimal(
                                    &decimal_model.value,
                                    decimal_model.scale,
                                ) {
```

- [ ] **Step 5: Chạy test và build**

Run: `cargo test decimal` → Expected: 7 test PASS.
Run: `cargo build` → Expected: build thành công (có thể có warning `numeric_value` never used — sẽ hết ở Task 4).

- [ ] **Step 6: Commit**

```bash
git add src/models/decimal.rs src/models/mod.rs src/models/postgres_destination.rs
git commit -m "refactor: extract Debezium decimal decoding into decimal module

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Parse và validate config

**Files:**
- Create: `src/models/sync_config.rs`
- Modify: `src/models/mod.rs`, `Cargo.toml`

**Interfaces:**
- Produces:
  - `pub enum Op { Eq, Ne, Gt, Gte, Lt, Lte, In, NotIn, IsNull, NotNull }` (`Debug, Clone, Copy, PartialEq, Eq, Hash`)
  - `pub struct Condition { pub column: String, pub op: Op, pub value: serde_json::Value }` (`Value::Null` cho `is_null`/`not_null`; `Value::Array` cho `in`/`not_in`)
  - `pub enum ColumnSelection { All, Include(Vec<String>), Exclude(Vec<String>) }`
  - `pub struct TableConfig { pub columns: ColumnSelection, pub conditions: Vec<Condition> }`
  - `pub struct SyncConfig` với `SyncConfig::load(path: &str) -> Result<SyncConfig, String>`, `SyncConfig::from_yaml_str(content: &str) -> Result<SyncConfig, String>`, `SyncConfig::table(&self, name: &str) -> Option<&TableConfig>`, `SyncConfig::table_names(&self) -> Vec<String>` (đã sort).

- [ ] **Step 1: Thêm dependency** — trong `Cargo.toml`, mục `[dependencies]`, thêm (theo thứ tự chữ cái, sau `serde_json`):

```toml
serde_yaml_ng = "0.10"
```

- [ ] **Step 2: Viết test trước** — tạo `src/models/sync_config.rs`:

```rust
use std::collections::{BTreeMap, HashMap};

use serde::Deserialize;
use serde_json::Value;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn config(yaml: &str) -> SyncConfig {
        SyncConfig::from_yaml_str(yaml).expect("config should be valid")
    }

    fn config_error(yaml: &str) -> String {
        SyncConfig::from_yaml_str(yaml).expect_err("config should be invalid")
    }

    #[test]
    fn parses_include_exclude_and_where() {
        let cfg = config(
            r#"
tables:
  orders:
    include: [id, total, status]
    where:
      - { column: tenant_id, op: eq, value: 5 }
      - { column: status, op: in, value: [paid, shipped] }
      - { column: deleted_at, op: is_null }
  users:
    exclude: [password_hash]
  logs:
"#,
        );
        let orders = cfg.table("orders").unwrap();
        assert_eq!(
            orders.columns,
            ColumnSelection::Include(vec!["id".into(), "total".into(), "status".into()])
        );
        assert_eq!(orders.conditions.len(), 3);
        assert_eq!(orders.conditions[0].op, Op::Eq);
        assert_eq!(orders.conditions[0].value, json!(5));
        assert_eq!(orders.conditions[1].value, json!(["paid", "shipped"]));
        assert_eq!(orders.conditions[2].op, Op::IsNull);
        assert_eq!(orders.conditions[2].value, Value::Null);

        let users = cfg.table("users").unwrap();
        assert_eq!(users.columns, ColumnSelection::Exclude(vec!["password_hash".into()]));
        assert!(users.conditions.is_empty());

        let logs = cfg.table("logs").unwrap();
        assert_eq!(logs.columns, ColumnSelection::All);

        assert_eq!(cfg.table_names(), vec!["logs", "orders", "users"]);
    }

    #[test]
    fn table_lookup_exact_then_case_insensitive() {
        let cfg = config("tables:\n  OrderItems:\n    exclude: [secret]\n");
        assert!(cfg.table("OrderItems").is_some());
        assert!(cfg.table("orderitems").is_some());
        assert!(cfg.table("ORDERITEMS").is_some());
        assert!(cfg.table("order_items").is_none());
        assert!(cfg.table("users").is_none());
    }

    #[test]
    fn collects_all_errors_at_once() {
        let err = config_error(
            r#"
tables:
  Orders:
    exclude: [a]
  orders:
    include: [id]
    exclude: [x]
    where:
      - { column: status, op: in, value: paid }
      - { column: status, op: like, value: "p%" }
  users:
    include: []
    where:
      - { column: deleted_at, op: is_null, value: 1 }
      - { column: tenant_id, op: eq }
      - { column: tenant_id, op: gt, value: [1, 2] }
"#,
        );
        assert!(err.starts_with("Invalid sync config:"), "{err}");
        assert!(err.contains("tables: 'Orders' and 'orders' collide (case-insensitive)"), "{err}");
        assert!(err.contains("tables.users: include must not be empty"), "{err}");
        assert!(err.contains("tables.users.where[0]: op 'is_null' must not have a value"), "{err}");
        assert!(err.contains("tables.users.where[1]: op 'eq' requires a scalar value"), "{err}");
        assert!(err.contains("tables.users.where[2]: op 'gt' requires a scalar value"), "{err}");
    }

    #[test]
    fn rejects_include_and_exclude_together() {
        let err = config_error("tables:\n  orders:\n    include: [id]\n    exclude: [x]\n");
        assert!(err.contains("tables.orders: cannot set both include and exclude"), "{err}");
    }

    #[test]
    fn rejects_bad_in_value_and_unknown_op() {
        let err = config_error(
            r#"
tables:
  orders:
    where:
      - { column: status, op: in, value: paid }
      - { column: status, op: not_in, value: [] }
      - { column: status, op: like, value: "p%" }
"#,
        );
        assert!(err.contains("tables.orders.where[0]: op 'in' requires a non-empty array of scalar values"), "{err}");
        assert!(err.contains("tables.orders.where[1]: op 'not_in' requires a non-empty array of scalar values"), "{err}");
        assert!(err.contains("tables.orders.where[2]: unknown op 'like'"), "{err}");
    }

    // Review Focus #2: gõ sai key không được âm thầm bỏ qua
    #[test]
    fn rejects_unknown_keys() {
        let err = config_error("tables:\n  users:\n    exlude: [password_hash]\n");
        assert!(err.contains("unknown field"), "{err}");
        let err = config_error("tables:\n  users:\n    wher:\n      - { column: a, op: eq, value: 1 }\n");
        assert!(err.contains("unknown field"), "{err}");
        let err = config_error("tables:\n  users:\n    where:\n      - { colum: a, op: eq, value: 1 }\n");
        assert!(err.contains("unknown field"), "{err}");
    }

    #[test]
    fn rejects_empty_file() {
        let err = config_error("   \n");
        assert!(err.contains("file is empty"), "{err}");
    }

    #[test]
    fn load_reports_missing_file() {
        let err = SyncConfig::load("does/not/exist.yaml").expect_err("missing file");
        assert!(err.contains("Cannot read sync config does/not/exist.yaml"), "{err}");
    }
}
```

Thêm vào `src/models/mod.rs` (sau `mod postgres_destination;`):

```rust
mod sync_config;
```

- [ ] **Step 3: Chạy test, xác nhận FAIL**

Run: `cargo test sync_config`
Expected: lỗi biên dịch `cannot find type SyncConfig` / `ColumnSelection` / `Op`.

- [ ] **Step 4: Viết code** — thêm vào `src/models/sync_config.rs`, giữa phần `use` và `#[cfg(test)]`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Op {
    Eq,
    Ne,
    Gt,
    Gte,
    Lt,
    Lte,
    In,
    NotIn,
    IsNull,
    NotNull,
}

impl Op {
    fn parse(s: &str) -> Option<Op> {
        match s {
            "eq" => Some(Op::Eq),
            "ne" => Some(Op::Ne),
            "gt" => Some(Op::Gt),
            "gte" => Some(Op::Gte),
            "lt" => Some(Op::Lt),
            "lte" => Some(Op::Lte),
            "in" => Some(Op::In),
            "not_in" => Some(Op::NotIn),
            "is_null" => Some(Op::IsNull),
            "not_null" => Some(Op::NotNull),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Condition {
    pub column: String,
    pub op: Op,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ColumnSelection {
    All,
    Include(Vec<String>),
    Exclude(Vec<String>),
}

#[derive(Debug, Clone)]
pub struct TableConfig {
    pub columns: ColumnSelection,
    pub conditions: Vec<Condition>,
}

#[derive(Debug, Default)]
pub struct SyncConfig {
    tables: HashMap<String, TableConfig>,
    // tên viết thường -> tên như trong config
    lowercase_index: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    tables: Option<BTreeMap<String, Option<RawTable>>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTable {
    include: Option<Vec<String>>,
    exclude: Option<Vec<String>>,
    #[serde(rename = "where")]
    conditions: Option<Vec<RawCondition>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCondition {
    column: String,
    op: String,
    value: Option<Value>,
}

impl SyncConfig {
    pub fn load(path: &str) -> Result<SyncConfig, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Cannot read sync config {}: {}", path, e))?;
        Self::from_yaml_str(&content)
    }

    pub fn from_yaml_str(content: &str) -> Result<SyncConfig, String> {
        if content.trim().is_empty() {
            return Err("Invalid sync config:\n  - file is empty".to_string());
        }
        let raw: RawConfig = serde_yaml_ng::from_str(content)
            .map_err(|e| format!("Invalid sync config:\n  - {}", e))?;

        let mut errors = Vec::new();
        let mut config = SyncConfig::default();
        for (name, raw_table) in raw.tables.unwrap_or_default() {
            let lower = name.to_lowercase();
            if let Some(existing) = config.lowercase_index.get(&lower) {
                errors.push(format!(
                    "tables: '{}' and '{}' collide (case-insensitive)",
                    existing, name
                ));
                continue;
            }
            let table = parse_table(&name, raw_table.unwrap_or_default(), &mut errors);
            config.lowercase_index.insert(lower, name.clone());
            config.tables.insert(name, table);
        }

        if errors.is_empty() {
            Ok(config)
        } else {
            Err(format!("Invalid sync config:\n  - {}", errors.join("\n  - ")))
        }
    }

    /// Tra config của table: khớp chính xác trước, sau đó không phân biệt hoa thường.
    pub fn table(&self, name: &str) -> Option<&TableConfig> {
        if let Some(table) = self.tables.get(name) {
            return Some(table);
        }
        self.lowercase_index
            .get(&name.to_lowercase())
            .and_then(|key| self.tables.get(key))
    }

    pub fn table_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.tables.keys().cloned().collect();
        names.sort();
        names
    }
}

fn parse_table(name: &str, raw: RawTable, errors: &mut Vec<String>) -> TableConfig {
    let columns = match (raw.include, raw.exclude) {
        (Some(_), Some(_)) => {
            errors.push(format!("tables.{}: cannot set both include and exclude", name));
            ColumnSelection::All
        }
        (Some(cols), None) if cols.is_empty() => {
            errors.push(format!("tables.{}: include must not be empty", name));
            ColumnSelection::All
        }
        (Some(cols), None) => ColumnSelection::Include(cols),
        (None, Some(cols)) => ColumnSelection::Exclude(cols),
        (None, None) => ColumnSelection::All,
    };

    let mut conditions = Vec::new();
    for (i, raw_condition) in raw.conditions.unwrap_or_default().into_iter().enumerate() {
        let path = format!("tables.{}.where[{}]", name, i);
        let Some(op) = Op::parse(&raw_condition.op) else {
            errors.push(format!("{}: unknown op '{}'", path, raw_condition.op));
            continue;
        };
        let value = raw_condition.value.unwrap_or(Value::Null);
        let problem = match op {
            Op::IsNull | Op::NotNull => (!value.is_null())
                .then(|| format!("op '{}' must not have a value", raw_condition.op)),
            Op::In | Op::NotIn => match &value {
                Value::Array(items) if !items.is_empty() && items.iter().all(is_scalar) => None,
                _ => Some(format!(
                    "op '{}' requires a non-empty array of scalar values",
                    raw_condition.op
                )),
            },
            _ => (!is_scalar(&value))
                .then(|| format!("op '{}' requires a scalar value", raw_condition.op)),
        };
        if let Some(problem) = problem {
            errors.push(format!("{}: {}", path, problem));
            continue;
        }
        conditions.push(Condition {
            column: raw_condition.column,
            op,
            value,
        });
    }

    TableConfig {
        columns,
        conditions,
    }
}

fn is_scalar(v: &Value) -> bool {
    matches!(v, Value::Number(_) | Value::String(_) | Value::Bool(_))
}
```

- [ ] **Step 5: Chạy test**

Run: `cargo test sync_config`
Expected: 8 test PASS. Nếu `rejects_unknown_keys` hoặc `rejects_empty_file` fail vì format thông báo của `serde_yaml_ng` khác, chỉnh assertion theo thông báo thật **nhưng giữ nguyên ý**: phải là lỗi, không được parse thành công.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/models/sync_config.rs src/models/mod.rs
git commit -m "feat: parse and validate per-table sync config (YAML)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Chọn cột (`retain_columns`)

**Files:**
- Modify: `src/models/sync_config.rs`

**Interfaces:**
- Consumes: `TableConfig`, `ColumnSelection` (Task 2); `crate::models::DataModel { value: Value, data_type: String, nullable: bool, simple_type: String }`.
- Produces: `pub const ID_COLUMN: &str = "id";`, `TableConfig::retain_columns(&self, row: &mut HashMap<String, DataModel>)`, hàm nội bộ `fn resolve_key<'a, V>(map: &'a HashMap<String, V>, name: &str) -> Option<&'a String>` (Task 4 dùng lại).

- [ ] **Step 1: Viết test trước** — thêm vào `mod tests` trong `src/models/sync_config.rs`:

```rust
    fn row(pairs: &[(&str, Value)]) -> HashMap<String, DataModel> {
        pairs
            .iter()
            .map(|(k, v)| {
                (
                    k.to_string(),
                    DataModel {
                        value: v.clone(),
                        data_type: "TEXT".to_string(),
                        nullable: true,
                        simple_type: "TEXT".to_string(),
                    },
                )
            })
            .collect()
    }

    fn sorted_keys(row: &HashMap<String, DataModel>) -> Vec<String> {
        let mut keys: Vec<String> = row.keys().cloned().collect();
        keys.sort();
        keys
    }

    // Review Focus #5: include không có id thì id vẫn được giữ
    #[test]
    fn include_keeps_listed_columns_and_always_id() {
        let cfg = config("tables:\n  orders:\n    include: [total, status]\n");
        let mut r = row(&[("id", json!(1)), ("total", json!(10)), ("status", json!("paid")), ("secret", json!("x"))]);
        cfg.table("orders").unwrap().retain_columns(&mut r);
        assert_eq!(sorted_keys(&r), vec!["id", "status", "total"]);
    }

    #[test]
    fn exclude_drops_listed_columns_but_never_id() {
        let cfg = config("tables:\n  users:\n    exclude: [password_hash, id]\n");
        let mut r = row(&[("id", json!(1)), ("name", json!("a")), ("password_hash", json!("h"))]);
        cfg.table("users").unwrap().retain_columns(&mut r);
        assert_eq!(sorted_keys(&r), vec!["id", "name"]);
    }

    #[test]
    fn column_names_match_case_insensitively() {
        let cfg = config("tables:\n  UserAccounts:\n    exclude: [passwordhash]\n");
        let mut r = row(&[("id", json!(1)), ("UserName", json!("a")), ("PasswordHash", json!("h"))]);
        cfg.table("UserAccounts").unwrap().retain_columns(&mut r);
        assert_eq!(sorted_keys(&r), vec!["UserName", "id"]);
    }

    #[test]
    fn exact_column_match_wins_over_case_insensitive() {
        let cfg = config("tables:\n  t:\n    include: [Status]\n");
        let mut r = row(&[("id", json!(1)), ("Status", json!("a")), ("status", json!("b"))]);
        cfg.table("t").unwrap().retain_columns(&mut r);
        assert_eq!(sorted_keys(&r), vec!["Status", "id"]);
    }

    #[test]
    fn all_selection_keeps_everything() {
        let cfg = config("tables:\n  logs:\n");
        let mut r = row(&[("id", json!(1)), ("msg", json!("x"))]);
        cfg.table("logs").unwrap().retain_columns(&mut r);
        assert_eq!(sorted_keys(&r), vec!["id", "msg"]);
    }
```

- [ ] **Step 2: Chạy test, xác nhận FAIL**

Run: `cargo test sync_config`
Expected: lỗi biên dịch `cannot find type DataModel` / `no method named retain_columns`.

- [ ] **Step 3: Viết code**

Sửa phần `use` đầu file `src/models/sync_config.rs` thành:

```rust
use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Deserialize;
use serde_json::Value;

use crate::models::DataModel;

pub const ID_COLUMN: &str = "id";
```

Thêm sau `fn is_scalar`:

```rust
impl TableConfig {
    /// Giữ lại các cột theo include/exclude. Cột `id` luôn được giữ.
    pub fn retain_columns(&self, row: &mut HashMap<String, DataModel>) {
        let names = match &self.columns {
            ColumnSelection::All => return,
            ColumnSelection::Include(names) | ColumnSelection::Exclude(names) => names,
        };
        let selected: HashSet<String> = names
            .iter()
            .filter_map(|name| resolve_key(row, name).cloned())
            .collect();
        let keep_selected = matches!(self.columns, ColumnSelection::Include(_));
        row.retain(|key, _| key == ID_COLUMN || selected.contains(key) == keep_selected);
    }
}

/// Tìm key trong map khớp `name`: chính xác trước, sau đó không phân biệt hoa thường.
fn resolve_key<'a, V>(map: &'a HashMap<String, V>, name: &str) -> Option<&'a String> {
    if let Some((key, _)) = map.get_key_value(name) {
        return Some(key);
    }
    let lower = name.to_lowercase();
    map.keys().find(|key| key.to_lowercase() == lower)
}
```

- [ ] **Step 4: Chạy test**

Run: `cargo test sync_config`
Expected: 13 test PASS.

- [ ] **Step 5: Commit**

```bash
git add src/models/sync_config.rs
git commit -m "feat: per-table column selection with id always kept

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Xét điều kiện `where` (`matches`)

**Files:**
- Modify: `src/models/sync_config.rs`

**Interfaces:**
- Consumes: `Condition`, `Op`, `resolve_key` (Task 2-3); `crate::models::decimal::numeric_value` (Task 1).
- Produces: `pub struct TypeError { pub column: String, pub op: Op, pub detail: String }`, `pub struct MatchOutcome { pub matched: bool, pub type_errors: Vec<TypeError> }` (cả hai `Debug, Clone, PartialEq`), `TableConfig::matches(&self, row: &HashMap<String, DataModel>) -> MatchOutcome`.

- [ ] **Step 1: Viết test trước** — thêm vào `mod tests`:

```rust
    fn table_with_where(conditions_yaml: &str) -> TableConfig {
        let yaml = format!("tables:\n  t:\n    where:\n{}", conditions_yaml);
        config(&yaml).table("t").unwrap().clone()
    }

    fn check(conditions_yaml: &str, pairs: &[(&str, Value)]) -> MatchOutcome {
        table_with_where(conditions_yaml).matches(&row(pairs))
    }

    #[test]
    fn no_conditions_always_matches() {
        let cfg = config("tables:\n  t:\n    exclude: [x]\n");
        let outcome = cfg.table("t").unwrap().matches(&row(&[("id", json!(1))]));
        assert_eq!(outcome, MatchOutcome { matched: true, type_errors: vec![] });
    }

    #[test]
    fn numeric_comparisons() {
        let r = [("n", json!(5))];
        assert!(check("      - { column: n, op: eq, value: 5 }\n", &r).matched);
        assert!(check("      - { column: n, op: eq, value: 5.0 }\n", &r).matched);
        assert!(!check("      - { column: n, op: ne, value: 5 }\n", &r).matched);
        assert!(check("      - { column: n, op: gt, value: 4 }\n", &r).matched);
        assert!(!check("      - { column: n, op: gt, value: 5 }\n", &r).matched);
        assert!(check("      - { column: n, op: gte, value: 5 }\n", &r).matched);
        assert!(check("      - { column: n, op: lt, value: 6 }\n", &r).matched);
        assert!(!check("      - { column: n, op: lt, value: 5 }\n", &r).matched);
        assert!(check("      - { column: n, op: lte, value: 5 }\n", &r).matched);
    }

    #[test]
    fn string_comparisons_including_dates() {
        let r = [("status", json!("paid")), ("created_at", json!("2025-03-01 10:00:00"))];
        assert!(check("      - { column: status, op: eq, value: paid }\n", &r).matched);
        // giá trị so chính xác, phân biệt hoa thường
        assert!(!check("      - { column: status, op: eq, value: Paid }\n", &r).matched);
        assert!(check("      - { column: status, op: in, value: [paid, shipped] }\n", &r).matched);
        assert!(!check("      - { column: status, op: not_in, value: [paid, shipped] }\n", &r).matched);
        assert!(check("      - { column: created_at, op: gte, value: \"2025-01-01\" }\n", &r).matched);
        assert!(!check("      - { column: created_at, op: lt, value: \"2025-01-01\" }\n", &r).matched);
    }

    #[test]
    fn boolean_comparisons() {
        let r = [("active", json!(true))];
        assert!(check("      - { column: active, op: eq, value: true }\n", &r).matched);
        assert!(check("      - { column: active, op: ne, value: false }\n", &r).matched);
        assert!(check("      - { column: active, op: in, value: [true] }\n", &r).matched);
        let outcome = check("      - { column: active, op: gt, value: false }\n", &r);
        assert!(!outcome.matched);
        assert_eq!(outcome.type_errors.len(), 1);
    }

    // Review Focus #4: decimal Debezium so với số
    #[test]
    fn debezium_decimal_compares_numerically() {
        let r = [("price", json!({"scale": 2, "value": "AeI="}))]; // 4.82
        assert!(check("      - { column: price, op: gt, value: 4 }\n", &r).matched);
        assert!(check("      - { column: price, op: eq, value: 4.82 }\n", &r).matched);
        let neg = [("price", json!({"scale": 2, "value": "/h4="}))]; // -4.82
        assert!(check("      - { column: price, op: lt, value: 0 }\n", &neg).matched);
    }

    #[test]
    fn null_and_missing_columns() {
        let null_row = [("deleted_at", Value::Null)];
        assert!(check("      - { column: deleted_at, op: is_null }\n", &null_row).matched);
        assert!(!check("      - { column: deleted_at, op: not_null }\n", &null_row).matched);
        assert!(!check("      - { column: deleted_at, op: eq, value: x }\n", &null_row).matched);
        assert!(!check("      - { column: deleted_at, op: ne, value: x }\n", &null_row).matched);
        assert!(!check("      - { column: deleted_at, op: not_in, value: [x] }\n", &null_row).matched);

        let missing: [(&str, Value); 0] = [];
        assert!(check("      - { column: deleted_at, op: is_null }\n", &missing).matched);
        assert!(!check("      - { column: deleted_at, op: not_null }\n", &missing).matched);
        let outcome = check("      - { column: deleted_at, op: eq, value: x }\n", &missing);
        assert_eq!(outcome, MatchOutcome { matched: false, type_errors: vec![] });
    }

    // Review Focus #3: "5" (chuỗi) so với cột số phải báo type error
    #[test]
    fn type_mismatch_is_false_and_reported() {
        let outcome = check("      - { column: tenant_id, op: eq, value: \"5\" }\n", &[("tenant_id", json!(5))]);
        assert!(!outcome.matched);
        assert_eq!(outcome.type_errors.len(), 1);
        assert_eq!(outcome.type_errors[0].column, "tenant_id");
        assert_eq!(outcome.type_errors[0].op, Op::Eq);
        assert!(outcome.type_errors[0].detail.contains("number"), "{:?}", outcome.type_errors[0]);

        let outcome = check("      - { column: name, op: gt, value: 3 }\n", &[("name", json!("abc"))]);
        assert!(!outcome.matched);
        assert_eq!(outcome.type_errors.len(), 1);
    }

    #[test]
    fn all_conditions_must_hold() {
        let conds = "      - { column: tenant_id, op: eq, value: 5 }\n      - { column: status, op: in, value: [paid] }\n";
        assert!(check(conds, &[("tenant_id", json!(5)), ("status", json!("paid"))]).matched);
        assert!(!check(conds, &[("tenant_id", json!(5)), ("status", json!("new"))]).matched);
        assert!(!check(conds, &[("tenant_id", json!(6)), ("status", json!("paid"))]).matched);
    }

    #[test]
    fn where_column_names_match_case_insensitively() {
        assert!(check("      - { column: tenantid, op: eq, value: 5 }\n", &[("TenantId", json!(5))]).matched);
    }
```

- [ ] **Step 2: Chạy test, xác nhận FAIL**

Run: `cargo test sync_config`
Expected: lỗi biên dịch `cannot find type MatchOutcome` / `no method named matches`.

- [ ] **Step 3: Viết code**

Thêm vào phần `use` đầu file:

```rust
use std::cmp::Ordering;

use crate::models::decimal::numeric_value;
```

Thêm sau `fn is_scalar` (trước `impl TableConfig`):

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct TypeError {
    pub column: String,
    pub op: Op,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchOutcome {
    pub matched: bool,
    pub type_errors: Vec<TypeError>,
}
```

Thêm vào trong `impl TableConfig` (sau `retain_columns`):

```rust
    /// Xét toàn bộ điều kiện `where` (AND). Không short-circuit để thu đủ lỗi kiểu.
    pub fn matches(&self, row: &HashMap<String, DataModel>) -> MatchOutcome {
        let mut matched = true;
        let mut type_errors = Vec::new();
        for condition in &self.conditions {
            let actual = resolve_key(row, &condition.column)
                .and_then(|key| row.get(key))
                .map(|data| &data.value);
            match eval_condition(condition, actual) {
                Ok(true) => {}
                Ok(false) => matched = false,
                Err(detail) => {
                    matched = false;
                    type_errors.push(TypeError {
                        column: condition.column.clone(),
                        op: condition.op,
                        detail,
                    });
                }
            }
        }
        MatchOutcome {
            matched,
            type_errors,
        }
    }
```

Thêm sau `fn resolve_key`:

```rust
fn eval_condition(condition: &Condition, actual: Option<&Value>) -> Result<bool, String> {
    // NULL hoặc thiếu cột: chỉ is_null đúng
    let actual = match actual {
        None | Some(Value::Null) => return Ok(condition.op == Op::IsNull),
        Some(value) => value,
    };
    match condition.op {
        Op::IsNull => Ok(false),
        Op::NotNull => Ok(true),
        Op::Eq => values_equal(actual, &condition.value),
        Op::Ne => values_equal(actual, &condition.value).map(|equal| !equal),
        Op::In | Op::NotIn => {
            let items = condition.value.as_array().map(Vec::as_slice).unwrap_or(&[]);
            let mut found = false;
            let mut comparable = false;
            let mut last_error = None;
            for item in items {
                match values_equal(actual, item) {
                    Ok(equal) => {
                        comparable = true;
                        found |= equal;
                    }
                    Err(e) => last_error = Some(e),
                }
            }
            if !comparable {
                return Err(last_error.unwrap_or_else(|| "empty value list".to_string()));
            }
            Ok(if condition.op == Op::In { found } else { !found })
        }
        Op::Gt | Op::Gte | Op::Lt | Op::Lte => {
            let ordering = compare_order(actual, &condition.value)?;
            Ok(match condition.op {
                Op::Gt => ordering == Ordering::Greater,
                Op::Gte => ordering != Ordering::Less,
                Op::Lt => ordering == Ordering::Less,
                _ => ordering != Ordering::Greater,
            })
        }
    }
}

fn values_equal(actual: &Value, expected: &Value) -> Result<bool, String> {
    if let (Some(a), Some(b)) = (numeric_value(actual), numeric_value(expected)) {
        return Ok(a == b);
    }
    match (actual, expected) {
        (Value::String(a), Value::String(b)) => Ok(a == b),
        (Value::Bool(a), Value::Bool(b)) => Ok(a == b),
        _ => Err(format!(
            "cannot compare {} with {}",
            value_kind(actual),
            value_kind(expected)
        )),
    }
}

fn compare_order(actual: &Value, expected: &Value) -> Result<Ordering, String> {
    if let (Some(a), Some(b)) = (numeric_value(actual), numeric_value(expected)) {
        return a
            .partial_cmp(&b)
            .ok_or_else(|| "cannot order NaN".to_string());
    }
    match (actual, expected) {
        (Value::String(a), Value::String(b)) => Ok(a.cmp(b)),
        _ => Err(format!(
            "cannot order {} against {}",
            value_kind(actual),
            value_kind(expected)
        )),
    }
}

fn value_kind(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) if numeric_value(v).is_some() => "decimal",
        Value::Object(_) => "object",
    }
}
```

- [ ] **Step 4: Chạy test**

Run: `cargo test sync_config`
Expected: 22 test PASS.

- [ ] **Step 5: Commit**

```bash
git add src/models/sync_config.rs
git commit -m "feat: evaluate per-table where conditions with type-error reporting

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: `classify` và đọc `SYNC_CONFIG_PATH`

**Files:**
- Modify: `src/models/sync_config.rs`

**Interfaces:**
- Consumes: `TableConfig::matches`, `MatchOutcome`, `SyncConfig::load` (Task 2-4).
- Produces: `pub const DELETED_FLAG_COLUMN: &str = "_PEERDB_IS_DELETED";`, `pub enum RowAction { Upsert, Delete }` (`Debug, Clone, Copy, PartialEq, Eq`), `pub fn classify(row: &HashMap<String, DataModel>, table: Option<&TableConfig>) -> (RowAction, MatchOutcome)`, `SyncConfig::from_env_value(path: Option<String>) -> Result<Option<SyncConfig>, String>`.

- [ ] **Step 1: Viết test trước** — thêm vào `mod tests`:

```rust
    #[test]
    fn classify_without_config_upserts() {
        let (action, outcome) = classify(&row(&[("id", json!(1))]), None);
        assert_eq!(action, RowAction::Upsert);
        assert!(outcome.matched);
    }

    #[test]
    fn classify_peerdb_deleted_flag_deletes() {
        let r = row(&[("id", json!(1)), ("_PEERDB_IS_DELETED", json!(true))]);
        assert_eq!(classify(&r, None).0, RowAction::Delete);
        let r = row(&[("id", json!(1)), ("_PEERDB_IS_DELETED", json!(false))]);
        assert_eq!(classify(&r, None).0, RowAction::Upsert);
    }

    #[test]
    fn classify_by_where() {
        let t = table_with_where("      - { column: status, op: eq, value: paid }\n");
        assert_eq!(classify(&row(&[("id", json!(1)), ("status", json!("paid"))]), Some(&t)).0, RowAction::Upsert);
        assert_eq!(classify(&row(&[("id", json!(1)), ("status", json!("new"))]), Some(&t)).0, RowAction::Delete);
    }

    #[test]
    fn where_can_use_excluded_column_and_delete_survives_excluded_flag() {
        let cfg = config(
            r#"
tables:
  orders:
    exclude: [tenant_id, _PEERDB_IS_DELETED]
    where:
      - { column: tenant_id, op: eq, value: 5 }
"#,
        );
        let t = cfg.table("orders").unwrap();

        // Thứ tự đúng như nats_receive: classify trước, retain_columns sau
        let mut r = row(&[("id", json!(1)), ("tenant_id", json!(5)), ("_PEERDB_IS_DELETED", json!(false))]);
        let (action, _) = classify(&r, Some(t));
        t.retain_columns(&mut r);
        assert_eq!(action, RowAction::Upsert);
        assert_eq!(sorted_keys(&r), vec!["id"]);

        let mut r = row(&[("id", json!(2)), ("tenant_id", json!(5)), ("_PEERDB_IS_DELETED", json!(true))]);
        let (action, _) = classify(&r, Some(t));
        t.retain_columns(&mut r);
        assert_eq!(action, RowAction::Delete);
    }

    // Review Focus #1: SYNC_CONFIG_PATH rỗng coi như không đặt
    #[test]
    fn env_value_absent_or_blank_means_no_config() {
        assert!(SyncConfig::from_env_value(None).unwrap().is_none());
        assert!(SyncConfig::from_env_value(Some(String::new())).unwrap().is_none());
        assert!(SyncConfig::from_env_value(Some("   ".into())).unwrap().is_none());
    }

    #[test]
    fn env_value_with_missing_file_is_error() {
        let err = SyncConfig::from_env_value(Some("does/not/exist.yaml".into())).unwrap_err();
        assert!(err.contains("Cannot read sync config"), "{err}");
    }
```

- [ ] **Step 2: Chạy test, xác nhận FAIL**

Run: `cargo test sync_config`
Expected: lỗi biên dịch `cannot find function classify` / `RowAction` / `from_env_value`.

- [ ] **Step 3: Viết code**

Thêm sau `pub const ID_COLUMN`:

```rust
pub const DELETED_FLAG_COLUMN: &str = "_PEERDB_IS_DELETED";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowAction {
    Upsert,
    Delete,
}
```

Thêm vào `impl SyncConfig` (sau `load`):

```rust
    /// Giá trị của env `SYNC_CONFIG_PATH`: không có hoặc rỗng → không dùng config.
    pub fn from_env_value(path: Option<String>) -> Result<Option<SyncConfig>, String> {
        match path {
            Some(path) if !path.trim().is_empty() => Self::load(path.trim()).map(Some),
            _ => Ok(None),
        }
    }
```

Thêm sau `impl TableConfig { ... }`:

```rust
/// Quyết định Upsert/Delete cho một message. Phải gọi TRƯỚC `retain_columns`
/// để cờ delete và các cột bị loại vẫn dùng được trong `where`.
pub fn classify(
    row: &HashMap<String, DataModel>,
    table: Option<&TableConfig>,
) -> (RowAction, MatchOutcome) {
    let is_deleted = row
        .get(DELETED_FLAG_COLUMN)
        .and_then(|data| data.value.as_bool())
        .unwrap_or(false);
    let outcome = table.map(|t| t.matches(row)).unwrap_or(MatchOutcome {
        matched: true,
        type_errors: Vec::new(),
    });
    let action = if is_deleted || !outcome.matched {
        RowAction::Delete
    } else {
        RowAction::Upsert
    };
    (action, outcome)
}
```

- [ ] **Step 4: Chạy test**

Run: `cargo test sync_config`
Expected: 28 test PASS.

- [ ] **Step 5: Commit**

```bash
git add src/models/sync_config.rs
git commit -m "feat: classify messages into upsert/delete and read SYNC_CONFIG_PATH

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Nối vào luồng NATS → Postgres

**Files:**
- Modify: `src/models/mod.rs`
- Modify: `src/models/nats_receive.rs`
- Modify: `src/models/postgres_destination.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `SyncConfig`, `RowAction`, `classify`, `TableConfig::retain_columns` (Task 2-5).
- Produces: `NatMessageReceive.action: RowAction`; `pub fn normalize_table_name(name: &str) -> String` trong `nats_receive.rs`; `NatsReceive::receive_messages(&self, consumer: &mut PullConsumer, sync_config: Option<&SyncConfig>, logged_type_errors: &mut HashSet<String>) -> Result<Vec<NatMessageReceive>, String>`.

- [ ] **Step 1: Viết test trước** — thêm cuối `src/models/nats_receive.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Review Focus #5: table _resync dùng chung config/tên với table gốc
    #[test]
    fn strips_resync_suffix() {
        assert_eq!(normalize_table_name("orders_resync"), "orders");
        assert_eq!(normalize_table_name("OrderItems_resync"), "OrderItems");
        assert_eq!(normalize_table_name("orders"), "orders");
        assert_eq!(normalize_table_name("resync_log"), "resync_log");
    }
}
```

- [ ] **Step 2: Chạy test, xác nhận FAIL**

Run: `cargo test nats_receive`
Expected: lỗi biên dịch `cannot find function normalize_table_name`.

- [ ] **Step 3: Cập nhật `src/models/mod.rs`** — thay toàn bộ nội dung bằng:

```rust
mod data_record;
mod decimal;
mod models_info;
mod nats_receive;
mod postgres_destination;
mod sync_config;

pub use data_record::DataRecord;
pub use models_info::DataModel;
pub use nats_receive::{NatMessageReceive, NatsReceive};
pub use postgres_destination::PostgresDestination;
pub use sync_config::{RowAction, SyncConfig};
```

- [ ] **Step 4: Cập nhật `src/models/nats_receive.rs`**

Thay phần `use crate::models::{DataModel, DataRecord};` bằng:

```rust
use crate::models::{
    DataModel, DataRecord, RowAction, SyncConfig,
    sync_config::classify,
};
```

Thêm trường vào `NatMessageReceive` (sau `primary_key`):

```rust
    pub action: RowAction,
```

Thêm hàm tự do (sau `struct NatMessageReceive`):

```rust
/// Bỏ hậu tố `_resync` để message resync dùng chung table và config với table gốc.
pub fn normalize_table_name(name: &str) -> String {
    name.strip_suffix("_resync").unwrap_or(name).to_string()
}
```

Thay toàn bộ hàm `receive_messages` bằng:

```rust
    pub async fn receive_messages(
        &self,
        consumer: &mut PullConsumer,
        sync_config: Option<&SyncConfig>,
        logged_type_errors: &mut HashSet<String>,
    ) -> Result<Vec<NatMessageReceive>, String> {
        let mut messages = consumer
            .fetch()
            .max_messages(self.number_pull_object)
            .expires(Duration::from_secs(5)) // 👈 MaxWait
            .messages()
            .await
            .map_err(|e| format!("Failed to receive messages: {}", e))?;

        let mut received_messages: Vec<NatMessageReceive> = Vec::new();
        // table -> (tổng số message, số message bị loại vì lỗi kiểu)
        let mut type_rejections: HashMap<String, (usize, usize)> = HashMap::new();
        let mut counter = 0;
        while let Some(Ok(message)) = messages.next().await {
            let data_record: DataRecord = serde_json::from_slice(&message.payload)
                .map_err(|e| format!("Failed to deserialize message payload: {}", e))?;

            let table_name = normalize_table_name(
                &data_record
                    .get_table_name()
                    .ok_or("Failed to get table name from data record")?,
            );

            let mut table_value = data_record
                .get_table_structure()
                .ok_or("Failed to get table structure from data record")?;

            let primary_key = match table_value.get("id") {
                Some(op) => Some(op.value.to_string()),
                None => continue,
            };

            let table_config = sync_config.and_then(|config| config.table(&table_name));
            let (action, outcome) = classify(&table_value, table_config);

            for error in &outcome.type_errors {
                let key = format!("{}|{}|{:?}", table_name, error.column, error.op);
                if logged_type_errors.insert(key) {
                    eprintln!(
                        "Sync filter type mismatch: table {} column {} op {:?}: {}",
                        table_name, error.column, error.op, error.detail
                    );
                }
            }
            let stats = type_rejections.entry(table_name.clone()).or_insert((0, 0));
            stats.0 += 1;
            if !outcome.matched && !outcome.type_errors.is_empty() {
                stats.1 += 1;
            }

            if let Some(config) = table_config {
                config.retain_columns(&mut table_value);
            }

            received_messages.push(NatMessageReceive {
                message,
                table_name,
                table_value,
                index: counter,
                primary_key,
                action,
            });
            counter += 1;
        }

        for (table_name, (total, rejected)) in &type_rejections {
            if *total > 0 && total == rejected {
                eprintln!(
                    "WARNING: all {} rows of table {} rejected due to type mismatch in where",
                    total, table_name
                );
            }
        }

        Ok(received_messages)
    }
```

- [ ] **Step 5: Cập nhật `src/models/postgres_destination.rs`**

Sửa import:

```rust
use crate::models::{DataModel, NatMessageReceive, RowAction, models_info::DecimalModel};
```

Trong `insert_value`, thay khối partition:

```rust
        let (to_delete, to_upsert): (Vec<&NatMessageReceive>, Vec<&NatMessageReceive>) =
            columns.into_iter().partition(|msg| {
                msg.table_value
                    .get("_PEERDB_IS_DELETED")
                    .and_then(|dm| dm.value.as_bool())
                    .unwrap_or(false)
            });
```
bằng:
```rust
        let (to_delete, to_upsert): (Vec<&NatMessageReceive>, Vec<&NatMessageReceive>) =
            columns
                .into_iter()
                .partition(|msg| msg.action == RowAction::Delete);
```

Quote schema ở câu DELETE — thay:
```rust
                "DELETE FROM {}.{} WHERE \"id\" = ANY($1::{}[]);",
                self.schema_expect,
```
bằng:
```rust
                "DELETE FROM {}.{} WHERE \"id\" = ANY($1::{}[]);",
                Self::quote_identifier(&self.schema_expect),
```

Quote schema ở câu INSERT — thay:
```rust
                "{}.{} (",
                self.schema_expect.clone(),
```
bằng:
```rust
                "{}.{} (",
                Self::quote_identifier(&self.schema_expect),
```

- [ ] **Step 6: Cập nhật `src/main.rs`** — thay toàn bộ nội dung bằng:

```rust
use std::{
    collections::{HashMap, HashSet},
    env,
    error::Error,
};

use chrono::Local;
use dotenvy::dotenv;

use crate::models::{NatMessageReceive, PostgresDestination, RowAction, SyncConfig};

mod models;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Starting CDC Sink application...");

    dotenv().ok();

    println!("Loading environment variables...");
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

    let sync_config = match SyncConfig::from_env_value(env::var("SYNC_CONFIG_PATH").ok()) {
        Ok(Some(config)) => {
            println!(
                "Sync config loaded: tables [{}]",
                config.table_names().join(", ")
            );
            Some(config)
        }
        Ok(None) => {
            println!("SYNC_CONFIG_PATH not set: syncing ALL tables/columns/rows (no filter)");
            None
        }
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    println!("Configuration loaded successfully");
    println!("NATS URL: {}", nats_url);
    println!("Topic: {}", topic_name);
    println!("Stream: {}", nats_stream_name);
    println!("Consumer: {}", nats_consumer_name);
    println!("Schema: {}", database_schema_expected);

    let nats_info = models::NatsReceive::new(
        nats_url,
        nats_consumer_name,
        topic_name,
        nats_stream_name,
        number_pull_object,
    );

    let mut consumer = nats_info.connected().await?;

    let pg_destination = PostgresDestination::new(db_url, database_schema_expected.clone());
    let pg_pool = pg_destination
        .connect()
        .await
        .map_err(|e| Box::<dyn Error>::from(e))?;

    pg_destination
        .ensure_schema_metadata_table(&pg_pool)
        .await
        .map_err(|e| Box::<dyn Error>::from(e))?;

    let mut schema_cache = pg_destination
        .get_schema_info(&pg_pool)
        .await
        .map_err(|e| Box::<dyn Error>::from(e))?;
    let mut logged_type_errors: HashSet<String> = HashSet::new();
    loop {
        let messages = nats_info
            .receive_messages(&mut consumer, sync_config.as_ref(), &mut logged_type_errors)
            .await?;
        if messages.is_empty() {
            continue;
        }
        println!("Received {} messages at {}", messages.len(), Local::now());
        let mut message_active: HashMap<String, Vec<&NatMessageReceive>> = HashMap::new();
        for msg in &messages {
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
                    if !buffered.is_empty() {
                        pg_destination
                            .insert_value(table_name, &buffered, &pg_pool)
                            .await;
                    }
                }
                pg_destination
                    .create_table_if_not_exists_query(
                        &database_schema_expected.clone(),
                        table_name,
                        &msg.table_value,
                        &pg_pool,
                    )
                    .await;
                schema_cache.insert(
                    table_name.clone(),
                    msg.table_value.keys().cloned().collect(),
                );
            } else {
                // Table đã tồn tại: kiểm tra xem có column mới không
                let cached_columns = schema_cache.get_mut(table_name).unwrap();
                let new_columns: Vec<(&String, &crate::models::DataModel)> = msg
                    .table_value
                    .iter()
                    .filter(|(col_name, _)| !cached_columns.contains(*col_name))
                    .collect();

                if !new_columns.is_empty() {
                    // Có column mới: nếu message_active đang có dữ liệu thì insert trước
                    if let Some(buffered) = message_active.remove(table_name) {
                        if !buffered.is_empty() {
                            pg_destination
                                .insert_value(table_name, &buffered, &pg_pool)
                                .await;
                        }
                    }
                    // Tạo các column mới
                    for (col_name, col_type) in &new_columns {
                        pg_destination
                            .add_column_if_not_exists(
                                &database_schema_expected,
                                table_name,
                                col_name,
                                col_type,
                                &pg_pool,
                            )
                            .await;
                        cached_columns.insert(col_name.to_string());
                    }
                }
            }
            message_active
                .entry(table_name.clone())
                .or_insert(Vec::new())
                .push(msg);
        }
        for active_item in message_active {
            pg_destination
                .insert_value(&active_item.0, &active_item.1, &pg_pool)
                .await;
        }
        nats_info.ack_message(&messages).await?;
    }
}
```

- [ ] **Step 7: Chạy toàn bộ test và build**

Run: `cargo test`
Expected: tất cả test PASS (7 decimal + 28 sync_config + 1 nats_receive = 36).
Run: `cargo build`
Expected: build thành công, không có warning mới liên quan tới `sync_config`, `decimal`, `nats_receive` (warning cũ ở `data_record.rs` như `operation` never used vẫn còn — ngoài phạm vi).

- [ ] **Step 8: Commit**

```bash
git add src/main.rs src/models/mod.rs src/models/nats_receive.rs src/models/postgres_destination.rs
git commit -m "feat: apply per-table column selection and row filter in sync loop

- classify messages before column filtering; unmatched rows are deleted
- delete messages never trigger DDL
- quote destination schema in INSERT/DELETE

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: File cấu hình mẫu và xác minh cuối

**Files:**
- Create: `sync_config.example.yaml`
- Modify: `.env.example`
- Modify: `src/models/sync_config.rs` (test parse file mẫu)

**Interfaces:**
- Consumes: `SyncConfig::load` (Task 2).

- [ ] **Step 1: Viết test trước** — thêm vào `mod tests` trong `src/models/sync_config.rs`:

```rust
    #[test]
    fn example_config_file_is_valid() {
        let cfg = SyncConfig::load(concat!(env!("CARGO_MANIFEST_DIR"), "/sync_config.example.yaml"))
            .expect("example config must be valid");
        assert!(cfg.table("orders").is_some());
        assert!(cfg.table("OrderItems").is_some());
        assert!(cfg.table("UserAccounts").is_some());
    }
```

- [ ] **Step 2: Chạy test, xác nhận FAIL**

Run: `cargo test example_config_file_is_valid`
Expected: FAIL với `Cannot read sync config ...sync_config.example.yaml`.

- [ ] **Step 3: Tạo `sync_config.example.yaml`** ở thư mục gốc `cdcsink/`:

```yaml
# Cấu hình cột và bộ lọc dòng theo từng table.
# Đường dẫn file đặt qua env SYNC_CONFIG_PATH. File chỉ được nạp khi khởi động.
#
# - Table không khai báo ở đây: sync toàn bộ cột, toàn bộ dòng.
# - Key là tên table như payload.source.table, không kèm schema, không kèm "_resync".
#   Khớp chính xác trước, sau đó không phân biệt hoa thường. "order_items" KHÔNG khớp "OrderItems".
# - include / exclude: chọn cột (không dùng cả hai). Cột "id" luôn được giữ.
# - where: danh sách điều kiện, tất cả phải đúng (AND).
#   op: eq, ne, gt, gte, lt, lte, in, not_in, is_null, not_null
#   Giá trị so chính xác (phân biệt hoa thường). Số viết không có dấu nháy: value: 5, không phải "5".
# - Dòng không khớp where sẽ bị XÓA khỏi DB đích (nếu có).
#
# CẢNH BÁO: cột đã có ở DB đích mà sau đó bị loại khỏi config sẽ không bị drop, chỉ ngừng cập nhật.
# Nếu cột đó là NOT NULL, insert dòng mới sẽ lỗi — hãy DROP hoặc bỏ NOT NULL cột đó bằng tay.

tables:
  # Tên snake_case
  orders:
    include: [id, customer_id, total, status, created_at]
    where:
      - { column: tenant_id, op: eq, value: 5 }        # tenant_id không sync nhưng vẫn dùng để lọc được
      - { column: status, op: in, value: [paid, shipped] }
      - { column: deleted_at, op: is_null }

  users:
    exclude: [password_hash, secret_token]

  # Tên CamelCase: viết đúng như ở nguồn
  OrderItems:
    include: [id, OrderId, ProductId, Quantity, UnitPrice]
    where:
      - { column: TenantId, op: eq, value: 5 }
      - { column: Status, op: in, value: [Paid, Shipped] }

  UserAccounts:
    exclude: [PasswordHash, SecurityStamp]
```

- [ ] **Step 4: Cập nhật `.env.example`** — thêm dòng cuối (đảm bảo dòng trước kết thúc bằng xuống dòng):

```
SYNC_CONFIG_PATH=./sync_config.example.yaml
```

- [ ] **Step 5: Xác minh toàn bộ**

Run: `cargo test` → Expected: 37 test PASS, 0 fail.
Run: `cargo build --release` → Expected: `Finished release profile`.

- [ ] **Step 6: Commit**

```bash
git add sync_config.example.yaml .env.example src/models/sync_config.rs
git commit -m "docs: add example sync config and SYNC_CONFIG_PATH to .env.example

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Checklist test thủ công (sau khi hoàn tất, cần NATS + Postgres)

1. Chạy `cargo run` với `SYNC_CONFIG_PATH=./sync_config.example.yaml`; log phải có `Sync config loaded: tables [OrderItems, UserAccounts, orders, users]`.
2. Dùng `nats pub debezium.test.orders '<json Debezium>'` đẩy: một dòng `orders` khớp `where`; một dòng không khớp; một update đổi dòng đang khớp thành `status: cancelled`; một message có `_PEERDB_IS_DELETED: true`.
3. Kiểm tra DB đích: `orders` chỉ có các cột trong `include`; dòng không khớp không có; dòng chuyển sang `cancelled` đã bị xóa; dòng có cờ delete bị xóa.
4. Xóa `SYNC_CONFIG_PATH` → log `SYNC_CONFIG_PATH not set: ...`, mọi cột được sync.
5. Sửa file config thành `exlude:` → ứng dụng dừng ngay, in `Invalid sync config:` kèm `unknown field`.
