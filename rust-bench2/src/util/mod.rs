

pub mod traffic;
pub mod async_rt;

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
