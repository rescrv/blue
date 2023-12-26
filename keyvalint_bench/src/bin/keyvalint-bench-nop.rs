//! A NOP benchmark.  Useful for testing benchmark overhead.

use std::ops::Bound;

use arrrg::CommandLine;
use biometrics::Collector;
use keyvalint::{Cursor as CursorTrait, KeyRef, WriteBatch as WriteBatchTrait};
use utilz::fmt::escape_str;

use keyvalint_bench::{workload, KeyValueStore};

const USAGE: &str = "USAGE: keyvalint-bench-nop [--nop-options] workload [--workload-options]";

//////////////////////////////////////////// NopOptions ////////////////////////////////////////////

#[derive(Debug, Default, Eq, PartialEq, arrrg_derive::CommandLine)]
struct NopOptions {
    #[arrrg(flag, "Print out the workload instead of NOP'ing.")]
    print: bool,
}

//////////////////////////////////////////// WriteBatch ////////////////////////////////////////////

#[derive(Debug, Default)]
struct WriteBatch {
    print: bool,
}

impl WriteBatch {
    pub fn new(print: bool) -> Self {
        Self { print }
    }
}

impl WriteBatchTrait for WriteBatch {
    fn put(&mut self, key: &[u8], timestamp: u64, value: &[u8]) {
        if self.print {
            println!(
                "WriteBatch::put(\"{}\" @ {} => \"{}\")",
                escape_str(key),
                timestamp,
                escape_str(value)
            );
        }
    }

    fn del(&mut self, key: &[u8], timestamp: u64) {
        if self.print {
            println!(
                "WriteBatch::del(\"{}\" @ {} => TOMBSTONE)",
                escape_str(key),
                timestamp
            );
        }
    }
}

////////////////////////////////////////////// Cursor //////////////////////////////////////////////

struct Cursor {
    print: bool,
}

impl CursorTrait for Cursor {
    type Error = &'static str;

    fn seek_to_first(&mut self) -> Result<(), Self::Error> {
        if self.print {
            println!("Cursor::seek_to_first()");
        }
        Ok(())
    }

    fn seek_to_last(&mut self) -> Result<(), Self::Error> {
        if self.print {
            println!("Cursor::seek_to_last()");
        }
        Ok(())
    }

    fn seek(&mut self, key: &[u8]) -> Result<(), Self::Error> {
        if self.print {
            println!("Cursor::seek({})", escape_str(key));
        }
        Ok(())
    }

    fn prev(&mut self) -> Result<(), Self::Error> {
        if self.print {
            println!("Cursor::prev()");
        }
        Ok(())
    }

    fn next(&mut self) -> Result<(), Self::Error> {
        if self.print {
            println!("Cursor::next()");
        }
        Ok(())
    }

    fn key(&self) -> Option<KeyRef> {
        None
    }

    fn value(&self) -> Option<&'_ [u8]> {
        None
    }
}

////////////////////////////////////////////// NopKvs //////////////////////////////////////////////

struct NopKvs {
    options: NopOptions,
}

impl keyvalint::KeyValueStore for NopKvs {
    type Error = &'static str;
    type WriteBatch<'a> = WriteBatch;

    fn put(&self, key: &[u8], timestamp: u64, value: &[u8]) -> Result<(), Self::Error> {
        let mut wb = Self::WriteBatch::new(self.options.print);
        wb.put(key, timestamp, value);
        self.write(wb)
    }

    fn del(&self, key: &[u8], timestamp: u64) -> Result<(), Self::Error> {
        let mut wb = Self::WriteBatch::new(self.options.print);
        wb.del(key, timestamp);
        self.write(wb)
    }

    fn write(&self, _: Self::WriteBatch<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl keyvalint::KeyValueLoad for NopKvs {
    type Error = &'static str;
    type RangeScan<'a> = Cursor;

    fn get(&self, key: &[u8], timestamp: u64) -> Result<Option<Vec<u8>>, Self::Error> {
        if self.options.print {
            println!("Cursor::get(\"{}\", {})", escape_str(key), timestamp);
        }
        Ok(None)
    }

    fn load(
        &self,
        key: &[u8],
        timestamp: u64,
        is_tombstone: &mut bool,
    ) -> Result<Option<Vec<u8>>, Self::Error> {
        *is_tombstone = false;
        if self.options.print {
            println!("Cursor::load(\"{}\", {})", escape_str(key), timestamp);
        }
        Ok(None)
    }

    fn range_scan<T: AsRef<[u8]>>(
        &self,
        start_bound: &Bound<T>,
        end_bound: &Bound<T>,
        timestamp: u64,
    ) -> Result<Self::RangeScan<'_>, Self::Error> {
        if self.options.print {
            fn bound_to_string<U: AsRef<[u8]>>(bound: &Bound<U>) -> String {
                match bound {
                    Bound::Unbounded => "Unbounded".to_string(),
                    Bound::Included(x) => format!("Included(\"{}\")", escape_str(x.as_ref())),
                    Bound::Excluded(x) => format!("Excluded(\"{}\")", escape_str(x.as_ref())),
                }
            }
            println!(
                "Cursor::range_scan(({}, {}), {})",
                bound_to_string(start_bound),
                bound_to_string(end_bound),
                timestamp
            );
        }
        Ok(Cursor {
            print: self.options.print,
        })
    }
}

impl KeyValueStore for NopKvs {
    fn register_biometrics(&self, _: &Collector) {}
}

/////////////////////////////////////////////// main ///////////////////////////////////////////////

fn main() {
    let (options, free) = NopOptions::from_command_line_relaxed(USAGE);
    if free.is_empty() {
        eprintln!("missing workload");
        eprintln!("{}", USAGE);
        std::process::exit(1);
    }
    let kvs = NopKvs { options };
    let mut workload = workload::from_command_line(USAGE, &free);
    workload.run(kvs);
}
