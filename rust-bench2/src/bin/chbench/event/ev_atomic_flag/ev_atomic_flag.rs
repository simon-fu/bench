use super::defines::{IEvent, AsyncRecv};
use super::atomic_flag::AtomicFlag;
use futures::Future;

#[derive(Clone)]
pub struct Event(Vec<AtomicFlag>);

impl Event {
    pub fn new() -> Self {
        Self(Vec::new())
    }
} 

impl IEvent for Event {
    type Sub = Sub;

    fn watch_me(&mut self) -> Self::Sub {
        let ev = AtomicFlag::new();
        self.0.push(ev.clone());
        Sub(ev)
    }

    type Signal<'a> = () where Self: 'a;

    fn signal_me<'a>(&'a mut self) -> Self::Signal<'a> {
        for flag in self.0.iter() {
            flag.signal();
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
