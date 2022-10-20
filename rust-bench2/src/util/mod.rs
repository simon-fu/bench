
use anyhow::{Result, anyhow, bail};

pub mod async_rt;
pub mod traffic;
pub mod ratio;
pub mod pacer;
pub mod quota;
pub mod atomic_count;
pub mod count_event;
pub mod period_call;
pub mod period_rate;
pub mod log;


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

