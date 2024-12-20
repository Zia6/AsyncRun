use crate::reactor::{IoOp, REACTOR};
use std::future::Future;
use std::io;
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

pub struct AsyncRead {
    fd: RawFd,               // 文件描述符
    buffer: Vec<u8>,         // 读取缓冲区
    event_id: Option<u64>,   // 注册的事件ID
    completed: bool,         // 是否完成
}

impl AsyncRead {
    pub fn new(fd: RawFd, buffer_size: usize) -> Self {
        Self {
            fd,
            buffer: vec![0; buffer_size],
            event_id: None,
            completed: false,
        }
    }

    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }
}

impl Future for AsyncRead {
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        println!("Read event polled");
        if self.completed {
            println!("Read event completed");
            // 如果事件已完成，返回结果
            return Poll::Ready(Ok(self.buffer.len()));
        }

        // 如果事件未注册，则注册到 Reactor
        if self.event_id.is_none() {
            println!("Registering read event...");
            let waker = cx.waker().clone();
            let event_id = REACTOR.register(self.fd, IoOp::Read, &mut self.buffer, waker);
            self.event_id = Some(event_id);
            return Poll::Pending;
        }
         // 检查事件是否完成
         if REACTOR.is_event_completed(self.event_id.unwrap()) {
            self.completed = true;
            println!("Read event completed");
            return Poll::Ready(Ok(self.buffer.len()));
        }
        println!("111");
        // 等待完成
        Poll::Pending
    }
}
