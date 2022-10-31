use anyhow::{bail, Result};
use event_listener::Event;
use rust_bench::util::{atomic_count::{I64Atomics, I64Values, ToRateHuman}, traffic::{AtomicTraffic, Traffic, TrafficRate, TrafficOp, ToHuman}, period_rate::{GetRateState, CalcRate, PeriodRate}, async_rt::async_tcp::VRuntime, period_call::{period_call, PeriodCallGuard}};
use tracing::info;

use std::sync::Arc;


pub fn period_stati<RT, C>(ctx: C) -> PeriodCallGuard<RT>
where
    RT: VRuntime,
    C: GetConnsStati + Send + 'static,
{
    let ctx = RateWrapper {
        inner: ctx,
        event: Event::new(),
    };

    let period_job = PeriodRate::new(ctx, |ctx, _completed, (delta, rate)| { 

        if delta.traffic.packets() != 0 { 
            if let Some(rate) = &rate {
                info!("traffic: {}", rate.traffic.to_human());
            }
        }
        
        if !delta.conns.is_zero() { 
            let rate = rate.map(|v|v.conns);
            let human = ctx.as_ref().conns().to_rate_human(&NAMES, &delta.conns, &rate);
            info!("connections: {}", human);
        }

        ctx.inner.get_conns_stati().event.notify(usize::MAX);
        ctx.event.notify(usize::MAX);
    });

    let guard = period_call::<RT, _>(period_job);
    guard
}

pub trait GetConnsStati {
    fn get_conns_stati(&self) -> &ConnsStati;
}

impl<T> GetConnsStati for Arc<T> 
where
    T: GetConnsStati
{
    fn get_conns_stati(&self) -> &ConnsStati {
        self.as_ref().get_conns_stati()
    }
}

pub type Conns = I64Atomics<CNUM>;

#[derive(Debug, Default)]
pub struct ConnsStati {
    conns: Conns,
    traffic: AtomicTraffic, 
    event: Event,
}

impl ConnsStati {
    pub fn conns(&self) -> &Conns {
        &self.conns
    }

    pub fn traffic(&self) -> &AtomicTraffic {
        &self.traffic
    }

    pub async fn wait_next_period(&self) { 
        self.event.listen().await
    }

    pub fn is_all_stop(&self) -> bool {
        let snapshot = self.conns.snapshot();
        let total = snapshot.get_at(TOTAL);
        if total > 0 {
            let inactives = snapshot.get_at(INACTIVE);
            let dones = snapshot.get_at(DONE);
            (inactives + dones) == total
        } else {
            false
        }
    }

    pub async fn wait_for_conns_setup(&self) -> bool {
        loop {
            self.wait_next_period().await; 
            let snapshot = self.conns().snapshot();
            
            if snapshot.is_equ2(TOTAL, ACTIVE) {
                return true;
            }
    
            if let Some(n) = snapshot.get(DONE) {
                if *n > 0 {
                    return false;
                }
            }
        }
    }

    pub async fn wati_for_conns_inactive(&self) -> Result<()> {
        loop { 
            self.wait_next_period().await; 
            let snapshot = self.conns().snapshot();
    
            if snapshot.is_equ2(TOTAL, INACTIVE) {
                break;
            }

            if snapshot.get_at(DONE) > 0 {
                bail!("waiting for connections inactive but some connections broken")
            }
        }
    
        Ok(())
    }

    pub async fn wati_for_conns_done<RT>(&self, guard: PeriodCallGuard<RT>) 
    where
        RT: VRuntime,
    {
        loop { 
            self.wait_next_period().await; 
            let snapshot = self.conns().snapshot();
    
            if snapshot.is_equ2(TOTAL, DONE) {
                break;
            }
        }
    
        if let Some(task) = guard.into_task() {
            task.await;
        }
    
    }
}

impl GetRateState for ConnsStati {
    type Output = RateState;
    fn get_rate_state(&self) -> Self::Output { 
        RateState {
            conns: self.conns.snapshot(),
            traffic: self.traffic.get_traffic(),
        }
    }
}


pub const NAMES: [&str; CNUM] = ["total", "active", "inactive", "done",];
pub const TOTAL: usize = 0;
pub const ACTIVE: usize = 1;
pub const INACTIVE: usize = 2;
pub const DONE: usize = 3;
const CNUM: usize = DONE+1;


pub struct RateState {
    pub conns: I64Values<CNUM> ,
    pub traffic: Traffic,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Rate {
    pub conns: I64Values<CNUM>,
    pub traffic: TrafficRate,
}

impl CalcRate for RateState {
    type Delta = RateState;
    type Rate = Rate;

    fn calc_rate(&self, delta: &Self::Delta, duration: std::time::Duration) -> Self::Rate  {
        let conns = self.conns.calc_rate(&delta.conns, duration);
        Rate {
            conns,
            traffic: self.traffic.calc_rate(&delta.traffic, duration),
        }
    }

    fn calc_delta_only(&self, new_state: &Self) -> Self::Delta  {
        RateState {
            conns: self.conns.calc_delta_only(&new_state.conns),
            traffic: self.traffic.calc_delta_only(&new_state.traffic),
        }
    }
}




struct RateWrapper<T>{
    inner: T,
    event: Event,
}

impl<T> AsRef<ConnsStati> for RateWrapper<T> 
where
    T: GetConnsStati
{
    fn as_ref(&self) -> &ConnsStati {
        self.inner.get_conns_stati()
    }
}


impl<T> GetRateState for RateWrapper<T> 
where
    T: GetConnsStati
{
    type Output = RateState;

    fn get_rate_state(&self) -> Self::Output {
        self.inner.get_conns_stati().get_rate_state()
    }
}

