//! KEY-VALue-INTerface benchmark.

use std::sync::Arc;

use biometrics::Collector;

pub mod metrics;
pub mod workload;

mod lsmtk;

#[cfg(feature = "rocksdb")]
mod rocksdb;

/////////////////////////////////////////// KeyValueStore //////////////////////////////////////////

/// A KeyValueStore implements the keyvalint traits, and is also Send + Sync as well as has a
/// method to register biometrics.
pub trait KeyValueStore: keyvalint::KeyValueStore + keyvalint::KeyValueLoad + Send + Sync {
    /// Register the KeyValueStore's biometrics with collector.
    fn register_biometrics(&self, collector: &Collector);
}

impl<KVS: KeyValueStore> KeyValueStore for Arc<KVS> {
    fn register_biometrics(&self, collector: &Collector) {
        KVS::register_biometrics(self, collector);
    }
}

///////////////////////////////////////////// Workload /////////////////////////////////////////////

/// A workload takes a key-value store and runs.
pub trait Workload<KVS: KeyValueStore> {
    /// Run the workload once.
    fn run(&mut self, kvs: KVS);
}
