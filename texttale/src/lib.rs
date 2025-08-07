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
    /// Handle an unexpected EOF.
    fn unexpected_eof(&mut self);
    /// Get the current prompt.
    fn get_prompt(&mut self) -> &'static str;
    /// Set the current prompt.
    fn set_prompt(&mut self, prompt: &'static str);
    /// Return the next command, according to the texttale's rules.
    fn next_command(&mut self) -> Option<String>;
}

/////////////////////////////////////////// ShellTextTale //////////////////////////////////////////

/// A [ShellTextTale] gives an interactive shell for testing.  It's intended to be interactive.
pub struct ShellTextTale {
    rl: Editor<(), MemHistory>,
    prompt: &'static str,
}

impl ShellTextTale {
    /// Create a new texttale shell, using the provided readline editor and prompt.
    pub fn new(rl: Editor<(), MemHistory>, prompt: &'static str) -> Self {
        Self { rl, prompt }
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
    fn unexpected_eof(&mut self) {
        std::process::exit(1);
    }

    fn get_prompt(&mut self) -> &'static str {
        self.prompt
    }

    fn set_prompt(&mut self, prompt: &'static str) {
        self.prompt = prompt;
    }

    fn next_command(&mut self) -> Option<String> {
        let line = self.rl.readline(self.prompt);
        match line {
            Ok(line) => Some(line.trim().to_owned()),
            Err(ReadlineError::Interrupted) => {
                std::process::exit(1);
            }
            Err(ReadlineError::Eof) => None,
            Err(err) => {
                panic!("could not read line: {err}");
            }
        }
    }
}

////////////////////////////////////////// ExpectTextTale //////////////////////////////////////////

/// An [ExpectTextTale] gives an adventure that gets recorded and compared against the input.  It's
/// intended to run the script and compare its output to the file.  See `CHECKSUMS.zsh` for an
/// example of the tests in the `scripts/` directory.
#[derive(Default)]
pub struct ExpectTextTale {
    prompt: &'static str,
    input_lines: Vec<String>,
    output_buffer: Vec<u8>,
}

impl ExpectTextTale {
    /// Create a new expect text tale that reads from script and compares the output of the
    /// texttale against the expected output in the script.
    pub fn new<P: AsRef<Path>>(script: P, prompt: &'static str) -> Result<Self, std::io::Error> {
        let script = read_to_string(script)?;
        let input_lines = script.lines().map(|s| s.to_string()).collect();
        Ok(Self {
            prompt,
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

fn diff(exp: &str, got: &str) {
    if exp == got {
        return;
    }
    let exp: Vec<String> = exp.trim_end().split('\n').map(String::from).collect();
    let got: Vec<String> = got.trim_end().split('\n').map(String::from).collect();
    let mut arr = vec![vec![0; got.len() + 1]; exp.len() + 1];
    for i in 0..exp.len() {
        #[allow(clippy::needless_range_loop)]
        for j in 0..got.len() {
            if exp[i] == got[j] {
                arr[i + 1][j + 1] = arr[i][j] + 1;
            } else {
                arr[i + 1][j + 1] = std::cmp::max(arr[i][j + 1], arr[i + 1][j]);
            }
        }
    }
    let mut e = exp.len();
    let mut g = got.len();
    let mut diff = vec![];
    while e > 0 && g > 0 {
        if exp[e - 1] == got[g - 1] {
            diff.push(" ".to_string() + &exp[e - 1]);
            e -= 1;
            g -= 1;
        } else if arr[e][g] == arr[e][g - 1] {
            diff.push("+".to_string() + &got[g - 1]);
            g -= 1;
        } else {
            diff.push("-".to_string() + &exp[e - 1]);
            e -= 1;
        }
    }
    while g > 0 {
        diff.push(format!("+{}", got[g - 1]));
        g -= 1;
    }
    while e > 0 {
        diff.push(format!("-{}", exp[e - 1]));
        e -= 1;
    }
    diff.reverse();
    panic!(
        "texttale doesn't meet expectations\n-expected +returned:\n{}",
        diff.join("\n")
    );
}

impl TextTale for ExpectTextTale {
    fn unexpected_eof(&mut self) {
        panic!("unexpected end of file");
    }

    fn get_prompt(&mut self) -> &'static str {
        self.prompt
    }

    fn set_prompt(&mut self, prompt: &'static str) {
        self.prompt = prompt;
    }

    fn next_command(&mut self) -> Option<String> {
        let mut expected_output = String::new();
        loop {
            if !self.input_lines.is_empty() && self.input_lines[0].starts_with(self.prompt) {
                let cmd = self.input_lines.remove(0);
                let exp = expected_output.trim();
                let got = String::from_utf8(self.output_buffer.clone()).unwrap();
                let got = got.trim();
                diff(exp, got);
                if !expected_output.is_empty() {
                    println!("{expected_output}");
                }
                println!("{cmd}");
                self.output_buffer.clear();
                return Some(cmd[self.prompt.len()..].to_owned());
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
    /// Continue with the story.
    Continue,
    /// Return from the current function defined by the story macro.
    Return,
    /// Print the provided help string before the next prompt.
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
    ($this:ident $cmd:ident, $story_title:ident by $story_teller:ty; $help:literal; $($command:literal => $code:tt)*) => {
        impl<T: TextTale> $story_teller {
            pub fn $story_title(&mut $this) {
                let mut print_help = true;
                'adventuring:
                loop {
                    if print_help {
                        writeln!($this.tale, "{}", $help).expect("print help");
                        print_help = false;
                    }
                    if let Some(ref line) = $this.tale.next_command() {
                        let $cmd: Vec<&str> = line.split_whitespace().collect();
                        if $cmd.is_empty() {
                            continue 'adventuring;
                        }
                        let element: $crate::StoryElement = match $cmd[0] {
                            $($command => $code),*
                            _ => {
                                writeln!($this.tale, "unknown command: {}", line.as_str()).expect("unknown command");
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

/////////////////////////////////////////////// Menu ///////////////////////////////////////////////

/// A [Menu] dictates what to do next within a menu.
pub enum Menu {
    /// Continue to the next prompt in the menu.
    Continue,
    /// Retry the current prompt; usually this is used for data that doesn't validate.
    Retry,
    /// Announce an unexpected end-of-file.
    UnexpectedEof,
}

//////////////////////////////////////////// menu macro ////////////////////////////////////////////

/// An [menu] is a series of interactive prompts to be answered in order.  Where a story provides
/// choice via branches, an menu sequences  prompts in-order and expects an answer to each prompt.
///
/// ```
/// use texttale::{menu, story, Menu, StoryElement, TextTale};
///
/// #[derive(Debug)]
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
/// help: ....... Print this help menu.
/// interview: .. Answer questions to fill in your character's details.
/// ";
///     "name" => {
///         menu! {
///             self cmd;
///             "name" => {
///                 /* code */
///                 Menu::Continue
///             }
///             "age" => {
///                 /* more code */
///                 Menu::Continue
///             }
///         }
///         StoryElement::Continue
///     }
/// }
/// ```
#[macro_export]
macro_rules! menu {
    ($this:ident $cmd:ident; $($prompt:literal => $code:tt)*) => {
        {
            let prompt = $this.tale.get_prompt();
            $(
                'retrying: loop {
                    $this.tale.set_prompt($prompt);
                    let $cmd = $this.tale.next_command();
                    let action = if let Some($cmd) = $cmd {
                        $code
                    } else {
                        $crate::Menu::UnexpectedEof
                    };
                    match action {
                        $crate::Menu::Continue => {
                            break 'retrying;
                        }
                        $crate::Menu::Retry => {}
                        $crate::Menu::UnexpectedEof => {
                            $this.tale.unexpected_eof();
                        },
                    }
                }
            )*
            $this.tale.set_prompt(prompt);
        }
    };
}
