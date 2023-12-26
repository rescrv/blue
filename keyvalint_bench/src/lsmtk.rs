use lsmtk::KeyValueStore;

impl super::KeyValueStore for KeyValueStore {
    fn register_biometrics(&self, collector: &biometrics::Collector) {
        sync42::register_biometrics(collector);
        sst::register_biometrics(collector);
        lsmtk::register_biometrics(collector);
    }
}
