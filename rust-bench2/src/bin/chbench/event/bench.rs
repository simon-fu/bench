use std::time::{Instant, Duration};
use anyhow::Result;
use futures::channel::oneshot;
use crate::{latency::MetricDuration, event::defines::{IEvent, AsyncRecv, AsyncSignalMe}};
use std::pin::Pin;
use futures::Future;
use super::{
    ev_tokio_broadcast,
    ev_event_listener, 
    ev_event_listener2,
    ev_tokio_unbound, 
    ev_tokio_mpsc, 
    ev_atomic_flag::{ev_atomic_flag, ev_atomic_flag2},
    // ev_kanal_mpsc,
};


pub async fn bench() -> Result<()> { 
    let task_num = 100_000;

    let all_cases: Vec<(&str, Pin<Box<dyn Future<Output = Result<ChMetric>>>>)> = vec![
        ("event_listener_ev", Box::pin(bench_one(ev_event_listener::Event::new(), task_num))),
        ("event_listener_ev2", Box::pin(bench_one(ev_event_listener2::Event::new(), task_num))),
        ("tokio_broadcast_ev", Box::pin(bench_one(ev_tokio_broadcast::Event::new(), task_num))),
        ("tokio_unbound_ev", Box::pin(bench_one(ev_tokio_unbound::Event::new(), task_num))),
        ("tokio_mpsc_ev", Box::pin(bench_one(ev_tokio_mpsc::Event::new(), task_num))),
        ("atomic_flag_ev", Box::pin(bench_one(ev_atomic_flag::Event::new(), task_num))),
        ("atomic_flag_ev2", Box::pin(bench_one(ev_atomic_flag2::Event::new(), task_num))),
        // ("kanal_mpsc_ev", Box::pin(bench_one(ev_kanal_mpsc::Event::new(), task_num))),
    ];

    let names: Vec<&str> = vec![
        // "kanal_mpsc_ev",
    ];


    let mut metrics = Vec::with_capacity(64);
    for c in all_cases { 
        
        let matched = names.len() == 0 
        || names.iter().find(|v| &c.0 == *v).is_some();

        if matched {
            println!("bench [{}]", c.0);
            let r = c.1.await?;
            metrics.push((c.0, r));
        }
    }

    println!("");
    println!("{:<30}{:<15}{:<15}{:<15}{:<15}", "event-impl", "signal", "min", "max", "average");
    println!("{:-<90}", ""); // 30+15*4=90
    for (name, m) in &metrics {
        println!(
            "{:<30}{:<15?}{:<15?}{:<15?}{:<15?}", 
            name, 
            m.signal_duration, 
            m.awake_latency.min(), 
            m.awake_latency.max(), 
            m.awake_latency.average(), 
        );
    }
    println!("");
    
    Ok(())
}

pub struct ChMetric {
    pub awake_latency: MetricDuration,
    pub signal_duration: Duration,
}

pub async fn bench_one<EV>(mut event: EV, task_num: usize) -> Result<ChMetric> 
where
    EV: IEvent,
{
    let mut tasks = Vec::with_capacity(task_num);
    
    {
        // let start_time = Instant::now();
        for n in 0..task_num { 
            let mut rx = event.watch_me();
            let (otx, orx) = oneshot::channel();
            let h = tokio::spawn(async move {
                let _r = otx.send(());
                let r = rx.async_recv().await;
                let now = Instant::now();
                // tokio::time::sleep(Duration::from_millis(1000)).await;
                (n, r, now)
            });
            tasks.push(h);
            let _r = orx.await;
        }
        // println!("spawn [{}] tasks in [{:?}] ", task_num, start_time.elapsed());
    }

    {
        // wait for all tasks ready
        tokio::time::sleep(Duration::from_millis(1000)).await;
    }

    let kick_time = Instant::now();
    let signal_duration = {
        let start_time = Instant::now();
        let _r = event.signal_me().async_signal_me().await;
        // println!("signal to [{}] tasks in [{:?}] ", task_num, start_time.elapsed());
        start_time.elapsed()
    };

    let mut awake_latency = MetricDuration::new();
    {
        // let start_time = Instant::now();
        for h in tasks {
            let (_n, _ts0, ts1) = h.await?;
            let d = ts1-kick_time;
            awake_latency.observe(d);
        }
        // println!("joined [{}] tasks in [{:?}] ", task_num, start_time.elapsed());
    }

    // println!("latency: num/min/max/average {:?}", (awake_latency.num(), awake_latency.min(), awake_latency.max(), awake_latency.average()));

    Ok(ChMetric {
        awake_latency,
        signal_duration
    })
}
