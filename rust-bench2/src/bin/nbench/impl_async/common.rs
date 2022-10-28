use rust_bench::util::histogram::{Histogram, ToPercentiler, Percentile};
use tracing::info;
use anyhow::Result;

pub fn print_latency_percentile(hist: &Histogram) -> Result<()> {
    let percent = hist.to_percentiler();
    if percent.samples() > 0 {
        let p900 = percent.percentile(90.0)?;
        let p990 = percent.percentile(99.0)?;
        let p999 = percent.percentile(99.9)?;
        info!("latency: samples {}, p900/p990/p999 {}/{}/{} ms", percent.samples(), p900, p990, p999);
    } else {
        info!("latency: no data");
    }
    Ok(())
}
