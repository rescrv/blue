use std::fs::File;

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
            if let Err(e) = collector.emit(&mut emit) {
                eprintln!("collector error: {}", e);
            }
            std::thread::sleep(std::time::Duration::from_millis(249));
        }
    });
    let mut verifier = LsmVerifier::open(options).as_z().pretty_unwrap();
    loop {
        std::thread::sleep(std::time::Duration::from_millis(1_000));
        let ret = verifier.verify();
        if let Err(Error::Backoff { setsum, .. }) = ret {
            // TODO(rescrv):  Output this error if we wait more than too many seconds.
            _ = setsum;
        } else if let Err(err) = ret {
            eprintln!("error:\n{}", err.long_form());
        }
    }
}
