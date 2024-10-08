use std::fs::{read_dir, remove_file, File};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use arrrg::CommandLine;
use biometrics::{Collector, PlainTextEmitter};
use zerror::Z;

use lsmtk::{IoToZ, LsmTree, LsmtkOptions, TRACING};

fn main() {
    let (options, free) =
        LsmtkOptions::from_command_line("USAGE: lsmtk-watch-for-ingest [OPTIONS]");
    if !free.is_empty() {
        eprintln!("command takes no arguments");
        std::process::exit(1);
    }
    std::thread::spawn(|| {
        let collector = Collector::new();
        sst::register_biometrics(&collector);
        lsmtk::register_biometrics(&collector);
        let fout = File::create("/dev/stdout").unwrap();
        let mut emit = PlainTextEmitter::new(fout);
        loop {
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("clock should never fail")
                .as_millis()
                .try_into()
                .expect("millis since epoch should fit u64");
            if let Err(e) = collector.emit(&mut emit, now) {
                eprintln!("collector error: {}", e);
            }
            std::thread::sleep(std::time::Duration::from_millis(249));
        }
    });
    let root = PathBuf::from(options.path());
    let tree = Arc::new(LsmTree::open(options).as_z().pretty_unwrap());
    let emitter = indicio::protobuf::ProtobufEmitter::new("clues", 1 << 30)
        .as_z()
        .pretty_unwrap();
    TRACING.register(emitter);
    TRACING.set_verbosity(indicio::INFO);
    let tree_p = Arc::clone(&tree);
    let _compactor = std::thread::spawn(move || loop {
        if let Err(err) = tree_p.compaction_thread() {
            eprintln!("{}", err.long_form());
        }
    });
    let tree_p = Arc::clone(&tree);
    let _compactor = std::thread::spawn(move || loop {
        if let Err(err) = tree_p.compaction_thread() {
            eprintln!("{}", err.long_form());
        }
    });
    let tree_p = Arc::clone(&tree);
    let _compactor = std::thread::spawn(move || loop {
        if let Err(err) = tree_p.compaction_thread() {
            eprintln!("{}", err.long_form());
        }
    });
    let tree_p = Arc::clone(&tree);
    let _compactor = std::thread::spawn(move || loop {
        if let Err(err) = tree_p.compaction_thread() {
            eprintln!("{}", err.long_form());
        }
    });
    let tree_p = Arc::clone(&tree);
    let _compactor = std::thread::spawn(move || loop {
        if let Err(err) = tree_p.compaction_thread() {
            eprintln!("{}", err.long_form());
        }
    });
    let tree_p = Arc::clone(&tree);
    let _compactor = std::thread::spawn(move || loop {
        if let Err(err) = tree_p.compaction_thread() {
            eprintln!("{}", err.long_form());
        }
    });
    let tree_p = Arc::clone(&tree);
    let _compactor = std::thread::spawn(move || loop {
        if let Err(err) = tree_p.compaction_thread() {
            eprintln!("{}", err.long_form());
        }
    });
    let tree_p = Arc::clone(&tree);
    let _compactor = std::thread::spawn(move || loop {
        if let Err(err) = tree_p.compaction_thread() {
            eprintln!("{}", err.long_form());
        }
    });
    let tree_p = Arc::clone(&tree);
    let _compactor = std::thread::spawn(move || loop {
        if let Err(err) = tree_p.compaction_thread() {
            eprintln!("{}", err.long_form());
        }
    });
    let tree_p = Arc::clone(&tree);
    let _compactor = std::thread::spawn(move || loop {
        if let Err(err) = tree_p.compaction_thread() {
            eprintln!("{}", err.long_form());
        }
    });
    let tree_p = Arc::clone(&tree);
    let _compactor = std::thread::spawn(move || loop {
        if let Err(err) = tree_p.compaction_thread() {
            eprintln!("{}", err.long_form());
        }
    });
    let tree_p = Arc::clone(&tree);
    let _compactor = std::thread::spawn(move || loop {
        if let Err(err) = tree_p.compaction_thread() {
            eprintln!("{}", err.long_form());
        }
    });
    loop {
        std::thread::sleep(std::time::Duration::from_millis(1_000));
        let mut ingest = Vec::new();
        for entry in read_dir(root.join("ingest")).as_z().pretty_unwrap() {
            let entry = entry.as_z().pretty_unwrap();
            ingest.push((
                entry.metadata().as_z().pretty_unwrap(),
                entry.path().to_path_buf(),
            ));
        }
        ingest.sort_by_key(|x| x.0.modified().expect("platform should provide mtime"));
        for (_, path) in ingest.iter() {
            tree.ingest(path).as_z().pretty_unwrap();
        }
        for (_, path) in ingest.into_iter() {
            remove_file(path).as_z().pretty_unwrap();
        }
    }
}
