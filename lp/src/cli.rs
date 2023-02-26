use clap::{App, Arg, ArgMatches};

use super::db::{DB, DBOptions};

pub fn db_args<'a>(app: App<'a, 'a>) -> App<'a, 'a> {
    let app = app.arg(
        Arg::with_name("db")
            .long("db")
            .takes_value(true)
            .help("Path to the database."));
    app
}

pub fn parse_db_options(args: &ArgMatches) -> DBOptions  {
    DBOptions::default()
}

pub fn parse_db_name<'a>(args: &'a ArgMatches) -> &'a str {
    args.value_of("db").unwrap_or("db")
}

pub fn parse_db_args(options: DBOptions, args: &ArgMatches) -> DB {
    let db = parse_db_name(args);
    DB::open(options, db).unwrap()
}
