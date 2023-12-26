//! Benchmark the [sst::log:ConcurrentLog].

use std::fs::File;
use std::ops::Bound;

use arrrg::CommandLine;
use biometrics::Collector;
use sst::log::{ConcurrentLogBuilder, LogOptions};
use sst::Builder;

use keyvalint_bench::{workload, KeyValueStore};

const USAGE: &str =
    "USAGE: keyvalint-bench-concurrent-log [--log-options] workload [--workload-options]";

//////////////////////////////////// ConcurrentLogKeyValueStore ////////////////////////////////////

struct WriteBatch {
    batch: sst::log::WriteBatch,
}

impl keyvalint::WriteBatch for WriteBatch {
    fn put(&mut self, key: &[u8], timestamp: u64, value: &[u8]) {
        self.batch
            .put(key, timestamp, value)
            .expect("put should always work");
    }

    fn del(&mut self, key: &[u8], timestamp: u64) {
        self.batch
            .del(key, timestamp)
            .expect("del should always work");
    }
}

struct ConcurrentLogKeyValueStore {
    conclog: ConcurrentLogBuilder<File>,
}

impl keyvalint::KeyValueStore for ConcurrentLogKeyValueStore {
    type Error = sst::Error;
    type WriteBatch<'a> = WriteBatch;

    fn put(&self, key: &[u8], timestamp: u64, value: &[u8]) -> Result<(), Self::Error> {
        self.conclog.put(key, timestamp, value)
    }

    fn del(&self, key: &[u8], timestamp: u64) -> Result<(), Self::Error> {
        self.conclog.del(key, timestamp)
    }

    fn write(&self, batch: Self::WriteBatch<'_>) -> Result<(), Self::Error> {
        self.conclog.append(batch.batch)
    }
}

impl keyvalint::KeyValueLoad for ConcurrentLogKeyValueStore {
    type Error = ();
    type RangeScan<'a> = ();

    fn get<'a>(&self, _: &[u8], _: u64) -> Result<Option<Vec<u8>>, Self::Error> {
        unimplemented!();
    }

    fn load(&self, _: &[u8], _: u64, _: &mut bool) -> Result<Option<Vec<u8>>, Self::Error> {
        unimplemented!();
    }

    fn range_scan<T: AsRef<[u8]>>(
        &self,
        _: &Bound<T>,
        _: &Bound<T>,
        _: u64,
    ) -> Result<Self::RangeScan<'_>, Self::Error> {
        panic!("range_scan() is not supported for concurrent log");
    }
}

impl KeyValueStore for ConcurrentLogKeyValueStore {
    fn register_biometrics(&self, collector: &Collector) {
        sst::register_biometrics(collector);
        sync42::register_biometrics(collector);
    }
}

/////////////////////////////////////// ConcurrentLogOptions ///////////////////////////////////////

#[derive(Debug, Default, Eq, PartialEq, arrrg_derive::CommandLine)]
struct ConcurrentLogOptions {
    #[arrrg(required, "Path to the concurrent log.")]
    path: String,
    #[arrrg(nested, "Log options.")]
    log: LogOptions,
}

/////////////////////////////////////////////// main ///////////////////////////////////////////////

fn main() {
    let (options, free) = ConcurrentLogOptions::from_command_line_relaxed(USAGE);
    if free.is_empty() {
        eprintln!("missing workload");
        eprintln!("{}", USAGE);
        std::process::exit(1);
    }
    let conclog =
        ConcurrentLogBuilder::new(options.log, options.path).expect("concurrent log should open");
    let kvs = ConcurrentLogKeyValueStore { conclog };
    let mut workload = workload::from_command_line(USAGE, &free);
    workload.run(kvs);
}
