use std::env;
use std::io;
use std::path::PathBuf;

use ci::select_ci::{Workspace, select_from_files};

const USAGE: &str =
    "usage: select-ci-packages --changed-files PATH --global-fixtures PATH --output PATH";

#[derive(Debug)]
struct Args {
    changed_files: PathBuf,
    global_fixtures: PathBuf,
    output: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args(env::args().skip(1))?;
    let workspace = Workspace::load()?;
    let selection = select_from_files(&workspace, &args.changed_files, &args.global_fixtures)?;
    selection.print_report();
    selection.write_facts(&args.output)?;
    Ok(())
}

fn parse_args<I, S>(args: I) -> Result<Args, Box<dyn std::error::Error>>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut changed_files = None;
    let mut global_fixtures = None;
    let mut output = None;
    let mut args = args.into_iter().map(Into::into);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--changed-files" => {
                changed_files = Some(PathBuf::from(args.next().ok_or_else(usage_error)?));
            }
            "--global-fixtures" => {
                global_fixtures = Some(PathBuf::from(args.next().ok_or_else(usage_error)?));
            }
            "--output" => {
                output = Some(PathBuf::from(args.next().ok_or_else(usage_error)?));
            }
            _ => return Err(usage_error().into()),
        }
    }
    Ok(Args {
        changed_files: changed_files.ok_or_else(usage_error)?,
        global_fixtures: global_fixtures.ok_or_else(usage_error)?,
        output: output.ok_or_else(usage_error)?,
    })
}

fn usage_error() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, USAGE)
}
