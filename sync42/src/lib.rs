pub mod collector;
pub mod monitor;
pub mod spin_lock;
pub mod state_hash_table;
pub mod wait_list;

///////////////////////////////////////////// Constants ////////////////////////////////////////////

pub const MAX_CONCURRENCY: usize = 1 << 22;

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

/// Register all biometrics for the crate.
pub fn register_biometrics(collector: &biometrics::Collector) {
    state_hash_table::register_biometrics(collector);
    wait_list::register_biometrics(collector);
}
