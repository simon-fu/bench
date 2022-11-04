// use std::slice::Iter;

// use futures::{Future, FutureExt};
// use super::defines::{IEvent, AsyncRecv, AsyncSignalMe};
// use kanal::{bounded_async, AsyncSender, AsyncReceiver};

// pub struct Event(Vec<AsyncSender<()>>);
// impl Event {
//     pub fn new() -> Self {
//         Self(Vec::new())
//     }
// } 

// impl IEvent for Event {
//     type Sub = Sub;

//     fn watch_me(&mut self) -> Self::Sub { 
//         let (tx, rx) = bounded_async(1);
//         // let (tx, rx) = channel(1);
//         self.0.push(tx);
//         Sub(rx)
//     }


//     type Signal<'a> = Signal<Iter<'a, AsyncSender<()>>> where Self: 'a;

//     fn signal_me<'a>(&'a mut self) -> Self::Signal<'a> {
//         Signal(self.0.iter())
//     }
// }

// pub struct Sub(AsyncReceiver<()>);

// impl AsyncRecv for Sub {
//     type Output<'a>
//     = impl Future<Output = ()> where Self: 'a;

//     #[inline(always)]
//     fn async_recv<'a>(&'a mut self) -> Self::Output<'a> {
//         self.0.recv().map(|_r|{})
//     }
// }

// pub struct Signal<I>(I);

// impl<'b, I> AsyncSignalMe for Signal<I> 
// where
//     I: Iterator<Item = &'b AsyncSender<()>> + Send,
// {
//     type Output<'a>
//     = impl Future<Output = ()> where Self: 'a;

//     #[inline(always)]
//     fn async_signal_me<'a>(&'a mut self) -> Self::Output<'a> {
//         async move {
//             while let Some(tx) = self.0.next() {
//                 let _r = tx.send(()).await;
//             }
//         }
//     }
// }
