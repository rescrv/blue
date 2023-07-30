pub mod wait_list;

///////////////////////////////////////////// Constants ////////////////////////////////////////////

pub const MAX_CONCURRENCY: usize = 1 << 22;

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

/// Register all biometrics for the crate.
pub fn register_biometrics(collector: &biometrics::Collector) {
    wait_list::register_biometrics(collector);
}
