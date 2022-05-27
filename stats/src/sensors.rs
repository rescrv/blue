use std::fmt;
use std::sync::Mutex;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use crate::moments;

////////////////////////////////////////////// Counter /////////////////////////////////////////////

pub struct Counter {
    count: AtomicU64,
}

impl Counter {
    pub fn new() -> Counter {
        Counter {
            count: AtomicU64::new(0),
        }
    }

    pub fn click(&self) {
        self.count(1)
    }

    pub fn count(&self, x: u64) {
        self.count.fetch_add(x, Ordering::SeqCst);
    }
}

impl fmt::Debug for Counter {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "counter({})", self.count.load(Ordering::SeqCst))
    }
}

/////////////////////////////////////////////// Gauge //////////////////////////////////////////////

pub struct Gauge {
    value: AtomicU64,
}

impl Gauge {
    pub fn new(init: f64) -> Gauge {
        Gauge {
            value: AtomicU64::new(init.to_bits()),
        }
    }

    pub fn set(&self, x: f64) {
        self.value.store(x.to_bits(), Ordering::SeqCst);
    }

    pub fn get(&self) -> f64 {
        let u = self.value.load(Ordering::SeqCst);
        f64::from_bits(u)
    }
}

impl fmt::Debug for Gauge {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "gauge({})", self.get())
    }
}

////////////////////////////////////////////// Moments /////////////////////////////////////////////

pub struct Moments {
    value: Mutex<moments::Moments>,
}

impl Moments {
    pub fn new() -> Self {
        Self {
            value: Mutex::new(moments::Moments::default()),
        }
    }

    pub fn add(&self, x: f64) {
        let mut moments = self.value.lock().unwrap();
        moments.push(x);
    }

    fn get(&self) -> moments::Moments {
        self.value.lock().unwrap().clone()
    }
}

impl fmt::Debug for Moments {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let moments = self.get();
        write!(f, "moments({}, {}, {}, {}, {})",
            moments.n, moments.m1, moments.m2, moments.m3, moments.m4)
    }
}
