use std::fs::File;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use arrrg::CommandLine;
use arrrg_derive::CommandLine;

use biometrics::{Collector, PlainTextEmitter};

use rpc_pb::Client;
use rpc_pb::{Context, IoToZ};

use busyrpc::{new_client, ClientOptions, StringResolver};

#[derive(CommandLine, Debug, Default, Eq, PartialEq)]
struct BenchmarkOptions {
    #[arrrg(required, "Host connection string in host:ID;host:port,host:ID;host:port format")]
    connect: StringResolver,
    #[arrrg(optional, "Threads to run.")]
    threads: usize,
    #[arrrg(optional, "RPCs to make before exiting.")]
    rpcs: u64,
    #[arrrg(nested)]
    client: ClientOptions,
}

fn worker(client: Arc<dyn Client>, counter: Arc<AtomicU64>, rpcs: u64) {
    while counter.fetch_add(1, Ordering::Relaxed) < rpcs {
        let context = Context::default();
        client.call(&context, "__builtins__", "nop", &[]).as_z().pretty_unwrap().unwrap();
    }
}

fn main() {
    let (options, free) = BenchmarkOptions::from_command_line("Usage: busyrpc-benchmark-client [OPTIONS]");
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
            if let Err(e) = collector.emit(&mut emit) {
                eprintln!("collector error: {}", e);
            }
            std::thread::sleep(std::time::Duration::from_millis(249));
        }
    });
    let client = new_client(options.client, options.connect.clone());
    let request_counter = Arc::new(AtomicU64::default());
    let mut threads = Vec::new();
    for _ in 0..options.threads {
        let cl = Arc::clone(&client);
        let rc = Arc::clone(&request_counter);
        threads.push(std::thread::spawn(move || {
            worker(cl, rc, options.rpcs);
        }));
    }
    for thread in threads.into_iter() {
        thread.join().unwrap();
    }
}
