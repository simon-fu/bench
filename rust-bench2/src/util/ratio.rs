// use num_rational::Ratio;
// use num::Integer;

#[derive(Debug, Clone, Copy)]
pub struct Ratio<T> {
    numer: T,
    denom: T,
}

impl<T> Ratio<T> {
    pub fn new(numer: T, denom: T) -> Self {
        Self { numer, denom }
    }

    #[inline]
    pub const fn numer(&self) -> &T {
        &self.numer
    }

    #[inline]
    pub const fn denom(&self) -> &T {
        &self.denom
    }
}

pub type Rate = Ratio<u64>;

impl Default for Rate {
    fn default() -> Self {
        Self { numer: 0, denom: 1 }
    }
}
