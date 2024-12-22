use futures::task::{waker_ref, ArcWake};
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use crate::reactor::REACTOR;

// 全局 Executor
lazy_static::lazy_static! {
    pub static ref GLOBAL_EXECUTOR: Executor = Executor::new();
}

// 任务结构
struct Task {
    future: Mutex<Pin<Box<dyn Future<Output = ()> + Send>>>, // 固定的 Future
}

impl Task {
    // 创建一个新的任务
    fn new(future: impl Future<Output = ()> + Send + 'static) -> Arc<Self> {
        Arc::new(Task {
            future: Mutex::new(Box::pin(future)),
        })
    }

    // 运行任务，返回是否已完成
    fn poll(self: &Arc<Self>) -> bool {
        let waker = waker_ref(self);
        let mut context = Context::from_waker(&*waker);
        let mut future = self.future.lock().unwrap();

        // 调用 poll 并检查任务状态
        match future.as_mut().poll(&mut context) {
            Poll::Pending => false,  // 未完成
            Poll::Ready(()) => true, // 已完成
        }
    }
}

// 实现 ArcWake，为 `Task` 创建自定义 Waker
impl ArcWake for Task {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        // println!("Task woke up!");
        GLOBAL_EXECUTOR.spawn_task(arc_self.clone());
    }
}

// 执行器结构
pub struct Executor {
    queue: Mutex<VecDeque<Arc<Task>>>, // 任务队列
}

impl Executor {
    // 创建新的 Executor
    fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
        }
    }

    // 添加一个 Future 到任务队列
    pub fn spawn(future: impl Future<Output = ()> + Send + 'static) {
        // println!("push");
        GLOBAL_EXECUTOR.spawn_task(Task::new(future));
    }

    // 将 Task 直接加入任务队列
    fn spawn_task(&self, task: Arc<Task>) {
        // println!("push");
        self.queue.lock().unwrap().push_back(task);
    }

    // 运行任务队列
    pub fn run(&self) {

        loop {
            // println!("Running executor");
            while let Some(task) = self.queue.lock().unwrap().pop_front() {
                // println!("Running task");
                let _ = task.poll();
            }
            REACTOR.wait();
        }
    }
}
