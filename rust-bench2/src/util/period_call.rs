
use std::{time::Duration, sync::{atomic::{Ordering, AtomicBool}, Arc}, pin::Pin, task::Poll};

use futures::{task::AtomicWaker, FutureExt};

use super::{async_rt::async_tcp::VRuntime};

pub trait PeriodCall {
    fn next(&mut self) -> Duration;
    fn call(&mut self, completed: bool);
}


pub struct PeriodCallGuard<RT> 
where
    RT: VRuntime,
{
    inner: Arc<PeriodCallGuardInner>,
    task: Option<RT::TaskHandle<()>>,
}

impl<RT> Drop for PeriodCallGuard<RT> 
where
    RT: VRuntime,
{
    fn drop(&mut self) { 
        self.inner.exit_req.store(true, ORDERING);
        self.inner.task_waker.wake();
    }
}

impl<RT> PeriodCallGuard<RT> 
where
    RT: VRuntime,
{
    pub fn into_task(mut self) -> Option<RT::TaskHandle<()>> {
        self.task.take()
    }
}


struct PeriodCallGuardInner {
    exit_req: AtomicBool,
    task_waker: AtomicWaker,
}

impl std::future::Future for &PeriodCallGuardInner {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {

        if self.exit_req.load(ORDERING) {
            return Poll::Ready(());
        }

        self.task_waker.register(cx.waker());

        if self.exit_req.load(ORDERING) {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}


pub fn period_call<RT, C>(mut job: C) -> PeriodCallGuard<RT>
where
    RT: VRuntime,
    C: PeriodCall + Send + 'static,
{
    let inner0 = Arc::new(PeriodCallGuardInner { 
        exit_req: AtomicBool::new(false),
        task_waker: AtomicWaker::new(),
    });

    let inner = inner0.clone();
    let task = RT::spawn(async move {
        loop {
            let duration = job.next();
            if duration.is_zero() {
                job.call(false);
            } else {
                futures::select! {
                    _r = RT::async_sleep(duration).fuse() => {
                        job.call(false);
                    },
                    _r = inner.as_ref().fuse() => {
                        if inner.exit_req.load(ORDERING) {
                            break;
                        }
                    }
                }
            } 
        }
        job.call(true);
    });

    let guard = PeriodCallGuard {
        inner: inner0,
        task: Some(task),
    };

    guard
}


const ORDERING: Ordering = Ordering::Relaxed;
