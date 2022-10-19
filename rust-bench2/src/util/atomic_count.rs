
use std::{time::Duration, sync::{atomic::{Ordering, AtomicI64}}, pin::Pin, task::Poll, ops::{Deref, DerefMut}};

use futures::task::AtomicWaker;

use array_init::array_init;

use crate::util::traffic::ToHuman;

use super::period_rate::CalcRate;

const ORDERING: Ordering = Ordering::Relaxed;

#[derive(Debug)]
pub struct AtomicI64Array<const N: usize>([AtomicI64; N]);

impl<const N: usize> Default for AtomicI64Array<N> {
    fn default() -> Self { 
        Self(array_init(|_i| AtomicI64::new(0)))
    }
}

impl <const N: usize> Deref for  AtomicI64Array<N> {
    type Target = [AtomicI64; N];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// impl <const N: usize> DerefMut for  AtomicI64Array<N> {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         &mut self.0
//     }
// }

impl<const N: usize> AtomicI64Array<N> {
    pub fn snapshot(&self) -> I64Array<N> {
        I64Array(array_init(|i| self[i].load(ORDERING)))
    }

    pub fn reset(&self, value: i64) {
        for item in &self.0 {
            item.store(value, ORDERING);
        }
    }
}



#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct I64Array<const N: usize>([i64; N]);

impl<const N: usize> Default for I64Array<N> {
    fn default() -> Self { 
        Self(array_init(|_i| 0))
    }
}

impl <const N: usize> Deref for  I64Array<N> {
    type Target = [i64; N];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl <const N: usize> DerefMut for  I64Array<N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<const N: usize> I64Array<N> { 
    pub fn is_zero(&self) -> bool {
        for v in &self.0 {
            if *v != 0 {
                return false;
            }
        }
        true
    }

    pub fn is_equ2(&self, n1: usize, n2: usize) -> bool {
        self[n1] == self[n2]
    }
}

impl<const N: usize> CalcRate for I64Array<N> { 
    type Delta = I64Array<N>;
    type Rate = I64Array<N>;

    fn calc_rate(&self, delta: &Self::Delta, duration: Duration) -> Self::Rate  { 
        I64Array(array_init(|i| self[i].calc_rate(&delta[i], duration)))
    }

    fn calc_delta_only(&self, new_state: &Self) -> Self::Delta  { 
        I64Array(array_init(|i| self[i].calc_delta_only(&new_state[i])))
    }
}


pub struct AtomicI64ArrayRateHuman<'a, const N:usize> {
    atomic: &'a AtomicI64Array<N>,
    delta: &'a I64Array<N>,
    rate: &'a Option<I64Array<N>>,
    names: &'a [&'a str; N],
}

impl<'a, const N:usize> AtomicI64ArrayRateHuman<'a, N> {
    fn fmt_me(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { 
        let rate = self.rate.as_ref().unwrap_or_else (||&self.delta);

        f.write_fmt(format_args!(
            "{} {}/{:+}/{:+}", 
            self.names[0],
            self.atomic[0].load(ORDERING).to_human(), 
            self.delta[0].to_human(), 
            rate[0].to_human(),
        ))?;

        for i in 1..N {
            f.write_fmt(format_args!(
                ", {} {}/{:+}/{:+}", 
                self.names[i],
                self.atomic[i].load(ORDERING).to_human(), 
                self.delta[i].to_human(), 
                rate[i].to_human(),
            ))?;
        }

        Ok(())
    }
}

impl<'a, const N:usize> std::fmt::Display for AtomicI64ArrayRateHuman<'a, N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt_me(f)
    }
}

impl<'a, const N:usize> std::fmt::Debug for AtomicI64ArrayRateHuman<'a, N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt_me(f)
    }
}



#[derive(Debug, Default)]
pub struct AtomicCounts<const N: usize> {
    waker: AtomicWaker,
    atomic: AtomicI64Array<N>,
}

impl <const N: usize> AtomicCounts<N> {
    pub fn reset(&self) { 
        self.atomic.reset(0);
    }

    pub fn snapshot(&self) -> I64Array<N> {
        self.atomic.snapshot()
    }

    pub fn add_only(&self, n: usize, delta: u64) {
        let delta = delta as i64;
        self.atomic[n].fetch_add(delta, ORDERING);
    }

    pub fn add_and_wake(&self, n: usize, delta: u64) {
        self.add_only(n, delta);
        self.waker.wake();
    }

    pub fn get(&self, n: usize) -> i64 {
        self.atomic[n].load(ORDERING)
    }

    pub fn rate_human_with_names<'a>(
        &'a self, 
        names: &'a [&'a str; N], 
        delta: &'a I64Array<N>, 
        rate: &'a Option<I64Array<N>>
    ) -> AtomicI64ArrayRateHuman<'a, N> {

        AtomicI64ArrayRateHuman {
            atomic: &self.atomic,
            delta,
            rate,
            names,
        }
    }

    pub fn watch(&self) -> AtomicWatcher<'_, N> {
        AtomicWatcher::new(self)
    }
}


#[derive(Debug)]
pub struct AtomicWatcher<'a, const N: usize> { 
    state: &'a AtomicCounts<N>,
    last: I64Array<N>,
}

impl<'a, const N: usize> AtomicWatcher<'a, N> {
    fn new(state: &'a AtomicCounts<N>) -> Self {
        Self { 
            last: state.atomic.snapshot(),
            state, 
        }
    }

    pub fn state(&self) -> &AtomicCounts<N> {
        self.state
    }

    pub fn last(&self) -> &I64Array<N>{
        &self.last
    }

    pub async fn wait_for(&mut self) {
        self.await
    }
}

impl<'a, const N: usize> std::future::Future for AtomicWatcher<'a, N> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<()> {
        let new_value = self.state.atomic.snapshot();

        if new_value != self.last {
            self.last = new_value;
            return Poll::Ready(());
        }

        self.state.waker.register(cx.waker());

        let new_value = self.state.atomic.snapshot();

        if new_value != self.last {
            self.last = new_value;
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

pub trait ToAtomicRateHuman<const N: usize> {
    type Output<'a>  where Self: 'a;
    
    // fn to_human<'a>(&'a self) -> Self::Output<'a>;

    fn to_atomic_rate_human<'a>(
        &'a self, 
        // names: &'a [&'a str; N], 
        delta: &'a I64Array<N>, 
        rate: &'a Option<I64Array<N>>
    ) -> AtomicI64ArrayRateHuman<'a, N> ;
}

// pub mod conn_count {
//     use std::ops::Deref;

//     use super::{AtomicCounts, ToAtomicRateHuman, AtomicI64ArrayRateHuman, I64Array};

//     pub const NAMES: [&str; 3] = ["total", "active", "done",];
//     pub const TOTAL: usize = 0;
//     pub const ACTIVE: usize = 1;
//     pub const DONE: usize = 2;

//     pub type ConnValues = I64Array<3>;

//     #[derive(Debug, Default)]
//     pub struct ConnCount(AtomicCounts<3>);

//     impl Deref for  ConnCount {
//         type Target = AtomicCounts<3>;
    
//         fn deref(&self) -> &Self::Target {
//             &self.0
//         }
//     }

//     impl ToAtomicRateHuman<3> for ConnCount {
//         type Output<'a> = AtomicI64ArrayRateHuman<'a, 3> where Self: 'a;

//         fn to_atomic_rate_human<'a>(
//             &'a self, 
//             delta: &'a super::I64Array<3>, 
//             rate: &'a Option<super::I64Array<3>>
//         ) -> super::AtomicI64ArrayRateHuman<'a, 3>  {
//             self.rate_human_with_names(&NAMES, delta, rate)
//         }
//     }
// }



// #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
// pub struct CountState {
//     pub all: i64,
//     // pub active: i64,
//     pub done: i64,
// }

// impl CountState {
//     pub fn is_zero(&self) -> bool {
//         self.all == 0  
//         && self.done == 0
//         // && self.active == 0
//     }

//     pub fn all_done(&self) -> bool {
//         self.all == self.done
//     }
// }

// impl CalcRate for CountState {
//     type Delta = CountState;
//     type Rate = CountState;

//     fn calc_rate(&self, delta: &Self::Delta, duration: Duration) -> Self::Rate  {
//         CountState {
//             all: self.all.calc_rate(&delta.all, duration),
//             // active: self.all.calc_rate(&delta.active, duration),
//             done: self.all.calc_rate(&delta.done, duration),
//         }
//     }

//     fn calc_delta_only(&self, new_state: &Self) -> Self::Delta  { 
//         CountState {
//             all: self.all.calc_delta_only(&new_state.all),
//             // active: self.active.calc_delta_only(&new_state.active),
//             done: self.done.calc_delta_only(&new_state.done),
//         }
//     }
// }

// #[derive(Debug, Default)]
// pub struct AtomicCount {
//     waker: AtomicWaker,
//     all: AtomicI64,
//     // active: AtomicI64,
//     done: AtomicI64,
// }

// impl AtomicCount {
//     const ORDERING: Ordering = Ordering::Relaxed;

//     pub fn reset(&self) {
//         self.all.store(0, Self::ORDERING);
//         // self.active.store(0, Self::ORDERING);
//         self.done.store(0, Self::ORDERING);
//     }

//     pub fn add_only(&self, delta: u64) {
//         let delta = delta as i64;
//         self.all.fetch_add(delta, Self::ORDERING);
//         // self.active.fetch_add(delta, Self::ORDERING);
//     }

//     pub fn sub_and_wake(&self, delta: u64) {
//         let delta = delta as i64;
//         // self.active.fetch_sub(delta, Self::ORDERING);
//         self.done.fetch_add(delta, Self::ORDERING);
//         self.waker.wake();
//     }

//     pub fn get_total(&self) -> i64 {
//         self.all.load(Self::ORDERING)
//     }

//     pub fn get(&self) -> CountState { 
//         CountState {
//             all: self.all.load(Self::ORDERING),
//             // active: self.active.load(Self::ORDERING),
//             done: self.done.load(Self::ORDERING),
//         }
//     }

//     pub fn rate_human<'a>(&'a self, delta: &'a CountState, rate: &'a Option<CountState>) -> CountRateHuman<'a> {
//         CountRateHuman(self, delta, rate)
//     }

//     pub fn watch(&self) -> CountWatcher<'_> {
//         CountWatcher::new(self)
//     }
// }

// pub struct CountRateHuman<'a>(&'a AtomicCount, &'a CountState, &'a Option<CountState>); 

// impl<'a> CountRateHuman<'a> {
//     fn fmt_me(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { 
//         let curr = self.0.get();
//         let delta = self.1;
//         let rate = self.2.as_ref().unwrap_or_else (||&delta);

//         f.write_fmt(format_args!(
//             "active {}", 
//             (curr.all - curr.done).to_human(), 
//         ))?;

//         f.write_fmt(format_args!(
//             ", all {}/{:+}/{:+}", 
//             curr.all.to_human(), 
//             delta.all.to_human(), 
//             rate.all.to_human(),
//         ))?;

//         f.write_fmt(format_args!(
//             ", done {}/{:+}/{:+}", 
//             curr.done.to_human(), 
//             delta.done.to_human(), 
//             rate.done.to_human(),
//         ))?;

//         // f.write_fmt(format_args!(
//         //     ", active {}/{:+}/{:+}", 
//         //     curr.active.to_human(), 
//         //     delta.active.to_human(), 
//         //     rate.active.to_human(),
//         // ))?;
//         Ok(())
//     }
// }

// impl<'a> std::fmt::Display for CountRateHuman<'a> {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         self.fmt_me(f)
//     }
// }

// impl<'a> std::fmt::Debug for CountRateHuman<'a> {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         self.fmt_me(f)
//     }
// }

// #[derive(Debug)]
// pub struct CountWatcher<'a> { 
//     state: &'a AtomicCount,
//     last: CountState,
// }

// impl<'a> CountWatcher<'a> {
//     fn new(state: &'a AtomicCount) -> Self {
//         Self { 
//             last: state.get(),
//             state, 
//         }
//     }

//     pub fn state(&self) -> &AtomicCount {
//         self.state
//     }

//     pub fn last(&self) -> CountState{
//         self.last
//     }

//     pub async fn wait_for(&mut self) {
//         self.await
//     }
// }

// impl<'a> std::future::Future for &mut CountWatcher<'a> {
//     type Output = ();

//     fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<()> {
//         let new_value = self.state.get();

//         if new_value != self.last {
//             self.last = new_value;
//             return Poll::Ready(());
//         }

//         self.state.waker.register(cx.waker());

//         let new_value = self.state.get();

//         if new_value != self.last {
//             self.last = new_value;
//             Poll::Ready(())
//         } else {
//             Poll::Pending
//         }
//     }
// }

