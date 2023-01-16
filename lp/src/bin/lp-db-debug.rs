use clap::{App, Arg};

use lp::db::{DBOptions, DB};

fn main() {
    let app = App::new("lp-db-debug")
        .version("0.1.0")
        .about("Output a debug dump of an LP database.");
    let app = app.arg(
        Arg::with_name("db")
            .long("db")
            .takes_value(true)
            .help("Path to the database."));

    // parse
    let args = app.get_matches();
    let db = args.value_of("db").unwrap_or("db");

    let opts = DBOptions::default();
    let db = DB::open(opts, db).expect("could not open database");
    db.debug_dump();
}
