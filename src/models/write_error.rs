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
        for code in [
            Some("08006"),
            Some("40P01"),
            Some("42P01"),
            Some("57P01"),
            None,
        ] {
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
