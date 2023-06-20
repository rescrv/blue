use std::fs::read_to_string;
use std::io::Write;
use std::path::Path;

use rustyline::error::ReadlineError;
use rustyline::history::MemHistory;
use rustyline::Editor;

///////////////////////////////////////////// TextTale /////////////////////////////////////////////

pub trait TextTale: Write {
    fn next_command(&mut self) -> Option<String>;
}

/////////////////////////////////////////// ShellTextTale //////////////////////////////////////////

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
                return None;
            }
        }
    }
}

/////////////////////////////////////////// StoryElement ///////////////////////////////////////////

pub enum StoryElement {
    Continue,
    Return,
    PrintHelp,
}

//////////////////////////////////////////// story macro ///////////////////////////////////////////

#[macro_export]
macro_rules! story {
    ($sel:ident $cmd:ident, $story_title:ident by $story_teller:ty; $help:literal; $($command:literal => $code:tt)*) => {
        impl<T: TextTale> $story_teller {
            fn $story_title(&mut $sel) {
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
