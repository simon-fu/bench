

use anyhow:: Result;
use rust_bench::util::async_rt::tokio_tcp::VRuntimeTokio;
use tokio::{runtime::{Runtime, self}, net::{TcpStream, TcpListener}};

use crate::args::{ClientArgs, ServerArgs};

use super::{run_as_client, run_as_server};

pub fn run_tokio_client(args: ClientArgs) -> Result<()> {
    let rt = runtime::Builder::new_multi_thread()
    .enable_all()
    .build()?;
    // let rt  = Runtime::new()?;

    rt.block_on(async {
        tokio::spawn(async move {
            run_as_client::<VRuntimeTokio, TcpStream>(args).await
        }).await?
    })?;

    Ok(())
}

pub fn run_tokio_server(args: ServerArgs) -> Result<()> {
    let rt  = Runtime::new()?;

    rt.block_on(async {
        tokio::spawn(async move {
            run_as_server::<VRuntimeTokio, TcpListener, TcpStream>(&args).await
        }).await?
    })?;

    Ok(())
}


// mod ttt {
//     use futures::future::Future;
//     use futures::task::{Context, Poll, AtomicWaker};
//     use std::sync::Arc;
//     use std::sync::atomic::{AtomicBool, AtomicU64};
//     use std::sync::atomic::Ordering::Relaxed;
//     use std::pin::Pin;
//     use std::time::Duration;
//     use event_listener::Event;
    
//     pub async fn test_atomic_waker() {
//         let count = CountEvent::new(); 

//         for tid in 0..1 {
//             let count = count.clone();
//             tokio::spawn(async move {
//                 let mut num = 0;
//                 loop {
//                     num += 1;
//                     tokio::time::sleep(Duration::from_millis(1000)).await;
//                     println!("signal task {}: {}", tid, num);
//                     count.inc_and_signal(1);
//                 }
                
//             });
//         }

//         for tid in 0..2 {
//             let mut watch = count.watcher();
//             tokio::spawn(async move {
//                 let mut num = 0;
//                 loop { 
//                     let val = watch.watch().await;
//                     num += 1;
//                     println!("await task {}: {}, {}", tid, num, val);
//                 }
                
//             });
//         }

//         tokio::time::sleep(Duration::from_secs(999999)).await;

//     }


//     struct CountShared {
//         event: Event,
//         count: AtomicU64,
//     }
    
//     #[derive(Clone)]
//     pub struct CountEvent {
//         shared: Arc<CountShared>
//     }
    
//     impl CountEvent {
//         pub fn new() -> Self {
//             Self {
//                 shared: Arc::new(CountShared {
//                     event: Event::new(),
//                     count: AtomicU64::new(0),
//                 }),
//             }
//         }
    
//         pub fn inc_and_signal(&self, n: u64) {
//             self.shared.count.fetch_add(n, Relaxed);
//             self.shared.event.notify(usize::MAX);
//         }

//         pub fn watcher(&self) -> CountWatcher { 
//             CountWatcher {
//                 val: self.shared.count.load(Relaxed),
//                 shared: self.shared.clone(),
//             }
//         }

//         pub async fn watch_until(&self, val: u64) -> u64 {
//             loop { 
//                 let curr = self.shared.count.load(Relaxed); 
//                 if curr >= val { 
//                     return curr;
//                 }

//                 let listener = self.shared.event.listen();
//                 listener.await; 
//             }
//         }
//     }

//     pub struct CountWatcher {
//         shared: Arc<CountShared>, 
//         val: u64,
//     }

//     impl CountWatcher { 

//         pub fn value(&self) -> u64 {
//             self.val
//         }

//         pub async fn watch(&mut self) -> u64 { 
//             loop { 
//                 let val = self.shared.count.load(Relaxed); 
//                 if self.val != val { 
//                     self.val = val;
//                     return val;
//                 }

//                 let listener = self.shared.event.listen();
//                 listener.await; 
//             }
//         }
//     }


//     struct FlagInner {
//         waker: AtomicWaker,
//         set: AtomicBool,
//     }
    
//     #[derive(Clone)]
//     pub struct Flag(Arc<FlagInner>);
    
//     impl Flag {
//         pub fn new() -> Self {
//             Self(Arc::new(FlagInner {
//                 waker: AtomicWaker::new(),
//                 set: AtomicBool::new(false),
//             }))
//         }
    
//         pub fn signal(&self) {
//             self.0.set.store(true, Relaxed);
//             self.0.waker.wake();
//         }
//     }
    
//     impl Future for &Flag {
//         type Output = ();
    
//         fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
//             // quick check to avoid registration if already done.
//             if self.0.set.load(Relaxed) {
//                 self.0.set.store(false, Relaxed);
//                 return Poll::Ready(());
//             }
    
//             self.0.waker.register(cx.waker());
    
//             // Need to check condition **after** `register` to avoid a race
//             // condition that would result in lost notifications.
//             if self.0.set.load(Relaxed) {
//                 self.0.set.store(false, Relaxed);
//                 Poll::Ready(())
//             } else {
//                 Poll::Pending
//             }
//         }
//     }
// }