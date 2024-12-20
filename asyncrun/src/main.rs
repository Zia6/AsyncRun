use asyncrun::asyncread::AsyncRead;
use asyncrun::asyncwrite::AsyncWrite;
use asyncrun::task::{Executor, GLOBAL_EXECUTOR};
use std::fs::OpenOptions;
use std::io;
use std::os::fd::AsRawFd;

fn main() -> io::Result<()> {
    // 使用 OpenOptions 来设置文件打开模式
    let file = match OpenOptions::new()
        .read(true)    // 打开文件进行读取
        .write(true)   // 打开文件进行写入
        .create(true)  // 如果文件不存在，创建文件
        .open("example.txt") {
        Ok(file) => file,
        Err(e) => {
            eprintln!("Failed to open file: {}", e);
            return Err(e);  // 直接返回错误
        }
    };

    // 获取文件描述符
    let fd = file.as_raw_fd();

    // 异步读取任务
    Executor::spawn(async move {
        let mut read_future = AsyncRead::new(fd, 1);
        match read_future.await {
            Ok(size) => println!("Read {} bytes", size),
            Err(err) => eprintln!("Read error: {}", err),
        }
    });

    // 异步写入任务
    Executor::spawn(async move {
        let write_future = AsyncWrite::new(fd, b"Hello, world!".to_vec());
        match write_future.await {
            Ok(size) => println!("Wrote {} bytes", size),
            Err(err) => eprintln!("Write error: {}", err),
        }
    });

    // 运行全局任务队列
    GLOBAL_EXECUTOR.run();

    Ok(())
}
