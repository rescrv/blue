use clap::{App, Arg};

use lp::db::{DBOptions, DB};

fn main() {
    let app = App::new("lp-setup-compaction")
        .version("0.1.0")
        .about("Setup a set of SSTs as the input to a compaction.");
    let app = app.arg(
        Arg::with_name("db")
            .long("db")
            .takes_value(true)
            .help("Path to the database."));
    let app = app.arg(
        Arg::with_name("sst")
            .index(1)
            .multiple(true)
            .help("List of ssts to compact."));

    // parse
    let args = app.get_matches();
    let db = args.value_of("db").unwrap_or("db");
    let ssts: Vec<_> = args.values_of("sst").unwrap().collect();

    let opts = DBOptions::default();
    let db = DB::open(opts, db).expect("could not open database");
    let path = db.setup_compaction(&ssts).expect("could not setup SSTs for compaction");
    println!("{}", path);
}
