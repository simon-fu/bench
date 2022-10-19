
use std::{time::Duration, sync::{atomic::{Ordering, AtomicI64}}, ops::{Deref, DerefMut}};

use array_init::array_init;

use crate::util::traffic::ToHuman;

use super::period_rate::CalcRate;


#[derive(Debug)]
pub struct GArray<T, const N: usize>(pub [T; N]);

impl<T, const N: usize> Clone for GArray<T, N> 
where
    T: Clone,
{
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T, const N: usize> Copy for GArray<T, N> 
where
    T: Copy,
{

}


impl<T, const N: usize> Default for GArray<T, N> 
where
    T: Default,
{
    fn default() -> Self { 
        Self(array_init(|_i| T::default()))
    }
}

impl <T, const N: usize> Deref for GArray<T, N>  {
    type Target = [T; N];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl <T, const N: usize> DerefMut for  GArray<T, N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}


pub type I64Atomics<const N: usize> = GArray<AtomicI64, N>; 
pub type I64Values<const N: usize> = GArray<i64, N>;

impl<const N: usize> I64Atomics<N> {
    pub fn get_at(&self, n: usize) -> i64 {
        self[n].load(ORDERING)
    }

    pub fn add_at(&self, n: usize, val: i64)  {
        self[n].fetch_add(val, ORDERING);
    }

    pub fn reset_to(&self, value: i64) {
        for item in &self.0 {
            item.store(value, ORDERING);
        }
    }

    pub fn snapshot(&self) -> I64Values<N> {
        GArray(array_init(|i| self[i].load(ORDERING)))
    }


}

impl<const N: usize> I64Values<N> {
    pub fn get_at(&self, n: usize) -> i64 {
        self[n]
    }

    pub fn is_equ2(&self, n1: usize, n2: usize) -> bool {
        self[n1] == self[n2]
    }

    pub fn is_zero(&self) -> bool {
        for v in &self.0 {
            if *v != 0 {
                return false;
            }
        }
        true
    }
}



// impl<const N: usize> Snapshot for I64Atomics<N> {
//     type Output = I64Values<N>; 
//     fn snapshot(&self) -> Self::Output {
//         GArray(array_init(|i| self[i].load(ORDERING)))
//     }
// }

// impl<const N: usize> IsEqu2 for I64Values<N> {
//     fn is_equ2(&self, n1: usize, n2: usize) -> bool {
//         self[n1] == self[n2]
//     }
// }

// impl<const N: usize> IsZero for I64Values<N> {
//     fn is_zero(&self) -> bool {
//         for v in &self.0 {
//             if *v != 0 {
//                 return false;
//             }
//         }
//         true
//     }
// }

// impl<const N: usize> AddAt for I64Atomics<N> {
//     type Value = i64;
//     fn add_at(&self, n: usize, val: Self::Value)  {
//         self[n].fetch_add(val, ORDERING);
//     }
// }




pub struct I64AtomicsRateHuman<'a, const N:usize> {
    atomic: &'a I64Atomics<N>,
    delta: &'a I64Values<N>,
    rate: &'a Option<I64Values<N>>,
    names: &'a [&'a str; N],
}

impl<'a, const N:usize> I64AtomicsRateHuman<'a, N> { 

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

impl<'a, const N:usize> std::fmt::Display for I64AtomicsRateHuman<'a, N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt_me(f)
    }
}

impl<'a, const N:usize> std::fmt::Debug for I64AtomicsRateHuman<'a, N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt_me(f)
    }
}



impl<const N: usize> ToRateHuman for GArray<AtomicI64, N> {
    type Name<'a> = [&'a str; N];
    type Delta<'a> = GArray<i64, N>;
    type Rate<'a> = GArray<i64, N>;

    type Output<'a> = I64AtomicsRateHuman<'a, N> where Self: 'a;

    fn to_rate_human<'a>(
        &'a self, 
        names: &'a Self::Name<'a>, 
        delta: &'a Self::Delta<'a>, 
        rate: &'a Option<Self::Rate<'a>>
    ) -> Self::Output<'a>  {
        I64AtomicsRateHuman {
            atomic: self,
            names,
            delta,
            rate,
        }
    }
}


// pub trait Snapshot {
//     type Output;
//     fn snapshot(&self) -> Self::Output;
// }

// pub trait IsEqu2 {
//     fn is_equ2(&self, n1: usize, n2: usize) -> bool ;
// }

// pub trait IsZero {
//     fn is_zero(&self) -> bool ;
// }

// pub trait AddAt {
//     type Value;
//     fn add_at(&self, n: usize, val: Self::Value) ;
// }

pub trait ToRateHuman { 
    type Name<'a> where Self: 'a;
    type Delta<'a> where Self: 'a;
    type Rate<'a> where Self: 'a;
    type Output<'a>  where Self: 'a;

    fn to_rate_human<'a>(
        &'a self, 
        name: &'a Self::Name<'a>, 
        delta: &'a Self::Delta<'a>, 
        rate: &'a Option<Self::Rate<'a>>
    ) -> Self::Output<'a> ;
}


impl<const N: usize> CalcRate for I64Values<N> { 
    type Delta = I64Values<N>;
    type Rate = I64Values<N>;

    fn calc_rate(&self, delta: &Self::Delta, duration: Duration) -> Self::Rate  { 
        GArray(array_init(|i| self[i].calc_rate(&delta[i], duration)))
    }

    fn calc_delta_only(&self, new_state: &Self) -> Self::Delta  { 
        GArray(array_init(|i| self[i].calc_delta_only(&new_state[i])))
    }
}

const ORDERING: Ordering = Ordering::Relaxed;



