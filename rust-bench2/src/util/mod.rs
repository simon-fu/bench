
use anyhow::{Result, anyhow, bail};

pub mod async_rt;
pub mod traffic;
pub mod ratio;
pub mod pacer;
pub mod atomic_count;
pub mod period_call;


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

/// normalize addresss as ip:port
pub fn normalize_addr(addr: &mut String, default_port: &str) -> Result<()> {
    let mut parts = addr.split(':');
    let ip = parts.next().ok_or_else(||anyhow!("empty"))?;
    let r = parts.next(); 
    match r {
        Some(port) => { 
            if parts.next().is_some() {
                bail!("too many \":\" in addrs")
            }

            if ip.is_empty() {
                if port.is_empty() {
                    // addr = ":"    
                    *addr = format!("0.0.0.0:{}", default_port);    
                } else {
                    // addr = ":0"
                    *addr = format!("0.0.0.0:{}", port);
                }   
            }
        },
        None => {
            // addr = "0.0.0.0"
            addr.push(':');
            addr.push_str(default_port);
        }
    }
    Ok(())
}


pub mod interval {
    use std::{time::{Instant, Duration}, sync::atomic::{AtomicI64, Ordering}};

    use super::period_call::{PeriodCall};


    // pub fn period_rate<RT, G, S, F>(state: S, interval: Duration, func: F) -> PeriodCallGuard
    // where
    //     RT: VRuntime,
    //     G: GetRateState + Send + Sync + 'static,
    //     <G as GetRateState>::Output: Send,
    //     S: AsRef<G> + Send + 'static ,
    //     F: FnMut(&S, bool, <<G as GetRateState>::Output as CalcRate>::Output) + Send + 'static,
    // {
    //     period_call::<RT, _>(RateLambda {
    //         estimator: RateEstimator::with_init(interval, state.as_ref().get_rate_state()),
    //         state,
    //         func,
    //     })
    // }

    // pub fn period_rate<G, S, F>(state: S, interval: Duration, func: F) -> PeriodRate<G, S, F>
    // where
    //     G: GetRateState + Send + Sync + 'static,
    //     <G as GetRateState>::Output: Send,
    //     S: AsRef<G> + Send + 'static ,
    //     F: FnMut(&S, bool, <<G as GetRateState>::Output as CalcRate>::Output) + Send + 'static,
    // {
    //     PeriodRate {
    //         estimator: RateEstimator::with_init(interval, state.as_ref().get_rate_state()),
    //         state,
    //         func,
    //     }
    // }

    pub struct PeriodRate<G, S, F> 
    where
        G: GetRateState,
        S: AsRef<G>,
    {
        estimator: RateEstimator<G::Output>,
        state: S, // Arc<G>,
        func: F,
    }

    impl<G, S, F> PeriodRate<G, S, F> 
    where
        G: GetRateState + Send + Sync + 'static,
        <G as GetRateState>::Output: Send,
        S: AsRef<G> + Send + 'static ,
        // F: FnMut(&S, bool, Option<<<G as GetRateState>::Output as CalcRate>::Rate>) + Send + 'static,
        F: FnMut(
            &S, 
            bool, 
            (<<G as GetRateState>::Output as CalcRate>::Delta, Option<<<G as GetRateState>::Output as CalcRate>::Rate>) 
        ) + Send + 'static,
    {
        pub fn new(state: S, func: F) -> Self {
            Self {
                estimator: RateEstimator::with_init(state.as_ref().get_rate_state()),
                state,
                func,
            }
        }
    }

    impl<G, S, F> PeriodCall for PeriodRate<G, S, F> 
    where
        G: GetRateState,
        S: AsRef<G>,
        F: FnMut(
            &S, 
            bool, 
            (<<G as GetRateState>::Output as CalcRate>::Delta, Option<<<G as GetRateState>::Output as CalcRate>::Rate>) 
        ) + Send + 'static,
    {
        fn next(&mut self) -> Duration {
            let now = Instant::now();
            let next = self.estimator.next_time();
            if now < next {
                next - now
            } else {
                Duration::ZERO
            }
        }

        fn call(&mut self, completed: bool) { 
            let now = Instant::now();
            let r = self.estimator.estimate2(now, self.state.as_ref());
            (self.func)(&self.state, completed, r);

            // if let Some(rate) = r {
                
            // } else if completed {
            //     (self.func)(&self.state, completed, None);
            // }
        }
    }


    pub trait CalcRate {
        type Delta;
        type Rate;
        fn calc_rate(&self, delta: &Self::Delta, duration: Duration) -> Self::Rate ;
        fn calc_delta_only(&self, new_state: &Self) -> Self::Delta ;
    }

    pub trait GetRateState { 
        type Output: CalcRate;
        fn get_rate_state(&self) -> Self::Output;
    }


    pub struct RateEstimator<R> { 
        next_time: Instant,
        interval: Duration,
        last: R,
    }

    impl<R> Default for RateEstimator<R> 
    where
        R: Default,
    {
        fn default() -> Self { 
            let interval = Self::DEFAULT_INTERVAL;
            Self {
                next_time: Instant::now() + interval,
                interval,
                last: Default::default(),
            }
        }
    }

    impl<R> RateEstimator<R> 
    where
        R: Default,
    {
        pub fn with_interval(interval: Duration) -> Self {
            Self::custom(interval, Default::default())
        }
    }

    impl<R> RateEstimator<R> {

        pub fn with_init(last: R) -> Self {
            Self::custom(Self::DEFAULT_INTERVAL, last)
        }

        pub fn custom(interval: Duration, last: R) -> Self {
            Self {
                next_time: Instant::now() + interval,
                interval,
                last,
            }
        }
    
        pub fn next_time(&self) -> Instant {
            self.next_time
        }
    
        fn reset(&mut self, now: Instant) {
            self.next_time = now + self.interval;
        }
    
        const DEFAULT_INTERVAL: Duration = Duration::from_millis(1000);
    }

    impl<R> RateEstimator<R> 
    where
        R: CalcRate,
    {
        pub fn estimate<G>(&mut self, now: Instant, new_value: &G) -> Option<R::Rate> 
        where
            G: GetRateState<Output = R>
        {
            self.estimate2(now, new_value).1
            // let new_state = new_value.get_rate_state();

            // if now < self.next_time {
            //     return None;
            // }
            // let d = now - self.next_time + self.interval;

            // if d.is_zero() {
            //     return None;
            // }

            // let delta = self.last.calc_delta_only(&new_state);
            // let rate = self.last.calc_rate(&delta, d);
            
    
            // self.last = new_state;

            // self.reset(now);
    
            // return Some(rate);
        }

        pub fn estimate2<G>(&mut self, now: Instant, new_value: &G) -> (R::Delta, Option<R::Rate>)
        where
            G: GetRateState<Output = R>
        {
            let new_state = new_value.get_rate_state();
            
            let delta = self.last.calc_delta_only(&new_state);

            let rate = if now < self.next_time { 
                None

            } else {                
                let d = now - self.next_time + self.interval;
                let rate = self.last.calc_rate(&delta, d);
                
                self.last = new_state;
                self.reset(now);

                Some(rate)
            };
          

            return (delta, rate);
        }
    }


    #[derive(Debug, Default, Clone, Copy)]
    pub struct I64Rate {
        pub delta: i64,
        pub rate: i64,
    }

    impl CalcRate for i64 {
        type Rate = i64;
        type Delta = i64;

        fn calc_rate(&self, delta: &Self::Delta, duration: Duration) -> Self::Rate {
            
            let millis = duration.as_millis() as i64;
            // let delta = *new_state - *self;

            let rate = if *delta >= 0 {
                (delta * 1000 + millis - 1) / millis
            } else {
                (delta * 1000 - millis + 1) / millis
            };

            rate
            // I64Rate {
            //     delta,
            //     rate: if delta >= 0 {
            //         (delta * 1000 + millis - 1) / millis
            //     } else {
            //         (delta * 1000 - millis + 1) / millis
            //     }
            // }
        }

        fn calc_delta_only(&self, new_state: &Self) -> Self::Delta  {
            *new_state - *self
        }

        
    }

    impl GetRateState for i64 {
        type Output = i64;
        fn get_rate_state(&self) -> Self::Output {
            *self
        }
    }

    impl GetRateState for AtomicI64 {
        type Output = i64;
        fn get_rate_state(&self) -> Self::Output {
            self.load(ORDERING)
        }
    }

    const ORDERING: Ordering = Ordering::Relaxed;
}