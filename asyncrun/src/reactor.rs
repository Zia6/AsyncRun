use io_uring::{CompletionQueue, IoUring, SubmissionQueue};
use std::collections::{HashMap, HashSet};
use std::io;
use std::os::unix::io::RawFd;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::task::Waker;
use std::thread;
use std::time::Duration;

static NEXT_USER_DATA: AtomicU64 = AtomicU64::new(1);

// I/O 操作类型
pub enum IoOp {
    Read,
    Write,
}

// Reactor，用于管理异步 I/O
pub struct Reactor {
    uring: Mutex<IoUring>,               // io_uring 实例
    waiters: Mutex<HashMap<u64, Waker>>,  // 事件ID -> Waker 映射
    completed_events: Mutex<HashMap<u64, io::Result<usize>>>, // 完成的事件ID -> 结果
}

impl Reactor {
    // 创建新的 Reactor
    pub fn new() -> Self {
        let uring = IoUring::new(256).expect("Failed to create io_uring");
        Self {
            uring: Mutex::new(uring),
            waiters: Mutex::new(HashMap::new()),
            completed_events: Mutex::new(HashMap::new()),
        }
    }

    // 注册 I/O 请求
    pub fn register(&self, fd: RawFd, op: IoOp, buffer: &mut [u8], waker: Waker) -> u64 {
        let mut uring = self.uring.lock().unwrap();
        let mut sq = uring.submission();
        let event_id = NEXT_USER_DATA.fetch_add(1, Ordering::SeqCst); // 生成唯一 ID

        unsafe {
            let entry = match op {
                IoOp::Read => io_uring::opcode::Read::new(
                    io_uring::types::Fd(fd),
                    buffer.as_mut_ptr(),
                    buffer.len() as u32,
                )
                .build()
                .user_data(event_id),
                IoOp::Write => io_uring::opcode::Write::new(
                    io_uring::types::Fd(fd),
                    buffer.as_ptr(),
                    buffer.len() as u32,
                )
                .build()
                .user_data(event_id),
            };

            sq.push(&entry).expect("Failed to submit I/O operation");
        }

        drop(sq); // Drop the mutable borrow before calling submit
        uring.submit().expect("Failed to submit to io_uring");
        self.waiters.lock().unwrap().insert(event_id, waker);
        event_id
    }

    // 等待并处理完成的事件
    pub fn wait(&self) {
        let mut uring = self.uring.lock().unwrap();
        let cq = uring.completion();
        let mut waiters = self.waiters.lock().unwrap();

        // 检查完成队列
        for cqe in cq {
            println!("Event completed: user_data={}", cqe.result());

            // 将完成的事件结果记录在 completed_events 中
            let result = if cqe.result() < 0 {
                Err(io::Error::from_raw_os_error(cqe.result()))
            } else {
                Ok(cqe.result() as usize) // 记录实际处理的字节数
            };

            // 将完成事件的结果保存到 completed_events
            self.completed_events.lock().unwrap().insert(cqe.user_data(), result);

            // 唤醒等待的任务
            if let Some(waker) = waiters.remove(&cqe.user_data()) {
                waker.wake();
            } else {
                println!("No waker found for user_data={}", cqe.user_data());
            }
        }
    }

    // 检查事件是否已完成，返回操作结果
    pub fn is_event_completed(&self, event_id: u64) -> Option<io::Result<usize>> {
        let mut completed_events = self.completed_events.lock().unwrap();
        completed_events.remove(&event_id)
    }
}

// 全局 Reactor
lazy_static::lazy_static! {
    pub static ref REACTOR: Reactor = Reactor::new();
}
