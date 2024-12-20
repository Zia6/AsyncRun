use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::task::Waker;
use io_uring::{IoUring, SubmissionQueue, CompletionQueue};
use std::os::unix::io::RawFd;
use std::thread;
use std::time::Duration;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_USER_DATA: AtomicU64 = AtomicU64::new(1);
// I/O 操作类型
pub enum IoOp {
    Read,
    Write,
}
    // 注册 I/O 请求

// Reactor，用于管理异步 I/O
pub struct Reactor {
    uring: Mutex<IoUring>,                // io_uring 实例
    waiters: Mutex<HashMap<u64, Waker>>, // 事件ID -> Waker 映射
}

impl Reactor {
    // 创建新的 Reactor
    pub fn new() -> Self {
        let uring = IoUring::new(256).expect("Failed to create io_uring");
        Self {
            uring: Mutex::new(uring),
            waiters: Mutex::new(HashMap::new()),
        }
    }



pub fn register(
    &self,
    fd: RawFd,
    op: IoOp,
    buffer: &mut [u8],
    waker: Waker,
) -> u64 {
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
    
    

    // 等待事件完成
    pub fn wait(&self) {
        let mut uring = self.uring.lock().unwrap();
        let cq = uring.completion();
        let mut waiters = self.waiters.lock().unwrap();

        // 检查完成队列
        for cqe in cq {
            println!("Event completed: user_data={}", cqe.result());
            
            if let Some(waker) = waiters.remove(&cqe.user_data()) {
                waker.wake();
            }else {
                println!("No waker found for user_data={}", cqe.user_data());
            }
        }
    }
    pub fn is_event_completed(&self, event_id: u64) -> bool {
        // 检查完成队列是否包含该事件
        let mut uring = self.uring.lock().unwrap();
        let x = uring.completion().any(|cqe| cqe.user_data() == event_id); x
    }
}

// 全局 Reactor
lazy_static::lazy_static! {
    pub static ref REACTOR: Reactor = Reactor::new();
}
