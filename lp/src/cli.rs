use clap::{App, Arg, ArgMatches};

use super::db::{DBOptions, DB, GenerationalDB, TreeDB};

pub fn db_args<'a>(app: App<'a, 'a>) -> App<'a, 'a> {
    app.arg(
        Arg::with_name("db")
            .long("db")
            .takes_value(true)
            .help("Path to the database."))
}

pub fn parse_db_options(args: &ArgMatches) -> DBOptions  {
    DBOptions::default()
}

pub fn parse_db_args(options: DBOptions, args: &ArgMatches) -> Box<dyn DB>{
    let db = args.value_of("db").unwrap_or("db");
    // TODO(rescrv): XXX open either type of db
    let db = TreeDB::open(options, db).expect("could not open database");
    Box::new(db)
}
