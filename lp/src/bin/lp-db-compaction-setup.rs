use clap::{App, Arg};

use lp::cli::{db_args, parse_db_args, parse_db_options, sst_args, parse_sst_args};

fn main() {
    let app = App::new("lp-db-compaction-setup")
        .version("0.1.0")
        .about("Setup the inputs for a single compaction.");
    let app = db_args(app);
    let app = sst_args(app, 1);

    // parse
    let args = app.get_matches();
    let opts = parse_db_options(&args);
    let db = parse_db_args(opts.clone(), &args);
    let ssts = parse_sst_args(&args);

    db.compaction_setup(opts.compaction, &ssts, 0).unwrap();
}
