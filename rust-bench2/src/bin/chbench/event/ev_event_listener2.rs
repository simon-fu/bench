use std::sync::Arc;

use event_listener::{Event as RawEvent};
use futures::{Future};
use parking_lot::RwLock;

use super::defines::{IEvent, AsyncRecv};

#[derive(Clone)]
pub struct Event(Arc<Shared<()>>);

impl Event {
    pub fn new() -> Self {
        Self(Arc::new(Shared { state: Default::default(), event: RawEvent::new() }))
    }
}

impl IEvent for Event {
    type Sub = Listener;

    #[inline(always)]
    fn watch_me(&mut self) -> Self::Sub {
        Listener(self.0.clone())
    }


    type Signal<'a> = () where Self: 'a;

    #[inline(always)]
    fn signal_me(&mut self) { 
        *self.0.state.write() = Some(());
        self.0.event.notify(usize::MAX);
    }
}

pub struct Listener(Arc<Shared<()>>);

impl AsyncRecv for Listener {
    type Output<'a>
    = impl Future<Output = ()> where Self: 'a;

    #[inline(always)]
    fn async_recv<'a>(&'a mut self) -> Self::Output<'a> {
        async move {
            let listener = self.0.event.listen();
            {
                if self.0.state.read().is_some() {
                    return;
                }
            }
            listener.await;
        }
    }
}



struct Shared<T> {
    state: RwLock<Option<T>> ,
    event: RawEvent,
}
