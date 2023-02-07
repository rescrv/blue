use clap::{App, Arg, ArgMatches};

use super::db::{DB, DBOptions, DBType, GenerationalDB, TreeDB};

pub fn db_args<'a>(app: App<'a, 'a>) -> App<'a, 'a> {
    let app = app.arg(
        Arg::with_name("db")
            .long("db")
            .takes_value(true)
            .help("Path to the database."));
    app.arg(
        Arg::with_name("db-format")
            .long("db-format")
            .takes_value(true)
            .help("Format of the database (e.g. generational or tree)."))
}

pub fn parse_db_options(args: &ArgMatches) -> DBOptions  {
    DBOptions::default()
}

pub fn parse_db_type<'a>(args: &'a ArgMatches) -> Option<DBType> {
    match args.value_of("db-format").unwrap_or("tree") {
        "generational" => { Some(DBType::Generational) },
        "tree" => { Some(DBType::Tree) },
        _ => { None }
    }
}

pub fn parse_db_name<'a>(args: &'a ArgMatches) -> &'a str {
    args.value_of("db").unwrap_or("db")
}

pub fn parse_db_args(options: DBOptions, args: &ArgMatches) -> Box<dyn DB> {
    let ty = parse_db_type(args);
    let db = parse_db_name(args);
    ty.expect("invalid type").open(options, db).expect("could not open database")
}
