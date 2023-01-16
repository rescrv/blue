use clap::{App, Arg};

use lp::db::{DB, DBOptions};

fn main() {
    let app = App::new("lp-db-compactions")
        .version("0.1.0")
        .about("List the most efficient compactions recommended for the database.");
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

    for compaction in db.suggest_compactions().expect("could not suggest compactions").into_iter() {
        let mut first = true;
        for input in compaction.inputs() {
            if first {
                print!("{}", input.setsum());
            } else {
                print!(" {}", input.setsum());
            }
            first = false;
        }
        print!("\n");
    }
}
