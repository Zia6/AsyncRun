use crate::accept;
use crate::reactor::REACTOR;
use std::future::Future;
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::task::{Context, Poll};
/// 异步 TcpListener，封装了 epoll_wait
pub struct AsyncTcpListener {
    fd: RawFd,
    event_id: Option<u64>,
}

impl AsyncTcpListener {
    /// 创建新的 TcpListener
    pub fn new(fd: RawFd) -> Self {
        let event_id = REACTOR.register_tcp(fd);
        Self {
            fd,
            event_id: Some(event_id),
        }
    }
    pub fn add_connection(&self, fd: RawFd) {
        let event_id = REACTOR.register_epoll(fd, self.event_id.unwrap());
        println!("New connection registered with fd: {}", fd);

        // 如果需要立即触发某些逻辑，可以注册 waker 或附加处理逻辑
        REACTOR.modify_waker(event_id, futures::task::noop_waker_ref().clone());
    }
    /// 异步等待新的连接并处理事件
    pub async fn accept<F>(&self, handle: F)
    where
        F: Fn(RawFd) + Send + 'static,
    {
        loop {
            // 等待事件触发
            self.wait().await;
            // 假设这里从 fd 中获取到新连接的 fd
            let events = REACTOR.get_epoll_events();
            for event in events {
                let fd = event.data;
                if fd == self.fd as u64 {
                    let newfd = accept(self.fd).await.unwrap();
                    self.add_connection(newfd);
                } else {
                    handle(fd as RawFd);
                }
            }
        }
    }

    /// 异步等待事件
    async fn wait(&self) {
        AsyncEpollWait {
            event_id: self.event_id.unwrap(),
        }
        .await;
    }
}

/// 异步封装 epoll_wait 的 Future
struct AsyncEpollWait {
    event_id: u64,
}

impl Future for AsyncEpollWait {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // 将 Waker 关联到该事件
        REACTOR.modify_waker(self.event_id, cx.waker().clone());
        // 检查事件是否完成
        if REACTOR.is_event_completed_epoll() > 0 {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}
