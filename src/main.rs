use std::{
    collections::{HashMap, HashSet},
    env,
    error::Error,
    time::{Duration, Instant},
};

use async_nats::jetstream::Message;
use dotenvy::dotenv;
use sqlx::PgPool;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use crate::models::{
    ChangeRow, DataModel, DeadLetterStore, NatsReceive, NewDeadLetter, Parsed, PoisonMessage,
    PostgresDestination, Rejected, RowAction, RowSource, SyncConfig, latest_per_key, parse_change,
    write_isolating,
};

mod models;

type SchemaCache = HashMap<String, HashSet<String>>;

/// Chờ giữa hai lần fetch khi NATS lỗi.
const FETCH_RETRY_DELAY: Duration = Duration::from_secs(1);
/// Backoff khi ghi batch lỗi: 1s, 2s, 4s... tối đa 60s.
const MAX_RETRY_DELAY: Duration = Duration::from_secs(60);
/// Số dòng dead-letter tối đa mỗi lần replay.
const REPLAY_BATCH_LIMIT: i64 = 500;

/// Phía DB đích, dùng chung cho batch thường và replay dead-letter.
struct Sink {
    destination: PostgresDestination,
    dead_letters: DeadLetterStore,
    pool: PgPool,
    schema: String,
}

/// Mức log lấy từ RUST_LOG (mặc định info), LOG_FORMAT=json để xuất JSON cho hệ thống gom log.
fn init_logging() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn,async_nats=warn"));
    let builder = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true);
    if env::var("LOG_FORMAT").is_ok_and(|v| v.eq_ignore_ascii_case("json")) {
        builder.json().flatten_event(true).init();
    } else {
        builder.init();
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();
    init_logging();

    info!(version = env!("CARGO_PKG_VERSION"), "Starting CDC Sink");
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
    let max_batch_retries = env::var("MAX_BATCH_RETRIES")
        .unwrap_or("5".to_string())
        .parse::<u32>()?;
    // Tối thiểu 1s: NATS không nhận ack_wait = 0
    let ack_wait = Duration::from_secs(
        env::var("NATS_ACK_WAIT_SECS")
            .unwrap_or("30".to_string())
            .parse::<u64>()?
            .max(1),
    );
    let replay_interval = Duration::from_secs(
        env::var("DEAD_LETTER_REPLAY_SECS")
            .unwrap_or("30".to_string())
            .parse::<u64>()?,
    );

    let sync_config = match SyncConfig::from_env_value(env::var("SYNC_CONFIG_PATH").ok()) {
        Ok(Some(config)) => {
            info!(tables = %config.table_names().join(", "), "Sync config loaded");
            Some(config)
        }
        Ok(None) => {
            info!("SYNC_CONFIG_PATH not set: syncing all tables/columns/rows (no filter)");
            None
        }
        Err(e) => {
            error!(error = %e, "Failed to load sync config");
            std::process::exit(1);
        }
    };

    info!(
        nats_url = %nats_url,
        topic = %topic_name,
        stream = %nats_stream_name,
        consumer = %nats_consumer_name,
        schema = %database_schema_expected,
        batch_size = number_pull_object,
        max_batch_retries,
        ack_wait_secs = ack_wait.as_secs(),
        replay_secs = replay_interval.as_secs(),
        "Configuration loaded"
    );

    let nats_info = NatsReceive::new(
        nats_url,
        nats_consumer_name,
        topic_name,
        nats_stream_name,
        number_pull_object,
        ack_wait,
    );

    let mut consumer = nats_info.connected().await?;

    let destination = PostgresDestination::new(db_url, database_schema_expected.clone());
    let pool = destination
        .connect()
        .await
        .map_err(Box::<dyn Error>::from)?;
    destination
        .ensure_schema_metadata_table(&pool)
        .await
        .map_err(Box::<dyn Error>::from)?;
    let dead_letters = DeadLetterStore::new(&database_schema_expected);
    dead_letters
        .ensure_table(&pool)
        .await
        .map_err(Box::<dyn Error>::from)?;
    let mut schema_cache = destination
        .get_schema_info(&pool)
        .await
        .map_err(Box::<dyn Error>::from)?;
    let sink = Sink {
        destination,
        dead_letters,
        pool,
        schema: database_schema_expected,
    };

    let mut last_replay = Instant::now();
    let mut logged_type_errors: HashSet<String> = HashSet::new();
    loop {
        // Replay chạy giữa hai batch nên không bao giờ song song với ghi batch thường
        if !replay_interval.is_zero() && last_replay.elapsed() >= replay_interval {
            last_replay = Instant::now();
            match replay_dead_letters(&sink, sync_config.as_ref(), &mut schema_cache).await {
                Ok(0) => {}
                Ok(count) => info!(count, "Replayed dead letters"),
                Err(e) => {
                    // Dòng vẫn ở trạng thái retry, thử lại ở chu kỳ sau
                    warn!(error = %e, "Dead letter replay failed, will retry next cycle");
                    match sink.destination.get_schema_info(&sink.pool).await {
                        Ok(fresh) => schema_cache = fresh,
                        Err(e) => warn!(error = %e, "Failed to reload schema info"),
                    }
                }
            }
        }

        let batch = match nats_info
            .receive_messages(&mut consumer, sync_config.as_ref(), &mut logged_type_errors)
            .await
        {
            Ok(batch) => batch,
            Err(e) => {
                // Message chưa ack sẽ được NATS gửi lại, chỉ cần thử fetch lại
                warn!(error = %e, "Failed to receive messages, retrying");
                tokio::time::sleep(FETCH_RETRY_DELAY).await;
                continue;
            }
        };
        if batch.messages.is_empty() {
            continue;
        }
        info!(
            rows = batch.rows.len(),
            poison = batch.poison.len(),
            "Received messages"
        );

        // Giữ batch và retry tại chỗ thay vì nak: nak để message mới đi trước message cũ,
        // bản cũ gửi lại sau sẽ ghi đè bản mới ở đích. Mọi thao tác đều idempotent
        // (IF NOT EXISTS, upsert, delete theo id) nên chạy lại cả batch là an toàn.
        let mut attempt: u32 = 0;
        loop {
            // Gia hạn ack song song trong lúc ghi: batch lớn hoặc bisection có thể lâu hơn ack_wait
            let persisted = tokio::select! {
                result = persist_batch(&sink, &batch.rows, &batch.poison, &[], &mut schema_cache) => result,
                _ = keep_messages_alive(&nats_info, &batch.messages) => unreachable!(),
            };
            match persisted {
                Ok(()) => break,
                Err(e) if attempt >= max_batch_retries => {
                    // Không ack: NATS gửi lại batch sau khi service khởi động lại
                    error!(
                        error = %e,
                        attempts = attempt + 1,
                        "Batch failed after all retries, exiting"
                    );
                    std::process::exit(1);
                }
                Err(e) => {
                    let delay = MAX_RETRY_DELAY.min(Duration::from_secs(1 << attempt.min(6)));
                    attempt += 1;
                    warn!(
                        error = %e,
                        attempt,
                        max_retries = max_batch_retries,
                        delay_secs = delay.as_secs(),
                        "Batch failed, retrying"
                    );
                    wait_keeping_messages(&nats_info, &batch.messages, delay).await;
                    // DDL có thể đã chạy một phần: đọc lại schema thật ở đích
                    match sink.destination.get_schema_info(&sink.pool).await {
                        Ok(fresh) => schema_cache = fresh,
                        Err(e) => warn!(error = %e, "Failed to reload schema info"),
                    }
                }
            }
        }

        if let Err(e) = nats_info.ack_messages(&batch.messages).await {
            // Dữ liệu đã lưu xong; message chưa ack sẽ được gửi lại và ghi lại, không mất dữ liệu
            warn!(error = %e, "Failed to acknowledge batch");
        }
    }
}

/// Gửi Progress cho `messages` theo chu kỳ, không bao giờ kết thúc: chạy trong `select!` cùng
/// việc đang giữ batch, xong việc đó thì future này bị hủy.
async fn keep_messages_alive(nats_info: &NatsReceive, messages: &[Message]) {
    loop {
        tokio::time::sleep(nats_info.progress_interval()).await;
        nats_info.extend_ack_deadline(messages).await;
    }
}

/// Chờ `delay`, gia hạn ack_wait định kỳ để NATS không gửi lại batch đang giữ.
async fn wait_keeping_messages(nats_info: &NatsReceive, messages: &[Message], delay: Duration) {
    tokio::select! {
        _ = tokio::time::sleep(delay) => {}
        _ = keep_messages_alive(nats_info, messages) => {}
    }
}

/// Replay các dòng `retry` của `_cdc_dead_letter` qua đúng luồng ghi của batch thường.
/// Ghi được thì `resolved`, lại lỗi dữ liệu thì về `pending`; `Err` (lỗi tạm thời) thì giữ `retry`.
async fn replay_dead_letters(
    sink: &Sink,
    sync_config: Option<&SyncConfig>,
    schema_cache: &mut SchemaCache,
) -> Result<usize, String> {
    let entries = sink
        .dead_letters
        .fetch_retry(REPLAY_BATCH_LIMIT, &sink.pool)
        .await?;
    let count = entries.len();
    if count == 0 {
        return Ok(0);
    }
    // Replay chạy cả lô: lỗi thì báo kèm id để vận hành tìm được dòng gây lỗi
    let ids: Vec<i64> = entries.iter().map(|entry| entry.id).collect();
    let mut rows = Vec::new();
    let mut poison = Vec::new();
    let mut skipped = Vec::new();
    // index theo thứ tự id: cùng khóa chính thì dòng dead-letter mới hơn thắng
    for (index, entry) in entries.into_iter().enumerate() {
        let id = entry.id;
        let source = RowSource {
            subject: entry.subject,
            stream_sequence: entry.stream_sequence.map(|sequence| sequence as u64),
            payload: entry.payload.into(),
            dead_letter_id: Some(id),
        };
        match parse_change(source, index as i64, sync_config) {
            Parsed::Row { row, .. } => rows.push(row),
            Parsed::Poison(message) => poison.push(message),
            Parsed::Skip(reason) => {
                info!(
                    dead_letter_id = id,
                    reason = ?reason,
                    "Dead letter no longer needs syncing, marked resolved"
                );
                skipped.push(id);
            }
        }
    }
    persist_batch(sink, &rows, &poison, &skipped, schema_cache)
        .await
        .map_err(|e| format!("{} (dead letter ids: {:?})", e, ids))?;
    Ok(count)
}

/// Ghi batch vào đích rồi lưu dữ liệu lỗi vào dead-letter trong một transaction.
/// `Err` là lỗi tạm thời: batch chưa được lưu trọn vẹn nên không được ack.
///
/// `resolved_ids`: id dead-letter replay xong mà không có dòng tương ứng (vd table giờ bị bỏ qua).
async fn persist_batch(
    sink: &Sink,
    rows: &[ChangeRow],
    poison: &[PoisonMessage],
    resolved_ids: &[i64],
    schema_cache: &mut SchemaCache,
) -> Result<(), String> {
    let rejected = newest_rejected(process_batch(sink, rows, schema_cache).await?, rows);

    let mut tx = sink
        .pool
        .begin()
        .await
        .map_err(|e| format!("Failed to begin dead letter transaction: {}", e))?;
    let mut recorded_ids: Vec<i64> = Vec::new();
    for item in &rejected {
        let row = item.item;
        let id = sink
            .dead_letters
            .record(
                &NewDeadLetter::rejected(row, &item.code, &item.message),
                &mut tx,
            )
            .await?;
        warn!(
            dead_letter_id = id,
            table = %row.table_name,
            primary_key = %row.primary_key,
            error_code = %item.code,
            error = %item.message,
            "Row rejected by destination, sent to dead letter"
        );
        recorded_ids.push(id);
    }
    for message in poison {
        let id = sink
            .dead_letters
            .record(&NewDeadLetter::poison(message), &mut tx)
            .await?;
        warn!(
            dead_letter_id = id,
            subject = %message.source.subject,
            reason = %message.reason,
            "Unprocessable message sent to dead letter"
        );
        recorded_ids.push(id);
    }

    // Bản mới nhất của các dòng replay đã ghi được; bản cũ hơn của cùng khóa bị superseded
    let replayed: Vec<i64> = latest_replay_ids(rows)
        .into_iter()
        .filter(|id| !recorded_ids.contains(id))
        .collect();
    let keep_ids: Vec<i64> = recorded_ids.iter().chain(&replayed).copied().collect();
    for (table_name, (keys, below_ids)) in supersede_keys(rows) {
        sink.dead_letters
            .supersede(table_name, &keys, &below_ids, &keep_ids, &mut tx)
            .await?;
    }
    let resolved: Vec<i64> = replayed.iter().chain(resolved_ids).copied().collect();
    sink.dead_letters.mark_resolved(&resolved, &mut tx).await?;

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit dead letters: {}", e))
}

/// Khóa chính của các dòng trong batch, gom theo table, kèm id dead-letter khi dòng đến từ replay
/// (replay chỉ được thay thế các dòng lỗi cũ hơn nó).
fn supersede_keys(rows: &[ChangeRow]) -> HashMap<&str, (Vec<String>, Vec<Option<i64>>)> {
    let mut keys: HashMap<&str, (Vec<String>, Vec<Option<i64>>)> = HashMap::new();
    for row in rows {
        let (primary_keys, below_ids) = keys.entry(row.table_name.as_str()).or_default();
        primary_keys.push(row.primary_key.clone());
        below_ids.push(row.source.dead_letter_id);
    }
    keys
}

/// Chỉ giữ dòng bị từ chối là bản mới nhất của (table, khóa chính) trong batch. Một khóa có thể được
/// ghi nhiều lần trong batch (flush trước DDL rồi sau DDL); bản cũ bị từ chối mà vẫn lưu thì replay
/// sau này sẽ ghi đè bản mới hơn.
fn newest_rejected<'a>(
    rejected: Vec<Rejected<'a, ChangeRow>>,
    rows: &[ChangeRow],
) -> Vec<Rejected<'a, ChangeRow>> {
    let mut newest: HashMap<(&str, &str), i64> = HashMap::new();
    for row in rows {
        let index = newest
            .entry((row.table_name.as_str(), row.primary_key.as_str()))
            .or_insert(row.index);
        *index = (*index).max(row.index);
    }
    rejected
        .into_iter()
        .filter(|r| {
            let key = (r.item.table_name.as_str(), r.item.primary_key.as_str());
            newest.get(&key) == Some(&r.item.index)
        })
        .collect()
}

/// id dead-letter của các dòng replay là bản mới nhất của (table, khóa chính) trong batch.
fn latest_replay_ids(rows: &[ChangeRow]) -> Vec<i64> {
    let mut latest: HashMap<(&str, &str), &ChangeRow> = HashMap::new();
    for row in rows {
        let key = (row.table_name.as_str(), row.primary_key.as_str());
        if latest
            .get(&key)
            .is_none_or(|existing| existing.index < row.index)
        {
            latest.insert(key, row);
        }
    }
    latest
        .values()
        .filter_map(|row| row.source.dead_letter_id)
        .collect()
}

/// Ghi các dòng vào đích: tạo table/cột khi cần, rồi upsert/delete theo từng table.
/// Chỉ cập nhật `schema_cache` sau khi DDL thành công. Trả về các dòng bị DB từ chối vì lỗi dữ liệu.
async fn process_batch<'a>(
    sink: &Sink,
    rows: &'a [ChangeRow],
    schema_cache: &mut SchemaCache,
) -> Result<Vec<Rejected<'a, ChangeRow>>, String> {
    let mut rejected: Vec<Rejected<'a, ChangeRow>> = Vec::new();
    let mut message_active: HashMap<String, Vec<&'a ChangeRow>> = HashMap::new();
    for msg in rows {
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
                flush_table(sink, table_name, &buffered, &mut rejected).await?;
            }
            sink.destination
                .create_table_if_not_exists_query(
                    &sink.schema,
                    table_name,
                    &msg.table_value,
                    &sink.pool,
                )
                .await?;
            schema_cache.insert(
                table_name.clone(),
                msg.table_value.keys().cloned().collect(),
            );
        } else {
            // Table đã tồn tại: kiểm tra xem có column mới không
            let cached_columns = schema_cache.get_mut(table_name).unwrap();
            let new_columns: Vec<(&String, &DataModel)> = msg
                .table_value
                .iter()
                .filter(|(col_name, _)| !cached_columns.contains(*col_name))
                .collect();

            if !new_columns.is_empty() {
                // Có column mới: nếu message_active đang có dữ liệu thì insert trước
                if let Some(buffered) = message_active.remove(table_name) {
                    flush_table(sink, table_name, &buffered, &mut rejected).await?;
                }
                // Tạo các column mới
                for (col_name, col_type) in &new_columns {
                    sink.destination
                        .add_column_if_not_exists(
                            &sink.schema,
                            table_name,
                            col_name,
                            col_type,
                            &sink.pool,
                        )
                        .await?;
                    cached_columns.insert(col_name.to_string());
                }
            }
        }
        message_active
            .entry(table_name.clone())
            .or_insert(Vec::new())
            .push(msg);
    }
    for (table_name, table_rows) in &message_active {
        flush_table(sink, table_name, table_rows, &mut rejected).await?;
    }
    Ok(rejected)
}

/// Ghi các dòng của một table: mỗi khóa chính một bản mới nhất, delete trước rồi upsert.
/// Lỗi dữ liệu thì chia đôi để tìm đúng dòng lỗi; dòng lỗi được thêm vào `rejected`.
async fn flush_table<'a>(
    sink: &Sink,
    table_name: &str,
    rows: &[&'a ChangeRow],
    rejected: &mut Vec<Rejected<'a, ChangeRow>>,
) -> Result<(), String> {
    let (to_delete, to_upsert): (Vec<&'a ChangeRow>, Vec<&'a ChangeRow>) = latest_per_key(rows)
        .into_iter()
        .partition(|row| row.action == RowAction::Delete);
    rejected.extend(
        write_isolating(to_delete, |chunk| {
            sink.destination.delete_rows(table_name, chunk, &sink.pool)
        })
        .await?,
    );
    rejected.extend(
        write_isolating(to_upsert, |chunk| {
            sink.destination.upsert_rows(table_name, chunk, &sink.pool)
        })
        .await?,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::RowSource;

    fn row(table: &str, key: &str, index: i64, dead_letter_id: Option<i64>) -> ChangeRow {
        let mut source = RowSource::new("s", None, b"{}");
        source.dead_letter_id = dead_letter_id;
        ChangeRow {
            table_name: table.to_string(),
            table_value: HashMap::new(),
            primary_key: key.to_string(),
            action: RowAction::Upsert,
            index,
            source,
        }
    }

    #[test]
    fn keys_are_grouped_by_table() {
        let rows = [
            row("a", "1", 0, None),
            row("b", "1", 1, None),
            row("a", "2", 2, None),
        ];
        let keys = supersede_keys(&rows);
        assert_eq!(
            keys["a"],
            (vec!["1".to_string(), "2".to_string()], vec![None, None])
        );
        assert_eq!(keys["b"], (vec!["1".to_string()], vec![None]));
    }

    // Final review #3: replay bản cũ chỉ supersede dead-letter cũ hơn nó, không đụng bản mới hơn
    #[test]
    fn replayed_rows_only_supersede_older_dead_letters() {
        let rows = [row("a", "5", 0, Some(3)), row("a", "6", 1, Some(9))];
        let keys = supersede_keys(&rows);
        assert_eq!(
            keys["a"],
            (
                vec!["5".to_string(), "6".to_string()],
                vec![Some(3), Some(9)]
            )
        );
    }

    #[test]
    fn only_newest_replayed_row_per_key_counts() {
        let rows = [
            row("a", "1", 0, Some(10)), // bị bản id 12 thay thế
            row("a", "2", 1, Some(11)),
            row("a", "1", 2, Some(12)),
            row("b", "1", 3, Some(13)), // khác table nên không đụng "a"/"1"
            row("a", "3", 4, None),     // không phải replay
        ];
        let mut ids = latest_replay_ids(&rows);
        ids.sort();
        assert_eq!(ids, vec![11, 12, 13]);
    }

    fn rejected(item: &ChangeRow) -> Rejected<'_, ChangeRow> {
        Rejected {
            item,
            code: "23514".to_string(),
            message: "violates check".to_string(),
        }
    }

    // Bản cũ bị từ chối ở lần flush trước DDL, bản mới hơn cùng khóa ghi được sau DDL:
    // không được lưu bản cũ, nếu không replay sau này sẽ ghi đè dữ liệu mới
    #[test]
    fn only_newest_rejected_version_is_recorded() {
        let rows = [
            row("a", "5", 0, None), // bị từ chối, nhưng có bản mới hơn ở index 1
            row("a", "5", 1, None),
            row("a", "6", 2, None), // bị từ chối, là bản mới nhất của khóa 6
            row("b", "5", 3, None), // khác table
        ];
        let kept: Vec<i64> = newest_rejected(
            vec![rejected(&rows[0]), rejected(&rows[2]), rejected(&rows[3])],
            &rows,
        )
        .iter()
        .map(|r| r.item.index)
        .collect();
        assert_eq!(kept, vec![2, 3]);
    }
}
