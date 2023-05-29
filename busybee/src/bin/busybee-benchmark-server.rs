use std::fs::File;

use biometrics::{Collector, PlainTextEmitter};

use busybee::{Server, ServerOptions};

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
    let options = ServerOptions::default()
        .with_ca_file("ca.pem")
        .with_private_key_file("home.key")
        .with_certificate_file("home.crt")
        .with_bind_to_host("localhost")
        .with_bind_to_port(2049)
        .with_thread_pool_size(64);
    let polling = rivulet::default_poll().expect("poll");
    let server = Server::new(options, polling);
    server.serve().unwrap();
}
