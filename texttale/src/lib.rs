use std::fs::read_to_string;
use std::io::Write;
use std::path::Path;

use rustyline::error::ReadlineError;
use rustyline::history::MemHistory;
use rustyline::Editor;

////////////////////////////////////////////// Harness /////////////////////////////////////////////

pub trait Harness: Write {
    fn next_command(&mut self) -> Option<String>;
}

/////////////////////////////////////////// ShellHarness ///////////////////////////////////////////

pub struct ShellHarness {
    rl: Editor<(), MemHistory>,
    prompt: &'static str,
}

impl ShellHarness {
    pub fn new(rl: Editor<(), MemHistory>, prompt: &'static str) -> Self {
        Self {
            rl,
            prompt,
        }
    }
}

impl Write for ShellHarness {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        std::io::stdout().write(buf)
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        std::io::stdout().flush()
    }
}

impl Harness for ShellHarness {
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

/////////////////////////////////////////// ExpectHarness //////////////////////////////////////////

#[derive(Default)]
pub struct ExpectHarness {
    input_lines: Vec<String>,
    output_buffer: Vec<u8>,
}

impl ExpectHarness {
    pub fn new<P: AsRef<Path>>(script: P) -> Result<Self, std::io::Error> {
        let script = read_to_string(script)?;
        let input_lines = script.lines().map(|s| s.to_owned()).collect();
        Ok(Self {
            input_lines,
            output_buffer: Vec::new(),
        })
    }
}

impl Write for ExpectHarness {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.output_buffer.write(buf)
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        self.output_buffer.flush()
    }
}

impl Harness for ExpectHarness {
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
