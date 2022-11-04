use event_listener::{Event as RawEvent, EventListener as RawListener};
use futures::{Future};

use super::defines::{IEvent, AsyncRecv};

pub struct Event(RawEvent);

impl Event {
    pub fn new() -> Self {
        Self(RawEvent::new())
    }
}

impl IEvent for Event { 

    type Sub = Listener;

    #[inline(always)]
    fn watch_me(&mut self) -> Self::Sub {
        Listener(self.0.listen())
    }


    type Signal<'a> = () where Self: 'a;

    #[inline(always)]
    fn signal_me(&mut self) {
        self.0.notify(usize::MAX);
    }
}

pub struct Listener(RawListener);

impl AsyncRecv for Listener {
    type Output<'a>
    = impl Future<Output = ()> where Self: 'a;

    #[inline(always)]
    fn async_recv<'a>(&'a mut self) -> Self::Output<'a> {
        &mut self.0
    }
}
