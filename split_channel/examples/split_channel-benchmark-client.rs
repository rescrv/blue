use std::fs::File;
use std::time::SystemTime;

use arrrg::CommandLine;

use biometrics::{Collector, PlainTextEmitter};

use split_channel::SplitChannelOptions;

fn main() {
    std::thread::spawn(|| {
        let mut collector = Collector::new();
        split_channel::register_biometrics(&mut collector);
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
    let (options, free) = SplitChannelOptions::from_command_line_relaxed(
        "Usage: split_channel-benchmark-client [OPTIONS]",
    );
    if !free.is_empty() {
        eprintln!("command ignores positional arguments");
    }
    let (mut recv_chan, mut send_chan) = match options.connect() {
        Ok((recv_chan, send_chan)) => (recv_chan, send_chan),
        Err(e) => {
            panic!("err: {}", e);
        }
    };
    let mut counter = 0u64;
    loop {
        let msg = format!("ping {}", counter);
        let buf = msg.as_bytes();
        counter += 1;
        send_chan.send(buf).expect("send");
        let _ = recv_chan.recv().expect("recv");
    }
}
