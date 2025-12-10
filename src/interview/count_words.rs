// "实现一个多线程单词统计器 `parallel_word_count`，给定文本列表与线程数量 `workers`，将文本均衡分配给多个工作线程并统计单词出现次数。使用标准库 `std::thread` 与 `std::sync::mpsc`（或 `crossbeam` channel）实现生产者-消费者模式，最终合并各线程结果。

// 1. 函数签名示例：
//    ```rust
//    pub fn parallel_word_count(texts: Vec<String>, workers: usize) -> Result<HashMap<String, usize>, WordCountError>
//    ```
// 2. 约束：
//    - `workers` 必须 > 0，否则返回 `WordCountError::InvalidWorkerCount`。
//    - 文本按照索引均匀分配给各线程（可轮询或按块分割）。
//    - 每个线程独立统计单词频率后发送给主线程；主线程负责合并。
//    - 单词定义：按空白字符拆分，转换为全小写；忽略空单词。
//    - 使用 `HashMap<String, usize>` 表示频率；合并时注意整数溢出（可假设合理范围）。
// 3. 考虑线程 panic 或 channel 关闭等异常，返回 `WordCountError::ThreadFailure`。
// 4. 为简单起见，可使用 `join` 等待线程结束；若任何线程结果传输失败，应及时返回错误。
// 5. 编写单元测试覆盖：
//    - 单线程与多线程结果一致
//    - 文本数量小于、等于、大于线程数
//    - 处理空字符串、大小写混合
//    - 非法 `workers` 参数

// Implement `parallel_word_count`, a multithreaded word-frequency counter. Given a list of texts and a worker count, distribute texts across threads, count word occurrences, and merge the results. Use `std::thread` plus channels (`std::sync::mpsc` or `crossbeam`) to coordinate workers.

// 1. Example signature:
//    ```rust
//    pub fn parallel_word_count(texts: Vec<String>, workers: usize) -> Result<HashMap<String, usize>, WordCountError>
//    ```
// 2. Constraints:
//    - `workers` must be > 0; otherwise return `WordCountError::InvalidWorkerCount`.
//    - Assign texts evenly (round robin or chunking) to each worker.
//    - Each worker counts words independently then sends its `HashMap` to the main thread for aggregation.
//    - Words are split by whitespace, lowercased, and empty strings skipped.
//    - Merge results carefully, assuming counts fit within `usize`.
// 3. Handle thread panics or channel errors by returning `WordCountError::ThreadFailure`.
// 4. Join threads at the end; if any worker fails to send results, propagate the error.
// 5. Unit tests should cover:
//    - Single-thread vs multi-thread consistency
//    - Fewer/more texts than workers
//    - Empty strings, mixed case inputs
//    - Invalid worker count

// ```rust
// let texts = vec![
//     ""Rust is great"".to_string(),
//     ""rust rust fearless"".to_string(),
// ]
// let counts = parallel_word_count(texts, 2).unwrap();
// assert_eq!(counts.get(""rust""), Some(&3));
// assert_eq!(counts.get(""is""), Some(&1));
// ```"

use std::collections::HashMap;
use std::sync::mpsc::{self, Sender};
use std::thread;
use tracing::info;

enum Event {
    IAmReady(Sender<Event>),
    ToProcess(String),
    Stop,
    Report {
        who: Sender<Event>,
        count_result: HashMap<String, usize>,
    },
}

// Define the required error types
#[derive(Debug)]
pub enum WordCountError {
    InvalidWorkerCount,
    ThreadFailure,
}

impl std::fmt::Display for WordCountError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WordCountError::InvalidWorkerCount => write!(f, "Worker count must be greater than 0"),
            WordCountError::ThreadFailure => write!(f, "A worker thread failed or panicked"),
        }
    }
}

impl std::error::Error for WordCountError {}

fn assign_work_if_available(
    worker: &Sender<Event>,
    texts: &mut Vec<String>,
) -> std::result::Result<(), WordCountError> {
    match texts.pop() {
        Some(todo) => {
            worker
                .send(Event::ToProcess(todo))
                .map_err(|_| WordCountError::ThreadFailure)?;
        }
        None => {
            worker
                .send(Event::Stop)
                .map_err(|_| WordCountError::ThreadFailure)?;
        }
    }
    Ok(())
}

pub fn run() {
    let texts = vec![
        "Rust is great".to_string(),
        "rust rust fearless".to_string(),
        "Rust is great".to_string(),
        "rust rust fearless".to_string(),
        "Rust is great".to_string(),
        "rust rust fearless".to_string(),
        "Rust is great".to_string(),
        "rust rust fearless".to_string(),
        "Rust is great".to_string(),
        "rust rust fearless".to_string(),
        "Rust is great".to_string(),
        "Rust is great".to_string(),
        "rust rust fearless".to_string(),
        "Rust is great".to_string(),
        "rust rust fearless".to_string(),
        "Rust is great".to_string(),
        "rust rust fearless".to_string(),
        "Rust is great".to_string(),
        "rust rust fearless".to_string(),
        "rust rust fearless".to_string(),
    ];
    let counts = parallel_word_count(texts, 2).unwrap();
    assert_eq!(counts.get("rust"), Some(&30));
    assert_eq!(counts.get("is"), Some(&10));
}

pub fn parallel_word_count(
    texts: Vec<String>,
    workers: usize,
) -> std::result::Result<HashMap<String, usize>, WordCountError> {
    if workers == 0 {
        return Err(WordCountError::InvalidWorkerCount);
    }

    let mut result: HashMap<String, usize> = HashMap::new();
    let mut texts = texts;
    let total_todo = texts.len();

    if total_todo == 0 {
        return Ok(result);
    }

    let (manager_sender, manager_receiver) = mpsc::channel::<Event>();

    // Start worker threads
    for _i in 0..workers {
        let (sender, receiver) = mpsc::channel::<Event>();
        let manager_sender_clone = manager_sender.clone();

        thread::spawn(move || {
            // Report that this worker is ready
            if manager_sender_clone
                .send(Event::IAmReady(sender.clone()))
                .is_err()
            {
                return; // Manager thread has likely ended
            }

            // Process tasks in a loop
            while let Ok(msg) = receiver.recv() {
                match msg {
                    Event::ToProcess(text) => {
                        info!("Worker processing: {}", text);

                        // Count words in the text
                        let mut local_result: HashMap<String, usize> = HashMap::new();
                        for word in text.split_whitespace() {
                            let cleaned_word = word.to_ascii_lowercase();
                            if !cleaned_word.is_empty() {
                                *local_result.entry(cleaned_word).or_insert(0) += 1;
                            }
                        }

                        // Report results back to manager
                        if manager_sender_clone
                            .send(Event::Report {
                                who: sender.clone(),
                                count_result: local_result,
                            })
                            .is_err()
                        {
                            return; // Manager thread has ended
                        }
                    }
                    Event::Stop => break, // Exit the loop when told to stop
                    _ => {
                        // Unexpected message type
                        return;
                    }
                }
            }
        });
    }

    // Manage work assignment and result collection
    let mut assigned_count = 0;
    let mut completed_count = 0;
    let mut worker_queue = Vec::new(); // Queue of available workers

    loop {
        match manager_receiver.recv() {
            Ok(msg) => match msg {
                Event::IAmReady(worker) => {
                    // A worker is ready to receive work
                    worker_queue.push(worker);
                }
                Event::Report {
                    who: worker,
                    count_result,
                } => {
                    // Merge results
                    for (word, count) in count_result {
                        *result.entry(word).or_insert(0) += count;
                    }
                    completed_count += 1;

                    if completed_count == total_todo {
                        break; // All tasks completed
                    }

                    // Give this worker more work if available
                    if !texts.is_empty() {
                        assign_work_if_available(&worker, &mut texts)?;
                        assigned_count += 1;
                    }
                }
                _ => {
                    // Unexpected message
                    return Err(WordCountError::ThreadFailure);
                }
            },
            Err(_) => {
                // Channel closed - likely because a thread panicked
                return Err(WordCountError::ThreadFailure);
            }
        }

        // Assign new work to available workers
        while !worker_queue.is_empty() && !texts.is_empty() {
            if let Some(worker) = worker_queue.pop() {
                assign_work_if_available(&worker, &mut texts)?;
                assigned_count += 1;
            }
        }

        // Exit if all work is assigned and we're waiting for it to complete
        if assigned_count == total_todo && completed_count == assigned_count {
            break;
        }
    }

    Ok(result)
}
