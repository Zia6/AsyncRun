use crate::reactor::{IoOp, REACTOR};
use std::io::{self, Write};
use std::os::unix::io::RawFd;
use std::task::{Context, Poll};
use std::pin::Pin;
use std::future::Future;

/// 异步写入函数，直接调用
pub fn write(fd: RawFd, buffer: &mut [u8]) -> AsyncWriteFuture {
    AsyncWriteFuture::new(fd, buffer)
}

pub struct AsyncWriteFuture {
    fd: RawFd,
    buffer: Vec<u8>,   // 使用 Vec<u8> 方便存储缓冲区
    event_id: Option<u64>,
}

impl AsyncWriteFuture {
    pub fn new(fd: RawFd, buffer: &mut [u8]) -> Self {
        Self {
            fd,
            buffer: buffer.to_vec(),
            event_id: None,
        }
    }
}

impl Future for AsyncWriteFuture {
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // 如果没有注册事件，则注册事件
        if self.event_id.is_none() {
            // println!("异步写开始");
            let waker = cx.waker().clone();
            let event_id = REACTOR.register(self.fd, IoOp::Write, &mut self.buffer, waker);
            self.event_id = Some(event_id);
            return Poll::Pending;
        }
    
        // 如果事件已经完成，返回结果
        if let Some(event_id) = self.event_id {
            if let Some(result) = REACTOR.is_event_completed(event_id) {
                match result {
                    Ok(bytes_written) => {
                        // 返回实际写入的字节数
                        return Poll::Ready(Ok(bytes_written));
                    }
                    Err(e) => {
                        // 如果写入操作失败，返回错误
                        return Poll::Ready(Err(e));
                    }
                }
            }
        }
        Poll::Pending
    }
    
}
