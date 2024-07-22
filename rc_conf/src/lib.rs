use std::collections::{HashMap, HashSet};
use std::fs::read_to_string;
use std::process::Command;

use utf8path::Path;

/////////////////////////////////////////////// Error //////////////////////////////////////////////

#[derive(Debug)]
pub enum Error {
    FileTooLarge {
        path: Path<'static>,
    },
    TrailingWhack {
        path: Path<'static>,
    },
    ProhibitedCharacter {
        path: Path<'static>,
        line: u32,
        string: String,
        character: char,
    },
    InvalidRcConf {
        path: Path<'static>,
        line: u32,
        message: String,
    },
    InvalidRcScript {
        path: Path<'static>,
        line: u32,
        message: String,
    },
    InvalidInvocation {
        message: String,
    },
    IoError(std::io::Error),
    ShvarError(shvar::Error),
    Utf8Error(std::str::Utf8Error),
}

impl Error {
    pub fn file_too_large(file: &Path) -> Self {
        Self::FileTooLarge {
            path: file.clone().into_owned(),
        }
    }

    pub fn trailing_whack(file: &Path) -> Self {
        Self::TrailingWhack {
            path: file.clone().into_owned(),
        }
    }

    pub fn prohibited_character(
        file: &Path,
        line: u32,
        string: impl AsRef<str>,
        character: char,
    ) -> Self {
        Self::ProhibitedCharacter {
            path: file.clone().into_owned(),
            line,
            string: string.as_ref().to_string(),
            character,
        }
    }

    pub fn invalid_rc_conf(file: &Path, line: u32, message: impl AsRef<str>) -> Self {
        Self::InvalidRcConf {
            path: file.clone().into_owned(),
            line,
            message: message.as_ref().to_string(),
        }
    }

    pub fn invalid_rc_script(file: &Path, line: u32, message: impl AsRef<str>) -> Self {
        Self::InvalidRcScript {
            path: file.clone().into_owned(),
            line,
            message: message.as_ref().to_string(),
        }
    }

    pub fn invalid_invocation(message: impl AsRef<str>) -> Self {
        Self::InvalidInvocation {
            message: message.as_ref().to_string(),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::IoError(err)
    }
}

impl From<shvar::Error> for Error {
    fn from(err: shvar::Error) -> Self {
        Self::ShvarError(err)
    }
}

impl From<std::str::Utf8Error> for Error {
    fn from(err: std::str::Utf8Error) -> Self {
        Self::Utf8Error(err)
    }
}

///////////////////////////////////////////// RcScript /////////////////////////////////////////////

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RcScript {
    name: String,
    provide: String,
    version: String,
    command: String,
}

impl RcScript {
    pub fn new(
        name: impl Into<String>,
        provide: impl Into<String>,
        version: impl Into<String>,
        command: impl Into<String>,
    ) -> Self {
        let name = name.into();
        let provide = provide.into();
        let version = version.into();
        let command = command.into();
        Self {
            name,
            provide,
            version,
            command,
        }
    }

    pub fn parse(path: &Path, contents: &str) -> Result<Self, Error> {
        let name = name_from_path(path);
        let mut provide = None;
        let mut version = None;
        let mut command = None;
        for (number, line) in linearize(path, contents)? {
            if line.trim().starts_with('#') {
                continue;
            }
            if let Some((var, val)) = line.split_once('=') {
                match var {
                    "PROVIDE" if provide.is_none() => {
                        if val.contains('$') {
                            return Err(Error::invalid_rc_script(
                                path,
                                number,
                                "PROVIDE takes no variables",
                            ));
                        }
                        provide = Some(val.to_string());
                    }
                    "VERSION" if version.is_none() => {
                        if val.contains('$') {
                            return Err(Error::invalid_rc_script(
                                path,
                                number,
                                "VERSION takes no variables",
                            ));
                        }
                        version = Some(val.to_string());
                    }
                    "COMMAND" if command.is_none() => {
                        command = Some(val.to_string());
                    }
                    "PROVIDE" | "VERSION" | "COMMAND" => {
                        return Err(Error::invalid_rc_script(
                            path,
                            number,
                            format!("{} was repeated", var),
                        ));
                    }
                    _ => {
                        return Err(Error::invalid_rc_script(
                            path,
                            number,
                            "unsupported command",
                        ));
                    }
                }
            } else {
                return Err(Error::invalid_rc_script(
                    path,
                    number,
                    "missing an '=' sign",
                ));
            }
        }
        match (provide, version, command) {
            (Some(provide), Some(version), Some(command)) => {
                let rc = RcScript {
                    name,
                    provide,
                    version,
                    command,
                };
                rc.rcvar()?;
                Ok(rc)
            }
            (None, _, _) => Err(Error::invalid_rc_script(
                path,
                1,
                "missing a PROVIDE declaration",
            )),
            (_, None, _) => Err(Error::invalid_rc_script(
                path,
                1,
                "missing a VERSION declaration",
            )),
            (_, _, None) => Err(Error::invalid_rc_script(
                path,
                1,
                "missing a COMMAND declaration",
            )),
        }
    }

    pub fn provide(&self) -> &str {
        &self.provide
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn command(&self) -> &str {
        &self.command
    }

    pub fn rcvar(&self) -> Result<Vec<String>, Error> {
        Ok(shvar::rcvar(&self.command)?
            .into_iter()
            .map(|v| format!("{}_{}", self.name, v))
            .collect())
    }

    pub fn invoke(&self, args: &[impl AsRef<str>]) -> Result<(), Error> {
        if args.is_empty() {
            Err(Error::invalid_invocation("must provide arguments"))
        } else {
            let args = args.iter().map(|s| s.as_ref()).collect::<Vec<_>>();
            match args[0] {
                "run" => self.run(&args[1..]),
                "describe" => {
                    if args.len() != 1 {
                        eprintln!("arguments ignored");
                    }
                    println!("{self:#?}");
                    Ok(())
                }
                "rcvar" => {
                    if args.len() != 1 {
                        eprintln!("arguments ignored");
                    }
                    for rcvar in self.rcvar()?.into_iter() {
                        println!("{rcvar}");
                    }
                    Ok(())
                }
                _ => Err(Error::invalid_invocation(format!(
                    "unknown command {:?}",
                    args[0]
                ))),
            }
        }
    }

    fn run(&self, args: &[&str]) -> Result<(), Error> {
        let mut evp = EnvironmentVariableProvider::new(Some(self.name.clone() + "_"));
        let exp = shvar::expand(&mut evp, &self.command)?;
        let mut cmd = shvar::split(&exp)?;
        cmd.push("--".to_string());
        cmd.extend(args.iter().map(|s| s.to_string()));
        let mut child = Command::new(&cmd[0]).args(&cmd[1..]).spawn()?;
        let exit_status = child.wait()?;
        std::process::exit(exit_status.code().unwrap_or(0));
    }
}

//////////////////////////////////// EnvironmentVariableProvider ///////////////////////////////////

/// A shvar VariableProvider that pulls from the environment (optionally) with a given prefix.  If
/// the prefix exists, it will be preferred.  Note that it is necessary to check both foo_VAR and
/// VAR for prefix foo_ in order to have global values in an rc.conf.  Consider the case of setting
/// all logging options in one parameter that gets expanded to the universally-agreed-upon value.
pub struct EnvironmentVariableProvider {
    prefix: Option<String>,
}

impl EnvironmentVariableProvider {
    pub const fn new(prefix: Option<String>) -> Self {
        Self { prefix }
    }
}

impl shvar::VariableProvider for EnvironmentVariableProvider {
    fn lookup(&self, ident: &str) -> Option<String> {
        let key = if let Some(prefix) = self.prefix.as_ref() {
            prefix.to_string() + ident
        } else {
            ident.to_string()
        };
        std::env::var(key).ok()
    }
}

////////////////////////////////////////////// RcConf //////////////////////////////////////////////

/// An RC Configuration.
pub trait RcConf {
    fn variables(&self) -> Vec<String>;
    fn lookup(&self, k: &str) -> Option<String>;
}

//////////////////////////////////////////// RcConfFile ////////////////////////////////////////////

/// An RC configuration found within a single file.
pub struct RcConfFile {
    values: HashMap<String, String>,
}

impl RcConfFile {
    pub fn parse(path: &Path, contents: &str) -> Result<Self, Error> {
        let mut values = HashMap::new();
        for (number, line) in linearize(path, contents)? {
            if let Some((var, val)) = line.split_once('=') {
                values.insert(var.to_string(), val.to_string());
            } else {
                return Err(Error::invalid_rc_conf(path, number, line));
            }
        }
        Ok(Self { values })
    }
}

impl RcConf for RcConfFile {
    fn variables(&self) -> Vec<String> {
        self.values.keys().map(String::from).collect()
    }

    fn lookup(&self, k: &str) -> Option<String> {
        self.values.get(k).cloned()
    }
}

//////////////////////////////////////////// RcConfChain ///////////////////////////////////////////

/// A chain of RC configurations.
pub struct RcConfChain {
    chain: Vec<Box<dyn RcConf>>,
}

impl RcConfChain {
    // Construct a new RcConfChain that will lookup in chain[i + 1] before chain[i].
    pub const fn new(chain: Vec<Box<dyn RcConf>>) -> Self {
        Self { chain }
    }
}

impl RcConf for RcConfChain {
    fn variables(&self) -> Vec<String> {
        let mut values: HashSet<String> = HashSet::default();
        for rc_conf in self.chain.iter() {
            for variable in rc_conf.variables().into_iter() {
                values.insert(variable);
            }
        }
        let mut values: Vec<String> = values.into_iter().collect();
        values.sort();
        values
    }

    fn lookup(&self, k: &str) -> Option<String> {
        let mut rc_confs = self.chain.iter().collect::<Vec<_>>();
        rc_confs.reverse();
        for rc_conf in rc_confs {
            if let Some(v) = rc_conf.lookup(k) {
                return Some(v);
            }
        }
        None
    }
}

//////////////////////////////////////// parse_rc_conf_chain ///////////////////////////////////////

/// Parse the chain of rc conf files.  Later files will override earlier files.
// NOTE(rescrv):  Direction differs from the internals because there is no reverse iterator.
pub fn parse_rc_conf_chain(chain: String) -> Result<impl RcConf, Error> {
    let mut rc_confs = RcConfChain { chain: vec![] };
    for piece in chain.split(':') {
        let rc_contents = read_to_string(piece)?;
        let piece = Path::from(piece);
        let rc_conf = RcConfFile::parse(&piece, &rc_contents)?;
        rc_confs.chain.push(Box::new(rc_conf));
    }
    Ok(rc_confs)
}

///////////////////////////////////////////// utilities ////////////////////////////////////////////

/// Turn the contents of a file into numbered lines, while respecting line continuation markers.
pub fn linearize(path: &Path, contents: &str) -> Result<Vec<(u32, String)>, Error> {
    let mut start = 1;
    let mut acc = String::new();
    let mut lines = vec![];
    for (number, line) in contents.split_terminator('\n').enumerate() {
        if number as u64 >= u32::MAX as u64 {
            return Err(Error::file_too_large(path));
        }
        let has_whack = line.contains('\\');
        let end_whack = line.ends_with('\\');
        if has_whack && line.chars().filter(|c| *c == '\\').count() > 1 {
            return Err(Error::prohibited_character(
                path,
                number as u32 + 1,
                line,
                '\\',
            ));
        }
        if has_whack && !end_whack {
            return Err(Error::prohibited_character(
                path,
                number as u32 + 1,
                line,
                '\\',
            ));
        }
        if !acc.is_empty() {
            acc.push(' ');
        }
        if !end_whack {
            acc += line.trim();
            let line = std::mem::take(&mut acc);
            if !line.is_empty() {
                lines.push((start, line));
            }
            start = number as u32 + 1;
        } else {
            acc += line[..line.len() - 1].trim();
        }
    }
    if !acc.is_empty() {
        return Err(Error::trailing_whack(path));
    }
    Ok(lines)
}

/// Return the service name from the given path.
pub fn name_from_path(path: &Path) -> String {
    path.basename().as_str().to_string()
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    mod rc_script {
        use super::super::*;

        #[test]
        fn new() {
            RcScript::new("name", "provide", "version", "command");
        }

        #[test]
        fn from() {
            let rc_script = RcScript::parse(
                &Path::from("name"),
                r#"
PROVIDE=command
VERSION=c0ffee1eaff00d
COMMAND=my-command --option
"#,
            )
            .unwrap();
            assert_eq!(
                RcScript::new("name", "command", "c0ffee1eaff00d", "my-command --option"),
                rc_script
            );
        }

        #[test]
        fn quoted() {
            let rc_script = RcScript::parse(
                &Path::from("name"),
                r#"
PROVIDE=command
VERSION=c0ffee1eaff00d
COMMAND="my-command" "--option"
"#,
            )
            .unwrap();
            assert_eq!(
                RcScript::new(
                    "name",
                    "command",
                    "c0ffee1eaff00d",
                    "\"my-command\" \"--option\""
                ),
                rc_script
            );
        }

        #[test]
        fn with_newline() {
            let rc_script = RcScript::parse(
                &Path::from("name"),
                r#"
PROVIDE=command
VERSION=c0ffee1eaff00d
COMMAND=my-command \
    --option
"#,
            )
            .unwrap();
            assert_eq!(
                RcScript::new("name", "command", "c0ffee1eaff00d", "my-command --option"),
                rc_script
            );
        }

        #[test]
        fn rcvar() {
            let rc_script = RcScript::parse(
                &Path::from("name"),
                r#"
PROVIDE=command
VERSION=c0ffee1eaff00d
COMMAND=my-command \
    --option ${MY_VARIABLE}
"#,
            )
            .unwrap();
            assert_eq!(
                vec!["name_MY_VARIABLE".to_string()],
                rc_script.rcvar().unwrap()
            );
        }
    }
}
