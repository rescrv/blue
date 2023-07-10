use std::fs::File;

use arrrg::CommandLine;

use biometrics::{Collector, PlainTextEmitter};

use split_channel::{SplitChannelCommandLine, RecvChannel, SendChannel};

fn main() {
    std::thread::spawn(|| {
        let mut collector = Collector::new();
        split_channel::register_biometrics(&mut collector);
        let fout = File::create("/dev/stdout").unwrap();
        let mut emit = PlainTextEmitter::new(fout);
        loop {
            if let Err(e) = collector.emit(&mut emit) {
                eprintln!("collector error: {}", e);
            }
            std::thread::sleep(std::time::Duration::from_millis(249));
        }
    });

    let (options, free) = SplitChannelCommandLine::from_command_line_relaxed("Usage: split_channel-benchmark-server [OPTIONS]");
    if !free.is_empty() {
        eprintln!("command ignores positional arguments");
    }
    let listener = options.bind_to().expect("bind-to");

    let handle_client = |mut recv_chan: RecvChannel, mut send_chan: SendChannel| {
        loop {
            let buf = recv_chan.recv().expect("recv");
            send_chan.send(buf.as_bytes()).expect("send");
        }
    };
    let mut threads = Vec::new();
    for stream in listener {
        match stream {
            Ok((recv_chan, send_chan)) => {
                threads.push(std::thread::spawn(move || {
                    handle_client(recv_chan, send_chan);
                }));
            }
            Err(e) => { eprintln!("failure: {}", e); }
        }
    }
    for thread in threads.into_iter() {
        thread.join().expect("join");
    }
}
