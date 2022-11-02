
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()>{
    // println!("Hello, world!");
    tokio_ch::bench().await?;
    Ok(())
}

mod xrs;

pub mod tokio_ch {
    use std::time::Instant;

    use anyhow::{Result, Context};
    use tokio::sync::mpsc;
    use super::xrs::{MetricDuration};

    pub async fn bench() -> Result<()> {
        const TASK_NUM: usize = 1_000_000;
        
        let mut senders = Vec::with_capacity(TASK_NUM);
        let mut tasks = Vec::with_capacity(TASK_NUM);

        for n in 0..TASK_NUM { 
            let (tx, mut rx) = mpsc::channel(1); 
            let h = tokio::spawn(async move {
                let r = rx.recv().await;
                (n, r, Instant::now())
            });
            senders.push(tx);
            tasks.push(h);
        }

        for tx in &senders {
            let _r = tx.send(Instant::now()).await;
        }

        let mut latency = MetricDuration::new();
        for h in tasks {
            let (n, ts0, ts1) = h.await?;
            let ts0 = ts0.with_context(||format!("task [{}] recv none", n))?;
            let d = ts1-ts0;
            latency.observe(d);
        }

        println!("tokio mpsc broadcast latency: num/min/max/average {:?}", (latency.num(), latency.min(), latency.max(), latency.average()));
        
        Ok(())
    }
}
