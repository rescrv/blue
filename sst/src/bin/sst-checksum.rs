use clap::{App, Arg};

use lp::cli::{parse_sst_args, sst_args};
use lp::setsum::Setsum;
use lp::sst::SST;
use lp::Cursor;

fn fast_setsum(sst: &str) -> String {
    let sst = SST::new(sst).expect("open SST");
    sst.setsum()
}

fn slow_setsum(sst: &str) -> String {
    let sst = SST::new(sst).expect("open SST");
    let mut cursor = sst.cursor();
    cursor.seek_to_first().expect("seek SST");
    let mut setsum = Setsum::default();
    loop {
        cursor.next().expect("next");
        if let Some(kvr) = cursor.value() {
            match kvr.value {
                Some(v) => { setsum.put(kvr.key, kvr.timestamp, v); },
                None => { setsum.del(kvr.key, kvr.timestamp); },
            }
        } else {
            break;
        }
    }
    setsum.hexdigest()
}

fn main() {
    let app = App::new("zataods-lp-sst-checksum")
        .version("0.1.0")
        .about("Checksum the provided SST files.");
    let app = sst_args(app, 1);
    let app = app.arg(
        Arg::with_name("fast")
            .long("fast")
            .takes_value(false)
            .help("Use the embedded setsum instead of computing one."));

    // parse
    let args = app.get_matches();
    let ssts = parse_sst_args(&args);
    let fast = args.is_present("fast");

    for sst in ssts.into_iter() {
        if fast {
            println!("{} {}", fast_setsum(&sst), sst);
        } else {
            println!("{} {}", slow_setsum(&sst), sst);
        }
    }
}
