//use clap::App;

//use lp::cli::{db_args, parse_db_args, parse_db_options};

fn main() {
    /*
    let app = App::new("zataods-lp-db-compactions")
        .version("0.1.0")
        .about("List the most efficient compactions recommended for the database.");
    let app = db_args(app);

    // parse
    let args = app.get_matches();
    let opts = parse_db_options(&args);
    let db = parse_db_args(opts, &args);

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
    */
}
