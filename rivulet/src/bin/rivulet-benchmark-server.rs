use std::fs::File;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;

use boring::ssl::{SslAcceptor, SslFiletype, SslMethod, SslStream};

use biometrics::{Collector, PlainTextEmitter};

fn main() {
    std::thread::spawn(|| {
        let mut collector = Collector::new();
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
    // Setup our SSL preferences.
    let mut acceptor = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
    acceptor.set_ca_file("ca.pem").unwrap();
    acceptor.set_private_key_file("home.key", SslFiletype::PEM).unwrap();
    acceptor.set_certificate_file("home.crt", SslFiletype::PEM).unwrap();
    acceptor.check_private_key().expect("invalid private key");
    // TODO(rescrv):  Production blocker.
    acceptor.set_verify(boring::ssl::SslVerifyMode::NONE);
    let acceptor = Arc::new(acceptor.build());
    // Establish a listener.
    let listener = TcpListener::bind("127.0.0.1:1982").unwrap();
    // Act as server from this thread.
    let handle_client = |stream: SslStream<TcpStream>| {
        let (mut recv_chan, mut send_chan) = rivulet::from_stream(stream).expect("channel from stream");
        loop {
            let buf = recv_chan.recv().expect("recv");
            send_chan.send(buf.as_bytes()).expect("send");
        }
    };
    let mut threads = Vec::new();
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let acceptor = acceptor.clone();
                threads.push(std::thread::spawn(move || {
                    let stream = acceptor.accept(stream).unwrap();
                    handle_client(stream);
                }));
            }
            Err(e) => { eprintln!("failure: {}", e); }
        }
    }
    for thread in threads.into_iter() {
        thread.join().expect("join");
    }
}
