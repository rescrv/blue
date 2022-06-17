use std::fmt;
use std::sync::atomic::{AtomicBool,AtomicU64};
use std::sync::atomic::Ordering;

use crate::moments;

////////////////////////////////////////////// Counter /////////////////////////////////////////////

pub struct Counter {
    count: AtomicU64,
    init: AtomicBool,
}

impl Counter {
    pub const fn new() -> Counter {
        Counter {
            count: AtomicU64::new(0),
            init: AtomicBool::new(false),
        }
    }

    #[inline(always)]
    pub fn click(&self) {
        self.count(1)
    }

    #[inline(always)]
    pub fn count(&self, x: u64) {
        self.count.fetch_add(x, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn read(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }
}

impl fmt::Debug for Counter {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "counter({})", self.read())
    }
}

/////////////////////////////////////////////// Gauge //////////////////////////////////////////////

const GAUGE_INIT: u64 = 0;

pub struct Gauge {
    value: AtomicU64,
    init: AtomicBool,
}

impl Gauge {
    pub const fn new() -> Gauge {
        Gauge {
            value: AtomicU64::new(GAUGE_INIT),
            init: AtomicBool::new(false),
        }
    }

    #[inline(always)]
    pub fn set(&self, x: f64) {
        self.value.store(x.to_bits(), Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn read(&self) -> f64 {
        let u = self.value.load(Ordering::Relaxed);
        f64::from_bits(u)
    }
}

impl fmt::Debug for Gauge {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "gauge({})", self.read())
    }
}

////////////////////////////////////////////// Moments /////////////////////////////////////////////

pub struct Moments {
    spin: AtomicBool,
    init: AtomicBool,
    value: moments::Moments,
}

impl Moments {
    pub const fn new() -> Self {
        Self {
            spin: AtomicBool::new(false),
            init: AtomicBool::new(false),
            value: moments::Moments::new(),
        }
    }

    pub fn add(&self, x: f64) {
        self.lock();
        self.ptr().push(x);
        self.unlock();
    }

    pub fn read(&self) -> moments::Moments {
        self.lock();
        let value = self.value.clone();
        self.unlock();
        value
    }

    fn lock(&self) {
        loop {
            match self.spin.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed) {
                Ok(_) => { break; }
                Err(_) => { std::hint::spin_loop(); }
            }
        }
    }

    fn ptr(&self) -> &mut moments::Moments {
        unsafe {
            let p = self as *const Self;
            let p = p as *mut Self;
            &mut (*p).value
        }
    }

    fn unlock(&self) {
        self.spin.store(false, Ordering::Release);
    }
}

impl fmt::Debug for Moments {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let moments = self.read();
        write!(f, "moments({}, {}, {}, {}, {})",
            moments.n, moments.m1, moments.m2, moments.m3, moments.m4)
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gauge_init() {
        let x: f64 = 0.0;
        let y: u64 = x.to_bits();
        assert_eq!(y, GAUGE_INIT);
    }

    #[test]
    fn counter_may_be_static() {
        static COUNTER: Counter = Counter::new();
        println!("{:?}", COUNTER);
    }

    #[test]
    fn gauge_may_be_static() {
        static GAUGE: Gauge = Gauge::new();
        println!("{:?}", GAUGE);
    }

    #[test]
    fn sync_moments_may_be_static() {
        static MOMENTS: Moments = Moments::new();
        println!("{:?}", MOMENTS);
    }

    #[test]
    fn sync_moments_multiple_add() {
        static MOMENTS: Moments = Moments::new();
        MOMENTS.add(0.0);
        MOMENTS.add(5.0);
        MOMENTS.add(10.0);
        assert_eq!(MOMENTS.read().n(), 3);
        assert_eq!(MOMENTS.read().mean(), 5.0);
    }
}
