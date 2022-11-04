use std::time::Duration;


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

