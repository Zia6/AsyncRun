use io_uring::{CompletionQueue, IoUring, SubmissionQueue};
use std::collections::{HashMap, HashSet};
use std::io;
use std::os::unix::io::RawFd;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::task::Waker;
use std::thread;
use std::time::Duration;

static NEXT_USER_DATA: AtomicU64 = AtomicU64::new(1);

// I/O 操作类型
pub enum IoOp {
    Read,
    Write,
    Accept,  // 增加 Accept 类型
}

// Reactor，用于管理异步 I/O
pub struct Reactor {
    uring: Mutex<IoUring>,               // io_uring 实例
    waiters: Mutex<HashMap<u64, Waker>>,  // 事件ID -> Waker 映射
    completed_events: Mutex<HashMap<u64, io::Result<usize>>>, // 完成的事件ID -> 结果
    client: Mutex<HashMap<i32,u64>>,  // 用于等待的事件ID
    epoll_fd: RawFd,
    epoll_events: RwLock<Vec<epoll::Event>>, // 使用 RwLock 包裹 epoll_events,不然无法修改
}

impl Reactor {
    // 创建新的 Reactor
    pub fn new() -> Self {
        let epoll_fd = epoll::create(true).expect("Failed to create epoll instance");
        let uring = IoUring::new(256).expect("Failed to create io_uring");
        Self {
            epoll_fd: epoll_fd,
            uring: Mutex::new(uring),
            waiters: Mutex::new(HashMap::new()),
            completed_events: Mutex::new(HashMap::new()),
            client: Mutex::new(HashMap::new()),
            epoll_events: Vec::with_capacity(1024).into(),
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
                IoOp::Accept => io_uring::opcode::Accept::new(
                    io_uring::types::Fd(fd),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
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
        let mut epoll_events = self.epoll_events.write().unwrap();
        let mut client = self.client.lock().unwrap();
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
        let _ = epoll::wait(self.epoll_fd, 1000, &mut epoll_events);
        for event in epoll_events.iter() {
            let fd = event.data as i32;
            if let Some(waker) = waiters.get(client.get(&fd).unwrap()) {
                waker.clone().wake();
            } else {
                println!("No waker found for user_data={}", fd);
            }

        }
    }

    // 检查事件是否已完成，返回操作结果
    pub fn is_event_completed(&self, event_id: u64) -> Option<io::Result<usize>> {
        let mut completed_events = self.completed_events.lock().unwrap();
        completed_events.remove(&event_id)
    }

    pub fn register_tcp(&self, fd: RawFd) -> u64 {
        let event_id = NEXT_USER_DATA.fetch_add(1, Ordering::SeqCst); // 生成唯一 ID
        let mut client = self.client.lock().unwrap();
        client.insert(fd,event_id);
        let _ = epoll::ctl(
            self.epoll_fd,
            epoll::ControlOptions::EPOLL_CTL_ADD,
            fd,
            epoll::Event::new(epoll::Events::EPOLLIN, event_id),
        );
        event_id
    }
    pub fn modify_waker(&self, event_id: u64,waker: Waker) {
        let mut waiters = self.waiters.lock().unwrap();
        waiters.insert(event_id, waker);
    }

    pub fn register_epoll(&self, fd: RawFd,event_id: u64) -> u64 {
        let mut client = self.client.lock().unwrap();
        client.insert(fd,event_id);
        let _ = epoll::ctl(
            self.epoll_fd,
            epoll::ControlOptions::EPOLL_CTL_ADD,
            fd,
            epoll::Event::new(epoll::Events::EPOLLIN, event_id),
        );
        event_id
    }

    //返回当前epoll_events的len()
    pub fn is_event_completed_epoll(&self) -> usize {
        let epoll_events = self.epoll_events.read().unwrap();
        epoll_events.len()
    }

    //返回当前epoll_events的clone()
    pub fn get_epoll_events(&self) -> Vec<epoll::Event> {
        let epoll_events = self.epoll_events.read().unwrap();
        epoll_events.clone()
    }

    // 将event清空
    pub fn clear_epoll_events(&self) {
        let mut epoll_events = self.epoll_events.write().unwrap();
        epoll_events.clear();
    }
}

// 全局 Reactor
lazy_static::lazy_static! {
    pub static ref REACTOR: Reactor = Reactor::new();
}
