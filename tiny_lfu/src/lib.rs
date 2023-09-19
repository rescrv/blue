#![doc = include_str!("../README.md")]

use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use std::iter::zip;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

mod vector;

pub const MIN_PROBABILITY: f64 = 0.000000001;
pub const MAX_KEYS: usize = 32;

const KEYS: [u64; MAX_KEYS] = [
    16473164755499224732,
    6247173169788583865,
    8465709353774189547,
    1018011193075750710,
    5005235901598854774,
    4608635830076970966,
    15319262698555796356,
    17493592001517989851,
    6595980886894675232,
    8700003585840933410,
    5681865357013068729,
    3701697702389491699,
    2662113382591523919,
    2729874135358566255,
    1024667377902173610,
    3558991661993990467,
    13339185366737598911,
    3218955893080835082,
    1794074658831355533,
    1537277809928656881,
    15858525474063029883,
    3875834857973360438,
    7980602034409828573,
    17653903999856459706,
    9406525102780850429,
    989671448099696322,
    9235904761508267332,
    11528437017542691722,
    2134906268640270929,
    3661546785564374744,
    9062261515874811040,
    6396837448873813959,
];

////////////////////////////////////////// AdmissionPolicy /////////////////////////////////////////

/// An [AdmissionPolicy] is a gatekeeping force at the entry to the cache.  A good admission policy
/// will only replace objects in the cache when the caching outcome is favorable.
pub trait AdmissionPolicy {
    /// `t` gets admitted to the cache.  Update accordingly.
    fn admit<T: std::hash::Hash>(&self, t: &T);
    /// Return true if the `candidate` should replace the `victim`.
    fn should_replace<T: std::hash::Hash>(&self, victim: &T, candidate: &T) -> bool;
}

////////////////////////////////////////// TinyLFUOptions //////////////////////////////////////////

/// [TinyLFUOptions] controls the layout of the [TinyLFU].
#[derive(Clone, Debug)]
pub struct TinyLFUOptions {
    target_memory_usage: usize,
    window_size: u32,
}

impl TinyLFUOptions {
    /// Use up to `target_memory_usage` bytes of memory for the TinyLFU instance.
    pub fn target_memory_usage(mut self, target_memory_usage: usize) -> Self {
        self.target_memory_usage = target_memory_usage;
        self
    }

    /// Track a window of `window_sz` admissions.  If this is not a power of two, it may be
    /// increased to the next-largest power of two.
    pub fn window_size(mut self, window_size: u32) -> Self {
        self.window_size = window_size;
        self
    }
}

impl Default for TinyLFUOptions {
    fn default() -> Self {
        Self {
            // Use 2GiB to track...
            target_memory_usage: 1usize << 31,
            // ... the last billion or so requests.
            window_size: 1u32 << 30,
        }
    }
}

////////////////////////////////////////////// TinyLFU /////////////////////////////////////////////

/// [TinyLFU] is a [AdmissionPolicy] that can recommend when one element is more popular than
/// another.
pub struct TinyLFU {
    opts: TinyLFUOptions,
    keys: &'static [u64],
    counters: vector::Vector,
    counter: AtomicU64,
    epoch: AtomicU64,
    mtx: Mutex<()>,
}

impl TinyLFU {
    /// Create a new [TinyLFU] using the [TinyLFUOptions] provided.
    pub fn new(mut opts: TinyLFUOptions) -> Result<Self, &'static str> {
        opts.window_size = match opts.window_size.checked_next_power_of_two() {
            Some(x) => x,
            None => {
                return Err("invalid parameters: window size too large");
            }
        };
        let counter_bits = (opts.window_size as f64).log2().ceil() as usize;
        let num_counters = (opts.target_memory_usage * 8 + counter_bits - 1) / counter_bits;
        let n = bloomcalc::N(opts.window_size as f64);
        let m = bloomcalc::M(num_counters as f64);
        let p = bloomcalc::calc_p_given_n_m(n, m);
        if p.0 < MIN_PROBABILITY {
            return Err("invalid parameters: false positive rate too small");
        }
        let k = bloomcalc::calc_keys_given_probability(p);
        let num_keys = k.0.ceil() as usize;
        if num_keys > MAX_KEYS {
            return Err("invalid parameters: too many hashing keys");
        }
        let keys = &KEYS[..num_keys];
        let counters = vector::Vector::new(counter_bits, num_counters);
        Ok(TinyLFU {
            opts,
            keys,
            counters,
            counter: AtomicU64::new(0),
            epoch: AtomicU64::new(0),
            mtx: Mutex::new(()),
        })
    }

    fn hash<'a, T: std::hash::Hash>(&self, t: &T, hashes: &'a mut [u64; MAX_KEYS]) -> &'a [u64] {
        for (k, h) in zip(self.keys.iter(), hashes.iter_mut()) {
            let mut hasher = DefaultHasher::new();
            hasher.write_u64(*k);
            t.hash(&mut hasher);
            *h = hasher.finish();
        }
        &hashes[0..self.keys.len()]
    }

    fn increment(&self, hashes: &[u64]) {
        let modulus = self.counters.len();
        let minimum = self.read(hashes);
        for hash in hashes.iter() {
            let index = *hash as usize % modulus;
            if self.counters.load(index) <= minimum {
                self.counters.increment(index);
            }
        }
    }

    fn read(&self, hashes: &[u64]) -> u64 {
        let mut count = u64::max_value();
        let modulus = self.counters.len();
        for hash in hashes.iter() {
            let value = self.counters.load(*hash as usize % modulus);
            if value < count {
                count = value;
            }
        }
        count
    }

    fn decimate(&self) {
        let _hold = self.mtx.lock().unwrap();
        let counter = self.counter.load(Ordering::Relaxed);
        if counter < self.opts.window_size as u64 {
            return;
        }
        self.counter.store(counter / 2, Ordering::Relaxed);
        self.epoch.fetch_add(1, Ordering::Relaxed);
        for i in 0..self.counters.len() {
            self.counters.divide_two(i);
        }
        self.epoch.fetch_add(1, Ordering::Relaxed);
    }
}

impl AdmissionPolicy for TinyLFU {
    fn admit<T: std::hash::Hash>(&self, t: &T) {
        let mut hashes: [u64; MAX_KEYS] = [0u64; MAX_KEYS];
        let hashes = self.hash(t, &mut hashes);
        self.increment(hashes);
        if self.counter.fetch_add(1, Ordering::Relaxed) + 1 >= self.opts.window_size as u64 {
            self.decimate();
        }
    }

    fn should_replace<T: std::hash::Hash>(&self, victim: &T, candidate: &T) -> bool {
        let mut victim_hashes: [u64; MAX_KEYS] = [0u64; MAX_KEYS];
        let mut candidate_hashes: [u64; MAX_KEYS] = [0u64; MAX_KEYS];
        let victim_hashes = self.hash(victim, &mut victim_hashes);
        let candidate_hashes = self.hash(candidate, &mut candidate_hashes);
        loop {
            let start_epoch = self.epoch.load(Ordering::Relaxed);
            let victim_count = self.read(victim_hashes);
            let candidate_count = self.read(candidate_hashes);
            let end_epoch = self.epoch.load(Ordering::Relaxed);
            if start_epoch == end_epoch && start_epoch & 0x1 == 0 {
                return victim_count < candidate_count;
            }
            std::hint::spin_loop();
        }
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants() {
        use bloomcalc::{calc_keys_given_probability, P};
        // The minimum probability must not yield more than MAX_KEYS hashes.
        assert!(MAX_KEYS >= calc_keys_given_probability(P(MIN_PROBABILITY)).0 as usize);
    }

    #[test]
    fn hello_goodbye() {
        let tlfu = TinyLFU::new(TinyLFUOptions::default()).unwrap();
        for _ in 0..16 {
            tlfu.admit(&"hello");
        }
        tlfu.admit(&"goodbye");
        assert!(tlfu.should_replace(&"goodbye", &"hello"));
        assert!(!tlfu.should_replace(&"hello", &"goodbye"));
    }
}
