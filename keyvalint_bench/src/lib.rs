use std::sync::Arc;

use biometrics::Collector;

pub mod metrics;
pub mod workload;

mod lsmtk;

#[cfg(feature = "rocksdb")]
mod rocksdb;

/////////////////////////////////////////// KeyValueStore //////////////////////////////////////////

pub trait KeyValueStore: keyvalint::KeyValueStore + keyvalint::KeyValueLoad + Send + Sync {
    fn register_biometrics(&self, collector: &Collector);
}

impl<KVS: KeyValueStore> KeyValueStore for Arc<KVS> {
    fn register_biometrics(&self, collector: &Collector) {
        KVS::register_biometrics(self, collector);
    }
}

///////////////////////////////////////////// Workload /////////////////////////////////////////////

pub trait Workload<KVS: KeyValueStore> {
    fn run(&mut self, kvs: KVS);
}
