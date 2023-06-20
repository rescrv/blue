use rustyline::history::MemHistory;
use rustyline::{Config, Editor, Result};

use arrrg::CommandLine;
use arrrg_derive::CommandLine;

use texttale::{ExpectTextTale, ShellTextTale};

use gremlins::{ControlCenter, ControlCenterOptions};

////////////////////////////////////////////// Options /////////////////////////////////////////////

#[derive(CommandLine, Debug, Default, Eq, PartialEq)]
pub struct Options {
    #[arrrg(nested)]
    control_center: ControlCenterOptions,
}

/////////////////////////////////////////////// main ///////////////////////////////////////////////

fn main() -> Result<()> {
    // Interpret the command line.
    let (options, args) = Options::from_command_line("Usage: gremlins [OPTIONS] [SCRIPTS*]");
    if args.is_empty() {
        let mut control_center = ControlCenter::new(options.control_center);
        // Create the line editor.
        let config = Config::builder()
            .max_history_size(1_000_000)?
            .history_ignore_dups(true)?
            .history_ignore_space(true)
            .build();
        let hist = MemHistory::new();
        let rl = Editor::with_history(config, hist)?;
        let mut tale = ShellTextTale::new(rl, "> ");
        control_center.main_menu(&mut tale).expect("main_menu");
    } else {
        for arg in args {
            let mut control_center = ControlCenter::new(options.control_center.clone());
            let mut tale = ExpectTextTale::new(arg)?;
            control_center.main_menu(&mut tale).expect("main_menu");
        }
    }
    Ok(())
}
