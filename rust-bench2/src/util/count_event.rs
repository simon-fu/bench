
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering::Relaxed;
use event_listener::Event;





#[derive(Clone)]
pub struct CountEvent {
    shared: Arc<CountShared>
}

impl CountEvent {
    pub fn new() -> Self {
        Self {
            shared: Arc::new(CountShared {
                event: Event::new(),
                count: AtomicU64::new(0),
            }),
        }
    }

    pub fn inc_and_signal(&self, n: u64) {
        self.shared.count.fetch_add(n, Relaxed);
        self.shared.event.notify(usize::MAX);
    }

    pub fn watcher(&self) -> CountWatcher { 
        CountWatcher {
            val: self.shared.count.load(Relaxed),
            shared: self.shared.clone(),
        }
    }

    pub async fn watch_until(&self, val: u64) -> u64 {
        loop { 
            let curr = self.shared.count.load(Relaxed); 
            if curr >= val { 
                return curr;
            }

            let listener = self.shared.event.listen();
            listener.await; 
        }
    }
}

pub struct CountWatcher {
    shared: Arc<CountShared>, 
    val: u64,
}

impl CountWatcher { 

    pub fn value(&self) -> u64 {
        self.val
    }

    pub async fn watch(&mut self) -> u64 { 
        loop { 
            let val = self.shared.count.load(Relaxed); 
            if self.val != val { 
                self.val = val;
                return val;
            }

            let listener = self.shared.event.listen();
            listener.await; 
        }
    }
}

struct CountShared {
    event: Event,
    count: AtomicU64,
}
