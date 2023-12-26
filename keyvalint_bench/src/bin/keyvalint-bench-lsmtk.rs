use std::sync::Arc;

use arrrg::CommandLine;
use lsmtk::{IoToZ, KeyValueStore, LsmtkOptions};
use zerror::Z;

use keyvalint_bench::workload;

const USAGE: &str = "USAGE: keyvalint-bench-lsmtk [--lsmtk-options] workload [--workload-options]";

/////////////////////////////////////////////// main ///////////////////////////////////////////////

fn main() {
    let (options, free) = LsmtkOptions::from_command_line_relaxed(USAGE);
    if free.is_empty() {
        eprintln!("missing workload");
        eprintln!("{}", USAGE);
        std::process::exit(1);
    }
    let kvs = Arc::new(KeyValueStore::open(options).as_z().pretty_unwrap());
    let kvs_p = Arc::clone(&kvs);
    let _memtable_thread = std::thread::spawn(move || loop {
        if let Err(err) = kvs_p.memtable_thread() {
            eprintln!("{}", err.long_form());
            std::thread::sleep(std::time::Duration::from_secs(10));
        }
    });
    let kvs_p = Arc::clone(&kvs);
    let _compaction_thread = std::thread::spawn(move || loop {
        if let Err(err) = kvs_p.compaction_thread() {
            eprintln!("{}", err.long_form());
            std::thread::sleep(std::time::Duration::from_secs(10));
        }
    });
    let kvs_p = Arc::clone(&kvs);
    let _compaction_thread = std::thread::spawn(move || loop {
        if let Err(err) = kvs_p.compaction_thread() {
            eprintln!("{}", err.long_form());
            std::thread::sleep(std::time::Duration::from_secs(10));
        }
    });
    let kvs_p = Arc::clone(&kvs);
    let _compaction_thread = std::thread::spawn(move || loop {
        if let Err(err) = kvs_p.compaction_thread() {
            eprintln!("{}", err.long_form());
            std::thread::sleep(std::time::Duration::from_secs(10));
        }
    });
    let kvs_p = Arc::clone(&kvs);
    let _compaction_thread = std::thread::spawn(move || loop {
        if let Err(err) = kvs_p.compaction_thread() {
            eprintln!("{}", err.long_form());
            std::thread::sleep(std::time::Duration::from_secs(10));
        }
    });
    let mut workload = workload::from_command_line(USAGE, &free);
    workload.run(kvs);
}
