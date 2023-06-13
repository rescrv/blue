use rustyline::history::MemHistory;
use rustyline::{Config, Editor, Result};

use texttale::{ExpectTextTale, TextTale, ShellTextTale};

////////////////////////////////////////////// Player //////////////////////////////////////////////

#[derive(Debug)]
struct Player {
    name: String,
    age: u8,
    gender: String,
    race: String,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            name: "Link".to_owned(),
            gender: "unspecified".to_owned(),
            age: 18,
            race: "Hylian".to_owned(),
        }
    }
}

///////////////////////////////////////////// bootstrap ////////////////////////////////////////////

const BOOTSTRAP_HELP: &str = "Welcome to the texttale library.

help: ....... Print this help menu.
character: .. Configure your character for this texttale.
begin: ...... Start off on your journey with the given character.
";

fn bootstrap<T: TextTale>(tale: &mut T) -> Result<()> {
    let mut print_help = true;
    let mut player = Player::default();
    'bootstrapping:
    loop {
        if print_help {
            writeln!(tale, "{}", BOOTSTRAP_HELP)?;
            print_help = false;
        }
        if let Some(ref line) = tale.next_command() {
            match line.as_str() {
                "help" => {
                    print_help = true;
                },
                "character" => {
                    character(tale, &mut player)?;
                    print_help = true;
                }
                "begin" => {
                    return steady_state(tale, player);
                }
                _ => {
                    writeln!(tale, "unknown command: {}", line.as_str())?;
                },
            }
        } else {
            break 'bootstrapping;
        }
    }
    Ok(())
}

///////////////////////////////////////////// character ////////////////////////////////////////////

const CHARACTER_HELP: &str = "Craft your character.

help: .... Print this help menu.
name: .... Set your character's name.
age: ..... Set your character's age.
gender: .. Set your character's gender.
race: .... Set your character's race.
print: ... Print your character.
save: .... Commit changes to the configuration and return to previous menu.
";

fn character<T: TextTale>(tale: &mut T, mut player: &mut Player) -> Result<()> {
    let mut print_help = true;
    'configuring:
    loop {
        if print_help {
            writeln!(tale, "{}", CHARACTER_HELP)?;
            print_help = false;
        }
        if let Some(ref line) = tale.next_command() {
            let cmd: Vec<&str> = line.split_whitespace().collect();
            if cmd.is_empty() {
                continue 'configuring;
            }
            match cmd[0] {
                "help" => {
                    print_help = true;
                },
                "name" => {
                    player.name = cmd[1..].to_vec().join(" ")
                },
                "gender" => {
                    player.gender = cmd[1..].to_vec().join(" ")
                },
                "race" => {
                    player.race = cmd[1..].to_vec().join(" ")
                },
                "age" => {
                    if cmd.len() != 2 {
                        writeln!(tale, "USAGE: age [age]")?;
                    } else {
                        player.age = match cmd[1].parse::<u8>() {
                            Ok(age) => age,
                            Err(err) => {
                                writeln!(tale, "invalid age: {}", err)?;
                                continue 'configuring;
                            },
                        };
                    }
                },
                "print" => {
                    writeln!(tale, "{:#?}", player)?;
                },
                "save" => {
                    break 'configuring;
                },
                _ => {
                    writeln!(tale, "unknown command: {}", line.as_str())?;
                },
            }
        } else {
            break 'configuring;
        }
    }
    Ok(())
}

/////////////////////////////////////////// steady_state ///////////////////////////////////////////

const STEADY_STATE_HELP: &str = "Welcome to adventure mode.

It's quite boring, but this simple quest is enough to test the texttale library.

help: ....... Print this help menu.
character: .. Configure your character for this texttale.
end: ........ Unceremoniously end this adventure.
";

fn steady_state<T: TextTale>(tale: &mut T, mut player: Player) -> Result<()> {
    let mut print_help = true;
    'adventuring:
    loop {
        if print_help {
            writeln!(tale, "{}", STEADY_STATE_HELP)?;
            print_help = false;
        }
        if let Some(ref line) = tale.next_command() {
            let cmd: Vec<&str> = line.split_whitespace().collect();
            if cmd.is_empty() {
                continue 'adventuring;
            }
            match cmd[0] {
                "help" => {
                    print_help = true;
                },
                "character" => {
                    character(tale, &mut player)?;
                    print_help = true;
                }
                "end" => {
                    break 'adventuring;
                }
                _ => {
                    writeln!(tale, "unknown command: {}", line.as_str())?;
                },
            }
        } else {
            break 'adventuring;
        }
    }
    Ok(())
}

/////////////////////////////////////////////// main ///////////////////////////////////////////////

fn main() -> Result<()> {
    // Create the line editor.
    let config = Config::builder()
        .max_history_size(1_000_000)?
        .history_ignore_dups(true)?
        .history_ignore_space(true)
        .build();
    let hist = MemHistory::new();
    let rl = Editor::with_history(config, hist)?;

    // Interpret the command line.
    let mut args: Vec<String> = std::env::args().collect();
    args.remove(0);
    if args.is_empty() {
        let mut tale = ShellTextTale::new(rl, "> ");
        bootstrap(&mut tale)
    } else {
        for arg in args {
            let mut tale = ExpectTextTale::new(arg)?;
            bootstrap(&mut tale)?;
        }
        Ok(())
    }
}
