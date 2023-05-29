use std::fs::File;

use biometrics::{Collector, PlainTextEmitter};

use rpc_pb::Context;
use rpc_pb::Client as ClientTrait;

use busybee::{Client, ClientOptions, DnsResolver};

fn main() {
    std::thread::spawn(|| {
        let mut collector = Collector::new();
        busybee::register_biometrics(&mut collector);
        rivulet::register_biometrics(&mut collector);
        let fout = File::create("/dev/stdout").unwrap();
        let mut emit = PlainTextEmitter::new(fout);
        loop {
            if let Err(e) = collector.emit(&mut emit) {
                eprintln!("collector error: {}", e);
            }
            std::thread::sleep(std::time::Duration::from_millis(249));
        }
    });
    let options = ClientOptions::default();
    let resolver = DnsResolver {
        host: "localhost".to_string(),
        port: 2049,
    };
    let context = Context::default();
    let client = Client::new(options, Box::new(resolver));
    for idx in 0..1_000_000 {
        client.call(&context, "__builtins__", "nop", &[]).expect("busybee rpc").expect("call");
        if idx % 10_000 == 0 {
            println!("FINDME idx={}", idx);
        }
    }
}
