// pub mod xrs;

#![allow(dead_code)]

use std::time::Duration;

pub(crate) mod time{
    // // from https://www.jianshu.com/p/e30eef29f66e
    use std::time::{SystemTime, UNIX_EPOCH};
    pub fn now_millis() -> i64 {
        let start = SystemTime::now();
        let since_the_epoch = start
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");
        let ms = since_the_epoch.as_secs() as i64 * 1000i64 + (since_the_epoch.subsec_nanos() as f64 / 1_000_000.0) as i64;
        ms
    }
}

pub mod tracing_subscriber{
    use tokio::time::Instant;
    use tracing_subscriber::fmt::time::FormatTime;

    pub struct UptimeMilli {
        epoch: Instant,
    }
    
    impl Default for UptimeMilli {
        fn default() -> Self {
            UptimeMilli {
                epoch: Instant::now(),
            }
        }
    }
    
    impl FormatTime for UptimeMilli {
        fn format_time(&self, w: &mut dyn std::fmt::Write) -> std::fmt::Result {
            let e = self.epoch.elapsed();
            write!(w, "{:03}.{:03}", e.as_secs(), e.subsec_millis())
        }
    }

    pub fn init_simple_milli() {
        use tracing_subscriber::EnvFilter;
    
        let env_filter = if std::env::var(EnvFilter::DEFAULT_ENV).is_ok() {
            EnvFilter::from_default_env()
        } else {
            EnvFilter::new("info")
        };

        tracing_subscriber::fmt()
            // .pretty()
            // .with_thread_names(true)
            // .with_thread_ids(true)
            // .without_time()
            //.with_max_level(tracing::Level::TRACE)
    
            // see https://tracing.rs/tracing_subscriber/fmt/time/index.html
            // .with_timer(time::ChronoLocal::default())
            //.with_timer(time::ChronoUtc::default())
            //.with_timer(time::SystemTime::default())
            //.with_timer(time::Uptime::default())
            .with_timer(UptimeMilli::default())
    
            // target is arg0 ?
            .with_target(false)
    
            // RUST_LOG environment variable
            // from https://docs.rs/tracing-subscriber/0.2.0-alpha.2/tracing_subscriber/fmt/index.html
            // from https://docs.rs/env_logger/0.8.3/env_logger/
            .with_env_filter(env_filter)
    
            // sets this to be the default, global collector for this application.
            .init();
    }
}


pub mod speed{
    use std::{collections::VecDeque, time::Duration};

    use tokio::time;
    use tokio::time::Instant;

    pub struct Test{
        d1 : i32,
    }

    #[derive(Debug)]
    pub struct TsI64{
        ts : i64,
        num : i64,
    }

    #[derive(Debug)]
    pub struct Speed{
        history : VecDeque<TsI64>, // ts, num
        sum : i64,
    }
    
    impl Speed{
        pub fn clear(self: &mut Self) {
            self.sum = 0;
            self.history.clear();
        }

        // pub fn debug(self: &Self) -> (i64, i64) {
        //     let duration = if self.history.is_empty() {
        //         0
        //     } else {
        //         let d = self.history.back().unwrap().ts - self.history.front().unwrap().ts;
        //         if d > 0 {d} else {0}
        //     };

        //     (self.sum, duration)
        // }

        pub fn add(self: &mut Self, ts : i64, num : i64){
            self.history.push_back(TsI64{ts, num});
            self.sum += num;
        }
    
        pub fn cap(self : &mut Self, duration : i64){
            while !self.history.is_empty() {
                let d = self.history.back().unwrap().ts - self.history.front().unwrap().ts;
                if d > duration {
                    self.sum -= self.history.front().unwrap().num;
                    self.history.pop_front();
                } else {
                    break;
                }
            }
    
            if !self.history.is_empty() {
                let &ts1 = &self.history.back().unwrap().ts;
                let &ts2 = &self.history.front().unwrap().ts;
                if ts1 == ts2 && ts1 > duration {
                    self.history.push_front(TsI64{ts:ts1-duration, num:0});
                }
            }
    
        }

        pub fn average(self : &mut Self) -> i64{
            if self.history.is_empty() {
                0
            } else {
                let d = self.history.back().unwrap().ts - self.history.front().unwrap().ts;
                if d > 0 {1000 * (self.sum as i64) / d}
                else {0}
            }
        }

        pub fn cap_average(self : &mut Self, duration_ms : i64, now_ms : i64) -> i64{
            self.cap(duration_ms);

            if self.history.is_empty() {
                0
            } else {
                let d = now_ms - self.history.front().unwrap().ts;
                if d > 0 {1000 * (self.sum as i64) / d}
                else {0}
            }
        }
    }
    
    impl Default for Speed {
        fn default() -> Speed {
            Speed {
                sum : 0,
                history : VecDeque::new()
            }
        }
    }

    pub struct Pacer{
        kick_time : Instant,
        speed : u64,
    }
    
    impl Pacer{
        pub fn new(speed : u64) -> Self {
            Pacer {
                kick_time: Instant::now(),
                speed,
            }
        }
    
        pub fn kick(&mut self) {
            self.kick_time = Instant::now();
            self.kick_time.elapsed();
        }
    
        pub fn elapsed(&self) -> Duration {
            self.kick_time.elapsed()
        }
    
        pub fn kick_time(&self) -> &Instant {
            &self.kick_time
        }
    
        pub fn get_wait_milli(&self, n : u64) -> i64{
            if self.speed == 0 {
                return std::i64::MAX/2;
            }
            
            let expect = 1000 * n / self.speed;
            let diff = expect as i64 - self.kick_time.elapsed().as_millis() as i64;
            return diff;
        }
    
        pub async fn check_wait(&self, n : u64) {
            let diff = self.get_wait_milli(n);
            if diff > 0 {
                time::sleep(Duration::from_millis(diff as u64)).await;
            }
        }
    }
    
}


pub mod traffic{

    #[derive(Debug)]
    #[derive(Copy, Clone)]
    pub struct Traffic {
        pub packets: u64,
        pub bytes: u64,
    }
    
    
    impl Traffic {
        pub fn clear(& mut self) {
            self.packets = 0;
            self.bytes = 0;
        }
    
        pub fn add(& mut self, pkt:u64, bytes:u64) {
            self.packets += pkt;
            self.bytes += bytes;
        }
    }
    
    impl Default for Traffic {
        fn default() -> Traffic {
            Traffic {
                packets: 0,
                bytes: 0,
            }
        }
    }
    
    impl std::ops::Add for Traffic {
        type Output = Traffic;
        fn add(self, other: Traffic) -> Traffic {
            Traffic {
                packets: self.packets+other.packets, 
                bytes: self.bytes+other.bytes
            }
        }
    }
    
    impl std::ops::AddAssign for Traffic {
        fn add_assign(&mut self, other: Traffic)  {
            self.packets += other.packets;
            self.bytes += other.bytes;
        }
    }
    
    #[derive(Debug)]
    pub struct Speeds {
        pub packets: super::speed::Speed,
        pub bytes: super::speed::Speed,
    }

    impl Speeds {
        pub fn clear(& mut self) {
            self.packets.clear();
            self.bytes.clear();
        }

        pub fn add(& mut self, ts : i64, traffic : &Traffic){
            self.packets.add(ts, traffic.packets as i64);
            self.bytes.add(ts, traffic.bytes as i64);
        }
    }

    impl Default for Speeds {
        fn default() -> Speeds {
            Speeds {
                packets: super::speed::Speed::default(),
                bytes: super::speed::Speed::default(),
            }
        }
    }


    #[derive(Debug)]
    #[derive(Copy, Clone)]
    pub struct Transfer {
        pub output: Traffic,
        pub input: Traffic,
    }
    
    impl Transfer {
        pub fn clear(& mut self) {
            self.output.clear();
            self.input.clear();
        }
    }
    
    impl Default for Transfer {
        fn default() -> Transfer {
            Transfer {
                output: Traffic::default(),
                input: Traffic::default(),
            }
        }
    }
    
    impl std::ops::Add for Transfer {
        type Output = Transfer;
        fn add(self, other: Transfer) -> Transfer {
            Transfer {
                output: self.output+other.output, 
                input: self.input+other.input
            }
        }
    }
    
    impl std::ops::AddAssign for Transfer {
        fn add_assign(&mut self, other: Transfer)  {
            self.output += other.output;
            self.input += other.input;
        }
    }
    
}

#[derive(Debug)]
pub struct Latency {
    min: i64,
    max: i64,
    sum: i64,
    num: i64,
}

impl Latency {
    pub fn new() -> Self {
        Self {
            min: i64::MAX,
            max: i64::MIN,
            sum: 0, 
            num: 0,
        }
    }


    pub fn min(&self) -> i64 {
        self.min
    }

    pub fn max(&self) -> i64 {
        self.max
    }

    pub fn num(&self) -> i64 {
        self.num
    }

    pub fn sum(&self) -> i64 {
        self.sum
    }

    pub fn average(&self) -> i64 {
        if self.num > 0 {
            (self.sum + self.sum-1) / self.num
        } else {
            0
        }
    }

    pub fn observe(&mut self, latency: i64) {
        if latency < self.min {
            self.min = latency;
        }
        if latency > self.max {
            self.max = latency;
        }

        self.sum += latency;
        self.num += 1;
    }

    pub fn merge(&mut self, other: &Self) {
        if other.min < self.min {
            self.min = other.min;
        }

        if other.max > self.max {
            self.max = other.max;
        }

        self.sum += other.sum;
        self.num += other.num;
    }

    pub fn merge_iter<'a, I>(&mut self, mut iter: I) 
    where 
        I: Iterator<Item = &'a Self>
    {
        while let Some(other) = iter.next() {
            self.merge(other);
        }
    }

    pub fn from_iter<'a, I>(iter: I) -> Self
    where 
        I: Iterator<Item = &'a Self>
    {
        let mut self0 = Self::new();
        self0.merge_iter(iter);
        self0
    }
}

#[derive(Debug)]
pub struct MetricDuration {
    min: Duration,
    max: Duration,
    sum: Duration,
    num: i64,
}

impl MetricDuration {
    pub fn new() -> Self {
        Self {
            min: Duration::MAX,
            max: Duration::ZERO,
            sum: Duration::ZERO, 
            num: 0,
        }
    }


    pub fn min(&self) -> Duration {
        self.min
    }

    pub fn max(&self) -> Duration {
        self.max
    }

    pub fn num(&self) -> i64 {
        self.num
    }

    pub fn sum(&self) -> Duration {
        self.sum
    }

    pub fn average(&self) -> Duration {
        if self.num > 0 {
            self.sum / self.num as u32 // why NOT u64 ?
        } else {
            Duration::ZERO
        }
    }

    pub fn observe(&mut self, latency: Duration) {
        if latency < self.min {
            self.min = latency;
        }
        if latency > self.max {
            self.max = latency;
        }

        self.sum += latency;
        self.num += 1;
    }

    pub fn merge(&mut self, other: &Self) {
        if other.min < self.min {
            self.min = other.min;
        }

        if other.max > self.max {
            self.max = other.max;
        }

        self.sum += other.sum;
        self.num += other.num;
    }

    pub fn merge_iter<'a, I>(&mut self, mut iter: I) 
    where 
        I: Iterator<Item = &'a Self>
    {
        while let Some(other) = iter.next() {
            self.merge(other);
        }
    }

    pub fn from_iter<'a, I>(iter: I) -> Self
    where 
        I: Iterator<Item = &'a Self>
    {
        let mut self0 = Self::new();
        self0.merge_iter(iter);
        self0
    }
}

