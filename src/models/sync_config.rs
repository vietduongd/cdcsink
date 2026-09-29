use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Deserialize;
use serde_json::Value;

use crate::models::DataModel;

pub const ID_COLUMN: &str = "id";

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

    #[test]
    fn load_reports_missing_file() {
        let err = SyncConfig::load("does/not/exist.yaml").expect_err("missing file");
        assert!(err.contains("Cannot read sync config does/not/exist.yaml"), "{err}");
    }
}
