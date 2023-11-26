use std::io::Write;
use std::ops::{Bound, RangeBounds};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use biometrics::Counter;
use buffertk::{stack_pack, Unpackable};
use tatl::{HeyListen, Stationary};

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static EMITTER_FAILURE: Counter = Counter::new("indicio.emitter_failure");
static EMITTER_FAILURE_MONITOR: Stationary =
    Stationary::new("indicio.emitter_failure", &EMITTER_FAILURE);

pub fn register_biometrics(collector: biometrics::Collector) {
    collector.register_counter(&EMITTER_FAILURE);
}

pub fn register_monitors(hey_listen: &mut HeyListen) {
    hey_listen.register_stationary(&EMITTER_FAILURE_MONITOR);
}

////////////////////////////////////////////// Emitter /////////////////////////////////////////////

pub trait Emitter<K, V>: Send {
    fn emit(&mut self, file: &'static str, line: u32, k: K, v: V);
    fn flush(&mut self) {}
}

///////////////////////////////////////////// Collector ////////////////////////////////////////////

pub struct Collector<K, V> {
    should_log: AtomicBool,
    emitter: Mutex<Option<Box<dyn Emitter<K, V>>>>,
}

impl<K, V> Collector<K, V> {
    pub const fn new() -> Self {
        Self {
            should_log: AtomicBool::new(false),
            emitter: Mutex::new(None),
        }
    }

    pub fn is_logging(&self) -> bool {
        self.should_log.load(Ordering::Relaxed)
    }

    pub fn emit(&self, file: &'static str, line: u32, k: K, v: V) {
        if !self.is_logging() {
            return;
        }
        let mut emitter = self.emitter.lock().unwrap();
        if let Some(emitter) = emitter.as_deref_mut() {
            emitter.emit(file, line, k, v);
        }
    }

    pub fn flush(&self) {
        let mut emitter = self.emitter.lock().unwrap();
        if let Some(emitter) = emitter.as_deref_mut() {
            emitter.flush();
        }
    }

    pub fn register<E: Emitter<K, V> + 'static>(&self, emitter: E) {
        let boxed: Box<dyn Emitter<K, V>> = Box::new(emitter);
        let mut emitter = self.emitter.lock().unwrap();
        *emitter = Some(boxed);
        self.should_log.store(true, Ordering::Relaxed);
    }

    pub fn deregister(&self) {
        let mut emitter = self.emitter.lock().unwrap();
        *emitter = None;
        self.should_log.store(false, Ordering::Relaxed);
    }
}

////////////////////////////////////////// PrintlnEmitter //////////////////////////////////////////

pub struct PrintlnEmitter {}

impl PrintlnEmitter {
    pub const fn new() -> Self {
        Self {}
    }
}

impl<K: std::fmt::Display, V: std::fmt::Display> Emitter<K, V> for PrintlnEmitter {
    fn emit(&mut self, file: &'static str, line: u32, k: K, v: V) {
        println!("{}:{} {} => {}", file, line, k, v);
    }
}

///////////////////////////////////////// ProtobufReadFrame ////////////////////////////////////////

#[derive(Default, prototk_derive::Message)]
pub struct ProtobufPair<K, V>
where
    K: for<'a> prototk::Message<'a>,
    V: for<'a> prototk::Message<'a>,
{
    #[prototk(1, string)]
    // TODO(rescrv):  Use std::borrow::Cow when prototk supports it.
    pub file: String,
    #[prototk(2, uint32)]
    pub line: u32,
    #[prototk(3, int64)]
    pub time: i64,
    #[prototk(4, message)]
    pub key: K,
    #[prototk(5, message)]
    pub value: V,
}

#[derive(Default, prototk_derive::Message)]
pub struct ProtobufFrame<K, V>
where
    K: for<'a> prototk::Message<'a>,
    V: for<'a> prototk::Message<'a>,
{
    #[prototk(1, message)]
    pub record: ProtobufPair<K, V>,
}

////////////////////////////////////////// ProtobufEmitter /////////////////////////////////////////

pub struct ProtobufEmitter<W: Write> {
    output: Mutex<W>,
}

impl<W: Write> ProtobufEmitter<W> {
    pub const fn new(output: W) -> Self {
        Self {
            output: Mutex::new(output),
        }
    }
}

impl<K, V, W> Emitter<K, V> for ProtobufEmitter<W>
where
    K: for<'a> prototk::Message<'a>,
    V: for<'a> prototk::Message<'a>,
    W: Write + Send,
{
    fn emit(&mut self, file: &'static str, line: u32, key: K, value: V) {
        let file = file.to_string();
        let time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time should be after UNIX epoch")
            .as_micros()
            .try_into()
            .expect("time should fit a u64; even for micros");
        let buf = stack_pack(ProtobufFrame {
            record: ProtobufPair {
                file,
                line,
                time,
                key,
                value,
            },
        })
        .to_vec();
        let mut output = self.output.lock().unwrap();
        if output.write_all(&buf).is_err() {
            EMITTER_FAILURE.click();
        }
        if output.flush().is_err() {
            EMITTER_FAILURE.click();
        }
    }
}

/////////////////////////////////////////// ProtobufFile ///////////////////////////////////////////

#[derive(Default, prototk_derive::Message)]
pub struct ProtobufFile<K, V>
where
    K: for<'a> prototk::Message<'a>,
    V: for<'a> prototk::Message<'a>,
{
    #[prototk(1, message)]
    pub records: Vec<ProtobufPair<K, V>>,
}

pub fn read_protobuf_file<P, K, V>(path: P) -> Result<ProtobufFile<K, V>, String>
where
    P: AsRef<Path>,
    K: for<'a> prototk::Message<'a>,
    V: for<'a> prototk::Message<'a>,
{
    let contents = std::fs::read(path).map_err(|err| err.to_string())?;
    <ProtobufFile<K, V> as Unpackable>::unpack(&contents)
        .map(|x| x.0)
        .map_err(|err| err.to_string())
}

///////////////////////////////////////////// the macro ////////////////////////////////////////////

#[macro_export]
macro_rules! clue {
    ($collector:ident, $key:expr => $value:expr) => {
        if $collector.is_logging() {
            $collector.emit(file!(), line!(), $key, $value);
        }
    };
}

///////////////////////////////////////////// Debugger /////////////////////////////////////////////

pub struct Debugger<K, V> {
    kv_filters: Vec<Box<dyn Filter<K, V>>>,
    time_filter: (Bound<i64>, Bound<i64>),
    key_display: Option<Box<dyn Display<K>>>,
    value_display: Option<Box<dyn Display<V>>>,
}

impl<K, V> Debugger<K, V>
where
    K: for<'a> prototk::Message<'a> + std::fmt::Debug,
    V: for<'a> prototk::Message<'a> + std::fmt::Debug,
{
    pub fn add_filter<F: Filter<K, V> + 'static>(&mut self, f: F) {
        let f: Box<dyn Filter<K, V>> = Box::new(f);
        self.kv_filters.push(f);
    }

    pub fn restrict_time_range(&mut self, start: Option<i64>, limit: Option<i64>) {
        let start = if let Some(start) = start {
            Bound::Included(start)
        } else {
            Bound::Unbounded
        };
        let limit = if let Some(limit) = limit {
            Bound::Excluded(limit)
        } else {
            Bound::Unbounded
        };
        self.time_filter = (start, limit);
    }

    pub fn add_key_display<D: Display<K> + 'static>(&mut self, d: D) {
        let d: Box<dyn Display<K>> = Box::new(d);
        self.key_display = Some(d);
    }

    pub fn add_value_display<D: Display<V> + 'static>(&mut self, d: D) {
        let d: Box<dyn Display<V>> = Box::new(d);
        self.value_display = Some(d);
    }

    pub fn execute<P: AsRef<Path>, W: Write>(&mut self, path: P, w: &mut W) -> Result<(), String> {
        let protobuf = read_protobuf_file(path)?;
        for record in protobuf.records.iter() {
            let filter_matches = self.kv_filters.is_empty()
                || self.kv_filters.iter_mut().any(|f| f.matches(&record.key, &record.value));
            let time_matches = self.time_filter.contains(&record.time);
            if filter_matches && time_matches {
                let key = if let Some(key_display) = self.key_display.as_mut() {
                    key_display.display(&record.key)
                } else {
                    format!("{:?}", record.key)
                };
                let value = if let Some(value_display) = self.value_display.as_mut() {
                    value_display.display(&record.value)
                } else {
                    format!("{:?}", record.value)
                };
                writeln!(
                    w,
                    "{} {}:{}: {} {}",
                    record.time, record.file, record.line, key, value
                )
                .map_err(|err| err.to_string())?;
            }
        }
        Ok(())
    }
}

impl<K, V> Default for Debugger<K, V>
where
    K: for<'a> prototk::Message<'a> + std::fmt::Debug,
    V: for<'a> prototk::Message<'a> + std::fmt::Debug,
{
    fn default() -> Self {
        let kv_filters = Vec::new();
        let time_filter = (Bound::Unbounded, Bound::Unbounded);
        let key_display = None;
        let value_display = None;
        Self {
            kv_filters,
            time_filter,
            key_display,
            value_display,
        }
    }
}

////////////////////////////////////////////// Filter //////////////////////////////////////////////

pub trait Filter<K, V> {
    fn matches(&mut self, key: &K, value: &V) -> bool;
}

////////////////////////////////////////////// Display /////////////////////////////////////////////

pub trait Display<T> {
    fn display(&mut self, t: &T) -> String;
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[derive(Debug, Default, prototk_derive::Message)]
    struct TestKey {
        #[prototk(1, string)]
        key: String,
    }

    #[derive(Debug, Default, prototk_derive::Message)]
    struct TestValue {
        #[prototk(2, string)]
        value: String,
    }

    #[derive(Clone)]
    struct OutputFile {
        output: Arc<Mutex<Vec<u8>>>,
    }

    impl std::io::Write for OutputFile {
        fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
            self.output.lock().unwrap().write(buf)
        }

        fn flush(&mut self) -> Result<(), std::io::Error> {
            Ok(())
        }
    }

    static TEST_LOG: Collector<TestKey, TestValue> = Collector::new();

    #[test]
    fn protobuf_file() {
        let output = Arc::new(Mutex::new(Vec::new()));
        let emitter = ProtobufEmitter::new(OutputFile {
            output: output.clone()
        });
        TEST_LOG.register(emitter);
        clue! { TEST_LOG, TestKey {
                key: "MyKey".to_string(),
            } => TestValue {
                value: "MyValue".to_string(),
            }
        };
        let protobuf = ProtobufFile::<TestKey, TestValue>::unpack(&output.lock().unwrap()).unwrap().0;
        assert_eq!(1, protobuf.records.len());
        assert_eq!("MyKey", protobuf.records[0].key.key);
        assert_eq!("MyValue", protobuf.records[0].value.value);
    }
}
