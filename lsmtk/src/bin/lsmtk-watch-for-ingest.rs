use std::fs::{read_dir, remove_file};
use std::path::PathBuf;
use std::sync::Arc;

use arrrg::CommandLine;

use zerror::Z;

use lsmtk::{IoToZ, LsmOptions};

fn main() {
    let (options, free) = LsmOptions::from_command_line("USAGE: lsmtk-watch-for-ingest [OPTIONS]");
    if !free.is_empty() {
        eprintln!("command takes no arguments");
        std::process::exit(1);
    }
    let root = PathBuf::from(options.path().clone());
    let db = Arc::new(options.open().as_z().pretty_unwrap());
    let db_p = Arc::clone(&db);
    let _compactor = std::thread::spawn(move || {
        loop {
            if let Err(err) = db_p.compaction_background_thread() {
                eprintln!("{}", err.long_form());
            }
        }
    });
    loop {
        std::thread::sleep(std::time::Duration::from_millis(1_000));
        let mut ingest = Vec::new();
        for entry in read_dir(root.join("ingest")).as_z().pretty_unwrap() {
            let entry = entry.as_z().pretty_unwrap();
            ingest.push(entry.path().to_path_buf());
            if ingest.len() > 12 {
                break;
            }
        }
        db.ingest(&ingest).as_z().pretty_unwrap();
        for path in ingest.into_iter() {
            remove_file(path).as_z().pretty_unwrap();
        }
    }
}
