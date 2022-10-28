
use prometheus::{Histogram as RawHistogram, local::LocalHistogram as RawLocalHistogram, core::Collector, proto::MetricFamily};
use anyhow::{Result, bail};

pub type Histogram = RawHistogram;
pub type LocalHistogram = RawLocalHistogram;

pub fn new<S1, S2>(name: S1, help: S2, buckets: Vec<f64>) -> Result<Histogram>
where 
    S1: Into<String>, 
    S2: Into<String>,
{
    Ok(Histogram::with_buckets(name, help, buckets)?)
}

pub fn new_latency<S1, S2>(name: S1, help: S2) -> Result<Histogram>
where 
    S1: Into<String>, 
    S2: Into<String>,
{
    new(name, help, vec![
        1.0, 5.0, 
        10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 
        100.0, 200.0, 300.0, 400.0, 
        500.0, 550.0, 
        600.0, 650.0, 
        700.0, 750.0,
        800.0, 850.0,
        900.0, 950.0,
        1000.0, 2000.0, 3000.0
    ])
}

pub trait ToPercentiler {
    type Output: Percentile;
    fn to_percentiler(&self) -> Self::Output ;
}

pub trait Percentile {
    type Output;
    fn percentile(&self, percentile: f64) -> Self::Output ;
    fn samples(&self) -> u64;
}

impl ToPercentiler for Histogram {
    type Output = Percentiler;

    fn to_percentiler(&self) -> Self::Output  {
        Percentiler(self.collect())
    }
}

pub struct Percentiler(Vec<MetricFamily>);

impl Percentile for Percentiler {
    type Output = Result<f64>;

    fn percentile(&self, percentile: f64) -> Self::Output  {
        self.0[0].get_metric()[0].get_histogram().percentile(percentile)
    }

    fn samples(&self) -> u64 {
        self.0[0].get_metric()[0].get_histogram().samples()
    }
}

impl Percentile for prometheus::proto::Histogram {
    type Output = Result<f64>;

    fn percentile(&self, percentile: f64) -> Self::Output  { 
        let total = self.get_sample_count();
        if total < 1 {
            bail!("percentile but no data");
        }

        if percentile > 100.0 || percentile < 0.0 { 
            bail!("percentile out of range");
        }

        let mut need = (total as f64 * (percentile / 100.0_f64)).ceil() as u64;
        if need > total {
            need = total;
        }

        for (i, bucket) in self.get_bucket().iter().enumerate() {
            if bucket.get_cumulative_count() == need { 
                return Ok(bucket.get_upper_bound())

            } else if bucket.get_cumulative_count() > need { 

                if i == 0 {
                    return Ok(bucket.get_upper_bound())
                }

                let v1 = bucket.get_upper_bound();
                let v2 = self.get_bucket()[i-1].get_upper_bound();
                let r = (v1+v2)/2.0;
                return Ok(r)
            }
        }

        dump_hist(self);
        bail!("percentile but unknown failure");
    }

    fn samples(&self) -> u64 {
        self.get_sample_count()
    }

}

fn dump_hist(hist: &prometheus::proto::Histogram) { 
    println!("  sample_count [{}]", hist.get_sample_count());
    println!("  sample_sum [{}]", hist.get_sample_sum());
    for (i, m) in hist.get_bucket().iter().enumerate() { 
        println!("  bucket[{}]: [{:?}]", i, m);
    }
}


pub trait WithBuckets: Sized {
    type Error;

    fn with_buckets<S1, S2>(name: S1, help: S2, buckets: Vec<f64>) -> Result<Self, Self::Error>
    where 
        S1: Into<String>, 
        S2: Into<String>,
    ;
}

impl WithBuckets for Histogram {
    type Error = prometheus::Error;
    fn with_buckets<S1, S2>(name: S1, help: S2, buckets: Vec<f64>) -> Result<Self, Self::Error>
    where 
        S1: Into<String>, 
        S2: Into<String>,
    {
        let opts = prometheus::histogram_opts!(
            name, 
            help, 
            buckets, 
        );
        Histogram::with_opts(opts)
    }
}


#[cfg(test)]
mod test {
    use prometheus::{Histogram, core::Collector};
    use super::{Percentile, WithBuckets};

    #[test]
    fn test_percentile() {
        let mut buckets = Vec::new();
        let mut v = 0.0;
        while v <= 1000.0 {
            buckets.push(v);
            v += 50.0;
        }

        let hist = Histogram::with_buckets("h", "h help", buckets).unwrap();

        for value in 1..1000 {
            hist.observe(value as f64);
        }

        let h = hist.collect();
        let h = h[0].get_metric()[0].get_histogram();
        assert_eq!(h.percentile(50.0).unwrap(), 500.0);
        assert_eq!(h.percentile(90.0).unwrap(), 900.0);
        assert_eq!(h.percentile(99.0).unwrap(), 975.0);
        assert_eq!(h.percentile(99.9).unwrap(), 1000.0);
    }
}
