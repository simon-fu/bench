

use std::{time::{Duration, Instant}, sync::atomic::{Ordering, AtomicI64}, fmt::Write};

use super::interval::{CalcRate, RateEstimator, GetRateState};




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



pub struct Quota {
    rate: u64,      // 常量
    last_time: u64, // 状态， 最后一次时间
    duration: u64,  // 常量， 最大可以使用时间范围
    max_quota: u64, // 常量， 最大可使用额度
}

impl Quota {
    pub fn new(rate: u64, last_time: u64, duration: u64) -> Self {
        Self {
            rate,
            last_time,
            duration,
            max_quota: rate * duration / 1000,
        }
    }

    /*
        说明：
            申请额度，并更新可用额度
        参数：
            now: 当前时间
            req_quota: 申请额度
        返回：
            实际申请到的额度
    */
    pub fn acquire_quota(&mut self, now: u64, req_quota: u64) -> u64 {
        let real_quota = self.try_quota(now, req_quota);
        self.sub_quota(real_quota);
        return real_quota;
    }

    /*
        说明：
            尝试申请额度，但不更新可用额度
        参数：
            now: 当前时间
            req_quota: 申请额度
        返回：
            实际可以申请的额度
    */
    pub fn try_quota(&mut self, now: u64, req_quota: u64) -> u64 {
        if now <= self.last_time {
            return 0; // 没有可用额度
        }

        // 计算可用额度
        let mut available_quota = self.rate * (now - self.last_time) / 1000;

        // 限制最大可用额度
        if available_quota >= self.max_quota {
            available_quota = self.max_quota;
            self.last_time = now - self.duration;
        }

        // 分配额度， 不能超过最大额度
        let real_quota = if req_quota > available_quota {
            available_quota
        } else {
            req_quota
        };

        // 返回分配的额度
        return real_quota;
    }

    /*
        说明：
            减少可用额度
        参数：
            real_quota: 要减少的可用额度
        返回：
            无
    */
    pub fn sub_quota(&mut self, real_quota: u64) {
        // 更新时间，也就是减掉可用额度
        self.last_time = self.last_time + (1000 * real_quota / self.rate);
    }
}

#[cfg(test)]
mod test {

    use super::{Quota, BitrateHuman};
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

    #[test]
    fn test_quota_silence() {
        let last = 10u64;
        let mut state = Quota::new(10000, last, 3000);

        // 尝试分配额度
        let req_quota = 2000u64;
        let req_duration = 200u64;

        // 和上次时间一样，没有额度
        assert!(state.acquire_quota(last, req_quota) == 0);

        // 比上次时间小，没有额度
        assert!(state.acquire_quota(last - last / 2, req_quota) == 0);

        // 可用额度只有申请额度的一半
        assert!(state.acquire_quota(last + req_duration / 2, req_quota) == req_quota / 2);

        // 同样的时间再申请一次，没有额度
        assert!(state.acquire_quota(last + req_duration / 2, req_quota) == 0);

        // 还是只有一半额度
        assert!(state.acquire_quota(last + req_duration, req_quota) == req_quota / 2);

        // 同样的时间再申请一次，没有额度
        assert!(state.acquire_quota(last + req_duration, req_quota) == 0);
    }

    fn get_milli() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    #[test]
    fn test_quota_with_log() {
        // cargo test --release test_quota_with_log -- --nocapture

        let mut state = Quota::new(10000, get_milli(), 1000);
        let req_quota = 100;

        let mut num = 0u64;
        let mut last_print_time = get_milli();
        loop {
            let realq = state.acquire_quota(get_milli(), req_quota);
            num += realq;

            if realq == 0 {
                // 没有额度时短暂休息一下，避免占用太多cpu
                thread::sleep(Duration::from_millis(1));
            }

            // 打印 qps
            let now = get_milli();
            let elapsed = now - last_print_time;
            if elapsed >= 1000 {
                let speed = num * 1000 / elapsed;
                num = 0;
                last_print_time = now;
                println!("speed: {} q/s", speed);
            }
        }

        // use super::{Traffic, TrafficSpeed};
        // let mut speed = TrafficSpeed::default();
        // let mut traffic = Traffic::default();
        // loop {
        //     let realq = state.acquire_quota(get_milli(), req_quota);
        //     traffic.inc(realq);
        //     if let Some(r) = speed.check_float(std::time::Instant::now(), &traffic) {
        //         println!("speed: [{:.2} q/s, {:.2} KB/s]", r.0, r.1,);
        //     }
        //     if realq == 0 {
        //         thread::sleep(Duration::from_millis(1));
        //     }
        // }

        // let subs = 1000; // 假如有1000个人订阅
        // let realq = state.try_quota(get_milli(), subs);
        // if realq >= subs {
        //     state.sub_quota(subs);
        //     // 发送消息给各个session进程
        // } else {
        //     // 丢弃消息
        // }
    }
}



