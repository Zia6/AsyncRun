
# Rust下的异步IO

这个项目使用 Rust 提供了基于`io_uring`的一个异步 I/O 框架。它支持执行 I/O 操作，例如`read`和`write`异步地，利用executor模式来有效地管理事件。

## 特征

- **异步 I/O 操作**：异步执行非阻塞读写操作。
- **反应器模式**：使用与“io_uring”集成的全局反应器有效管理多个 I/O 事件。
- **轻量级执行器**：用于运行异步任务的简单执行器。
- **高效的事件处理**：使用 `io_uring` 来优化事件提交和完成，从而实现高性能 I/O 操作。

## 安装
将以下内容添加到您的“Cargo.toml”中：

```toml
[dependencies]
futures = "0.3"
tokio = { version = "1", features = ["full"] }
io-uring = "0.7"
```

## 用法

该框架提供了异步执行“读”和“写”操作的函数。以下是如何使用“read”和“write”函数的示例：
### 异步读取示例

```rust
use std::os::unix::io::RawFd;
use tokio::fs::File;
use tokio::io::{self, AsyncReadExt};

#[tokio::main]
async fn main() -> io::Result<()> {
    let file = File::open("example.txt").await?;
    let mut buffer = vec![0; 1024];

    let bytes_read = read(file.as_raw_fd(), &mut buffer).await?;

    println!("Read {} bytes: {:?}", bytes_read, buffer);

    Ok(())
}
```

### 异步写入示例

```rust
use std::os::unix::io::RawFd;

#[tokio::main]
async fn main() -> io::Result<()> {
    let file = File::create("output.txt").await?;

    let bytes_written = write(file.as_raw_fd(), b"Hello, world!").await?;

    println!("Wrote {} bytes", bytes_written);

    Ok(())
}
```

## 它是如何工作的

### Reactor

Reactor` 负责使用 `io_uring` 注册和管理 I/O 事件。它提供了一种提交 I/O 操作（如读取和写入）并等待它们完成的机制。一旦事件完成，反应器就会通知相应的“Waker”以恢复相应的未来。

`Reactor` 管理 I/O 提交队列 (`SubmissionQueue`)、完成队列 (`CompletionQueue`) 和事件处理程序。它利用“io_uring”进行高效的事件驱动编程。

```rust
pub struct Reactor {
    uring: Mutex<IoUring>,
    waiters: Mutex<HashMap<u64, Waker>>,
    completed_events: Mutex<HashMap<u64, io::Result<usize>>>,
}
```

### Executor

`Executor` 是一个简单的任务执行器，允许您生成异步任务。它的工作原理是管理 future 并轮询它们直到完成。

```rust
pub struct Executor {
    tasks: Vec<Box<dyn Future<Output = ()> + Send>>,
}

impl Executor {
    pub fn new() -> Self {
        Self { tasks: Vec::new() }
    }

    pub fn spawn<F>(&mut self, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.tasks.push(Box::new(future));
    }

    pub fn run(&mut self) {
        while !self.tasks.is_empty() {
            for i in 0..self.tasks.len() {
                let mut task = self.tasks.remove(i);
                let waker = futures::task::noop_waker();
                let mut cx = Context::from_waker(&waker);
                let _ = Pin::new(&mut task).poll(&mut cx);
            }
        }
    }
}
```

### `AsyncReadFuture` and `AsyncWriteFuture`

-`AsyncReadFuture` 用于异步读取数据。它向“Reactor”注册一个读操作并等待其完成。
-`AsyncWriteFuture` 用于异步写入数据。与`AsyncReadFuture`类似，它注册一个写操作并等待它完成。

Both of these futures leverage `poll` to handle the registration of the I/O operations and their subsequent completion.

### 注册事件

要注册读或写事件，您可以使用“Reactor”上的“register”方法。这将启动 I/O 操作并提供一个唯一的“event_id”，您可以使用它来跟踪事件的完成情况。
```rust
pub fn register(&self, fd: RawFd, op: IoOp, buffer: &mut [u8], waker: Waker) -> u64 {
    let mut uring = self.uring.lock().unwrap();
    let mut sq = uring.submission();
    let event_id = NEXT_USER_DATA.fetch_add(1, Ordering::SeqCst); // Generate unique ID

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

    uring.submit().expect("Failed to submit to io_uring");
    self.waiters.lock().unwrap().insert(event_id, waker);
    event_id
}
```

### 等待完成

一旦 I/O 操作完成，您可以使用 wait 方法检查完成状态。该方法将处理完成队列并通知相应的“Waker”恢复关联的 future。

```rust
pub fn wait(&self) {
    let mut uring = self.uring.lock().unwrap();
    let cq = uring.completion();
    let mut waiters = self.waiters.lock().unwrap();

    for cqe in cq {
        println!("Event completed: user_data={}", cqe.result());

        if let Some(waker) = waiters.remove(&cqe.user_data()) {
            self.completed_events
                .lock()
                .unwrap()
                .insert(cqe.user_data());
            waker.wake();
        } else {
            println!("No waker found for user_data={}", cqe.user_data());
        }
    }
}
```
