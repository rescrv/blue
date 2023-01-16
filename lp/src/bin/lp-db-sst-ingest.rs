use clap::{App, Arg};

use lp::db::{DBOptions, DB};

fn main() {
    let app = App::new("lp-db-sst-ingest")
        .version("0.1.0")
        .about("Ingest a set of SSTs into a database.");
    let app = app.arg(
        Arg::with_name("db")
            .long("db")
            .takes_value(true)
            .help("Path to the database."));
    let app = app.arg(
        Arg::with_name("sst")
            .index(1)
            .multiple(true)
            .help("List of ssts to ingest."));

    // parse
    let args = app.get_matches();
    let db = args.value_of("db").unwrap_or("db");
    let ssts: Vec<_> = args.values_of("sst").unwrap().collect();

    let opts = DBOptions::default();
    let db = DB::open(opts, db).expect("could not open database");
    db.ingest_ssts(&ssts).expect("could not ingest SSTs");
}
