use std::io::{self, BufRead, Write};
use std::time::Duration;

use arrrg::CommandLine;

use asllm::{AsLlm, DEFAULT_TOKEN_DELAY};

#[derive(Clone, Debug, arrrg_derive::CommandLine, Default, Eq, PartialEq)]
struct Options {
    #[arrrg(
        optional,
        "Delay in milliseconds between token writes.",
        "MILLISECONDS"
    )]
    delay_ms: Option<u64>,
    #[arrrg(flag, "Write output to stderr instead of stdout.")]
    stderr: bool,
}

fn main() -> io::Result<()> {
    let (options, free) =
        Options::from_command_line("USAGE: asllm [--delay-ms MILLISECONDS] [--stderr]");
    if !free.is_empty() {
        eprintln!("unexpected positional arguments: {:?}", free);
        std::process::exit(1);
    }

    let delay = options
        .delay_ms
        .map(Duration::from_millis)
        .unwrap_or(DEFAULT_TOKEN_DELAY);

    let stdin = io::stdin();
    let mut reader = io::BufReader::new(stdin.lock());
    if options.stderr {
        write_lines_through_asllm(&mut AsLlm::with_delay(io::stderr(), delay), &mut reader)?;
    } else {
        write_lines_through_asllm(&mut AsLlm::with_delay(io::stdout(), delay), &mut reader)?;
    }

    Ok(())
}

fn write_lines_through_asllm<W: Write>(
    writer: &mut AsLlm<W>,
    reader: &mut dyn BufRead,
) -> io::Result<()> {
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            break;
        }
        writer.write_all(line.as_bytes())?;
    }
    writer.flush()?;
    Ok(())
}
