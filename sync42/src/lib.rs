pub mod wait_list;

///////////////////////////////////////////// Constants ////////////////////////////////////////////

pub const MAX_CONCURRENCY: usize = 1 << 22;

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

pub fn register_biometrics(collector: &mut biometrics::Collector) {
    if !collector.ingest_swizzle(module_path!(), file!(), line!()) {
        return;
    }
    wait_list::register_biometrics(collector);
}
