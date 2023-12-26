use std::fmt::{Debug, Formatter};

use rustyline::history::MemHistory;
use rustyline::{Config, Editor, Result};

use texttale::{menu, story, ExpectTextTale, Menu, ShellTextTale, StoryElement, TextTale};

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
            age: 18,
            gender: "unspecified".to_owned(),
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

story! {
    self cmd,
    bootstrap by Player<T>;
"Welcome to the texttale menu demo.

help: .. Print this help menu.
menu: .. Answer some questions interactively.
";
    "help" => {
        StoryElement::PrintHelp
    }
    "menu" => {
        menu! {
            self cmd;
            "name: " => {
                self.name = cmd;
                Menu::Continue
            }
            "age: " => {
                let age = cmd.parse::<u8>().unwrap();
                self.age = age;
                Menu::Continue
            }
            "gender: " => {
                self.gender = cmd;
                Menu::Continue
            }
            "race: " => {
                self.race = cmd;
                Menu::Continue
            }
        }
        let s = format!("{:#?}", self);
        write!(self.tale, "{}", s).unwrap();
        StoryElement::Continue
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
    if args.is_empty() || args[0] == "shell" {
        let tale = ShellTextTale::new(rl, "> ");
        let mut player = Player::new(tale);
        player.bootstrap();
        Ok(())
    } else if args[0] == "expect" {
        let tale = ExpectTextTale::new("examples/10.arc", "> ")?;
        let mut player = Player::new(tale);
        player.bootstrap();
        Ok(())
    } else {
        eprintln!("unknown command");
        Ok(())
    }
}
