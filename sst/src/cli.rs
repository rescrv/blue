use clap::{App, Arg, ArgMatches};

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
