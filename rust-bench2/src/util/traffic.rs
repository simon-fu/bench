

use std::{time::{Duration, Instant}, sync::atomic::{Ordering, AtomicI64}, fmt::Write};

use super::period_rate::{CalcRate, RateEstimator, GetRateState};



#[derive(Debug)]
pub struct Interval {
    time: Instant,
    milli: u64,
}

impl Interval {
    pub fn new(milli: u64) -> Self {
        Interval {
            time: Instant::now(),
            milli,
        }
    }

    pub fn check(&mut self) -> bool {
        let next = self.time + std::time::Duration::from_millis(self.milli);
        let now = Instant::now();
        if now >= next {
            self.time = now + std::time::Duration::from_millis(self.milli);
            return true;
        } else {
            return false;
        }
    }
}






const ORDERING: Ordering = Ordering::Relaxed;

#[derive(Debug, Default)]
pub struct AtomicTraffic {
    pub packets: AtomicI64,
    pub bytes: AtomicI64,
}

impl AtomicTraffic {
    pub fn inc_traffic(&self, bytes: i64) {
        self.packets.fetch_add(1, ORDERING);
        self.bytes.fetch_add(bytes, ORDERING);
    }
}

impl TrafficOp for AtomicTraffic {
    fn get_traffic(&self) -> Traffic {
        Traffic::new(
            self.packets.load(ORDERING),
            self.bytes.load(ORDERING),
        )
    }
}

impl GetRateState for AtomicTraffic {
    type Output = Traffic;
    fn get_rate_state(&self) -> Self::Output {
        self.get_traffic()
    }
}

pub type TrafficEstimator = RateEstimator<Traffic>;

impl CalcRate for Traffic {
    type Rate = TrafficRate;
    type Delta = Traffic;

    fn calc_rate(&self, delta: &Self::Delta, duration: Duration) -> Self::Rate {
        TrafficRate{
            qps:     self.packets.calc_rate(&delta.packets, duration),
            bitrate: self.bytes.calc_rate(&delta.bytes, duration) * 8,
        }
    }

    fn calc_delta_only(&self, new_state: &Self) -> Self::Delta { 
        Traffic {
            packets: new_state.packets - self.packets,
            bytes: new_state.bytes - self.bytes,
        }
    }
}

impl GetRateState for Traffic {
    type Output = Traffic;
    fn get_rate_state(&self) -> Self::Output {
        self.get_traffic()
    }
}


#[derive(Debug, Default, Clone, Copy)]
pub struct Traffic {
    packets: i64,
    bytes: i64,
}

impl Traffic {
    pub fn new(packets: i64, bytes: i64) -> Self {
        Self { packets, bytes }
    }

    pub fn inc_traffic(&mut self, bytes: i64) {
        self.packets += 1; 
        self.bytes += bytes ; 
    }

    pub fn packets(&self) -> i64 {
        self.packets
    }

    pub fn bytes(&self) -> i64 {
        self.bytes
    }
}

impl TrafficOp for Traffic {
    fn get_traffic(&self) -> Traffic {
        *self
    }
}

pub trait TrafficOp {
    // fn inc_traffic(&mut self, bytes: i64);
    fn get_traffic(&self) -> Traffic;
}


#[derive(Debug, Clone, Copy, Default)]
pub struct TrafficRate{
    pub qps: i64,
    pub bitrate: i64,
}

impl ToHuman for TrafficRate {
    type Output<'a> = TrafficRateHuman<'a> where Self: 'a;

    fn to_human<'a>(&'a self) -> Self::Output<'a> {
        TrafficRateHuman(self)
    }
}


pub trait ToHuman {
    type Output<'a>  where Self: 'a;
    fn to_human<'a>(&'a self) -> Self::Output<'a>;
}

pub struct TrafficRateHuman<'a >(&'a TrafficRate);
impl<'a> std::fmt::Display for TrafficRateHuman<'a > {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { 
        f.write_fmt(format_args!("{} q/s, {}", self.0.qps.to_human(), BitrateHuman(&self.0.bitrate)))
    }
}

pub struct BitrateHuman<'a>(&'a i64);
impl<'a> BitrateHuman<'a> {
    fn fmt_me(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { 
        if *self.0 < 1_000 {
            f.write_fmt(format_args!("{} b/s", self.0))?;
        } else if *self.0 < 1_000_000 {
            f.write_fmt(format_args!("{}.{:02} Kb/s", self.0/1_000, self.0%1_000/1_0))?;
        } else if *self.0 < 1_000_000_000 {
            f.write_fmt(format_args!("{}.{:02} Mb/s", self.0/1_000_000, self.0%1_000_000/1_000_0))?;
        } else if *self.0 < 1_000_000_000_000 {
            f.write_fmt(format_args!("{}.{:02} Gb/s", self.0/1_000_000_000, self.0%1_000_000_000/1_000_000_0))?;
        } else {
            f.write_fmt(format_args!("{}.{:02} Tb/s", self.0/1_000_000_000_000, self.0%1_000_000_000_000/1_000_000_000_0))?;
        }
        Ok(())
    }
}

impl<'a> std::fmt::Display for BitrateHuman<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { 
        self.fmt_me(f)
    }
}

impl<'a> std::fmt::Debug for BitrateHuman<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt_me(f)
    }
}


impl ToHuman for i64 {
    type Output<'a> = I64Human where Self: 'a;

    fn to_human<'a>(&'a self) -> Self::Output<'a> {
        I64Human(*self)
    }
}

pub struct I64Human(i64);
impl I64Human {
    fn fmt_me(&self, mut f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { 
        if f.sign_plus() && self.0 >= 0{
            f.write_char('+')?;
        }
        num_format::WriteFormatted::write_formatted(&mut f, &self.0, &num_format::Locale::en)
        .map_err(|_e| std::fmt::Error)?;
        Ok(())
    }
}

impl std::fmt::Display for I64Human {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { 
        self.fmt_me(f)
    }
}

impl std::fmt::Debug for I64Human {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt_me(f)
    }
}



#[cfg(test)]
mod test {

    use super::BitrateHuman;

    #[test]
    fn test_bitrate_human() {
        assert_eq!(BitrateHuman(&0).to_string(), "0 b/s");
        assert_eq!(BitrateHuman(&1).to_string(), "1 b/s");
        assert_eq!(BitrateHuman(&23).to_string(), "23 b/s");
        assert_eq!(BitrateHuman(&912).to_string(), "912 b/s");
        assert_eq!(BitrateHuman(&999).to_string(), "999 b/s");
        assert_eq!(BitrateHuman(&1_000).to_string(), "1.00 Kb/s");
        assert_eq!(BitrateHuman(&1_001).to_string(), "1.00 Kb/s");
        assert_eq!(BitrateHuman(&1_009).to_string(), "1.00 Kb/s");
        assert_eq!(BitrateHuman(&1_010).to_string(), "1.01 Kb/s");
        assert_eq!(BitrateHuman(&1_011).to_string(), "1.01 Kb/s");
        assert_eq!(BitrateHuman(&1_019).to_string(), "1.01 Kb/s");
        assert_eq!(BitrateHuman(&1_020).to_string(), "1.02 Kb/s");
        assert_eq!(BitrateHuman(&999_029).to_string(), "999.02 Kb/s");
        assert_eq!(BitrateHuman(&999_999).to_string(), "999.99 Kb/s");
        assert_eq!(BitrateHuman(&1_000_000).to_string(), "1.00 Mb/s");
        assert_eq!(BitrateHuman(&999_999_999).to_string(), "999.99 Mb/s");
        assert_eq!(BitrateHuman(&1_000_000_000).to_string(), "1.00 Gb/s");
        assert_eq!(BitrateHuman(&999_999_999_999).to_string(), "999.99 Gb/s");
        assert_eq!(BitrateHuman(&1_000_000_000_000).to_string(), "1.00 Tb/s");
        assert_eq!(BitrateHuman(&999_999_999_999_999).to_string(), "999.99 Tb/s");
        assert_eq!(BitrateHuman(&1_000_000_000_000_000).to_string(), "1000.00 Tb/s");
        assert_eq!(BitrateHuman(&999_999_999_999_999_999).to_string(), "999999.99 Tb/s");
    }
}



