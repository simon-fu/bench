

use anyhow::Result;


#[tokio::main]
async fn main() -> Result<()>{

    let task_num = 100_000;

    // ch_wakeup::bench(task_num).await?;

    println!("--- tokio_ch::bench_mpsc_1_to_n ---");
    tokio_ch::bench_mpsc_1_to_n(task_num).await?;

    println!("--- tokio_ch::bench_broadcast_1_to_n ---");
    tokio_ch::bench_broadcast_1_to_n(task_num).await?;

    println!("--- futures_ch::bench_mpsc_1_to_n ---");
    futures_ch::bench_mpsc_1_to_n(task_num).await?;

    println!("--- custom_event_ch::bench_1_to_n ---");
    custom_event_ch::bench_1_to_n(task_num).await?;

    println!("--- custom_event_ch::bench_1_to_n_2 ---");
    custom_event_ch::bench_1_to_n_2(task_num).await?;

    println!("--- custom_waker1_ch::bench_1_to_n ---");
    custom_waker1_ch::bench_1_to_n(task_num).await?;

    println!("--- custom_waker2_ch::bench_1_to_n ---");
    custom_waker2_ch::bench_1_to_n(task_num).await?;

    println!("--- final ---");

    Ok(())
}

mod xrs;

pub mod ch_wakeup {
    use std::time::Instant;
    use anyhow::Result;
    use tokio::sync::{mpsc, broadcast};

    pub async fn bench(task_num: usize) -> Result<()> {
        
        println!("--- bench_tokio_mpsc ---");
        bench_tokio_mpsc(task_num).await?;

        println!("--- bench_tokio_broadcast ---");
        bench_tokio_broadcast(task_num).await?;

        println!("--- final ---");

        Ok(())
    }
    
    pub async fn bench_tokio_mpsc(task_num: usize) -> Result<()> {
        let mut senders = Vec::with_capacity(task_num);
        {
            let start_time = Instant::now();
            for _n in 0..task_num { 
                let (tx, mut rx) = mpsc::channel(1); 
                tokio::spawn(async move {
                    let _r = rx.recv().await;
                });
                senders.push(tx);
            }
            println!("spawn [{}] tasks in [{:?}] ", task_num, start_time.elapsed());
        }

        {
            let start_time = Instant::now();
            for tx in &senders {
                let _r = tx.send(start_time).await;
            }
            println!("sent to [{}] tasks in [{:?}] ", task_num, start_time.elapsed());
        }

        Ok(())
    }

    pub async fn bench_tokio_broadcast(task_num: usize) -> Result<()> {
        let (tx, _rx) = broadcast::channel(1); 
        {
            let start_time = Instant::now();
            for _n in 0..task_num { 
                let mut rx = tx.subscribe();
                tokio::spawn(async move {
                    let _r = rx.recv().await;
                });
            }
            println!("spawn [{}] tasks in [{:?}] ", task_num, start_time.elapsed());
        }

        {
            let start_time = Instant::now();
            let _r = tx.send(start_time);
            println!("sent to [{}] tasks in [{:?}] ", task_num, start_time.elapsed());
        }

        Ok(())
    }
}


pub mod tokio_ch {
    use std::time::{Instant, Duration};

    use anyhow::{Result, Context};
    use tokio::sync::{mpsc, broadcast};
    use super::xrs::{MetricDuration};

    pub async fn bench_mpsc_1_to_n(task_num: usize) -> Result<()> {
        let mut senders = Vec::with_capacity(task_num);
        let mut tasks = Vec::with_capacity(task_num);

        {
            let start_time = Instant::now();
            for n in 0..task_num { 
                let (tx, mut rx) = mpsc::channel(1); 
                let h = tokio::spawn(async move {
                    let _r = rx.recv().await;
                    let r = rx.recv().await;
                    (n, r, Instant::now())
                });
                senders.push(tx);
                tasks.push(h);
            }
            println!("spawn [{}] tasks in [{:?}] ", task_num, start_time.elapsed());
        }

        {
            // wait for all tasks ready
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }

        {
            let start_time = Instant::now();
            for tx in &senders {
                let _r = tx.send(start_time).await;
            }
            println!("warm up [{}] tasks in [{:?}] ", task_num, start_time.elapsed());
        }

        {
            let d = Duration::from_millis(1000);
            println!("sleep [{:?}] ", d);
            tokio::time::sleep(d).await;
        }

        {
            let start_time = Instant::now();
            for tx in &senders {
                let _r = tx.send(start_time).await;
            }
            println!("sent to [{}] tasks in [{:?}] ", task_num, start_time.elapsed());
        }

        let mut latency = MetricDuration::new();
        {
            let start_time = Instant::now();
            for h in tasks {
                let (n, ts0, ts1) = h.await?;
                let ts0 = ts0.with_context(||format!("task [{}] recv none", n))?;
                let d = ts1-ts0;
                latency.observe(d);
            }
            println!("joined [{}] tasks in [{:?}] ", task_num, start_time.elapsed());
        }

        println!("tokio mpsc latency: num/min/max/average {:?}", (latency.num(), latency.min(), latency.max(), latency.average()));

        Ok(())
    }

    pub async fn bench_broadcast_1_to_n(task_num: usize) -> Result<()> {
        // let mut senders = Vec::with_capacity(task_num);
        let mut tasks = Vec::with_capacity(task_num);

        let (tx, _rx) = broadcast::channel(1); 
        {
            let start_time = Instant::now();
            for n in 0..task_num { 
                let mut rx = tx.subscribe();
                let h = tokio::spawn(async move {
                    let _r = rx.recv().await;
                    let r = rx.recv().await;
                    (n, r, Instant::now())
                });
                tasks.push(h);
            }
            println!("spawn [{}] tasks in [{:?}] ", task_num, start_time.elapsed());
        }

        {
            // wait for all tasks ready
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }

        {
            let start_time = Instant::now();
            let _r = tx.send(start_time);
            println!("warm up [{}] tasks in [{:?}] ", task_num, start_time.elapsed());
        }

        {
            let d = Duration::from_millis(1000);
            println!("sleep [{:?}] ", d);
            tokio::time::sleep(d).await;
        }

        {
            let start_time = Instant::now();
            let _r = tx.send(start_time);
            println!("sent to [{}] tasks in [{:?}] ", task_num, start_time.elapsed());
        }

        let mut latency = MetricDuration::new();
        {
            let start_time = Instant::now();
            for h in tasks {
                let (n, ts0, ts1) = h.await?;
                let ts0 = ts0.with_context(||format!("task [{}] recv none", n))?;
                let d = ts1-ts0;
                latency.observe(d);
            }
            println!("joined [{}] tasks in [{:?}] ", task_num, start_time.elapsed());
        }

        println!("tokio broadcast latency: num/min/max/average {:?}", (latency.num(), latency.min(), latency.max(), latency.average()));

        Ok(())
    }

}


pub mod futures_ch {
    use std::time::{Instant, Duration};

    use anyhow::{Result, Context};
    use futures::{channel::mpsc, StreamExt, SinkExt};
    use super::xrs::{MetricDuration};

    pub async fn bench_mpsc_1_to_n(task_num: usize) -> Result<()> {
        let mut senders = Vec::with_capacity(task_num);
        let mut tasks = Vec::with_capacity(task_num);

        {
            let start_time = Instant::now();
            for n in 0..task_num { 
                let (tx, mut rx) = mpsc::channel(1); 
                let h = tokio::spawn(async move {
                    let _r = rx.next().await;
                    let r = rx.next().await;
                    (n, r, Instant::now())
                });
                senders.push(tx);
                tasks.push(h);
            }
            println!("spawn [{}] tasks in [{:?}] ", task_num, start_time.elapsed());
        }

        {
            // wait for all tasks ready
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }

        {
            let start_time = Instant::now();
            for tx in &mut senders {
                let _r = tx.send(start_time).await;
            }
            println!("warm up [{}] tasks in [{:?}] ", task_num, start_time.elapsed());
        }

        {
            let d = Duration::from_millis(1000);
            println!("sleep [{:?}] ", d);
            tokio::time::sleep(d).await;
        }

        {
            let start_time = Instant::now();
            for tx in &mut senders {
                let _r = tx.send(start_time).await;
            }
            println!("sent to [{}] tasks in [{:?}] ", task_num, start_time.elapsed());
        }

        let mut latency = MetricDuration::new();
        {
            let start_time = Instant::now();
            for h in tasks {
                let (n, ts0, ts1) = h.await?;
                let ts0 = ts0.with_context(||format!("task [{}] recv none", n))?;
                let d = ts1-ts0;
                latency.observe(d);
            }
            println!("joined [{}] tasks in [{:?}] ", task_num, start_time.elapsed());
        }

        println!("futures mpsc latency: num/min/max/average {:?}", (latency.num(), latency.min(), latency.max(), latency.average()));

        Ok(())
    }

}

pub mod custom_event_ch {
    use std::{time::{Instant, Duration}, sync::Arc};

    use anyhow::{Result, Context};
    use event_listener::{Event};
    use parking_lot::RwLock;
    use super::xrs::{MetricDuration};

    pub async fn bench_1_to_n(task_num: usize) -> Result<()> {
        let mut tasks = Vec::with_capacity(task_num);

        let (tx, rx) = channel(); 
        {
            let start_time = Instant::now();
            for n in 0..task_num { 
                let rx = rx.clone();
                let h = tokio::spawn(async move {
                    let _r = rx.recv().await;
                    let r = rx.recv().await;
                    (n, r, Instant::now())
                });
                tasks.push(h);
            }
            println!("spawn [{}] tasks in [{:?}] ", task_num, start_time.elapsed());
        }

        {
            // wait for all tasks ready
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }

        {
            let start_time = Instant::now();
            let _r = tx.send(start_time);
            println!("sent to [{}] tasks in [{:?}] ", task_num, start_time.elapsed());
        }

        let mut latency = MetricDuration::new();
        {
            let start_time = Instant::now();
            for h in tasks {
                let (n, ts0, ts1) = h.await?;
                let ts0 = ts0.with_context(||format!("task [{}] recv none", n))?;
                let d = ts1-ts0;
                latency.observe(d);
            }
            println!("joined [{}] tasks in [{:?}] ", task_num, start_time.elapsed());
        }

        println!("latency: num/min/max/average {:?}", (latency.num(), latency.min(), latency.max(), latency.average()));

        Ok(())
    }

    pub async fn bench_1_to_n_2(task_num: usize) -> Result<()> {
        let mut tasks = Vec::with_capacity(task_num);

        let event = Event::new();
        let kick_time = Instant::now();
        {
            let start_time = Instant::now();
            for n in 0..task_num { 
                let rx = event.listen();
                let h = tokio::spawn(async move {
                    let r = rx.await;
                    let now = Instant::now();
                    // tokio::time::sleep(Duration::from_millis(1000)).await;
                    (n, r, now)
                });
                tasks.push(h);
            }
            println!("spawn [{}] tasks in [{:?}] ", task_num, start_time.elapsed());
        }

        {
            // wait for all tasks ready
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }

        {
            let start_time = Instant::now();
            let _r = event.notify(usize::MAX);
            println!("sent to [{}] tasks in [{:?}] ", task_num, start_time.elapsed());
        }

        let mut latency = MetricDuration::new();
        {
            let start_time = Instant::now();
            for h in tasks {
                let (_n, _ts0, ts1) = h.await?;
                let d = ts1-kick_time;
                latency.observe(d);
            }
            println!("joined [{}] tasks in [{:?}] ", task_num, start_time.elapsed());
        }

        println!("latency: num/min/max/average {:?}", (latency.num(), latency.min(), latency.max(), latency.average()));

        Ok(())
    }

    pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
        let shared = Arc::new(Shared {
            state: RwLock::new(None),
            event: Event::new(),
        });
        (Sender{shared: shared.clone()}, Receiver{shared})
    }

    pub struct Sender<T> {
        shared: Arc<Shared<T>>
    }

    impl<T> Sender<T> 
    where
        T: Clone,
    {
        pub fn send(&self, value: T) {
            *self.shared.state.write() = Some(value);
            self.shared.event.notify(usize::MAX);
        }
    }

    #[derive(Clone)]
    pub struct Receiver<T> {
        shared: Arc<Shared<T>>
    }


    impl<T> Receiver<T> 
    where
        T: Clone,
    {
        pub async fn recv(&self) -> Option<T> {
            let listener = self.shared.event.listen();
            {
                let value = self.shared.state.read().clone();
                if value.is_some() {
                    return value
                }
            }
            listener.await;
            self.shared.state.read().clone()
        }
    }

    struct Shared<T> {
        state: RwLock<Option<T>> ,
        event: Event,
    }



}

pub mod custom_waker1_ch {
    use std::{time::{Instant, Duration}, sync::Arc};
    use anyhow::{Result, Context};
    use futures::{task::AtomicWaker, Future};
    use parking_lot::{RwLock, Mutex};
    use super::xrs::{MetricDuration};

    pub async fn bench_1_to_n(task_num: usize) -> Result<()> {
        let mut tasks = Vec::with_capacity(task_num);

        let tx = channel(); 
        {
            let start_time = Instant::now();
            for n in 0..task_num { 
                let rx = tx.subscribe();
                let h = tokio::spawn(async move {
                    // let _r = rx.recv().await;
                    let r = rx.recv().await;
                    (n, r, Instant::now())
                });
                tasks.push(h);
            }
            println!("spawn [{}] tasks in [{:?}] ", task_num, start_time.elapsed());
        }

        {
            // wait for all tasks ready
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }

        {
            let start_time = Instant::now();
            let _r = tx.send(start_time);
            println!("sent to [{}] tasks in [{:?}] ", task_num, start_time.elapsed());
        }

        let mut latency = MetricDuration::new();
        {
            let start_time = Instant::now();
            for h in tasks {
                let (n, ts0, ts1) = h.await?;
                let ts0 = ts0.with_context(||format!("task [{}] recv none", n))?;
                let d = ts1-ts0;
                latency.observe(d);
            }
            println!("joined [{}] tasks in [{:?}] ", task_num, start_time.elapsed());
        }

        println!("custom channel latency: num/min/max/average {:?}", (latency.num(), latency.min(), latency.max(), latency.average()));

        Ok(())
    }

    pub fn channel<T>() -> Sender<T> {
        let shared = Arc::new(Shared {
            state: RwLock::new(None),
            wakers: Mutex::new(Vec::new()),

        });
        Sender{shared: shared.clone()}
    }

    pub struct Sender<T> {
        shared: Arc<Shared<T>>
    }

    impl<T> Sender<T> 
    where
        T: Clone,
    {
        pub fn subscribe(&self) -> Receiver<T> {
            let waker = Arc::new(AtomicWaker::new());
            self.shared.wakers.lock().push(waker.clone());
            Receiver {
                shared: self.shared.clone(),
                waker,
            }
        }

        pub fn send(&self, value: T) { 
            {
                *self.shared.state.write() = Some(value);
            }
            
            {
                let wakers = self.shared.wakers.lock();
                for waker in wakers.iter() {
                    waker.wake();
                }
            }
        }
    }

    #[derive(Clone)]
    pub struct Receiver<T> {
        shared: Arc<Shared<T>>,
        waker: Arc<AtomicWaker>,
    }


    impl<T> Receiver<T> 
    where
        T: Clone,
    {
        pub fn recv<'a>(&'a self) -> WaitFuture<'a, T> { 
            WaitFuture(self)
        }
    }

    struct Shared<T> {
        state: RwLock<Option<T>> ,
        wakers: Mutex<Vec<Arc<AtomicWaker>>>,
    }

    pub struct WaitFuture<'a, T>(&'a Receiver<T>);
    impl<'a, T> Future for WaitFuture<'a, T> 
    where
        T: Clone,
    {
        type Output = Option<T>;

        fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> { 
            let state = self.0.shared.state.read();
            if state.is_some() {
                return std::task::Poll::Ready(state.clone()) 
            }
            self.0.waker.register(cx.waker());
            std::task::Poll::Pending
        }
    }


}

pub mod custom_waker2_ch {
    use std::{time::{Instant, Duration}};
    use anyhow::{Result};
    use crate::atomic_flag::AtomicFlag;

    use super::xrs::{MetricDuration};

    pub async fn bench_1_to_n(task_num: usize) -> Result<()> { 

        let mut tasks = Vec::with_capacity(task_num);
        let mut senders = Vec::with_capacity(task_num);

        let kick_time = Instant::now();
        {
            let start_time = Instant::now();
            for n in 0..task_num { 
                let flag = AtomicFlag::new();
                let rx = flag.clone();
                let h = tokio::spawn(async move {
                    let r = rx.await;
                    let now = Instant::now();
                    tokio::time::sleep(Duration::from_millis(1000)).await;
                    (n, r, now)
                });
                tasks.push(h);
                senders.push(flag);
            }
            println!("spawn [{}] tasks in [{:?}] ", task_num, start_time.elapsed());
        }

        {
            // wait for all tasks ready
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }
        
        {
            let start_time = Instant::now();
            for tx in &senders {
                tx.signal();
            }
            println!("sent to [{}] tasks in [{:?}] ", task_num, start_time.elapsed());
        }

        let mut latency = MetricDuration::new();
        {
            let start_time = Instant::now();
            for h in tasks {
                let (_n, _r, ts1) = h.await?;
                let d = ts1-kick_time;
                latency.observe(d);
            }
            println!("joined [{}] tasks in [{:?}] ", task_num, start_time.elapsed());
        }

        println!("latency: num/min/max/average {:?}", (latency.num(), latency.min(), latency.max(), latency.average()));

        Ok(())
    }

}


pub mod atomic_flag {
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
    }
    
    impl Future for AtomicFlag {
        type Output = ();
    
        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
            // quick check to avoid registration if already done.
            if self.0.set.load(Relaxed) {
                return Poll::Ready(());
            }
    
            self.0.waker.register(cx.waker());
    
            // Need to check condition **after** `register` to avoid a race
            // condition that would result in lost notifications.
            if self.0.set.load(Relaxed) {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        }
    }
}
