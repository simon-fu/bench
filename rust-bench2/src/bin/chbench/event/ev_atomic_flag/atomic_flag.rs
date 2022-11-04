use futures::future::Future;
use futures::task::{Context, Poll, AtomicWaker};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use std::pin::Pin;

struct Inner {
    waker: AtomicWaker,
    set: AtomicBool,
}
#[derive(Clone)]
pub struct AtomicFlag(Arc<Inner>);

impl AtomicFlag {
    pub fn new() -> Self {
        Self(Arc::new(Inner {
            waker: AtomicWaker::new(),
            set: AtomicBool::new(false),
        }))
    }

    pub fn signal(&self) {
        self.0.set.store(true, Relaxed);
        self.0.waker.wake();
    }

    fn poll_me(&self, cx: &mut Context<'_>) -> Poll<()> {
        if self.0.set.load(Relaxed) {
            return Poll::Ready(());
        }

        self.0.waker.register(cx.waker());

        if self.0.set.load(Relaxed) {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

impl Future for &AtomicFlag { 
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        self.poll_me(cx)
    }
}

impl Future for AtomicFlag {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        self.poll_me(cx)
    }
}
