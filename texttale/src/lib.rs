#![doc = include_str!("../README.md")]

use std::fs::read_to_string;
use std::io::Write;
use std::path::Path;

use rustyline::error::ReadlineError;
use rustyline::history::MemHistory;
use rustyline::Editor;

///////////////////////////////////////////// TextTale /////////////////////////////////////////////

/// A [TextTale] creates a text-mode adventure by feeding the next command from the command prompt
/// as a string.  Will return None when the user kills the session or the underlying writer is
/// exhausted.
pub trait TextTale: Write {
    fn next_command(&mut self) -> Option<String>;
}

/////////////////////////////////////////// ShellTextTale //////////////////////////////////////////

/// A [ShellTextTale] gives an interactive shell for testing.  It's intended to be interactive.
pub struct ShellTextTale {
    rl: Editor<(), MemHistory>,
    prompt: &'static str,
}

impl ShellTextTale {
    pub fn new(rl: Editor<(), MemHistory>, prompt: &'static str) -> Self {
        Self {
            rl,
            prompt,
        }
    }
}

impl Write for ShellTextTale {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        std::io::stdout().write(buf)
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        std::io::stdout().flush()
    }
}

impl TextTale for ShellTextTale {
    fn next_command(&mut self) -> Option<String> {
        let line = self.rl.readline(self.prompt);
        match line {
            Ok(line) => {
                Some(line.trim().to_owned())
            },
            Err(ReadlineError::Interrupted) => {
                std::process::exit(1);
            },
            Err(ReadlineError::Eof) => {
                None
            },
            Err(err) => {
                panic!("could not read line: {}", err);
            },
        }
    }
}

////////////////////////////////////////// ExpectTextTale //////////////////////////////////////////

/// An [ExpectTextTale] gives an adventure that gets recorded and compared against the input.  It's
/// intended to run the script and compare its output to the file.  See `CHECKSUMS.zsh` for an
/// example of the tests in the `scripts/` directory.
#[derive(Default)]
pub struct ExpectTextTale {
    input_lines: Vec<String>,
    output_buffer: Vec<u8>,
}

impl ExpectTextTale {
    pub fn new<P: AsRef<Path>>(script: P) -> Result<Self, std::io::Error> {
        let script = read_to_string(script)?;
        let input_lines = script.lines().map(|s| s.to_owned()).collect();
        Ok(Self {
            input_lines,
            output_buffer: Vec::new(),
        })
    }
}

impl Write for ExpectTextTale {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.output_buffer.write(buf)
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        self.output_buffer.flush()
    }
}

impl TextTale for ExpectTextTale {
    fn next_command(&mut self) -> Option<String> {
        let mut expected_output = String::new();
        loop {
            if !self.input_lines.is_empty() && self.input_lines[0].starts_with("> ") {
                let cmd = self.input_lines.remove(0);
                let exp = expected_output.trim();
                let got = String::from_utf8(self.output_buffer.clone()).unwrap();
                let got = got.trim();
                assert_eq!(exp, got);
                if !expected_output.is_empty() {
                    println!("{}", expected_output);
                }
                println!("{}", cmd);
                self.output_buffer.clear();
                return Some(cmd[2..].to_owned());
            } else if !self.input_lines.is_empty() {
                if !expected_output.is_empty() {
                    expected_output += "\n";
                }
                expected_output += &self.input_lines.remove(0);
            } else {
                if !expected_output.is_empty() {
                    panic!("expected output truncated: are you ending with a prompt?");
                }
                return None;
            }
        }
    }
}

/////////////////////////////////////////// StoryElement ///////////////////////////////////////////

/// A [StoryElement] dictates what to do next in the story.
pub enum StoryElement {
    Continue,
    Return,
    PrintHelp,
}

//////////////////////////////////////////// story macro ///////////////////////////////////////////

/// A [story] always takes the same form.  It is addressed to the method that it will generate for
/// the author.  It is always followed by the prompt that will be displayed when the user asks for
/// help.  Then, it's a sequence of commands that need to be interpreted.
///
/// ```
/// use texttale::{story, StoryElement, TextTale};
///
/// struct Player<T: TextTale> {
///     name: String,
///     age: u8,
///     gender: String,
///     race: String,
///     tale: T,
/// }
///
/// story! {
///     self cmd,
///     character by Player<T>;
/// "Craft your character.
///
/// help: .... Print this help menu.
/// name: .... Set your character's name.
/// age: ..... Set your character's age.
/// gender: .. Set your character's gender.
/// race: .... Set your character's race.
/// print: ... Print your character.
/// save: .... Commit changes to the configuration and return to previous menu.
/// ";
///     "name" => {
///         self.name = cmd[1..].to_vec().join(" ");
///         StoryElement::Continue
///     }
///     "gender" => {
///         self.gender = cmd[1..].to_vec().join(" ");
///         StoryElement::Continue
///     }
///     "race" => {
///         self.race = cmd[1..].to_vec().join(" ");
///         StoryElement::Continue
///     }
///     "age" => {
///         if cmd.len() != 2 {
///             writeln!(self.tale, "USAGE: age [age]").unwrap();
///         } else {
///             match cmd[1].parse::<u8>() {
///                 Ok(age) => {
///                     self.age = age;
///                 },
///                 Err(err) => {
///                     writeln!(self.tale, "invalid age: {}", err).unwrap();
///                 },
///             };
///         }
///         StoryElement::Continue
///     }
///     "print" => {
///         let debug = format!("{:#?}", self);
///         writeln!(self.tale, "{}", debug).unwrap();
///         StoryElement::Continue
///     }
///     "save" => {
///         StoryElement::Return
///     }
/// }
///
/// impl<T: TextTale> std::fmt::Debug for Player<T> {
///     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
///         f.debug_struct("Player")
///             .field("name", &self.name)
///             .field("age", &self.age)
///             .field("gender", &self.gender)
///             .field("race", &self.race)
///             .finish()
///     }
/// }
/// ```
#[macro_export]
macro_rules! story {
    ($sel:ident $cmd:ident, $story_title:ident by $story_teller:ty; $help:literal; $($command:literal => $code:tt)*) => {
        impl<T: TextTale> $story_teller {
            pub fn $story_title(&mut $sel) {
                let mut print_help = true;
                'adventuring:
                loop {
                    if print_help {
                        writeln!($sel.tale, "{}", $help).expect("print help");
                        print_help = false;
                    }
                    if let Some(ref line) = $sel.tale.next_command() {
                        let $cmd: Vec<&str> = line.split_whitespace().collect();
                        if $cmd.is_empty() {
                            continue 'adventuring;
                        }
                        let element: $crate::StoryElement = match $cmd[0] {
                            $($command => $code),*
                            _ => {
                                writeln!($sel.tale, "unknown command: {}", line.as_str()).expect("unknown command");
                                continue 'adventuring;
                            },
                        };
                        match element {
                            StoryElement::Continue => {
                                continue 'adventuring;
                            },
                            StoryElement::Return => {
                                break 'adventuring;
                            }
                            StoryElement::PrintHelp => {
                                print_help = true;
                                continue 'adventuring;
                            }
                        }
                    } else {
                        break 'adventuring;
                    }
                }
            }
        }
    };
}
