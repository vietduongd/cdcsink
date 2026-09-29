use base64::{Engine, engine::general_purpose};
use serde_json::Value;

use crate::models::models_info::DecimalModel;

/// Giải mã decimal Debezium: big-endian two's complement dạng base64, chia 10^scale.
/// Trả về chuỗi thập phân chính xác, không giới hạn số chữ số. Base64 sai trả về `None`.
pub fn decode_base64_decimal_exact(base64_value: &str, scale: i32) -> Option<String> {
    let mut bytes = general_purpose::STANDARD.decode(base64_value).ok()?;
    let negative = bytes.first().is_some_and(|b| b & 0x80 != 0);
    if negative {
        // Đổi dấu two's complement: đảo bit rồi cộng 1
        for b in bytes.iter_mut() {
            *b = !*b;
        }
        for b in bytes.iter_mut().rev() {
            let (sum, overflow) = b.overflowing_add(1);
            *b = sum;
            if !overflow {
                break;
            }
        }
    }
    let digits = magnitude_to_decimal(&bytes);
    let mut result = apply_scale(&digits, scale);
    if negative && digits != "0" {
        result.insert(0, '-');
    }
    Some(result)
}

/// Số nguyên không dấu big-endian -> chuỗi thập phân (chia liên tiếp cho 10^9).
fn magnitude_to_decimal(bytes: &[u8]) -> String {
    const CHUNK: u64 = 1_000_000_000;
    let mut number: Vec<u8> = bytes.iter().copied().skip_while(|b| *b == 0).collect();
    if number.is_empty() {
        return "0".to_string();
    }
    let mut chunks: Vec<u64> = Vec::new();
    while !number.is_empty() {
        let mut remainder: u64 = 0;
        let mut quotient: Vec<u8> = Vec::with_capacity(number.len());
        for &b in &number {
            let current = (remainder << 8) | b as u64;
            let q = current / CHUNK;
            remainder = current % CHUNK;
            if !(quotient.is_empty() && q == 0) {
                quotient.push(q as u8);
            }
        }
        chunks.push(remainder);
        number = quotient;
    }
    let mut text = chunks.last().unwrap().to_string();
    for chunk in chunks.iter().rev().skip(1) {
        text.push_str(&format!("{:09}", chunk));
    }
    text
}

fn apply_scale(digits: &str, scale: i32) -> String {
    if scale <= 0 {
        if digits == "0" {
            return digits.to_string();
        }
        return format!("{}{}", digits, "0".repeat((-scale) as usize));
    }
    let scale = scale as usize;
    let padded = if digits.len() <= scale {
        format!("{}{}", "0".repeat(scale + 1 - digits.len()), digits)
    } else {
        digits.to_string()
    };
    let (integer, fraction) = padded.split_at(padded.len() - scale);
    format!("{}.{}", integer, fraction)
}

/// Chuỗi thập phân chính xác của một ô NUMERIC: số JSON, chuỗi, hoặc decimal Debezium `{scale, value}`.
pub fn decimal_text(v: &Value) -> Option<String> {
    match v {
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) => Some(s.clone()),
        Value::Object(_) => {
            let decimal = serde_json::from_value::<DecimalModel>(v.clone()).ok()?;
            decode_base64_decimal_exact(&decimal.value, decimal.scale)
        }
        _ => None,
    }
}

/// Giá trị số của một ô dữ liệu: số JSON hoặc decimal Debezium `{scale, value}`.
pub fn numeric_value(v: &Value) -> Option<f64> {
    if let Some(n) = v.as_f64() {
        return Some(n);
    }
    if v.is_object() {
        return decimal_text(v)?.parse().ok();
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
        assert_eq!(decode_base64_decimal_exact("AeI=", 2).as_deref(), Some("4.82"));
    }

    #[test]
    fn decodes_negative_decimal() {
        // 0xFE1E = -482 (two's complement), scale 2
        assert_eq!(decode_base64_decimal_exact("/h4=", 2).as_deref(), Some("-4.82"));
    }

    #[test]
    fn empty_bytes_is_zero() {
        assert_eq!(decode_base64_decimal_exact("", 3).as_deref(), Some("0.000"));
    }

    #[test]
    fn invalid_base64_is_none() {
        assert_eq!(decode_base64_decimal_exact("!!!", 2), None);
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

    // numeric(38,10) vượt i64: trước đây bị tràn thành số sai
    #[test]
    fn decodes_large_decimal_exactly() {
        // 1234567890123456789012345678.0123456789 * 10^10 dạng two's complement
        let unscaled: i128 = 12345678901234567890123456780123456789;
        let bytes = unscaled.to_be_bytes();
        let encoded = general_purpose::STANDARD.encode(bytes);
        assert_eq!(
            decode_base64_decimal_exact(&encoded, 10).as_deref(),
            Some("1234567890123456789012345678.0123456789")
        );
        let encoded = general_purpose::STANDARD.encode((-unscaled).to_be_bytes());
        assert_eq!(
            decode_base64_decimal_exact(&encoded, 10).as_deref(),
            Some("-1234567890123456789012345678.0123456789")
        );
    }

    #[test]
    fn decodes_small_fraction_and_scale_zero() {
        assert_eq!(decode_base64_decimal_exact("AQ==", 10).as_deref(), Some("0.0000000001"));
        assert_eq!(decode_base64_decimal_exact("/w==", 3).as_deref(), Some("-0.001"));
        assert_eq!(decode_base64_decimal_exact("AeI=", 0).as_deref(), Some("482"));
        assert_eq!(decode_base64_decimal_exact("", 2).as_deref(), Some("0.00"));
        // 0x0080 = 128 dương dù byte sau có bit cao
        assert_eq!(decode_base64_decimal_exact("AIA=", 1).as_deref(), Some("12.8"));
    }

    #[test]
    fn decimal_text_reads_all_forms() {
        assert_eq!(decimal_text(&json!({"scale": 2, "value": "AeI="})).as_deref(), Some("4.82"));
        assert_eq!(decimal_text(&json!(1.5)).as_deref(), Some("1.5"));
        assert_eq!(decimal_text(&json!("7.25")).as_deref(), Some("7.25"));
        assert_eq!(decimal_text(&Value::Null), None);
    }
}
