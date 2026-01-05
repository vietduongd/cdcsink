use crate::Result;
use async_trait::async_trait;
use std::env;

/// Trait for sending notifications
#[async_trait]
pub trait Notifier: Send + Sync {
    /// Send error notification when flow stops due to errors
    async fn send_error_notification(&self, flow_name: &str, error_details: &str) -> Result<()>;
}

/// Email notifier using SMTP
pub struct EmailNotifier {
    smtp_host: String,
    smtp_port: u16,
    smtp_username: String,
    smtp_password: String,
    from_email: String,
    to_emails: Vec<String>,
}

impl EmailNotifier {
    /// Create email notifier from environment variables
    ///
    /// Required environment variables:
    /// - SMTP_HOST: SMTP server hostname (e.g., smtp.gmail.com)
    /// - SMTP_PORT: SMTP server port (e.g., 587)
    /// - SMTP_USERNAME: SMTP authentication username
    /// - SMTP_PASSWORD: SMTP authentication password
    /// - SMTP_FROM_EMAIL: Sender email address
    /// - SMTP_TO_EMAILS: Comma-separated list of recipient emails
    pub fn from_env() -> Result<Self> {
        let smtp_host = env::var("SMTP_HOST")
            .map_err(|_| crate::Error::Configuration("SMTP_HOST not set".to_string()))?;

        let smtp_port = env::var("SMTP_PORT")
            .map_err(|_| crate::Error::Configuration("SMTP_PORT not set".to_string()))?
            .parse::<u16>()
            .map_err(|_| {
                crate::Error::Configuration("SMTP_PORT must be a valid port number".to_string())
            })?;

        let smtp_username = env::var("SMTP_USERNAME")
            .map_err(|_| crate::Error::Configuration("SMTP_USERNAME not set".to_string()))?;

        let smtp_password = env::var("SMTP_PASSWORD")
            .map_err(|_| crate::Error::Configuration("SMTP_PASSWORD not set".to_string()))?;

        let from_email = env::var("SMTP_FROM_EMAIL")
            .map_err(|_| crate::Error::Configuration("SMTP_FROM_EMAIL not set".to_string()))?;

        let to_emails_str = env::var("SMTP_TO_EMAILS")
            .map_err(|_| crate::Error::Configuration("SMTP_TO_EMAILS not set".to_string()))?;

        let to_emails: Vec<String> = to_emails_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if to_emails.is_empty() {
            return Err(crate::Error::Configuration(
                "SMTP_TO_EMAILS must contain at least one email".to_string(),
            ));
        }

        Ok(Self {
            smtp_host,
            smtp_port,
            smtp_username,
            smtp_password,
            from_email,
            to_emails,
        })
    }

    /// Create email notifier with explicit configuration (for testing)
    pub fn new(
        smtp_host: String,
        smtp_port: u16,
        smtp_username: String,
        smtp_password: String,
        from_email: String,
        to_emails: Vec<String>,
    ) -> Self {
        Self {
            smtp_host,
            smtp_port,
            smtp_username,
            smtp_password,
            from_email,
            to_emails,
        }
    }
}

#[async_trait]
impl Notifier for EmailNotifier {
    async fn send_error_notification(&self, flow_name: &str, error_details: &str) -> Result<()> {
        use lettre::{
            message::header::ContentType, transport::smtp::authentication::Credentials, Message,
            SmtpTransport, Transport,
        };

        let subject = format!("[CDC Alert] Flow '{}' stopped due to errors", flow_name);

        let body = format!(
            r#"
CDC Flow Error Alert
====================

Flow Name: {}
Status: STOPPED
Reason: Error threshold reached (20 consecutive errors)

Error Details:
{}

Timestamp: {}

Action Required:
- Check destination connectivity and configuration
- Review error logs for more details
- Restart the flow after fixing the issue

This is an automated notification from CDC Sink.
"#,
            flow_name,
            error_details,
            chrono::Utc::now().to_rfc3339()
        );

        // Build email for each recipient
        for to_email in &self.to_emails {
            let email = Message::builder()
                .from(self.from_email.parse().map_err(|e| {
                    crate::Error::Configuration(format!("Invalid from email: {}", e))
                })?)
                .to(to_email.parse().map_err(|e| {
                    crate::Error::Configuration(format!("Invalid to email '{}': {}", to_email, e))
                })?)
                .subject(&subject)
                .header(ContentType::TEXT_PLAIN)
                .body(body.clone())
                .map_err(|e| {
                    crate::Error::Configuration(format!("Failed to build email: {}", e))
                })?;

            // Create SMTP transport
            let creds = Credentials::new(self.smtp_username.clone(), self.smtp_password.clone());

            let mailer = SmtpTransport::relay(&self.smtp_host)
                .map_err(|e| {
                    crate::Error::Connection(format!("Failed to create SMTP transport: {}", e))
                })?
                .port(self.smtp_port)
                .credentials(creds)
                .build();

            // Send email
            mailer.send(&email).map_err(|e| {
                crate::Error::Connection(format!("Failed to send email to {}: {}", to_email, e))
            })?;

            tracing::info!("Error notification email sent to {}", to_email);
        }

        Ok(())
    }
}

/// No-op notifier for when email is not configured
pub struct NoOpNotifier;

#[async_trait]
impl Notifier for NoOpNotifier {
    async fn send_error_notification(&self, flow_name: &str, error_details: &str) -> Result<()> {
        tracing::warn!(
            "Email notification not configured. Flow '{}' error: {}",
            flow_name,
            error_details
        );
        Ok(())
    }
}
