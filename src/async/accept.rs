use crate::reactor::{IoOp, REACTOR};
use std::os::unix::io::{RawFd, AsRawFd};
use std::task::{Context, Poll};
use std::pin::Pin;
use std::future::Future;
use std::io::{self, ErrorKind};

/// 异步accept函数，直接调用
pub fn accept(fd: RawFd) -> AsyncAcceptFuture {
    AsyncAcceptFuture::new(fd)
}

pub struct AsyncAcceptFuture {
    fd: RawFd,
    event_id: Option<u64>,
    client_fd: Option<RawFd>, // 用来存储新的连接的文件描述符
}

impl AsyncAcceptFuture {
    pub fn new(fd: RawFd) -> Self {
        Self {
            fd,
            event_id: None,
            client_fd: None,
        }
    }
}

impl Future for AsyncAcceptFuture {
    type Output = io::Result<RawFd>; // 返回新的客户端连接的 fd

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // 第一次进入 poll，需要注册异步接收连接事件
        if self.event_id.is_none() {
            let waker = cx.waker().clone();
            // 向 reactor 注册异步accept事件
            let event_id = REACTOR.register(self.fd, IoOp::Accept, &mut [], waker);
            self.event_id = Some(event_id);
            return Poll::Pending;
        }

        // 检查事件是否完成
        if let Some(event_id) = self.event_id {
            if let Some(result) = REACTOR.is_event_completed(event_id) {
                match result {
                    Ok(client_fd) => {
                        self.client_fd = Some(client_fd as RawFd);
                        Poll::Ready(Ok(client_fd as RawFd))
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
