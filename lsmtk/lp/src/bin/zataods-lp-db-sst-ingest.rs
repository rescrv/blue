//use std::path::PathBuf;

//use clap::{App, Arg};

//use lp::cli::{db_args, parse_db_args, parse_db_options};

fn main() {
    /*
    let app = App::new("zataods-lp-db-sst-ingest")
        .version("0.1.0")
        .about("Ingest a set of SSTs into a database.");
    let app = db_args(app);
    let app = app.arg(
        Arg::with_name("sst")
            .index(1)
            .multiple(true)
            .help("List of ssts to ingest."));

    // parse
    let args = app.get_matches();
    let opts = parse_db_options(&args);
    let db = parse_db_args(opts, &args);

    let ssts: Vec<PathBuf> = args.values_of("sst").unwrap()
        .map(|x| PathBuf::from(x))
        .collect();

    db.ingest(&ssts).expect("could not ingest SSTs");
    */
}
