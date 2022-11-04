use std::ops::DerefMut;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use std::task::Poll;

use super::defines::{IEvent, AsyncRecv};
use super::atomic_flag::AtomicFlag;
use futures::Future;
use futures::task::AtomicWaker;
use parking_lot::Mutex;

#[derive(Clone)]
pub struct Event {
    slots: Box<[Slot]>,
    pos: usize,
}

impl Event {
    pub fn new() -> Self {
        let mut slots = Vec::new();
        for _ in 0..8 {
            slots.push(Slot::new());
        }
        Self { 
            slots: slots.into_boxed_slice(), 
            pos: 0
        }
    }
} 

impl IEvent for Event {
    type Sub = Sub;

    fn watch_me(&mut self) -> Self::Sub {
        let flag = AtomicFlag::new();
        self.slots[self.pos].add(flag.clone());
        Sub(flag)
    }

    type Signal<'a> = () where Self: 'a;

    fn signal_me<'a>(&'a mut self) -> Self::Signal<'a> {
        for slot in self.slots.iter() {
            slot.signal();
        }
    }
}

#[derive(Clone, Default)]
struct Slot {
    shared: Arc<SlotShared>,
}

impl Slot {
    pub fn new() -> Self {
        let shared = Arc::new(SlotShared::default());
        {
            let shared = shared.clone();
            tokio::spawn(async move { 
                let mut worker = Worker {
                    shared,
                    local_flags: Vec::new(),
                };

                loop {
                    let r = (&mut worker).await;
                    if r { break; }
                }
            });
        }

        Self { shared }
    }

    pub fn signal(&self) {
        self.shared.signaled.store(true, Relaxed);
        self.shared.work_waker.wake();
    }

    pub fn add(&self, flag: AtomicFlag) {
        self.shared.flags.lock().push(flag);
        self.shared.work_waker.wake();
    }

}

impl Drop for Slot {
    fn drop(&mut self) {
        self.shared.dropped.store(true, Relaxed);
        self.shared.work_waker.wake();
    }
}

#[derive(Default)]
struct SlotShared {
    flags: Mutex<Vec<AtomicFlag>>,
    work_waker: AtomicWaker,
    signaled: AtomicBool,
    dropped: AtomicBool,
}

struct Worker {
    shared: Arc<SlotShared>,
    local_flags: Vec<AtomicFlag>,
}

impl Future for &mut Worker {
    type Output = bool;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> { 
        let self0 = self.deref_mut();

        self0.shared.work_waker.register(cx.waker());
        
        let mut ready = false;
        if self0.shared.signaled.load(Relaxed) { 
            for flag in self0.local_flags.iter() {
                flag.signal();
            }
            self0.shared.signaled.store(false, Relaxed);
            ready = true;
        }

        {
            let mut flags = self0.shared.flags.lock();
            if flags.len() > 0 {
                while let Some(flag) = flags.pop() {
                    self0.local_flags.push(flag);
                }
                ready = true;
            }
        }

        let dropped = self0.shared.dropped.load(Relaxed);

        if ready || dropped {
            Poll::Ready(dropped)
        } else {
            Poll::Pending
        }
    }
}


pub struct Sub(AtomicFlag);
impl AsyncRecv for Sub {
    type Output<'a>
    = impl Future<Output = ()> where Self: 'a;

    #[inline(always)]
    fn async_recv<'a>(&'a mut self) -> Self::Output<'a> {
        &self.0
    }
}
