use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Deserialize;
use serde_json::Value;

use crate::models::DataModel;
use crate::models::decimal::numeric_value;

pub const ID_COLUMN: &str = "id";
pub const DELETED_FLAG_COLUMN: &str = "_PEERDB_IS_DELETED";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowAction {
    Upsert,
    Delete,
}

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

    /// Giá trị của env `SYNC_CONFIG_PATH`: không có hoặc rỗng → không dùng config.
    pub fn from_env_value(path: Option<String>) -> Result<Option<SyncConfig>, String> {
        match path {
            Some(path) if !path.trim().is_empty() => Self::load(path.trim()).map(Some),
            _ => Ok(None),
        }
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
}

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

/// Tìm key trong map khớp `name`: chính xác trước, sau đó không phân biệt hoa thường.
fn resolve_key<'a, V>(map: &'a HashMap<String, V>, name: &str) -> Option<&'a String> {
    if let Some((key, _)) = map.get_key_value(name) {
        return Some(key);
    }
    let lower = name.to_lowercase();
    map.keys().find(|key| key.to_lowercase() == lower)
}

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

    #[test]
    fn example_config_file_is_valid() {
        let cfg = SyncConfig::load(concat!(env!("CARGO_MANIFEST_DIR"), "/sync_config.example.yaml"))
            .expect("example config must be valid");
        assert!(cfg.table("orders").is_some());
        assert!(cfg.table("OrderItems").is_some());
        assert!(cfg.table("UserAccounts").is_some());
    }

    #[test]
    fn load_reports_missing_file() {
        let err = SyncConfig::load("does/not/exist.yaml").expect_err("missing file");
        assert!(err.contains("Cannot read sync config does/not/exist.yaml"), "{err}");
    }
}
