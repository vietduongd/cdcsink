use base64::{Engine, engine::general_purpose};
use serde_json::Value;

use crate::models::models_info::DecimalModel;

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
