use std::fmt::{Debug, Formatter};

use rustyline::history::MemHistory;
use rustyline::{Config, Editor, Result};

use texttale::{story, ExpectTextTale, ShellTextTale, StoryElement, TextTale};

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

story! {
    self cmd,
    bootstrap by Player<T>;
"Welcome to the texttale library.

help: ....... Print this help menu.
character: .. Configure your character for this texttale.
begin: ...... Start off on your journey with the given character.
";
    "help" => {
        StoryElement::PrintHelp
    }
    "character" => {
        self.character();
        StoryElement::PrintHelp
    }
    "begin" => {
        self.steady_state();
        StoryElement::Return
    }
}

///////////////////////////////////////////// character ////////////////////////////////////////////

story! {
    self cmd,
    character by Player<T>;
"Craft your character.

help: .... Print this help menu.
name: .... Set your character's name.
age: ..... Set your character's age.
gender: .. Set your character's gender.
race: .... Set your character's race.
print: ... Print your character.
save: .... Commit changes to the configuration and return to previous menu.
";
    "name" => {
        self.name = cmd[1..].to_vec().join(" ");
        StoryElement::Continue
    }
    "gender" => {
        self.gender = cmd[1..].to_vec().join(" ");
        StoryElement::Continue
    }
    "race" => {
        self.race = cmd[1..].to_vec().join(" ");
        StoryElement::Continue
    }
    "age" => {
        if cmd.len() != 2 {
            writeln!(self.tale, "USAGE: age [age]").unwrap();
        } else {
            match cmd[1].parse::<u8>() {
                Ok(age) => {
                    self.age = age;
                },
                Err(err) => {
                    writeln!(self.tale, "invalid age: {}", err).unwrap();
                },
            };
        }
        StoryElement::Continue
    }
    "print" => {
        let debug = format!("{:#?}", self);
        writeln!(self.tale, "{}", debug).unwrap();
        StoryElement::Continue
    }
    "save" => {
        StoryElement::Return
    }
}

/////////////////////////////////////////// steady_state ///////////////////////////////////////////

story! {
    self cmd,
    steady_state by Player<T>;
"Welcome to adventure mode.

It's quite boring, but this simple quest is enough to test the texttale library.

help: ....... Print this help menu.
character: .. Configure your character for this texttale.
end: ........ Unceremoniously end this adventure.
";
    "help" => {
        StoryElement::PrintHelp
    }
    "character" => {
        self.character();
        StoryElement::PrintHelp
    }
    "end" => {
        StoryElement::Return
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
        player.bootstrap();
        Ok(())
    } else {
        for arg in args {
            let tale = ExpectTextTale::new(arg, "> ")?;
            let mut player = Player::new(tale);
            player.bootstrap();
        }
        Ok(())
    }
}
