use keyvalint::rocksdb::KeyValueStore;

impl super::KeyValueStore for KeyValueStore {
    fn register_biometrics(&self, _: &biometrics::Collector) {}
}
