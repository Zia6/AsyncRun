use crate::reactor::{IoOp, REACTOR};
use std::future::Future;
use std::io;
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

pub struct AsyncWrite {
    fd: RawFd,               // 文件描述符
    buffer: Vec<u8>,         // 写入缓冲区
    event_id: Option<u64>,   // 注册的事件ID
    completed: bool,         // 是否完成
}

impl AsyncWrite {
    pub fn new(fd: RawFd, data: Vec<u8>) -> Self {
        Self {
            fd,
            buffer: data,
            event_id: None,
            completed: false,
        }
    }
}

impl Future for AsyncWrite {
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // print!("Write event completed");
        if self.completed {
            // 如果事件已完成，返回结果
            return Poll::Ready(Ok(self.buffer.len()));
        }
        // 如果事件未注册，则注册到 Reactor
        if self.event_id.is_none() {
            let waker = cx.waker().clone();
            let event_id = REACTOR.register(self.fd, IoOp::Write, &mut self.buffer, waker);
            self.event_id = Some(event_id);
            return Poll::Pending;
        }

        if REACTOR.is_event_completed(self.event_id.unwrap()) {
            print!("Write event completed");
            self.completed = true;
            return Poll::Ready(Ok(self.buffer.len()));
        }
        // 等待完成
        Poll::Pending
    }
}
