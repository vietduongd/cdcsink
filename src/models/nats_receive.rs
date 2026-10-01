use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use async_nats::{
    connect,
    jetstream::{self, AckKind, Message},
};

use async_nats::jetstream::consumer::{FromConsumer, PullConsumer, pull};
use futures_util::StreamExt;
use tracing::{debug, info, warn};

use crate::models::{
    ChangeRow, Parsed, PoisonMessage, RowSource, SyncConfig, change::SkipReason, parse_change,
    sync_config::MatchOutcome,
};

pub struct NatsReceive {
    pub url: String,
    pub durable_name: String,
    pub topic_name: String,
    pub stream: String,
    pub number_pull_object: usize,
    /// Thời gian NATS chờ ack trước khi gửi lại message
    pub ack_wait: Duration,
}

/// Kết quả một lần fetch.
pub struct ReceivedBatch {
    /// Dòng hợp lệ, theo thứ tự nhận.
    pub rows: Vec<ChangeRow>,
    /// Message không parse được, được ghi vào dead-letter trước khi ack.
    pub poison: Vec<PoisonMessage>,
    /// Message của `rows` và `poison`, chỉ ack sau khi batch được lưu xong.
    pub messages: Vec<Message>,
}

impl NatsReceive {
    pub fn new(
        url: String,
        durable_name: String,
        topic_name: String,
        stream: String,
        number_pull_object: usize,
        ack_wait: Duration,
    ) -> Self {
        NatsReceive {
            url,
            durable_name,
            topic_name,
            stream,
            number_pull_object,
            ack_wait,
        }
    }

    pub async fn connected(&self) -> Result<PullConsumer, String> {
        let client = connect(&self.url)
            .await
            .map_err(|e| format!("Failed to connect to NATS server: {}", e))?;

        let jetstream_context = jetstream::new(client);
        let stream_info = jetstream_context
            .get_or_create_stream(async_nats::jetstream::stream::Config {
                name: self.stream.clone(),
                ..Default::default()
            })
            .await
            .map_err(|e| format!("Failed to get or create stream: {}", e))?;

        let consumer: PullConsumer = stream_info
            .get_or_create_consumer(
                &self.durable_name.clone(),
                pull::Config {
                    durable_name: Some(self.durable_name.clone()),
                    filter_subject: self.topic_name.clone(),
                    ack_wait: self.ack_wait,
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| format!("Failed to get or create consumer: {}", e))?;
        if consumer.cached_info().config.ack_wait == self.ack_wait {
            return Ok(consumer);
        }
        // Consumer đã có từ trước với ack_wait khác: get_or_create không đổi config nên phải update
        let mut config = consumer.cached_info().config.clone();
        config.ack_wait = self.ack_wait;
        let config = pull::Config::try_from_consumer_config(config)
            .map_err(|e| format!("Failed to build consumer config: {}", e))?;
        let consumer = stream_info
            .update_consumer(config)
            .await
            .map_err(|e| format!("Failed to update consumer ack_wait: {}", e))?;
        info!(
            consumer = %self.durable_name,
            ack_wait_secs = self.ack_wait.as_secs(),
            "Consumer ack_wait updated"
        );
        Ok(consumer)
    }

    pub async fn receive_messages(
        &self,
        consumer: &mut PullConsumer,
        sync_config: Option<&SyncConfig>,
        logged_type_errors: &mut HashSet<String>,
    ) -> Result<ReceivedBatch, String> {
        let mut stream = consumer
            .fetch()
            .max_messages(self.number_pull_object)
            .expires(Duration::from_secs(5)) // 👈 MaxWait
            .messages()
            .await
            .map_err(|e| format!("Failed to receive messages: {}", e))?;

        let mut batch = ReceivedBatch {
            rows: Vec::new(),
            poison: Vec::new(),
            messages: Vec::new(),
        };
        // table -> (tổng số message, số message bị loại vì lỗi kiểu hoặc thiếu cột)
        let mut type_rejections: HashMap<String, (usize, usize)> = HashMap::new();
        let mut counter = 0;
        while let Some(item) = stream.next().await {
            let message = match item {
                Ok(message) => message,
                Err(e) => {
                    // Phần còn lại của batch chưa nhận sẽ được NATS gửi lại ở lần fetch sau
                    warn!(error = %e, "Failed to read message from batch");
                    break;
                }
            };
            let stream_sequence = message.info().ok().map(|info| info.stream_sequence);
            let source = RowSource::new(&message.subject, stream_sequence, &message.payload);
            match parse_change(source, counter, sync_config) {
                Parsed::Row { row, outcome } => {
                    log_filter_problems(&row.table_name, &outcome, logged_type_errors);
                    let stats = type_rejections
                        .entry(row.table_name.clone())
                        .or_insert((0, 0));
                    stats.0 += 1;
                    if !outcome.matched
                        && (!outcome.type_errors.is_empty() || !outcome.missing_columns.is_empty())
                    {
                        stats.1 += 1;
                    }
                    batch.rows.push(row);
                    batch.messages.push(message);
                    counter += 1;
                }
                Parsed::Poison(poison) => {
                    batch.poison.push(poison);
                    batch.messages.push(message);
                }
                Parsed::Skip(reason) => {
                    log_skip(&reason, logged_type_errors);
                    Self::ack_skipped(&message).await?;
                }
            }
        }

        for (table_name, (total, rejected)) in &type_rejections {
            if *total > 0 && total == rejected {
                warn!(
                    table = %table_name,
                    total,
                    "All rows rejected due to type mismatch or missing column in where"
                );
            }
        }

        Ok(batch)
    }

    /// Chu kỳ gửi Progress: 3 lần trong mỗi ack_wait để một lần gửi chậm/lỗi chưa làm NATS gửi lại.
    pub fn progress_interval(&self) -> Duration {
        Duration::from_millis((self.ack_wait.as_millis() / 3) as u64)
    }

    /// Gia hạn ack_wait cho cả batch (đang ghi hoặc đang chờ retry), để NATS không gửi lại giữa chừng.
    pub async fn extend_ack_deadline(&self, messages: &[Message]) {
        for message in messages {
            if let Err(e) = message.ack_with(AckKind::Progress).await {
                warn!(error = %e, "Failed to extend ack deadline");
                return;
            }
        }
    }

    /// Message bị bỏ qua vẫn phải ack, nếu không NATS gửi lại sau mỗi ack_wait mãi mãi.
    async fn ack_skipped(message: &Message) -> Result<(), String> {
        message
            .ack()
            .await
            .map_err(|e| format!("Failed to acknowledge skipped message: {}", e))
    }

    pub async fn ack_messages(&self, messages: &[Message]) -> Result<(), String> {
        for message in messages {
            message
                .ack()
                .await
                .map_err(|e| format!("Failed to acknowledge message: {}", e))?;
        }
        Ok(())
    }
}

/// Log một lần cho mỗi (table, lý do) để không lặp log mỗi batch.
fn log_skip(reason: &SkipReason, logged: &mut HashSet<String>) {
    match reason {
        SkipReason::Tombstone => debug!("Skipping tombstone message"),
        SkipReason::IgnoredTable(table) => {
            if logged.insert(format!("{}|ignored", table)) {
                warn!(table = %table, "Table is never synced, messages are skipped");
            }
        }
        SkipReason::NoPrimaryKey(table) => {
            if logged.insert(format!("{}|no-primary-key", table)) {
                warn!(table = %table, "Table has no \"id\" column, messages are skipped");
            }
        }
    }
}

fn log_filter_problems(table_name: &str, outcome: &MatchOutcome, logged: &mut HashSet<String>) {
    for error in &outcome.type_errors {
        let key = format!("{}|{}|{:?}", table_name, error.column, error.op);
        if logged.insert(key) {
            warn!(
                table = %table_name,
                column = %error.column,
                op = ?error.op,
                detail = %error.detail,
                "Sync filter type mismatch"
            );
        }
    }
    for column in &outcome.missing_columns {
        let key = format!("{}|{}|missing", table_name, column);
        if logged.insert(key) {
            warn!(
                table = %table_name,
                column = %column,
                "Sync filter column not found (check the name in where; rows are deleted)"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receiver(ack_wait_secs: u64) -> NatsReceive {
        NatsReceive::new(
            "nats://x".to_string(),
            "c".to_string(),
            "t".to_string(),
            "s".to_string(),
            10,
            Duration::from_secs(ack_wait_secs),
        )
    }

    // Gia hạn 3 lần trong mỗi ack_wait để một lần gửi Progress chậm/lỗi không làm NATS gửi lại batch
    #[test]
    fn progress_interval_is_a_third_of_ack_wait() {
        assert_eq!(receiver(30).progress_interval(), Duration::from_secs(10));
        assert_eq!(
            receiver(10).progress_interval(),
            Duration::from_millis(3333)
        );
        assert!(receiver(1).progress_interval() < Duration::from_secs(1));
    }
}
