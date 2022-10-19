
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

    use super::Quota;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

        
    }
}
