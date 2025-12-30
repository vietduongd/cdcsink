use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;

/// Standard API response structure
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub data: Option<T>,
    pub message: String,
    pub code: String,
    pub errors: Vec<String>,
}

impl<T: Serialize> ApiResponse<T> {
    /// Create a successful response with data
    pub fn success(data: T, message: impl Into<String>) -> Self {
        Self {
            data: Some(data),
            message: message.into(),
            code: "SUCCESS".to_string(),
            errors: vec![],
        }
    }

    /// Create a successful response without data
    pub fn success_no_data(message: impl Into<String>) -> ApiResponse<()> {
        ApiResponse {
            data: None,
            message: message.into(),
            code: "SUCCESS".to_string(),
            errors: vec![],
        }
    }

    /// Create an error response
    pub fn error(code: impl Into<String>, message: impl Into<String>, errors: Vec<String>) -> Self {
        Self {
            data: None,
            message: message.into(),
            code: code.into(),
            errors,
        }
    }

    /// Create a not found error response
    pub fn not_found(resource: impl Into<String>) -> Self {
        let resource = resource.into();
        Self {
            data: None,
            message: format!("{} not found", resource),
            code: "NOT_FOUND".to_string(),
            errors: vec![],
        }
    }

    /// Create a bad request error response
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            data: None,
            message: message.into(),
            code: "BAD_REQUEST".to_string(),
            errors: vec![],
        }
    }

    /// Create a conflict error response
    pub fn conflict(message: impl Into<String>) -> Self {
        Self {
            data: None,
            message: message.into(),
            code: "CONFLICT".to_string(),
            errors: vec![],
        }
    }

    /// Create an internal error response
    pub fn internal_error(message: impl Into<String>) -> Self {
        Self {
            data: None,
            message: message.into(),
            code: "INTERNAL_ERROR".to_string(),
            errors: vec![],
        }
    }
}

impl<T: Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(self) -> axum::response::Response {
        let status = match self.code.as_str() {
            "SUCCESS" => StatusCode::OK,
            "NOT_FOUND" => StatusCode::NOT_FOUND,
            "BAD_REQUEST" => StatusCode::BAD_REQUEST,
            "CONFLICT" => StatusCode::CONFLICT,
            "INTERNAL_ERROR" => StatusCode::INTERNAL_SERVER_ERROR,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status, Json(self)).into_response()
    }
}
