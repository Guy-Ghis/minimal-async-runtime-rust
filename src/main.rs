use std::{
    cell::RefCell,
    collections::{BinaryHeap, VecDeque},
    future::Future,
    pin::Pin,
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
    time::{Duration, Instant},
};
use std::rc::Rc;

thread_local! {
    static RUNTIME: RefCell<Option<MiniRuntime>> = RefCell::new(None);
}

pub struct MiniRuntime {
    tasks: VecDeque<Task>,
    timers: BinaryHeap<Timer>,
}

#[derive(Clone)]
struct Timer {
    when: Instant,
    task: Task,
}

impl PartialEq for Timer {
    fn eq(&self, other: &Self) -> bool {
        self.when.eq(&other.when)
    }
}

impl Eq for Timer {}

impl PartialOrd for Timer {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(other.when.cmp(&self.when))
    }
}

impl Ord for Timer {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.when.cmp(&self.when)
    }
}

#[derive(Clone)]
struct Task {
    future: Rc<RefCell<Pin<Box<dyn Future<Output = ()>>>>>,
}

impl Task {
    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        self.future.borrow_mut().as_mut().poll(cx)
    }
}

impl MiniRuntime {
    pub fn new() -> Self {
        Self {
            tasks: VecDeque::new(),
            timers: BinaryHeap::new(),
        }
    }

    pub fn block_on<F: Future<Output = ()> + 'static>(&mut self, future: F) {
        self.spawn(future);
        RUNTIME.with(|rt| *rt.borrow_mut() = Some(self.clone()));

        while !self.tasks.is_empty() || !self.timers.is_empty() {
            while let Some(mut task) = self.tasks.pop_front() {
                let waker = dummy_waker();
                let mut cx = Context::from_waker(&waker);
                if let Poll::Pending = task.poll(&mut cx) {
                    self.tasks.push_back(task);
                }
            }

            if let Some(timer) = self.timers.peek() {
                if timer.when <= Instant::now() {
                    let timer = self.timers.pop().unwrap();
                    self.tasks.push_back(timer.task);
                }
            }
        }
    }

    pub fn spawn<F: Future<Output = ()> + 'static>(&mut self, future: F) {
        self.tasks.push_back(Task {
            future: Rc::new(RefCell::new(Box::pin(future))),
        });
    }

    fn schedule_timer(&mut self, when: Instant, task: Task) {
        self.timers.push(Timer { when, task });
    }
}

impl Clone for MiniRuntime {
    fn clone(&self) -> Self {
        Self {
            tasks: self.tasks.clone(),
            timers: self.timers.clone(),
        }
    }
}

fn dummy_waker() -> Waker {
    fn noop(_: *const ()) {}
    const VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
    fn clone(_: *const ()) -> RawWaker {
        RawWaker::new(std::ptr::null(), &VTABLE)
    }

    let raw = RawWaker::new(std::ptr::null(), &VTABLE);
    unsafe { Waker::from_raw(raw) }
}

pub async fn sleep(duration: Duration) {
    struct SleepFuture {
        when: Instant,
    }

    impl Future for SleepFuture {
        type Output = ();

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            if Instant::now() >= self.when {
                Poll::Ready(())
            } else {
                let _waker = cx.waker().clone();
                let task = Task {
                    future: Rc::new(RefCell::new(Box::pin(Self { when: self.when }))),
                };
                RUNTIME.with(|rt| {
                    if let Some(rt) = &mut *rt.borrow_mut() {
                        rt.schedule_timer(self.when, task);
                    }
                });
                Poll::Pending
            }
        }
    }

    SleepFuture {
        when: Instant::now() + duration,
    }
    .await
}

pub async fn yield_now() {
    struct YieldNow(bool);

    impl Future for YieldNow {
        type Output = ();

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            if self.0 {
                Poll::Ready(())
            } else {
                self.0 = true;
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }
    }

    YieldNow(false).await;
}

#[macro_export]
macro_rules! mini_rt {
    (async fn $name:ident() $body:block) => {
        fn main() {
            let mut rt = $crate::MiniRuntime::new();
            rt.block_on(async $body);
        }
    };
}

#[macro_export]
macro_rules! join_all {
    ($($fut:expr),+ $(,)?) => {
        async {
            $(let _ = $fut.await;)+
        }
    };
}

fn main() {
    let mut rt = MiniRuntime::new();
    rt.block_on(async {
        println!("Main task starting...");
        let h1 = async {
            println!("Task 1 started");
            sleep(Duration::from_secs(1)).await;
            println!("Task 1 done");
        };

        let h2 = async {
            println!("Task 2 started");
            sleep(Duration::from_secs(2)).await;
            println!("Task 2 done");
        };

        join_all!(h1, h2).await;
    });
}
