#![doc = include_str!("../README.md")]

pub mod background;
pub mod collector;
pub mod lru;
pub mod monitor;
pub mod registry;
pub mod spin_lock;
pub mod state_hash_table;
pub mod wait_list;
pub mod work_coalescing_queue;

///////////////////////////////////////////// Constants ////////////////////////////////////////////

/// The maximum concurrency expected by any type in `sync42`.  Performance is allowed to degrade if
/// there are more than this many concurrent threads accessing a structure.
pub const MAX_CONCURRENCY: usize = 1 << 16;

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

/// Register all biometrics for the crate.
pub fn register_biometrics(collector: &biometrics::Collector) {
    state_hash_table::register_biometrics(collector);
    wait_list::register_biometrics(collector);
    work_coalescing_queue::register_biometrics(collector);
}
