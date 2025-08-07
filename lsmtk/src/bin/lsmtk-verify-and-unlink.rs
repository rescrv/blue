use std::fs::File;
use std::time::SystemTime;

use arrrg::CommandLine;
use biometrics::{Collector, PlainTextEmitter};
use zerror::Z;

use lsmtk::{Error, IoToZ, LsmVerifier, LsmtkOptions};

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
                eprintln!("collector error: {e}");
            }
            std::thread::sleep(std::time::Duration::from_millis(249));
        }
    });
    let mut verifier = LsmVerifier::open(options).as_z().pretty_unwrap();
    loop {
        std::thread::sleep(std::time::Duration::from_millis(1_000));
        let ret = verifier.verify();
        if let Err(Error::Backoff { path, .. }) = ret {
            // TODO(rescrv):  Output this error if we wait more than too many seconds.
            _ = path;
        } else if let Err(err) = ret {
            eprintln!("error:\n{}", err.long_form());
        }
    }
}
