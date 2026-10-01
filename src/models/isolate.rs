use std::future::Future;

use crate::models::WriteError;

/// Một dòng bị DB đích từ chối vì lỗi dữ liệu.
#[derive(Debug)]
pub struct Rejected<'a, T> {
    pub item: &'a T,
    pub code: String,
    pub message: String,
}

/// Ghi `items` bằng `write`; gặp lỗi dữ liệu thì chia đôi và ghi lại từng nửa cho đến khi
/// còn đúng dòng lỗi. Với k dòng lỗi trong n dòng, số lần gọi `write` ≤ 1 + 2·k·⌈log₂n⌉.
///
/// Trả về các dòng bị từ chối; mọi dòng khác đã được ghi. Gặp lỗi tạm thời thì dừng ngay và
/// trả `Err` để cả batch được retry (mọi thao tác ghi đều idempotent nên chạy lại là an toàn).
/// Thứ tự giữa hai nửa không quan trọng vì mỗi khóa chính chỉ còn một bản.
pub async fn write_isolating<'a, T, F, Fut>(
    items: Vec<&'a T>,
    mut write: F,
) -> Result<Vec<Rejected<'a, T>>, String>
where
    F: FnMut(Vec<&'a T>) -> Fut,
    Fut: Future<Output = Result<(), WriteError>>,
{
    let mut rejected = Vec::new();
    let mut pending = vec![items];
    while let Some(chunk) = pending.pop() {
        if chunk.is_empty() {
            continue;
        }
        match write(chunk.clone()).await {
            Ok(()) => {}
            Err(WriteError::Transient(message)) => return Err(message),
            Err(WriteError::Data { code, message }) if chunk.len() == 1 => {
                rejected.push(Rejected {
                    item: chunk[0],
                    code,
                    message,
                });
            }
            Err(WriteError::Data { .. }) => {
                let mut left = chunk;
                let right = left.split_off(left.len() / 2);
                pending.push(right);
                pending.push(left);
            }
        }
    }
    Ok(rejected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, collections::HashSet};

    /// DB giả: chunk chứa phần tử thuộc `bad` thì lỗi dữ liệu, ngược lại ghi thành công.
    /// `transient_on_call`: lần gọi thứ n (tính từ 1) trả lỗi tạm thời.
    struct FakeDb {
        bad: HashSet<i32>,
        transient_on_call: Option<usize>,
        calls: RefCell<usize>,
        written: RefCell<Vec<i32>>,
    }

    impl FakeDb {
        fn new(bad: &[i32]) -> Self {
            FakeDb {
                bad: bad.iter().copied().collect(),
                transient_on_call: None,
                calls: RefCell::new(0),
                written: RefCell::new(Vec::new()),
            }
        }

        fn write(&self, chunk: Vec<&i32>) -> std::future::Ready<Result<(), WriteError>> {
            let call = {
                let mut calls = self.calls.borrow_mut();
                *calls += 1;
                *calls
            };
            let result = if Some(call) == self.transient_on_call {
                Err(WriteError::Transient("connection reset".to_string()))
            } else if let Some(bad) = chunk.iter().find(|item| self.bad.contains(item)) {
                Err(WriteError::Data {
                    code: "23514".to_string(),
                    message: format!("row {} violates check", bad),
                })
            } else {
                self.written.borrow_mut().extend(chunk.iter().copied());
                Ok(())
            };
            std::future::ready(result)
        }

        fn calls(&self) -> usize {
            *self.calls.borrow()
        }

        fn written_sorted(&self) -> Vec<i32> {
            let mut written = self.written.borrow().clone();
            written.sort();
            written
        }
    }

    fn rejected_sorted(rejected: &[Rejected<'_, i32>]) -> Vec<i32> {
        let mut items: Vec<i32> = rejected.iter().map(|r| *r.item).collect();
        items.sort();
        items
    }

    #[tokio::test]
    async fn no_errors_writes_whole_batch_once() {
        let db = FakeDb::new(&[]);
        let data: Vec<i32> = (1..=100).collect();
        let rejected = write_isolating(data.iter().collect(), |c| db.write(c))
            .await
            .unwrap();
        assert!(rejected.is_empty());
        assert_eq!(db.calls(), 1);
        assert_eq!(db.written_sorted(), data);
    }

    #[tokio::test]
    async fn empty_input_does_not_call_writer() {
        let db = FakeDb::new(&[]);
        let rejected = write_isolating(Vec::<&i32>::new(), |c| db.write(c))
            .await
            .unwrap();
        assert!(rejected.is_empty());
        assert_eq!(db.calls(), 0);
    }

    #[tokio::test]
    async fn single_bad_row_in_1000_is_found_with_few_statements() {
        let db = FakeDb::new(&[537]);
        let data: Vec<i32> = (1..=1000).collect();
        let rejected = write_isolating(data.iter().collect(), |c| db.write(c))
            .await
            .unwrap();
        assert_eq!(rejected_sorted(&rejected), vec![537]);
        assert_eq!(rejected[0].code, "23514");
        assert_eq!(rejected[0].message, "row 537 violates check");
        // 1 + 2·k·⌈log₂n⌉ với k = 1, n = 1000
        assert!(db.calls() <= 21, "calls = {}", db.calls());
        let expected: Vec<i32> = data.into_iter().filter(|x| *x != 537).collect();
        assert_eq!(db.written_sorted(), expected);
    }

    #[tokio::test]
    async fn several_bad_rows_are_all_isolated() {
        let db = FakeDb::new(&[1, 50, 51, 100]);
        let data: Vec<i32> = (1..=100).collect();
        let rejected = write_isolating(data.iter().collect(), |c| db.write(c))
            .await
            .unwrap();
        assert_eq!(rejected_sorted(&rejected), vec![1, 50, 51, 100]);
        let expected: Vec<i32> = data
            .into_iter()
            .filter(|x| ![1, 50, 51, 100].contains(x))
            .collect();
        assert_eq!(db.written_sorted(), expected);
    }

    // Review Focus #3: mọi dòng đều lỗi
    #[tokio::test]
    async fn every_row_bad_returns_every_row() {
        let data: Vec<i32> = (1..=8).collect();
        let db = FakeDb::new(&data);
        let rejected = write_isolating(data.iter().collect(), |c| db.write(c))
            .await
            .unwrap();
        assert_eq!(rejected_sorted(&rejected), data);
        assert!(db.written_sorted().is_empty());
    }

    #[tokio::test]
    async fn transient_error_mid_bisection_aborts() {
        let mut db = FakeDb::new(&[7]);
        db.transient_on_call = Some(2);
        let data: Vec<i32> = (1..=16).collect();
        let result = write_isolating(data.iter().collect(), |c| db.write(c)).await;
        assert_eq!(result.unwrap_err(), "connection reset");
        assert_eq!(db.calls(), 2);
    }
}
