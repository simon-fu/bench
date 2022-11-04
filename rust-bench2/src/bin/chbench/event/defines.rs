use futures::{Future, future::ready};


pub trait IEvent { 

    type Sub: AsyncRecv + Send + 'static;
    fn watch_me(&mut self) -> Self::Sub;

    type Signal<'a>: AsyncSignalMe where Self: 'a;
    fn signal_me<'a>(&'a mut self) -> Self::Signal<'a>;
}

pub trait AsyncRecv { 
    type Output<'a>: Future< Output = () > + Send
    where
        Self: 'a;

    fn async_recv<'a>(&'a mut self) -> Self::Output<'a>;
}

pub trait AsyncSignalMe { 

    type Output<'a>: Future< Output = () > + Send
    where
        Self: 'a;
    fn async_signal_me<'a>(&'a mut self) -> Self::Output<'a>;
}

impl AsyncSignalMe for () {
    type Output<'a>
    = impl Future<Output = ()> where Self: 'a;

    #[inline(always)]
    fn async_signal_me<'a>(&'a mut self) -> Self::Output<'a> {
        ready(())
    }
}



