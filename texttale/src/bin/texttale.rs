use std::fmt::{Debug, Formatter};

use rustyline::history::MemHistory;
use rustyline::{Config, Editor, Result};

use texttale::{ExpectTextTale, TextTale, ShellTextTale};

////////////////////////////////////////////// Player //////////////////////////////////////////////

struct Player<T: TextTale> {
    name: String,
    age: u8,
    gender: String,
    race: String,
    tale: T,
}

impl<T: TextTale> Player<T> {
    fn new(tale: T) -> Self {
        Self {
            name: "Link".to_owned(),
            gender: "unspecified".to_owned(),
            age: 18,
            race: "Hylian".to_owned(),
            tale,
        }
    }
}

impl<T: TextTale> Debug for Player<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Player")
            .field("name", &self.name)
            .field("age", &self.age)
            .field("gender", &self.gender)
            .field("race", &self.race)
            .finish()
    }
}

///////////////////////////////////////////// bootstrap ////////////////////////////////////////////

const BOOTSTRAP_HELP: &str = "Welcome to the texttale library.

help: ....... Print this help menu.
character: .. Configure your character for this texttale.
begin: ...... Start off on your journey with the given character.
";

impl<T: TextTale> Player<T> {
    fn bootstrap(&mut self) -> Result<()> {
        let mut print_help = true;
        'bootstrapping:
        loop {
            if print_help {
                writeln!(self.tale, "{}", BOOTSTRAP_HELP)?;
                print_help = false;
            }
            if let Some(ref line) = self.tale.next_command() {
                match line.as_str() {
                    "help" => {
                        print_help = true;
                    },
                    "character" => {
                        self.character()?;
                        print_help = true;
                    }
                    "begin" => {
                        return self.steady_state();
                    }
                    _ => {
                        writeln!(self.tale, "unknown command: {}", line.as_str())?;
                    },
                }
            } else {
                break 'bootstrapping;
            }
        }
        Ok(())
    }
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

impl<T: TextTale> Player<T> {
    fn character(&mut self) -> Result<()> {
        let mut print_help = true;
        'configuring:
        loop {
            if print_help {
                writeln!(self.tale, "{}", CHARACTER_HELP)?;
                print_help = false;
            }
            if let Some(ref line) = self.tale.next_command() {
                let cmd: Vec<&str> = line.split_whitespace().collect();
                if cmd.is_empty() {
                    continue 'configuring;
                }
                match cmd[0] {
                    "help" => {
                        print_help = true;
                    },
                    "name" => {
                        self.name = cmd[1..].to_vec().join(" ")
                    },
                    "gender" => {
                        self.gender = cmd[1..].to_vec().join(" ")
                    },
                    "race" => {
                        self.race = cmd[1..].to_vec().join(" ")
                    },
                    "age" => {
                        if cmd.len() != 2 {
                            writeln!(self.tale, "USAGE: age [age]")?;
                        } else {
                            self.age = match cmd[1].parse::<u8>() {
                                Ok(age) => age,
                                Err(err) => {
                                    writeln!(self.tale, "invalid age: {}", err)?;
                                    continue 'configuring;
                                },
                            };
                        }
                    },
                    "print" => {
                        let debug = format!("{:#?}", self);
                        writeln!(self.tale, "{}", debug)?;
                    },
                    "save" => {
                        break 'configuring;
                    },
                    _ => {
                        writeln!(self.tale, "unknown command: {}", line.as_str())?;
                    },
                }
            } else {
                break 'configuring;
            }
        }
        Ok(())
    }
}

/////////////////////////////////////////// steady_state ///////////////////////////////////////////

const STEADY_STATE_HELP: &str = "Welcome to adventure mode.

It's quite boring, but this simple quest is enough to test the texttale library.

help: ....... Print this help menu.
character: .. Configure your character for this texttale.
end: ........ Unceremoniously end this adventure.
";

impl<T: TextTale> Player<T> {
    fn steady_state(&mut self) -> Result<()> {
        let mut print_help = true;
        'adventuring:
        loop {
            if print_help {
                writeln!(self.tale, "{}", STEADY_STATE_HELP)?;
                print_help = false;
            }
            if let Some(ref line) = self.tale.next_command() {
                let cmd: Vec<&str> = line.split_whitespace().collect();
                if cmd.is_empty() {
                    continue 'adventuring;
                }
                match cmd[0] {
                    "help" => {
                        print_help = true;
                    },
                    "character" => {
                        self.character()?;
                        print_help = true;
                    }
                    "end" => {
                        break 'adventuring;
                    }
                    _ => {
                        writeln!(self.tale, "unknown command: {}", line.as_str())?;
                    },
                }
            } else {
                break 'adventuring;
            }
        }
        Ok(())
    }
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
        let tale = ShellTextTale::new(rl, "> ");
        let mut player = Player::new(tale);
        player.bootstrap()
    } else {
        for arg in args {
            let tale = ExpectTextTale::new(arg)?;
            let mut player = Player::new(tale);
            player.bootstrap()?;
        }
        Ok(())
    }
}
