use futures::{Future, FutureExt};
use tokio::sync::mpsc::{UnboundedSender as Sender, UnboundedReceiver as Receiver, unbounded_channel};
use super::defines::{IEvent, AsyncRecv};

#[derive(Default)]
pub struct Event(Vec<Sender<()>>);

impl Event {
    pub fn new() -> Self {
        Self::default()
    }
}

impl IEvent for Event {
    type Sub = Sub;

    #[inline(always)]
    fn watch_me(&mut self) -> Self::Sub {
        let (tx, rx) = unbounded_channel();
        self.0.push(tx);
        Sub(rx)
    }

    type Signal<'a> = () where Self: 'a;

    #[inline(always)]
    fn signal_me(&mut self) {
        for tx in self.0.iter() {
            let _r = tx.send(());
        }
    }
}


pub struct Sub(Receiver<()>);

impl AsyncRecv for Sub {
    type Output<'a>
    = impl Future<Output = ()> where Self: 'a;

    #[inline(always)]
    fn async_recv<'a>(&'a mut self) -> Self::Output<'a> {
        self.0.recv().map(|_r|{})
    }
}
