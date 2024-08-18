use std::fs::File;
use std::time::SystemTime;

use arrrg::CommandLine;
use arrrg_derive::CommandLine;

use biometrics::{Collector, PlainTextEmitter};

use busyrpc::{Server, ServerOptions, ServiceRegistry, SslOptions};

use rpc_pb::IoToZ;

#[derive(CommandLine, Debug, Default, Eq, PartialEq)]
struct BenchmarkOptions {
    #[arrrg(nested)]
    ssl: SslOptions,
    #[arrrg(nested)]
    server: ServerOptions,
}

fn main() {
    let (options, free) =
        BenchmarkOptions::from_command_line("Usage: busyrpc-benchmark-server [OPTIONS]");
    if !free.is_empty() {
        eprintln!("command takes no arguments");
        std::process::exit(1);
    }
    std::thread::spawn(|| {
        let mut collector = Collector::new();
        busyrpc::register_biometrics(&mut collector);
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
    let services = ServiceRegistry::new();
    let (server, _) = Server::new(options.ssl, options.server, services)
        .as_z()
        .pretty_unwrap();
    server.serve().as_z().pretty_unwrap();
}
