use std::io::Write;

use arrrg::CommandLine;
use arrrg_derive::CommandLine;

use guacamole::Guacamole;

#[derive(CommandLine, Default, Eq, PartialEq)]
struct GuacamoleCommandLine {
    #[arrrg(
        optional,
        "Number of bytes to generate before exiting.  By default, 1<<64.",
        "N"
    )]
    bytes: Option<u64>,
    #[arrrg(optional, "Guacamole seed.")]
    seed: Option<u64>,
}

/// Generate pseudo-random, predictable bytes.
fn main() {
    let (cmdline, free) = GuacamoleCommandLine::from_command_line("Usage: guacamole [OPTIONS]");
    if !free.is_empty() {
        panic!("free arguments are not accepted");
    }
    let mut guac = Guacamole::new(cmdline.seed.unwrap_or(0));
    let mut remain = cmdline.bytes.unwrap_or(u64::max_value()) as usize;
    let mut buf = [0u8; 1 << 20];
    let buf: &mut [u8] = &mut buf;
    while remain > 0 {
        let amt = if remain > buf.len() {
            buf.len()
        } else {
            remain
        };
        guac.generate(&mut buf[..amt]);
        std::io::stdout()
            .write_all(&buf[..amt])
            .expect("failed to write");
        remain -= amt;
    }
}
