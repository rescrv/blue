use clap::{App, Arg};

use lp::cli::{db_args, parse_db_name, parse_db_type, parse_db_options};

use lp::db::{DB, GenerationalDB, TreeDB};

fn main() {
    let app = App::new("lp-db-fsck")
        .version("0.1.0")
        .about("Output a debug dump of an LP database.");
    let app = db_args(app);

    // parse
    let args = app.get_matches();
    let opts = parse_db_options(&args);
    let db = parse_db_name(&args);
    let ty = parse_db_type(&args);

    let errors = match ty {
        Some(Generational) => { GenerationalDB::fsck(opts, db) }
        Some(Tree) => { TreeDB::fsck(opts, db) }
        None => { Vec::new() },
    };
    for error in errors.into_iter() {
        println!("{}", error.to_string())
    }
}
