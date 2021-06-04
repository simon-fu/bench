// pub mod xrs;

#![allow(dead_code)]

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
    use std::collections::VecDeque;

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