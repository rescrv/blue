use clap::{App, Arg, ArgMatches};

use super::db::{DB, DBOptions};

/////////////////////////////////////////////// --db ///////////////////////////////////////////////

pub fn db_args<'a>(app: App<'a, 'a>) -> App<'a, 'a> {
    let app = app.arg(
        Arg::with_name("db")
            .long("db")
            .takes_value(true)
            .help("Path to the database."));
    app
}

pub fn parse_db_options(_: &ArgMatches) -> DBOptions  {
    DBOptions::default()
}

pub fn parse_db_name<'a>(args: &'a ArgMatches) -> &'a str {
    args.value_of("db").unwrap_or("db")
}

pub fn parse_db_args(options: DBOptions, args: &ArgMatches) -> DB {
    let db = parse_db_name(args);
    DB::open(options, db).unwrap()
}

/////////////////////////////////////////////// ssts ///////////////////////////////////////////////

pub fn sst_args<'a>(app: App<'a, 'a>, index: u64) -> App<'a, 'a> {
    let app = app.arg(
        Arg::with_name("ssts")
            .index(index)
            .multiple(true)
            .help("List of ssts to use."));
    app
}

pub fn parse_sst_args<'a>(args: &'a ArgMatches) -> Vec<&'a str> {
    args.values_of("ssts").unwrap().collect()
}
