//! A workload that's capable of running something similar to YCSB.
//!
//! NOTE:  This workload currently doesn't support either D or F workload types.

use std::fmt::Debug;
use std::fs::File;
use std::ops::Bound;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Instant, SystemTime};

use armnod::{Armnod, ArmnodOptions};
use biometrics::{Collector, Counter, Moments};
use biometrics_sys::BiometricsSys;
use guacamole::{FromGuacamole, Guacamole};
use keyvalint::Cursor;

use crate::metrics::PlainTextEmitter;
use crate::{KeyValueStore, Workload as WorkloadTrait};

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static INTERARRIVAL_TIME: Moments =
    Moments::new("keyvalint_bench.ycsb.target_interarrival_time_micros");
static STALL: Counter = Counter::new("keyvalint_bench.ycsb.stall");
static CATCHUP: Counter = Counter::new("keyvalint_bench.ycsb.catchup");
static FINISHED: Counter = Counter::new("keyvalint_bench.ycsb.finished");

static WRITE: Counter = Counter::new("keyvalint_bench.ycsb.requests.write");
static READ: Counter = Counter::new("keyvalint_bench.ycsb.requests.read");
static SCAN: Counter = Counter::new("keyvalint_bench.ycsb.requests.scan");

static WRITE_LATENCY: Moments = Moments::new("keyvalint_bench.ycsb.requests.write_latency_micros");
static READ_LATENCY: Moments = Moments::new("keyvalint_bench.ycsb.requests.read_latency_micros");
static SCAN_LATENCY: Moments = Moments::new("keyvalint_bench.ycsb.requests.scan_latency_micros");

fn register_biometrics(collector: &Collector) {
    collector.register_moments(&INTERARRIVAL_TIME);
    collector.register_counter(&STALL);
    collector.register_counter(&CATCHUP);
    collector.register_counter(&FINISHED);
    collector.register_counter(&WRITE);
    collector.register_counter(&READ);
    collector.register_counter(&SCAN);
    collector.register_moments(&WRITE_LATENCY);
    collector.register_moments(&READ_LATENCY);
    collector.register_moments(&SCAN_LATENCY);
}

/////////////////////////////////////////////// State //////////////////////////////////////////////

struct State {
    options: WorkloadOptions,
    started: Instant,
    stopped: AtomicU64,
}

impl State {
    fn run<KVS: KeyValueStore>(&self, index: u64, kvs: Arc<KVS>) {
        let mut guac = Guacamole::new((u64::MAX / self.options.worker_threads) * index);
        let mut keys = self
            .options
            .key
            .clone()
            .try_parse_sharded(index, self.options.worker_threads)
            .unwrap();
        let mut values = self
            .options
            .value
            .clone()
            .try_parse_sharded(index, self.options.worker_threads)
            .unwrap();
        let interarrival_time =
            self.options.target_throughput as f64 / self.options.worker_threads as f64;
        let total_weight =
            self.options.write_weight + self.options.read_weight + self.options.scan_weight;
        let write_thresh = self.options.write_weight / total_weight;
        let read_thresh = write_thresh + (self.options.read_weight / total_weight);
        let scan_thresh = read_thresh + (self.options.scan_weight / total_weight);
        let mut backlog = 0u64;
        while self.options.load || self.started.elapsed().as_secs() < self.options.duration_secs {
            let start = Instant::now();
            let next_request_micros = (0.0
                - f64::from_guacamole(&mut (), &mut guac).ln() / interarrival_time)
                * 1_000_000.0;
            let weight: f64 = f64::from_guacamole(&mut (), &mut guac);
            if weight < write_thresh {
                if !self.write(&*kvs, &mut guac, &mut keys, &mut values) {
                    break;
                }
            } else if weight < read_thresh {
                self.read(&*kvs, &mut guac, &mut keys);
            } else if weight < scan_thresh {
                self.scan(&*kvs, &mut guac, &mut keys);
            } else {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
            let elapsed_micros = start.elapsed().as_micros() as f64;
            if next_request_micros < elapsed_micros {
                backlog =
                    backlog.saturating_add((elapsed_micros - next_request_micros).ceil() as u64);
                STALL.click();
            } else {
                let delta = (next_request_micros - elapsed_micros) as u64;
                if delta < backlog {
                    backlog -= delta;
                } else if backlog > 0 {
                    CATCHUP.click();
                    backlog = 0;
                }
                std::thread::sleep(std::time::Duration::from_micros(delta));
            }
            INTERARRIVAL_TIME.add(next_request_micros);
        }
        self.stopped.fetch_add(1, Ordering::Relaxed);
        FINISHED.click();
    }

    fn write<KVS: KeyValueStore>(
        &self,
        kvs: &KVS,
        guac: &mut Guacamole,
        keys: &mut Armnod,
        values: &mut Armnod,
    ) -> bool {
        let key = keys.choose(guac);
        let value = values.choose(guac);
        if let (Some(key), Some(value)) = (key, value) {
            let start = Instant::now();
            kvs.put(key.as_bytes(), value.as_bytes())
                .expect("key value store should not fail");
            WRITE.click();
            WRITE_LATENCY.add(start.elapsed().as_micros() as f64);
            true
        } else {
            false
        }
    }

    fn read<KVS: KeyValueStore>(&self, kvs: &KVS, guac: &mut Guacamole, keys: &mut Armnod) {
        if let Some(key) = keys.choose(guac) {
            let start = Instant::now();
            kvs.get(key.as_bytes())
                .expect("key value store should not fail");
            READ.click();
            READ_LATENCY.add(start.elapsed().as_micros() as f64);
        }
    }

    fn scan<KVS: KeyValueStore>(&self, kvs: &KVS, guac: &mut Guacamole, keys: &mut Armnod) {
        if let Some(key) = keys.choose(guac) {
            let start = Instant::now();
            let mut cursor = kvs
                .range_scan(&Bound::Included(key.as_bytes()), &Bound::Unbounded)
                .expect("key value store should not fail");
            for _ in 0..self.options.scan_keys {
                cursor.next().unwrap();
            }
            SCAN.click();
            SCAN_LATENCY.add(start.elapsed().as_micros() as f64);
        }
    }
}

////////////////////////////////////////// WorkloadOptions /////////////////////////////////////////

/// YCSB workload options.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "command_line", derive(arrrg_derive::CommandLine))]
pub struct WorkloadOptions {
    /// The armnod options for generating keys.
    #[cfg_attr(feature = "command_line", arrrg(nested))]
    key: ArmnodOptions,
    /// The armnod options for generating values.
    #[cfg_attr(feature = "command_line", arrrg(nested))]
    value: ArmnodOptions,
    /// The weight assigned to write operations.
    #[cfg_attr(
        feature = "command_line",
        arrrg(optional, "Weight to assign to write operations")
    )]
    write_weight: f64,
    /// The weight assigned to read operations.
    #[cfg_attr(
        feature = "command_line",
        arrrg(optional, "Weight to assign to read operations")
    )]
    read_weight: f64,
    /// The weight assigned to scan operations.
    #[cfg_attr(
        feature = "command_line",
        arrrg(optional, "Weight to assign to scan operations")
    )]
    scan_weight: f64,
    /// The number of keys to scan.
    #[cfg_attr(
        feature = "command_line",
        arrrg(optional, "Number of keys to scan (constant).")
    )]
    scan_keys: u64,
    /// Change behavior to enumerate all keys instead of running for [duration_secs].
    #[cfg_attr(
        feature = "command_line",
        arrrg(
            flag,
            "Run until all keys are enumerated instead of for --duration-secs."
        )
    )]
    load: bool,
    /// The number of worker threads to spawn.
    #[cfg_attr(feature = "command_line", arrrg(optional, "Number of threads to run."))]
    worker_threads: u64,
    /// The target throughput.
    #[cfg_attr(
        feature = "command_line",
        arrrg(optional, "Target throughput to sustain.")
    )]
    target_throughput: u64,
    /// The number of seconds to run the test.
    #[cfg_attr(
        feature = "command_line",
        arrrg(optional, "Number of seconds to run the experiment.")
    )]
    duration_secs: u64,
    /// The path to which metrics should be written in biometrics plaintext form.
    #[cfg_attr(
        feature = "command_line",
        arrrg(optional, "Metrics output (default: \"ycsb.txt\").")
    )]
    metrics: String,
}

impl Default for WorkloadOptions {
    fn default() -> Self {
        Self {
            key: ArmnodOptions::default(),
            value: ArmnodOptions::default(),
            write_weight: 0.05,
            read_weight: 0.95,
            scan_weight: 0.0,
            scan_keys: 10,
            load: false,
            worker_threads: 64,
            target_throughput: 10_000,
            duration_secs: 60,
            metrics: "ycsb.txt".to_string(),
        }
    }
}

impl PartialEq for WorkloadOptions {
    fn eq(&self, other: &WorkloadOptions) -> bool {
        fn approx_eq(lhs: f64, rhs: f64) -> bool {
            lhs * 0.999 < rhs && lhs * 1.001 > rhs
        }
        self.key == other.key
            && self.value == other.value
            && approx_eq(self.write_weight, other.write_weight)
            && approx_eq(self.read_weight, other.read_weight)
            && approx_eq(self.scan_weight, other.scan_weight)
            && self.scan_keys == other.scan_keys
            && self.load == other.load
            && self.worker_threads == other.worker_threads
            && self.target_throughput == other.target_throughput
            && self.duration_secs == other.duration_secs
            && self.metrics == other.metrics
    }
}

impl Eq for WorkloadOptions {}

///////////////////////////////////////////// Workload /////////////////////////////////////////////

/// The YCSB workload.
pub struct Workload<KVS: KeyValueStore> {
    options: WorkloadOptions,
    _phantom_kvs: std::marker::PhantomData<KVS>,
}

impl<KVS: KeyValueStore> Workload<KVS> {
    /// Create a new workload from options.
    pub fn new(options: WorkloadOptions) -> Self {
        Self {
            options,
            _phantom_kvs: std::marker::PhantomData,
        }
    }
}

impl<KVS: KeyValueStore + 'static> WorkloadTrait<KVS> for Workload<KVS> {
    fn run(&mut self, kvs: KVS) {
        // Setup the biometrics.
        let mut metrics = PlainTextEmitter::new(
            File::create(&self.options.metrics).expect("metrics file should be writable"),
        );
        let collector = Collector::new();
        register_biometrics(&collector);
        kvs.register_biometrics(&collector);
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("clock should never fail")
            .as_millis()
            .try_into()
            .expect("millis since epoch should fit u64");
        if let Err(e) = collector.emit(&mut metrics, now) {
            eprintln!("collector error: {}", e);
        }
        // Spawn the worker threads.
        let kvs = Arc::new(kvs);
        let state = Arc::new(State {
            options: self.options.clone(),
            started: Instant::now(),
            stopped: AtomicU64::new(0),
        });
        let _keys = self
            .options
            .key
            .clone()
            .try_parse()
            .expect("--key* should be valid");
        let _values = self
            .options
            .value
            .clone()
            .try_parse()
            .expect("--value* should be valid");
        let mut threads = vec![];
        for idx in 0..self.options.worker_threads {
            let k = Arc::clone(&kvs);
            let s = Arc::clone(&state);
            threads.push(std::thread::spawn(move || {
                s.run(idx, k);
            }));
        }
        // Emit metrics until the end of the test.
        let mut bio_sys = BiometricsSys::default();
        while (self.options.load || state.started.elapsed().as_secs() < self.options.duration_secs)
            && state.stopped.load(Ordering::Relaxed) < self.options.worker_threads
        {
            std::thread::sleep(std::time::Duration::from_millis(1000));
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("clock should never fail")
                .as_millis()
                .try_into()
                .expect("millis since epoch should fit u64");
            if let Err(e) = collector.emit(&mut metrics, now) {
                eprintln!("collector error: {}", e);
            }
            bio_sys.emit(&mut metrics, now);
        }
        // Join the test threads.
        for thread in threads.into_iter() {
            thread.join().expect("thread should finish successfully");
        }
    }
}
