use crate::reactor::{IoOp, REACTOR};
use std::io::{self, Read};
use std::os::unix::io::RawFd;
use std::task::{Context, Poll};
use std::pin::Pin;
use std::future::Future;

/// 异步读取函数，直接调用
pub fn read(fd: RawFd, buffer: &mut [u8]) -> AsyncReadFuture {
    AsyncReadFuture::new(fd, buffer)
}

pub struct AsyncReadFuture<'a> {
    fd: RawFd,
    buffer: &'a mut [u8],
    event_id: Option<u64>,
}

impl<'a> AsyncReadFuture<'a> {
    pub fn new(fd: RawFd, buffer: &'a mut [u8]) -> Self {
        Self {
            fd,
            buffer,
            event_id: None,
        }
    }
}

impl<'a> Future for AsyncReadFuture<'a> {
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // 第一次进入 poll，需要注册异步读取事件
        if self.event_id.is_none() {
            let waker = cx.waker().clone();
            let event_id = REACTOR.register(self.fd, IoOp::Read, self.buffer, waker);
            self.event_id = Some(event_id);
            return Poll::Pending;
        }
        // 检查事件是否完成
        if let Some(event_id) = self.event_id {
            if let Some(result) = REACTOR.is_event_completed(event_id) {
                match result {
                    Ok(bytes_read) => {
                        Poll::Ready(Ok(bytes_read))
                    }
                    Err(e) => Poll::Ready(Err(e)),
                }
            } else {
                Poll::Pending
            }
        } else {
            Poll::Pending
        }
    }
}
