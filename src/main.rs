use asyncrun::read;
use asyncrun::task::{Executor, GLOBAL_EXECUTOR};
use asyncrun::write;
use std::fs::OpenOptions;
use std::io;
use std::os::fd::AsRawFd;

fn main() -> io::Result<()> {
    // 使用 OpenOptions 来设置文件打开模式
    let file = match OpenOptions::new()
        .read(true) // 打开文件进行读取
        .write(true) // 打开文件进行写入
        .create(true) // 如果文件不存在，创建文件
        .open("example.txt")
    {
        Ok(file) => file,
        Err(e) => {
            eprintln!("Failed to open file: {}", e);
            return Err(e); // 直接返回错误
        }
    };

    // 获取文件描述符
    let fd = file.as_raw_fd();
    let mut cnt = 0;
    while (cnt < 10) {
        // 异步读取任务
        Executor::spawn(async move {
            let mut buffer = [0; 1024];
            // 异步读取操作
            match read(fd, &mut buffer).await {
                Ok(bytes_read) => {
                    // 打印读取的字节数
                    println!("Read {} bytes", bytes_read);

                    // 打印字节数组内容
                    // 直接用 `String::from_utf8_lossy` 来尝试将字节转为字符串
                    let output = String::from_utf8_lossy(&buffer[..bytes_read]);
                    println!("Read as string: {}", output);
                }
                Err(err) => {
                    eprintln!("Read error: {}", err);
                }
            }
        });
        cnt += 1;
    }
    while (cnt < 20) {
        // 异步写入任务
        Executor::spawn(async move {
            let write_future = write(fd, b"Hello, world!".to_vec().as_mut_slice());

            match write_future.await {
                Ok(size) => {
                    println!("Wrote {} bytes", size);
                }
                Err(err) => {
                    eprintln!("Write error: {}", err);
                }
            }
        });
        cnt += 1;
    }

    // 运行全局任务队列
    GLOBAL_EXECUTOR.run();
    Ok(())
}
