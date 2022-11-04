
use futures::{Future, FutureExt};
use tokio::sync::broadcast::{Sender, Receiver, channel};
use super::defines::{IEvent, AsyncRecv};

pub struct Event(Sender<()>);

impl Event {
    pub fn new() -> Self {
        let (tx, _rx) = channel(1);
        Self(tx)
    }
}

impl IEvent for Event {
    type Sub = Sub;

    #[inline(always)]
    fn watch_me(&mut self) -> Self::Sub {
        Sub(self.0.subscribe())
    }

    type Signal<'a> = () where Self: 'a;

    #[inline(always)]
    fn signal_me(&mut self) {
        let _r = self.0.send(());
    }
}

pub struct Sub(Receiver<()>);

impl AsyncRecv for Sub {
    type Output<'a>
    = impl Future<Output = ()> where Self: 'a;

    #[inline(always)]
    fn async_recv<'a>(&'a mut self) -> Self::Output<'a> {
        self.0.recv().map(|_r| {})
    }
}
