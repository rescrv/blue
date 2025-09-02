use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::os::fd::AsRawFd;
use std::time::{Duration, Instant};

use biometrics::{Counter, Gauge, Histogram, Moments, Sensor};
use utf8path::Path;

////////////////////////////////////////////// Options /////////////////////////////////////////////

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Options {
    pub segment_size: usize,
    pub flush_interval: Duration,
    pub prefix: Path<'static>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            segment_size: 64 * 1048576,
            flush_interval: Duration::from_secs(30),
            prefix: Path::new("biometrics."),
        }
    }
}

////////////////////////////////////////////// Emitter /////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct SlashMetrics {
    output: Option<String>,
}

impl SlashMetrics {
    pub fn new() -> Self {
        let output = None;
        Self { output }
    }

    pub fn take(mut self) -> String {
        self.output.take().unwrap_or_default()
    }

    fn write_line(&mut self, line: impl AsRef<str>) -> Result<(), std::io::Error> {
        let output = self.output.get_or_insert_with(String::new);
        *output += line.as_ref();
        Ok(())
    }
}

impl biometrics::Emitter for SlashMetrics {
    type Error = std::io::Error;

    fn emit_counter(&mut self, counter: &Counter, now: u64) -> Result<(), std::io::Error> {
        let label = counter.label().replace(".", "_");
        let reading = counter.read();
        self.write_line(format!(
            "# TYPE {label} counter
{label} {reading} {now}\n",
        ))?;
        Ok(())
    }

    fn emit_gauge(&mut self, gauge: &Gauge, now: u64) -> Result<(), std::io::Error> {
        let label = gauge.label().replace(".", "_");
        let reading = gauge.read();
        self.write_line(format!(
            "# TYPE {label} gauge
{label} {reading} {now}\n"
        ))?;
        Ok(())
    }

    fn emit_moments(&mut self, moments: &Moments, now: u64) -> Result<(), std::io::Error> {
        let label = moments.label().replace(".", "_");
        let reading = moments.read();
        self.write_line(format!(
            "# TYPE {label}_count counter
{label}_count {} {now}
# TYPE {label}_mean gauge
{label}_mean {} {now}
# TYPE {label}_variance gauge
{label}_variance {} {now}
# TYPE {label}_skewness gauge
{label}_skewness {} {now}
# TYPE {label}_kurtosis gauge
{label}_kurtosis {} {now}\n",
            reading.n(),
            reading.mean(),
            reading.variance(),
            reading.skewness(),
            reading.kurtosis(),
        ))?;
        Ok(())
    }

    fn emit_histogram(&mut self, histogram: &Histogram, now: u64) -> Result<(), std::io::Error> {
        let label = histogram.label().replace(".", "_");
        self.write_line(format!("# TYPE {label} histogram\n"))?;
        let mut total = 0;
        let mut acc = 0.0;
        for (bucket, count) in histogram.read().iter() {
            total += count;
            acc += bucket * count as f64;
            self.write_line(format!(
                "{label}_bucket{{le=\"{bucket:0.4}\"}} {total} {now}\n"
            ))?;
        }
        self.write_line(format!("{label}_sum {acc} {now}\n"))?;
        self.write_line(format!("{label}_count {total} {now}\n"))?;
        let exceeds_max = histogram.exceeds_max().read();
        self.write_line(format!("{label}_exceeds_max {exceeds_max} {now}\n"))?;
        let is_negative = histogram.is_negative().read();
        self.write_line(format!("{label}_is_negative {is_negative} {now}\n"))?;
        Ok(())
    }
}

////////////////////////////////////////////// Emitter /////////////////////////////////////////////

pub struct Emitter {
    options: Options,
    output: Option<BufWriter<File>>,
    written: usize,
    last_flush: Instant,
    flush_trigger: Option<u64>,
}

impl Emitter {
    pub fn new(options: Options) -> Self {
        let output = None;
        let written = 0;
        let last_flush = Instant::now();
        let flush_trigger = None;
        Self {
            options,
            output,
            written,
            last_flush,
            flush_trigger,
        }
    }

    pub fn flush(&mut self) -> Result<(), std::io::Error> {
        if let Some(output) = self.output.as_mut() {
            output.flush()?;
        }
        Ok(())
    }

    fn write_line(&mut self, line: impl AsRef<str>, now_millis: u64) -> Result<(), std::io::Error> {
        if let Some(flush_trigger) = self.flush_trigger {
            if now_millis > flush_trigger {
                self.flush()?;
                self.output.take();
                self.written = 0;
                self.last_flush = Instant::now();
                self.flush_trigger = None;
            }
        }
        let options = self.options.clone();
        let flush_trigger = self.flush_trigger;
        let last_flush = self.last_flush;
        self.written += line.as_ref().len();
        let written = self.written;
        let output = self.get_output(now_millis)?;
        output.write_all(line.as_ref().as_bytes())?;
        if flush_trigger.is_none()
            && (written > options.segment_size || last_flush.elapsed() > options.flush_interval)
        {
            self.flush_trigger = Some(now_millis);
        }
        Ok(())
    }

    fn get_output(&mut self, now_millis: u64) -> Result<&mut dyn std::io::Write, std::io::Error> {
        if self.output.is_none() {
            let path = self.options.prefix.as_str().to_owned() + &format!("{now_millis}.prom");
            let file = OpenOptions::new().create_new(true).write(true).open(path)?;
            // NOTE(rescrv): l_type,l_whence is 16 bits on some platforms and 32 bits on others.
            // The annotations here are for cross-platform compatibility.
            #[allow(clippy::useless_conversion)]
            #[allow(clippy::unnecessary_cast)]
            let flock = libc::flock {
                l_type: libc::F_WRLCK as i16,
                l_whence: libc::SEEK_SET as i16,
                l_start: 0,
                l_len: 0,
                l_pid: 0,
                #[cfg(target_os = "freebsd")]
                l_sysid: 0,
            };
            // NOTE(rescrv):  This should never fail, as others will only acquire the lock when the
            // file size is non-zero.  Just bail impolitely if it does.
            if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETLK, &flock) < 0 } {
                return Err(std::io::Error::last_os_error());
            }
            let output = BufWriter::new(file);
            self.output = Some(output);
        }
        Ok(self.output.as_mut().unwrap())
    }
}

impl biometrics::Emitter for Emitter {
    type Error = std::io::Error;

    fn emit_counter(&mut self, counter: &Counter, now: u64) -> Result<(), std::io::Error> {
        let label = counter.label();
        let reading = counter.read();
        self.write_line(
            format!(
                "# TYPE {label} counter
{label} {reading} {now}\n",
            ),
            now,
        )?;
        Ok(())
    }

    fn emit_gauge(&mut self, gauge: &Gauge, now: u64) -> Result<(), std::io::Error> {
        let label = gauge.label();
        let reading = gauge.read();
        self.write_line(
            format!(
                "# TYPE {label} gauge
{label} {reading} {now}\n"
            ),
            now,
        )?;
        Ok(())
    }

    fn emit_moments(&mut self, moments: &Moments, now: u64) -> Result<(), std::io::Error> {
        let label = moments.label();
        let reading = moments.read();
        self.write_line(
            format!(
                "# TYPE {label}_count counter
{label}_count {} {now}
# TYPE {label}_mean gauge
{label}_mean {} {now}
# TYPE {label}_variance gauge
{label}_variance {} {now}
# TYPE {label}_skewness gauge
{label}_skewness {} {now}
# TYPE {label}_kurtosis gauge
{label}_kurtosis {} {now}\n",
                reading.n(),
                reading.mean(),
                reading.variance(),
                reading.skewness(),
                reading.kurtosis(),
            ),
            now,
        )?;
        Ok(())
    }

    fn emit_histogram(&mut self, histogram: &Histogram, now: u64) -> Result<(), std::io::Error> {
        let label = histogram.label();
        self.write_line(format!("# TYPE {label} histogram\n"), now)?;
        let mut total = 0;
        let mut acc = 0.0;
        for (bucket, count) in histogram.read().iter() {
            total += count;
            acc += bucket * count as f64;
            self.write_line(
                format!("{label}_bucket{{le=\"{bucket:0.4}\"}} {total} {now}\n"),
                now,
            )?;
        }
        self.write_line(format!("{label}_sum {acc} {now}\n"), now)?;
        self.write_line(format!("{label}_count {total} {now}\n"), now)?;
        let exceeds_max = histogram.exceeds_max().read();
        self.write_line(format!("{label}_exceeds_max {exceeds_max} {now}\n"), now)?;
        let is_negative = histogram.is_negative().read();
        self.write_line(format!("{label}_is_negative {is_negative} {now}\n"), now)?;
        Ok(())
    }
}

////////////////////////////////////////////// Reader //////////////////////////////////////////////

#[derive(Debug)]
pub struct Reader(Path<'static>, File);

impl Reader {
    pub fn open(path: Path) -> Result<Self, std::io::Error> {
        Self::_open(path, libc::F_SETLKW)
    }

    fn _open(path: Path, cmd: libc::c_int) -> Result<Self, std::io::Error> {
        let path = path.into_owned();
        let file = OpenOptions::new().read(true).open(&path)?;
        // NOTE(rescrv): l_type,l_whence is 16 bits on some platforms and 32 bits on others.
        // The annotations here are for cross-platform compatibility.
        #[allow(clippy::useless_conversion)]
        #[allow(clippy::unnecessary_cast)]
        let flock = libc::flock {
            l_type: libc::F_RDLCK as i16,
            l_whence: libc::SEEK_SET as i16,
            l_start: 0,
            l_len: 0,
            l_pid: 0,
            #[cfg(target_os = "freebsd")]
            l_sysid: 0,
        };
        // NOTE(rescrv):  This should never fail, as others will only acquire the lock when the
        // file size is non-zero.  Just bail impolitely if it does.
        if unsafe { libc::fcntl(file.as_raw_fd(), cmd, &flock) < 0 } {
            return Err(std::io::Error::last_os_error());
        }
        Ok(Self(path, file))
    }

    pub fn path(&self) -> &Path<'static> {
        &self.0
    }

    pub fn unlink(self) -> Result<(), std::io::Error> {
        std::fs::remove_file(self.path())
    }
}

impl std::ops::Deref for Reader {
    type Target = File;

    fn deref(&self) -> &Self::Target {
        &self.1
    }
}

////////////////////////////////////////////// Watcher /////////////////////////////////////////////

pub struct Watcher {
    path: Path<'static>,
}

impl Watcher {
    pub fn new(path: Path) -> Self {
        let path = path.into_owned();
        Self { path }
    }

    /// Ingest the least recently modified file in the directory, subject to locking rules.  If the
    /// lock cannot be acquired, the function will pass over the file.  Once one call is made to
    /// `process`, the function returns Ok(()).  If the `process` function does not use
    /// `reader.unlink`, the next call to ingest_one will process the same file with high
    /// probability.
    pub fn ingest_one<E: From<std::io::Error>>(
        &self,
        mut process: impl FnMut(Reader) -> Result<(), E>,
    ) -> Result<(), E> {
        let mut path_and_timestamp = vec![];
        for dirent in std::fs::read_dir(&self.path)? {
            let dirent = dirent?;
            let metadata = dirent.metadata()?;
            let path = Path::try_from(dirent.path()).map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid path: not UTF-8")
            })?;
            path_and_timestamp.push((metadata.modified()?, path));
        }
        path_and_timestamp.sort_by_key(|(timestamp, _)| *timestamp);
        for (_, path) in path_and_timestamp.into_iter() {
            let reader = match Reader::_open(path, libc::F_SETLK) {
                Ok(reader) => reader,
                Err(err) => {
                    if err.kind() == std::io::ErrorKind::WouldBlock {
                        continue;
                    }
                    return Err(err.into());
                }
            };
            return process(reader);
        }
        Ok(())
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use std::fs::remove_file;

    use biometrics::Emitter as EmitterTrait;

    use super::*;

    #[test]
    fn slash_metrics() {
        static COUNTER: Counter = Counter::new("foo");
        let collector = biometrics::Collector::new();
        collector.register_counter(&COUNTER);
        let mut slash_metrics = SlashMetrics::new();
        let _ = collector.emit(&mut slash_metrics, 42);
        assert_eq!(
            "# TYPE biometrics_collector_register_counter counter
biometrics_collector_register_counter 11 42
# TYPE biometrics_collector_register_gauge counter
biometrics_collector_register_gauge 0 42
# TYPE biometrics_collector_register_moments counter
biometrics_collector_register_moments 0 42
# TYPE biometrics_collector_register_histogram counter
biometrics_collector_register_histogram 0 42
# TYPE biometrics_collector_emit_counter counter
biometrics_collector_emit_counter 4 42
# TYPE biometrics_collector_emit_gauge counter
biometrics_collector_emit_gauge 0 42
# TYPE biometrics_collector_emit_moments counter
biometrics_collector_emit_moments 0 42
# TYPE biometrics_collector_emit_histogram counter
biometrics_collector_emit_histogram 0 42
# TYPE biometrics_collector_emit_failure counter
biometrics_collector_emit_failure 0 42
# TYPE biometrics_collector_time_failure counter
biometrics_collector_time_failure 0 42
# TYPE foo counter
foo 0 42
",
            slash_metrics.take()
        );
    }

    #[test]
    fn emitter() {
        static MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());
        // SAFETY(rescrv):  Mutex poisoning.
        let _guard = MUTEX.lock().unwrap();
        if Path::from("tmp.foo.42.prom").exists() {
            remove_file("tmp.foo.42.prom").unwrap();
        }
        let mut emitter = Emitter::new(Options {
            segment_size: 1024,
            flush_interval: Duration::from_secs(1),
            prefix: Path::new("tmp.foo."),
        });
        emitter.emit_counter(&Counter::new("foo"), 42).unwrap();
        drop(emitter);
    }

    #[test]
    fn reader() {
        let _reader = Reader::open(Path::new("README.md")).unwrap();
    }

    #[test]
    fn watcher() {
        emitter();
        let watcher = Watcher::new(Path::new("."));
        let mut watched = vec![];
        watcher
            .ingest_one(|reader| {
                watched.push(reader);
                Ok::<(), std::io::Error>(())
            })
            .unwrap();
        let found = watched[0].0.clone();
        assert!(
            found.basename().as_str().starts_with("README.md")
                || found.basename().as_str().starts_with("Cargo.toml")
                || found.basename().as_str().starts_with("k8s.metrics")
                || found.basename().as_str().starts_with("src")
                || found.basename().as_str().starts_with("tmp.foo.")
                || found.basename().as_str().starts_with(".gitignore"),
            "found: {found:?}",
        );
    }
}
