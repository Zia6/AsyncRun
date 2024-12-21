
# Async IO with io_uring in Rust

This project provides an asynchronous I/O framework in Rust using the `io_uring` library. It supports performing I/O operations like `read` and `write` asynchronously, utilizing a reactor pattern to manage events efficiently.

## Features

- **Async I/O Operations**: Perform non-blocking read and write operations asynchronously.
- **Reactor Pattern**: Efficiently manage multiple I/O events using a global reactor that integrates with `io_uring`.
- **Lightweight Executor**: A simple executor for running asynchronous tasks.
- **Efficient Event Handling**: Uses `io_uring` for optimized event submission and completion, allowing high-performance I/O operations.

## Installation

Add the following to your `Cargo.toml`:

```toml
[dependencies]
futures = "0.3"
tokio = { version = "1", features = ["full"] }
io-uring = "0.7"
```

## Usage

The framework provides functions to perform `read` and `write` operations asynchronously. Here's an example of how to use the `read` and `write` functions:

### Asynchronous Read Example

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

### Asynchronous Write Example

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

## How it Works

### Reactor

The `Reactor` is responsible for registering and managing I/O events using `io_uring`. It provides a mechanism for submitting I/O operations (like read and write) and waiting for them to complete. Once an event completes, the reactor notifies the appropriate `Waker` to resume the corresponding future.

The `Reactor` manages the I/O submission queue (`SubmissionQueue`), the completion queue (`CompletionQueue`), and the event handlers. It utilizes `io_uring` for efficient event-driven programming.

```rust
pub struct Reactor {
    uring: Mutex<IoUring>,
    waiters: Mutex<HashMap<u64, Waker>>,
    completed_events: Mutex<HashMap<u64, io::Result<usize>>>,
}
```

### Executor

The `Executor` is a simple task executor that allows you to spawn asynchronous tasks. It works by managing the futures and polling them until completion.

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

- The `AsyncReadFuture` is used to read data asynchronously. It registers a read operation with the `Reactor` and awaits its completion.
- The `AsyncWriteFuture` is used to write data asynchronously. Similar to `AsyncReadFuture`, it registers a write operation and waits for it to complete.

Both of these futures leverage `poll` to handle the registration of the I/O operations and their subsequent completion.

### Registering Events

To register a read or write event, you can use the `register` method on the `Reactor`. This will initiate the I/O operation and provide a unique `event_id` that you can use to track the completion of the event.

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

### Waiting for Completion

Once the I/O operation is completed, you can check the completion status using the `wait` method. This method will process the completion queue and notify the corresponding `Waker` to resume the associated future.

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

## Contributing

Feel free to open issues and pull requests if you'd like to contribute to this project!

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
