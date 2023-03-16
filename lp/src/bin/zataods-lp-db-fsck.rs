use clap::App;

use lp::cli::{db_args, parse_db_name, parse_db_options};

use lp::db::DB;

fn main() {
    let app = App::new("zataods-lp-db-fsck")
        .version("0.1.0")
        .about("Output a debug dump of an LP database.");
    let app = db_args(app);

    // parse
    let args = app.get_matches();
    let opts = parse_db_options(&args);
    let db = parse_db_name(&args);
    let errors = DB::fsck(opts, db);

    for error in errors.into_iter() {
        println!("{}", error.to_string())
    }
}
