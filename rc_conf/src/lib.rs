#![doc = include_str!("../README.md")]
#![deny(missing_docs)]

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::time::SystemTime;

use biometrics::Counter;
use shvar::{PrefixingVariableProvider, VariableProvider};
use utf8path::Path;

///////////////////////////////////////////// constants ////////////////////////////////////////////

const RESTRICTED_VARIABLES: &[&str] = &["NAME"];

///////////////////////////////////////////// counters /////////////////////////////////////////////

static MISSING_ENABLED_VAR: Counter = Counter::new("rc_conf.service_switch.missing_enabled_var");
static SPLIT_FAILURE: Counter = Counter::new("rc_conf.service_switch.split_failure");
static INVALID_SWITCH_VALUE: Counter = Counter::new("rc_conf.service_switch.invalid_switch_value");
static NO_SERVICES_FOUND: Counter = Counter::new("rc_conf.service_switch.no_services_found");

/// Register all rc_conf counters with the provided collector.
pub fn register_counters(collector: &biometrics::Collector) {
    collector.register_counter(&MISSING_ENABLED_VAR);
    collector.register_counter(&SPLIT_FAILURE);
    collector.register_counter(&INVALID_SWITCH_VALUE);
    collector.register_counter(&NO_SERVICES_FOUND);
}

/////////////////////////////////////////////// Error //////////////////////////////////////////////

/// The Error type.
#[derive(Debug)]
pub enum Error {
    /// The file specified by `path` is too large to parse.
    FileTooLarge {
        /// The path that's too large to parse.
        path: Path<'static>,
    },
    /// The file specified by `path` ends with a r"\" or r"\\n".
    TrailingWhack {
        /// The path with a trailing \.
        path: Path<'static>,
    },
    /// The file specified by `path` contails the prohibited `character` in `string` on `line`.
    ProhibitedCharacter {
        /// The path with a prohibited character.
        path: Path<'static>,
        /// The line with a prohibited character.
        line: u32,
        /// The string with a prohibited characater.
        string: String,
        /// The prohibited character.
        character: char,
    },
    /// The file specified by `path` is invalid in the way specified by `message` on `line`.
    InvalidRcConf {
        /// The invalid rc_conf file.
        path: Path<'static>,
        /// The line that's invalid.
        line: u32,
        /// The reason for it being invalid.
        message: String,
    },
    /// An error for an invalid rc script.
    InvalidRcScript {
        /// The invalid rc.d service stub.
        path: Path<'static>,
        /// The line that's invalid.
        line: u32,
        /// The reason for it being invalid.
        message: String,
    },
    /// The invocation failed.
    InvalidInvocation {
        /// The reason the invocation failed.
        message: String,
    },
    /// Command execution failed.
    ExecFailed {
        /// The command that failed to execute.
        command: String,
        /// The underlying error from exec.
        error: std::io::Error,
    },
    /// An error from the standard library.
    IoError(std::io::Error),
    /// An error parsing variables or splitting strings.
    ShvarError(shvar::Error),
    /// An error relating to utf8.
    Utf8Error(std::str::Utf8Error),
    /// An error relating to utf8.
    FromUtf8Error(std::string::FromUtf8Error),
}

impl Error {
    /// Construct a new "FileTooLarge" variant.
    pub fn file_too_large(file: &Path) -> Self {
        Self::FileTooLarge {
            path: file.clone().into_owned(),
        }
    }

    /// Construct a new "TrailingWhack" variant.
    pub fn trailing_whack(file: &Path) -> Self {
        Self::TrailingWhack {
            path: file.clone().into_owned(),
        }
    }

    /// Construct a new "ProhibitedCharacter" variant.
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

    /// Construct a new "InvalidRcConf" variant.
    pub fn invalid_rc_conf(file: &Path, line: u32, message: impl AsRef<str>) -> Self {
        Self::InvalidRcConf {
            path: file.clone().into_owned(),
            line,
            message: message.as_ref().to_string(),
        }
    }

    /// Construct a new "InvalidRcScript" variant.
    pub fn invalid_rc_script(file: &Path, line: u32, message: impl AsRef<str>) -> Self {
        Self::InvalidRcScript {
            path: file.clone().into_owned(),
            line,
            message: message.as_ref().to_string(),
        }
    }

    /// Construct a new "InvalidInvocation" variant.
    pub fn invalid_invocation(message: impl AsRef<str>) -> Self {
        Self::InvalidInvocation {
            message: message.as_ref().to_string(),
        }
    }

    /// Construct a new "ExecFailed" variant.
    pub fn exec_failed(command: impl AsRef<str>, error: std::io::Error) -> Self {
        Self::ExecFailed {
            command: command.as_ref().to_string(),
            error,
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

impl From<std::string::FromUtf8Error> for Error {
    fn from(err: std::string::FromUtf8Error) -> Self {
        Self::FromUtf8Error(err)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::FileTooLarge { path } => write!(f, "File too large to parse: {}", path.as_str()),
            Error::TrailingWhack { path } => {
                write!(f, "File ends with trailing backslash: {}", path.as_str())
            }
            Error::ProhibitedCharacter {
                path,
                line,
                string,
                character,
            } => {
                write!(
                    f,
                    "Prohibited character '{}' in file {} at line {}: {}",
                    character,
                    path.as_str(),
                    line,
                    string
                )
            }
            Error::InvalidRcConf {
                path,
                line,
                message,
            } => {
                write!(
                    f,
                    "Invalid rc.conf in file {} at line {}: {}",
                    path.as_str(),
                    line,
                    message
                )
            }
            Error::InvalidRcScript {
                path,
                line,
                message,
            } => {
                write!(
                    f,
                    "Invalid rc.d script in file {} at line {}: {}",
                    path.as_str(),
                    line,
                    message
                )
            }
            Error::InvalidInvocation { message } => write!(f, "Invalid invocation: {}", message),
            Error::ExecFailed { command, error } => {
                write!(f, "Command execution failed for '{}': {}", command, error)
            }
            Error::IoError(err) => write!(f, "IO error: {}", err),
            Error::ShvarError(err) => write!(f, "Shell variable error: {}", err),
            Error::Utf8Error(err) => write!(f, "UTF-8 error: {}", err),
            Error::FromUtf8Error(err) => write!(f, "UTF-8 conversion error: {}", err),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::IoError(err) => Some(err),
            Error::ShvarError(err) => Some(err),
            Error::Utf8Error(err) => Some(err),
            Error::FromUtf8Error(err) => Some(err),
            _ => None,
        }
    }
}

////////////////////////////////////////// SwitchPosition //////////////////////////////////////////

/// An enum representing the valid values for _ENABLED variables.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SwitchPosition {
    /// The service is disabled.  It cannot be run.  Even manually.
    No,
    /// The service is enabled.  It should be starated automatically.
    Yes,
    /// The service is provisionally enabled.  It will not be run automatically, but can be started
    /// manually or programmatically (e.g. by a cron-like daemon).
    Manual,
}

impl SwitchPosition {
    /// Parse the literal strings "YES", "NO", or "MANUAL" (no lowercasing) into a valid
    /// SwitchPosition enum.
    pub fn from_enable<S: AsRef<str>>(s: S) -> Option<Self> {
        let s = s.as_ref();
        match s {
            "YES" => Some(SwitchPosition::Yes),
            "NO" => Some(SwitchPosition::No),
            "MANUAL" => Some(SwitchPosition::Manual),
            _ => None,
        }
    }

    /// True if the service can run.
    pub fn can_be_started(self) -> bool {
        match self {
            Self::Yes => true,
            Self::Manual => true,
            Self::No => false,
        }
    }
}

///////////////////////////////////////////// RcScript /////////////////////////////////////////////

/// An RcScript implements the rc.d service interface in a declarative way.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RcScript {
    /// The name of the rcscript.
    name: String,
    describe: String,
    command: String,
}

impl RcScript {
    /// Create a new RcScript using the provided name, description, and command.
    ///
    /// # Arguments
    /// * `name` - The service name
    /// * `describe` - Human-readable description of the service
    /// * `command` - Shell command to execute for this service
    ///
    /// # Examples
    /// ```
    /// # use rc_conf::RcScript;
    /// let script = RcScript::new("myservice", "A sample service", "echo hello");
    /// assert_eq!(script.name(), "myservice");
    /// ```
    pub fn new(
        name: impl Into<String>,
        describe: impl Into<String>,
        command: impl Into<String>,
    ) -> Self {
        let name = name.into();
        let describe = describe.into();
        let command = command.into();
        Self {
            name,
            describe,
            command,
        }
    }

    /// Parse the file at path assuming its contents are contents.  It will not re-read path.
    ///
    /// # Arguments
    /// * `path` - Path to the file being parsed (for error reporting)
    /// * `contents` - File contents to parse
    ///
    /// # Returns
    /// Parsed RcScript or an error describing what went wrong
    pub fn parse(path: &Path, contents: &str) -> Result<Self, Error> {
        let name = if let Ok(path) = std::env::var("RCVAR_ARGV0") {
            path.to_string()
        } else {
            name_from_path(path)
        };
        let mut describe = None;
        let mut command = None;
        for (number, line, _) in linearize(path, contents)? {
            if line.trim().starts_with('#') || line.trim().is_empty() {
                continue;
            }
            if let Some((var, val)) = line.split_once('=') {
                match var {
                    "DESCRIBE" if describe.is_none() => {
                        if val.contains('$') {
                            return Err(Error::invalid_rc_script(
                                path,
                                number,
                                "DESCRIBE takes no variables",
                            ));
                        }
                        describe = Some(val.to_string());
                    }
                    "COMMAND" if command.is_none() => {
                        command = Some(val.to_string());
                    }
                    "DESCRIBE" | "COMMAND" => {
                        return Err(Error::invalid_rc_script(
                            path,
                            number,
                            format!("{var} was repeated"),
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
        match (describe, command) {
            (Some(describe), Some(command)) => {
                let rc = RcScript {
                    name,
                    describe,
                    command,
                };
                rc.rcvar()?;
                Ok(rc)
            }
            (None, _) => Err(Error::invalid_rc_script(
                path,
                1,
                "missing a DESCRIBE declaration",
            )),
            (_, None) => Err(Error::invalid_rc_script(
                path,
                1,
                "missing a COMMAND declaration",
            )),
        }
    }

    /// The name of the rcscript.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Set the name of the rcscript.
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    /// The description of the command provided in the RcScript.
    pub fn describe(&self) -> &str {
        &self.describe
    }

    /// The command to be run, interpreted as a shell-quoted string suitable for splitting.
    pub fn command(&self) -> &str {
        &self.command
    }

    /// Return the set of rc_conf variables to be set for this service stub.
    pub fn rcvar(&self) -> Result<Vec<String>, Error> {
        let name = var_prefix_from_service(&self.name);
        Ok(shvar::rcvar(&self.command)?
            .into_iter()
            .map(|v| format!("{name}{v}"))
            .collect())
    }

    /// Invoke the RcScript, providing args to the invocation.  If args is non-empty, it will be
    /// appened with an additional '--' to separate it from the args interpreted from the RcScript
    /// command field.
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
                        if RESTRICTED_VARIABLES.iter().any(|v| *v == rcvar) {
                            continue;
                        }
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
        let name = var_prefix_from_service(&self.name);
        let evp = EnvironmentVariableProvider::new(Some(name));
        let meta = HashMap::from([("NAME".to_string(), self.name.to_string())]);

        for arg in args {
            if arg.contains('\0') {
                return Err(Error::invalid_invocation(
                    "arguments cannot contain null bytes",
                ));
            }
        }

        let exp = shvar::expand_recursive(&(&meta, &evp), &self.command)?;
        let mut cmd = shvar::split(&exp)?;
        if !args.is_empty() {
            cmd.push("--".to_string());
        }
        cmd.extend(args.iter().map(|s| s.to_string()));

        let status = Command::new(&cmd[0])
            .args(&cmd[1..])
            .status()
            .map_err(|err| Error::exec_failed(&cmd[0], err))?;

        if !status.success() {
            return Err(Error::invalid_invocation(format!(
                "command {} failed with exit code {:?}",
                &cmd[0],
                status.code()
            )));
        }

        Ok(())
    }
}

//////////////////////////////////// EnvironmentVariableProvider ///////////////////////////////////

/// A shvar VariableProvider that pulls from the environment (optionally) with a given prefix.  If
/// the prefix exists, it will be preferred.  Note that it is necessary to check both foo_VAR and
/// VAR for prefix foo_ in order to have global values in an rc.conf.  Consider the case of setting
/// all logging options in one parameter that gets expanded to the universally-agreed-upon value.
#[derive(Debug)]
pub struct EnvironmentVariableProvider {
    prefix: Option<String>,
}

impl EnvironmentVariableProvider {
    /// Create a new environmental variable provider that looks values up in the environment,
    /// optionally under some prefix.
    pub const fn new(prefix: Option<String>) -> Self {
        Self { prefix }
    }
}

/// Check if a string is a valid environment variable name.
/// Must start with a letter or underscore, and contain only letters, digits, and underscores.
fn is_valid_env_var_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Check if a source path is safe to include (prevent directory traversal attacks).
fn is_safe_source_path(path: &str) -> bool {
    // Reject empty paths or paths containing null bytes
    if path.is_empty() || path.contains('\0') {
        return false;
    }

    // Reject paths that attempt directory traversal
    if path.contains("..") || path.starts_with('/') {
        return false;
    }

    // Additional safety: reject paths with suspicious characters
    if path.contains('\\') || path.contains('\n') || path.contains('\r') {
        return false;
    }

    true
}

impl shvar::VariableProvider for EnvironmentVariableProvider {
    fn lookup(&self, ident: &str) -> Option<String> {
        // Sanitize environment variable name: must be valid identifier
        if !is_valid_env_var_name(ident) {
            return None;
        }

        let key = if let Some(prefix) = self.prefix.as_ref() {
            prefix.to_string() + ident
        } else {
            ident.to_string()
        };

        // Get environment variable value and sanitize it
        std::env::var(key).ok().and_then(|value| {
            if value.contains('\0') {
                None // Reject values containing null bytes
            } else {
                Some(value)
            }
        })
    }
}

/////////////////////////////////////////////// Alias //////////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Alias {
    // The physical service stub this service aliases.
    aliases: String,
    // True if this alias inherits from what it aliases in rc.conf.
    inherit: bool,
    // Values to inject into the bound values map.
    vp: HashMap<String, String>,
}

////////////////////////////////////////////// RcConf //////////////////////////////////////////////

/// An RcConf is a parsed RcFile.  All IO happens in parse, so behavior should be deterministc
/// after parsing.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RcConf {
    items: HashMap<String, String>,
    aliases: HashMap<String, Alias>,
    autogens: HashSet<String>,
    values: HashMap<String, RcConf>,
    filters: HashMap<String, RcConf>,
}

impl RcConf {
    /// Parse `path` to get a new RcConf.
    pub fn parse(path: &str) -> Result<Self, Error> {
        let mut seen = HashSet::default();
        let mut items = HashMap::default();
        for piece in path.split(':') {
            let piece = Path::from(piece);
            if !piece.exists() {
                continue;
            }
            Self::parse_recursive(&piece, &mut seen, &mut items)?;
        }
        let mut aliases = HashMap::default();
        let mut autogens = HashSet::default();
        for (varname, alias) in items.iter() {
            let Some(name) = varname.strip_suffix("_ALIASES") else {
                continue;
            };
            let inherit = if let Some(flag) = items.lookup(&(name.to_string() + "_INHERIT")) {
                if flag == "NO" {
                    false
                } else if flag == "YES" {
                    true
                } else {
                    return Err(Error::invalid_rc_conf(
                        &Path::from(path),
                        0,
                        format!("invalid _INHERIT binding for {name}"),
                    ));
                }
            } else {
                false
            };
            if items.lookup(&(name.to_string() + "_AUTOGEN")).is_some() {
                autogens.insert(name.to_string());
            }
            aliases.insert(
                name.to_string(),
                Alias {
                    aliases: alias.clone(),
                    inherit,
                    vp: HashMap::new(),
                },
            );
        }
        let mut values = HashMap::new();
        for (varname, values_conf) in items.iter() {
            let Some(name) = varname.strip_prefix("VALUES_") else {
                continue;
            };
            let mut values_items = HashMap::default();
            Self::parse_error_on_source(&Path::from(values_conf.clone()), &mut values_items)?;
            values.insert(
                name.to_string(),
                RcConf {
                    items: values_items,
                    aliases: HashMap::default(),
                    autogens: HashSet::default(),
                    values: HashMap::default(),
                    filters: HashMap::default(),
                },
            );
        }
        let mut filters = HashMap::new();
        for (varname, filters_conf) in items.iter() {
            let Some(name) = varname.strip_prefix("FILTER_") else {
                continue;
            };
            let mut filters_items = HashMap::default();
            Self::parse_error_on_source(&Path::from(filters_conf.clone()), &mut filters_items)?;
            let filters_conf = Path::from(filters_conf.as_str());
            let (sep, vars) = split_for_filter(name);
            let name = vars.join(&sep);
            for varname in filters_items.keys() {
                let pieces = varname.split(&sep).collect::<Vec<_>>();
                if pieces.len() != vars.len() {
                    return Err(Error::invalid_rc_conf(
                        &filters_conf,
                        0,
                        format!("{pieces:?} doesn't match format {name:?}"),
                    ));
                }
                for (value, binding) in std::iter::zip(vars.iter(), pieces.iter()) {
                    let Some(values) = values.get(value) else {
                        return Err(Error::invalid_rc_conf(
                            &filters_conf,
                            0,
                            format!("VALUES_{value} not declared"),
                        ));
                    };
                    if values.lookup(binding).is_none() {
                        return Err(Error::invalid_rc_conf(
                            &filters_conf,
                            0,
                            format!("{binding} not declared as {value}"),
                        ));
                    }
                }
            }
            filters.insert(
                name.to_string(),
                RcConf {
                    items: filters_items,
                    aliases: HashMap::default(),
                    autogens: HashSet::default(),
                    values: HashMap::default(),
                    filters: HashMap::default(),
                },
            );
        }
        for (varname, autogen_switch) in items.iter() {
            let Some(alias) = varname.strip_suffix("_AUTOGEN") else {
                continue;
            };
            if autogen_switch == "NO" {
                continue;
            } else if autogen_switch != "YES" {
                return Err(Error::invalid_rc_conf(
                    &Path::from(path),
                    0,
                    format!("{varname} must be set to YES or NO"),
                ));
            }
            let (template, variables, filter) = strip_prefix_values(&values, alias);
            if variables.is_empty() {
                return Err(Error::invalid_rc_conf(
                    &Path::from(path),
                    0,
                    "autogen requires one or more VALUES_-declared variables",
                ));
            }
            let filter_rc_conf = filters.get(&variables.join("_"));
            let bindings = variables
                .iter()
                .filter_map(|v| values.get(v).map(|rc| rc.variables()))
                .collect::<Vec<_>>();
            if variables.len() != bindings.len() {
                return Err(Error::invalid_rc_conf(
                    &Path::from(path),
                    0,
                    "inconsistent autogen statement (you'll have to pull code to debug this one)",
                ));
            }
            let mut cursors = vec![0; bindings.len()];
            while cursors[0] < bindings[0].len() {
                let candidate = bindings
                    .iter()
                    .enumerate()
                    .map(|(idx, set)| &set[cursors[idx]])
                    .collect::<Vec<_>>();
                let vp = std::iter::zip(variables.iter(), candidate)
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect::<HashMap<_, _>>();
                let candidate = shvar::expand_recursive(&vp, &template)?;
                let filter_key = shvar::expand_recursive(&vp, &filter)?;
                if aliases.contains_key(&candidate) {
                    return Err(Error::invalid_rc_conf(
                        &Path::from(path),
                        0,
                        format!("{candidate} comes from both autogen and alias"),
                    ));
                }
                let insert = if let Some(filter_rc_conf) = filter_rc_conf.as_ref() {
                    filter_rc_conf.lookup(&filter_key).is_some()
                } else {
                    true
                };
                if insert {
                    aliases.insert(
                        candidate,
                        Alias {
                            aliases: alias.to_string(),
                            inherit: true,
                            vp,
                        },
                    );
                }
                for idx in (0..bindings.len()).rev() {
                    cursors[idx] = cursors[idx].saturating_add(1);
                    if idx > 0 && cursors[idx] >= bindings[idx].len() {
                        cursors[idx] = 0;
                    } else {
                        break;
                    }
                }
            }
        }
        Ok(Self {
            items,
            aliases,
            autogens,
            values,
            filters,
        })
    }

    fn parse_recursive(
        path: &Path,
        seen: &mut HashSet<Path>,
        items: &mut HashMap<String, String>,
    ) -> Result<(), Error> {
        if seen.contains(path) {
            return Ok(());
        }
        seen.insert(path.clone().into_owned());
        let contents = std::fs::read_to_string(path.as_str())?;
        for (number, line, _) in linearize(path, &contents)? {
            if line.trim().starts_with('#') || line.trim().is_empty() {
                continue;
            }
            if let Some(source) = line.trim().strip_prefix("source ") {
                if is_safe_source_path(source) {
                    Self::parse_recursive(&Path::from(source), seen, items)?;
                } else {
                    return Err(Error::invalid_rc_conf(path, number, "unsafe source path"));
                }
            } else if let Some((var, val)) = line.split_once('=') {
                let split = shvar::split(val)?;
                if split.is_empty() {
                    items.insert(var.to_string(), String::new());
                } else if split.len() == 1 {
                    items.insert(var.to_string(), split[0].clone());
                } else {
                    return Err(Error::invalid_rc_conf(path, number, line));
                }
            } else {
                return Err(Error::invalid_rc_conf(path, number, line));
            }
        }
        Ok(())
    }

    fn parse_error_on_source(
        path: &Path,
        items: &mut HashMap<String, String>,
    ) -> Result<(), Error> {
        let contents = std::fs::read_to_string(path.as_str())?;
        for (number, line, _) in linearize(path, &contents)? {
            if line.trim().starts_with('#') || line.trim().is_empty() {
                continue;
            }
            if let Some((var, val)) = line.split_once('=') {
                let split = shvar::split(val)?;
                if split.is_empty() {
                    items.insert(var.to_string(), String::new());
                } else if split.len() == 1 {
                    items.insert(var.to_string(), split[0].clone());
                } else {
                    return Err(Error::invalid_rc_conf(path, number, line));
                }
            } else {
                return Err(Error::invalid_rc_conf(path, number, line));
            }
        }
        Ok(())
    }

    /// Examine the rc_conf and output the rc_conf as a string, showing how the parser sees it.
    pub fn examine(path: &str) -> Result<String, Error> {
        let mut seen = HashSet::default();
        let mut rc_conf = String::new();
        for (idx, piece) in path.split(':').enumerate() {
            let piece = Path::from(piece);
            if !piece.exists() {
                continue;
            }
            if seen.contains(&piece) {
                rc_conf += &format!(
                    "# rc_conf[{}] = {:?}; already sourced\n",
                    idx,
                    piece.as_str()
                );
                continue;
            }
            rc_conf += &format!("# rc_conf[{}] = {:?}\n", idx, piece.as_str());
            seen.insert(piece.clone().into_owned());
            Self::examine_recursive(&piece, &mut seen, &mut rc_conf)?;
        }
        Ok(rc_conf)
    }

    fn examine_recursive(
        path: &Path,
        seen: &mut HashSet<Path>,
        rc_conf: &mut String,
    ) -> Result<(), Error> {
        seen.insert(path.clone().into_owned());
        let contents = std::fs::read_to_string(path.as_str())?;
        for (_, line, raw) in linearize(path, &contents)? {
            if let Some(source) = line.trim().strip_prefix("source ") {
                let source = Path::from(source);
                if !seen.contains(&source) {
                    *rc_conf += &format!("# begin source {source:?}\n");
                    seen.insert(path.clone().into_owned());
                    Self::examine_recursive(&source, seen, rc_conf)?;
                    *rc_conf += &format!("# end source {source:?}\n");
                } else {
                    *rc_conf += &format!("# already sourced {source:?}\n");
                }
            } else {
                for line in raw {
                    *rc_conf += &line;
                    rc_conf.push('\n');
                }
            }
        }
        Ok(())
    }

    /// The variables defined within this RcConf.
    pub fn variables(&self) -> Vec<String> {
        self.items.keys().cloned().collect()
    }

    /// Merge the other rc_conf into this one, overwriting values where necessary.
    ///
    /// Note that merge does not perform parameter expansion on the variables, so merging
    /// "${FOO:+${FOO} more foo}" won't do anything except overwrite the value of FOO to be a
    /// self-referential expansion.
    pub fn merge(&mut self, other: Self) {
        for (key, value) in other.items.into_iter() {
            self.items.insert(key, value);
        }
    }

    /// List all services and aliases inferrable from the rc.conf.
    pub fn list(&self) -> Result<impl Iterator<Item = String> + '_, Error> {
        let mut services = vec![];
        let aliases = self.aliases();

        // Collect canonical forms of all aliases for deduplication
        let alias_canonical: std::collections::HashSet<String> = aliases
            .iter()
            .map(|alias| service_from_var_name(&var_name_from_service(alias)))
            .collect();

        for var in self.variables() {
            if let Some(service) = var.strip_suffix("_ENABLED") {
                if self.lookup_suffix_direct(service, "AUTOGEN").is_some() {
                    continue;
                }
                let canonical_service = service_from_var_name(service);
                // Only add if there's no alias that represents the same service
                if !alias_canonical.contains(&canonical_service) {
                    services.push(canonical_service);
                }
            }
        }
        services.extend(aliases);
        services.sort();
        Ok(services.into_iter())
    }

    /// List the services with the ServiceSwitch::Yes flag.  This will return the canonical service
    /// name for each _ENABLED="YES" variable or alias.
    pub fn list_services(&self) -> Result<impl Iterator<Item = String> + '_, Error> {
        Ok(self
            .list()?
            .filter(|s| self.service_switch(s) == SwitchPosition::Yes))
    }

    /// List the tasks with the ServiceSwitch::Manual flag.  This will return the canonical service
    /// name for each _ENABLED="MANUAL" variable or alias.
    pub fn list_tasks(&self) -> Result<impl Iterator<Item = String> + '_, Error> {
        Ok(self
            .list()?
            .filter(|s| self.service_switch(s) == SwitchPosition::Manual))
    }

    /// Create a variable provider that will lookup variables for service.
    /// `service`.
    pub fn variable_provider_for(
        &self,
        service: &str,
    ) -> Result<impl VariableProvider + '_, Error> {
        // NOTE(rescrv):  Don't use lookup_suffix here because we need the full variable provider
        // to be able to expand the suffix.
        let (alias_lookup_order, pre_lookup) = self.alias_lookup_order(service);
        let mut vp = Vec::with_capacity(alias_lookup_order.len());
        for a in alias_lookup_order.iter() {
            vp.push(PrefixingVariableProvider {
                nested: self,
                prefix: var_prefix_from_service(a),
            });
            if !self
                .aliases
                .get(a.to_string().as_str())
                .map(|a| a.inherit)
                .unwrap_or(false)
            {
                break;
            }
        }
        let vp = (pre_lookup, vp, self);
        Ok(vp)
    }

    /// Generate the set of rcvariables that are expected by the script at `path` when invoked as
    /// `service`.
    pub fn bind_for_invoke(
        &self,
        service: &str,
        path: &Path,
    ) -> Result<HashMap<String, String>, Error> {
        let output = Command::new(path.clone().into_std())
            .arg("rcvar")
            .env("RCVAR_ARGV0", var_name_from_service(service))
            .output()?;
        if !output.status.success() {
            return Err(Error::InvalidInvocation {
                message: "rcvar command failed".to_string(),
            });
        }
        let keys = String::from_utf8(output.stdout)?;
        let keys = keys.split_whitespace().collect::<Vec<_>>();
        self.generate_rcvars(service, &keys)
    }

    /// Generate the set of rcvariables that are expected by the script at `path` when invoked as
    /// `service`.
    pub fn bind_for_container(
        &self,
        command: &str,
        container: &str,
        service: &str,
    ) -> Result<HashMap<String, String>, Error> {
        let output = Command::new(command)
            .arg("run")
            .arg("-t")
            .arg("-e")
            .arg(format!("RCVAR_ARGV0={}", var_name_from_service(service)))
            .arg("-e")
            .arg("RCCONF_OVERRIDE_SERVICE_SWITCH=true")
            .arg("--entrypoint")
            .arg("rcvar")
            .arg(container)
            .arg(service)
            .output()?;
        if !output.status.success() {
            return Err(Error::InvalidInvocation {
                message: "rcvar command failed".to_string(),
            });
        }
        let keys = String::from_utf8(output.stdout)?;
        let keys = keys.split_whitespace().collect::<Vec<_>>();
        self.generate_rcvars(service, &keys)
    }

    /// Generate the set of rcvariables from the provided set of keys.
    pub fn generate_rcvars(
        &self,
        service: &str,
        keys: &[&str],
    ) -> Result<HashMap<String, String>, Error> {
        let mut bindings = HashMap::new();
        let vp = self.variable_provider_for(service)?;
        let prefix = var_prefix_from_service(service);
        for var in keys {
            let Some(short) = var.strip_prefix(&prefix) else {
                continue;
            };
            if let Some(value) = vp.lookup(short) {
                let value = shvar::expand_recursive(&vp, &value)?;
                let quoted = shvar::quote(shvar::split(&value)?);
                bindings.insert(var.to_string(), quoted);
            }
        }
        Ok(bindings)
    }

    /// Return a vector of strings suitable for passing to exec.
    pub fn argv(
        &self,
        service: &str,
        variable: &str,
        additional: &impl VariableProvider,
    ) -> Result<Vec<String>, Error> {
        let meta = HashMap::from([("NAME".to_string(), service.to_string())]);
        let vp = self.variable_provider_for(service)?;
        let vp = (additional, &meta, &vp);
        let Some(argv) = self.lookup_suffix(service, variable) else {
            return Ok(vec![]);
        };
        let argv = shvar::expand_recursive(&vp, &argv)?;
        if argv.trim().is_empty() {
            return Ok(vec![]);
        }
        Ok(shvar::split(&argv)?)
    }

    /// Lookup the service switch for `service`.
    pub fn service_switch(&self, service: &str) -> SwitchPosition {
        let (alias_lookup_order, _) = self.alias_lookup_order(service);
        for service in alias_lookup_order {
            let Some(enable) = self.lookup_suffix_direct(service, "ENABLED") else {
                MISSING_ENABLED_VAR.click();
                continue;
            };
            let Ok(split) = shvar::split(&enable) else {
                SPLIT_FAILURE.click();
                return SwitchPosition::No;
            };
            let enable = if split.len() == 1 {
                // SAFETY(rescrv): Length is one, so pop will succeed.
                &split[0]
            } else {
                &enable
            };
            let Some(switch) = SwitchPosition::from_enable(enable) else {
                INVALID_SWITCH_VALUE.click();
                return SwitchPosition::No;
            };
            return switch;
        }
        NO_SERVICES_FOUND.click();
        SwitchPosition::No
    }

    /// Lookup the value of the variable as service_SUFFIX, any alias_SUFFIX, and finally SUFFIX.
    pub fn lookup_suffix(&self, service: &str, suffix: &str) -> Option<String> {
        self.variable_provider_for(service).ok()?.lookup(suffix)
    }

    fn lookup_suffix_direct(&self, service: &str, suffix: &str) -> Option<String> {
        let mut varname = var_prefix_from_service(service);
        varname += suffix;
        self.lookup(&varname)
    }

    /// Return the list of aliases.
    pub fn aliases(&self) -> Vec<String> {
        let mut aliases = self
            .aliases
            .keys()
            .filter(|a| !self.autogens.contains(*a))
            .cloned()
            .collect::<Vec<_>>();
        aliases.sort();
        aliases
    }

    /// Resolve the alias `service` one-hop.
    pub fn direct_alias<'a>(&'a self, service: &'a str) -> &'a str {
        if let Some(alias) = self.aliases.get(service) {
            &alias.aliases
        } else {
            service
        }
    }

    /// Recursively resolve the alias `service`.
    pub fn resolve_alias<'a>(&'a self, service: &'a str) -> &'a str {
        if let Some(alias) = self.aliases.get(service) {
            self.resolve_alias(&alias.aliases)
        } else {
            service
        }
    }

    /// Generate the alias lookup order for `service` and a cascade of variables.
    pub fn alias_lookup_order<'a>(
        &'a self,
        service: &'a str,
    ) -> (Vec<&'a str>, HashMap<String, String>) {
        let mut alias_lookup_order = vec![service];
        let mut direct_alias = service;
        let mut pre_lookup = HashMap::new();
        while let Some(alias) = self.aliases.get(direct_alias) {
            alias_lookup_order.push(&alias.aliases);
            direct_alias = &alias.aliases;
            for (k, v) in alias.vp.iter() {
                if !pre_lookup.contains_key(k) {
                    pre_lookup.insert(k.clone(), v.clone());
                }
            }
        }
        (alias_lookup_order, pre_lookup)
    }
}

impl shvar::VariableProvider for RcConf {
    fn lookup(&self, ident: &str) -> Option<String> {
        self.items.get(ident).cloned()
    }
}

/////////////////////////////////////////////// rc.d ///////////////////////////////////////////////

/// Load the rc.d services from a given rc.d path.
pub fn load_services(
    rc_d_path: &str,
) -> Result<HashMap<String, Result<Path<'static>, String>>, Error> {
    let mut services = HashMap::default();
    for rc_d in rc_d_path.split(':') {
        if !Path::from(rc_d).exists() {
            continue;
        }
        for dirent in std::fs::read_dir(rc_d)? {
            let dirent = dirent?;
            let path = Path::try_from(dirent.path())?.into_owned();
            let name = dirent.file_name().to_string_lossy().to_string();
            match services.entry(name) {
                Entry::Occupied(mut entry) => {
                    let value: &mut Result<Path<'static>, String> = entry.get_mut();
                    if value.is_ok() {
                        *value = Err("duplicate service definition".to_string());
                    }
                }
                Entry::Vacant(entry) => {
                    entry.insert(Ok(path));
                }
            };
        }
    }
    Ok(services)
}

////////////////////////////////////////////// exec_rc /////////////////////////////////////////////

/// Exec a service using the provided rc_conf_path, rc_d_path, service name, and command arguments.
///
/// This does not return.
pub fn exec_rc(rc_conf_path: &str, rc_d_path: &str, service: &str, cmd: &[&str]) -> ! {
    exec_rc_with_override(rc_conf_path, rc_d_path, false, service, cmd)
}

fn exec_rc_with_override(
    rc_conf_path: &str,
    rc_d_path: &str,
    override_service_switch: bool,
    service: &str,
    cmd: &[&str],
) -> ! {
    let rc_conf = RcConf::parse(rc_conf_path).unwrap_or_else(|e| {
        eprintln!("failed to parse rc_conf: {e}");
        std::process::exit(133);
    });
    let rc_d = load_services(rc_d_path).unwrap_or_else(|e| {
        eprintln!("failed to load services: {e}");
        std::process::exit(134);
    });
    if !override_service_switch && !rc_conf.service_switch(service).can_be_started() {
        eprintln!("service not enabled");
        std::process::exit(132);
    }
    let mut env = HashMap::new();
    let path = if let Some(alias) = rc_conf.aliases.get(service) {
        let Some(path) = rc_d.get(rc_conf.resolve_alias(&alias.aliases)) else {
            eprintln!("expected alias of service to be available via --rc-d-path");
            std::process::exit(130);
        };
        env.insert("RCVAR_ARGV0".to_string(), var_name_from_service(service));
        path
    } else {
        let Some(path) = rc_d.get(service) else {
            eprintln!("expected service to be available via --rc-d-path");
            std::process::exit(130);
        };
        env.insert("RCVAR_ARGV0".to_string(), var_name_from_service(service));
        path
    };
    let path = match path {
        Ok(path) => path,
        Err(err) => {
            eprintln!("service encountered an error: {err:?}");
            std::process::exit(131);
        }
    };
    let mut bound = rc_conf.bind_for_invoke(service, path).unwrap_or_else(|e| {
        eprintln!("failed to bind variables for service: {e}");
        std::process::exit(135);
    });
    bound.extend(env);
    let argv = rc_conf.argv(service, "WRAPPER", &()).unwrap_or_else(|e| {
        eprintln!("failed to generate argv: {e}");
        std::process::exit(136);
    });
    let err = if !argv.is_empty() {
        Command::new(&argv[0])
            .args(&argv[1..])
            .arg(path.as_str())
            .args(cmd)
            .envs(bound)
            .exec()
    } else {
        Command::new(path.as_str()).args(cmd).envs(bound).exec()
    };
    eprintln!("command unexpectedly failed: {err}");
    std::process::exit(137);
}

////////////////////////////////////////// exec_container //////////////////////////////////////////

/// Exec a service using the provided rc_conf_path, rc_d_path, service name, and command arguments.
///
/// This does not return.
pub fn exec_container(
    rc_conf_path: &str,
    _: &str,
    command: &str,
    container: &str,
    service: &str,
    cmd: &[&str],
) -> ! {
    let rc_conf = RcConf::parse(rc_conf_path).unwrap_or_else(|e| {
        eprintln!("failed to parse rc_conf: {e}");
        std::process::exit(133);
    });
    if !rc_conf.service_switch(service).can_be_started() {
        eprintln!("service not enabled");
        std::process::exit(132);
    }
    let mut env = HashMap::new();
    env.insert("RCVAR_ARGV0".to_string(), var_name_from_service(service));
    let mut bound = rc_conf
        .bind_for_container(command, container, service)
        .unwrap_or_else(|e| {
            eprintln!("failed to bind variables for container: {e}");
            std::process::exit(135);
        });
    bound.extend(env);
    let mut argv = vec![command.to_string(), "run".to_string(), "-t".to_string()];
    for (key, value) in bound.iter() {
        argv.push("--env".to_string());
        argv.push(format!("{key}={value}"));
    }
    argv.push("--env".to_string());
    argv.push("RCCONF_OVERRIDE_SERVICE_SWITCH=true".to_string());
    argv.push(container.to_string());
    argv.extend(rc_conf.argv(service, "WRAPPER", &()).unwrap_or_else(|e| {
        eprintln!("failed to generate argv: {e}");
        std::process::exit(136);
    }));
    let err = Command::new(&argv[0])
        .args(&argv[1..])
        .arg(service)
        .args(cmd)
        .envs(bound)
        .exec();
    eprintln!("command unexpectedly failed: {err}");
    std::process::exit(137);
}

///////////////////////////////////////////// rcinvoke /////////////////////////////////////////////

/// exec_rc the service in a way that runs it.
pub fn invoke(rc_conf_path: &str, rc_d_path: &str, service: &str, args: &[&str]) -> ! {
    let mut cmd = vec!["run"];
    cmd.extend(args);
    let override_service_switch = std::env::var("RCCONF_OVERRIDE_SERVICE_SWITCH").is_ok();
    exec_rc_with_override(
        rc_conf_path,
        rc_d_path,
        override_service_switch,
        service,
        &cmd,
    )
}

/////////////////////////////////////////////// rcvar //////////////////////////////////////////////

/// exec_rc the service in a way that prints rcvariables.
pub fn rcvar(rc_conf_path: &str, rc_d_path: &str, service: &str) -> ! {
    let override_service_switch = std::env::var("RCCONF_OVERRIDE_SERVICE_SWITCH").is_ok();
    exec_rc_with_override(
        rc_conf_path,
        rc_d_path,
        override_service_switch,
        service,
        &["rcvar"],
    )
}

///////////////////////////////////////////// bootstrap ////////////////////////////////////////////

fn vendor(path: utf8path::Path, crate_name: &str, spec: &str) -> Result<(), Error> {
    let tmp = std::env::temp_dir().join(format!(
        "{}_{}_{}",
        crate_name,
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("Time should not go before UNIX epoch")
            .as_millis(),
        std::process::id()
    ));
    std::fs::create_dir(&tmp)?;
    std::fs::create_dir(tmp.join("src"))?;
    std::fs::write(tmp.join("src/lib.rs"), [])?;
    let tmp = tmp.join("Cargo.toml");
    std::fs::write(
        &tmp,
        format!(
            r#"
[package]
name = "rc-conf-dummy"
version = "0.1.0"
edition = "2021"

[dependencies]
{crate_name} = {spec}
"#
        ),
    )?;
    std::process::Command::new("cargo")
        .arg("vendor")
        .arg("--no-delete")
        .arg("--manifest-path")
        .arg(tmp)
        .arg(path.as_str())
        .output()?;
    Ok(())
}

/// Prepare the output directory for running the provided rc_conf_path.  Return the minimal
/// rc_d_path that will allow it to run.
pub fn bootstrap<'a>(
    rc_conf_path: &str,
    output: impl Into<utf8path::Path<'a>>,
) -> Result<String, Error> {
    let output = output.into();
    let rc_conf = RcConf::parse(rc_conf_path)?;
    let mut rc_d_path = String::new();
    for variable in rc_conf.variables() {
        if let Some(crate_name) = variable.strip_suffix("_SPEC") {
            if !rc_d_path.is_empty() {
                rc_d_path.push(':');
            }
            // SAFETY(rescrv):  We got the value from variables above and it's a hash map.  It's
            // still in there, so lookup should succeed.
            vendor(
                output.clone(),
                crate_name,
                &rc_conf.lookup(&variable).unwrap(),
            )?;
            rc_d_path.push_str(output.join(crate_name).join("rc.d").as_str());
        }
    }
    Ok(rc_d_path)
}

///////////////////////////////////////////// utilities ////////////////////////////////////////////

/// Turn the contents of a file into numbered lines, while respecting line continuation markers.
///
/// This function processes configuration file contents and handles line continuation
/// using backslash (`\`) characters at the end of lines.
///
/// # Arguments
/// * `path` - File path for error reporting
/// * `contents` - Raw file contents to process
///
/// # Returns
/// A vector of tuples containing:
/// * Line number (1-based)
/// * Processed line content with continuations resolved
/// * Raw lines that contributed to this logical line
///
/// # Errors
/// Returns an error if:
/// * File is too large (more than u32::MAX lines)
/// * Invalid backslash usage (not at end of line, or multiple backslashes)
/// * File ends with a trailing backslash
pub fn linearize(path: &Path, contents: &str) -> Result<Vec<(u32, String, Vec<String>)>, Error> {
    let mut start = 1;
    let mut acc = String::new();
    let mut raw = vec![];
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
            raw.push(line.to_string());
            let line = std::mem::take(&mut acc);
            let raw = std::mem::take(&mut raw);
            lines.push((start, line, raw));
            start = number as u32 + 1;
        } else {
            acc += line[..line.len() - 1].trim();
            raw.push(line.to_string());
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

/// Return the var name for a service.  Converts non-alphanumerics to underscores.
pub fn var_name_from_service(service: &str) -> String {
    service
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect()
}

/// Return _a_ canonical service name from a variable name.
pub fn service_from_var_name(var_name: &str) -> String {
    var_name
        .chars()
        .flat_map(|c| {
            if c.is_alphanumeric() {
                c.to_lowercase()
            } else {
                '-'.to_lowercase()
            }
        })
        .collect()
}

/// Return the variable prefix for a service or alias.
pub fn var_prefix_from_service(service: &str) -> String {
    var_name_from_service(service) + "_"
}

////////////////////////////////////////////// filters /////////////////////////////////////////////

fn split_for_filter(var: &str) -> (String, Vec<String>) {
    if var.contains("__") {
        (
            "__".to_string(),
            var.split("__").map(String::from).collect(),
        )
    } else {
        ("_".to_string(), var.split('_').map(String::from).collect())
    }
}

fn strip_prefix_values(
    values: &HashMap<String, RcConf>,
    template: &str,
) -> (String, Vec<String>, String) {
    let mut still_pulling_values = true;
    let mut vars = vec![];
    let mut built = vec![];
    let mut filter = vec![];
    let pieces = template.split('_').collect::<Vec<_>>();
    for piece in pieces.iter() {
        let contains = values.contains_key(&piece.to_string());
        if !piece.is_empty() {
            if contains && still_pulling_values {
                vars.push(piece.to_string());
                let var = format!("${{{piece}}}");
                built.push(var.clone());
                filter.push(var);
            } else if !contains || !still_pulling_values {
                still_pulling_values = false;
                built.push(piece.to_string());
            }
        } else {
            built.push(piece.to_string());
        }
    }
    (built.join("_"), vars, filter.join("_"))
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    mod rc_script {
        use super::super::*;

        #[test]
        fn new() {
            RcScript::new("name", "describe", "command");
        }

        #[test]
        fn from() {
            let rc_script = RcScript::parse(
                &Path::from("name"),
                r#"
DESCRIBE=my description
COMMAND=my-command --option
"#,
            )
            .unwrap();
            assert_eq!(
                RcScript::new("name", "my description", "my-command --option"),
                rc_script
            );
        }

        #[test]
        fn quoted() {
            let rc_script = RcScript::parse(
                &Path::from("name"),
                r#"
DESCRIBE=my description
COMMAND="my-command" "--option"
"#,
            )
            .unwrap();
            assert_eq!(
                RcScript::new("name", "my description", "\"my-command\" \"--option\""),
                rc_script
            );
        }

        #[test]
        fn with_newline() {
            let rc_script = RcScript::parse(
                &Path::from("name"),
                r#"
DESCRIBE=my description
COMMAND=my-command \
    --option
"#,
            )
            .unwrap();
            assert_eq!(
                RcScript::new("name", "my description", "my-command --option"),
                rc_script
            );
        }

        #[test]
        fn rcvar() {
            let rc_script = RcScript::parse(
                &Path::from("name"),
                r#"
DESCRIBE=my description
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

    mod rcexamine {
        use super::super::RcConf;

        #[test]
        fn examine() {
            let examined =
                RcConf::examine("bar.conf:foo.conf").expect("examine should always succeed");
            assert_eq!(
                r#"
# rc_conf[0] = "bar.conf"
# begin source "foo.conf"
foo_ENABLE=YES
# end source "foo.conf"

bar_ENABLE=YES

# already sourced "foo.conf"
# rc_conf[1] = "foo.conf"; already sourced
            "#
                .trim(),
                examined.trim()
            );
        }
    }

    mod rclist {
        use std::collections::HashMap;

        use utf8path::Path;

        #[test]
        fn list_rc_d_once() {
            let services =
                super::super::load_services("rc.d").expect("load_services should always succeed");
            assert_eq!(
                HashMap::from([
                    ("example1".to_string(), Ok(Path::from("rc.d/example1"))),
                    ("example2".to_string(), Ok(Path::from("rc.d/example2"))),
                    ("runbook1".to_string(), Ok(Path::from("rc.d/runbook1"))),
                ]),
                services
            );
        }

        #[test]
        fn list_rc_d_twice() {
            let services = super::super::load_services("rc.d:rc.d")
                .expect("load_services should always succeed");
            assert_eq!(
                HashMap::from([
                    (
                        "example1".to_string(),
                        Err("duplicate service definition".to_string())
                    ),
                    (
                        "example2".to_string(),
                        Err("duplicate service definition".to_string())
                    ),
                    (
                        "runbook1".to_string(),
                        Err("duplicate service definition".to_string())
                    ),
                ]),
                services
            );
        }
    }

    #[test]
    fn strip_prefix_values() {
        let values = HashMap::from([
            ("FOO".to_string(), super::RcConf::default()),
            ("BAR".to_string(), super::RcConf::default()),
        ]);
        assert_eq!(
            (
                "${FOO}_${BAR}_service".to_string(),
                vec!["FOO".to_string(), "BAR".to_string()],
                "${FOO}_${BAR}".to_string(),
            ),
            super::strip_prefix_values(&values, "FOO_BAR_service")
        );
    }

    #[test]
    fn error_handling_invalid_switch() {
        let mut rc_conf = super::RcConf::default();
        rc_conf
            .items
            .insert("test_ENABLED".to_string(), "INVALID".to_string());
        assert_eq!(rc_conf.service_switch("test"), super::SwitchPosition::No);
    }

    #[test]
    fn error_handling_split_failure() {
        let mut rc_conf = super::RcConf::default();
        // Insert a value that would cause shvar::split to fail
        rc_conf
            .items
            .insert("test_ENABLED".to_string(), "\"unclosed".to_string());
        assert_eq!(rc_conf.service_switch("test"), super::SwitchPosition::No);
    }

    #[test]
    fn switch_position_from_enable() {
        assert_eq!(
            super::SwitchPosition::from_enable("YES"),
            Some(super::SwitchPosition::Yes)
        );
        assert_eq!(
            super::SwitchPosition::from_enable("NO"),
            Some(super::SwitchPosition::No)
        );
        assert_eq!(
            super::SwitchPosition::from_enable("MANUAL"),
            Some(super::SwitchPosition::Manual)
        );
        assert_eq!(super::SwitchPosition::from_enable("invalid"), None);
    }

    #[test]
    fn switch_position_can_be_started() {
        assert!(super::SwitchPosition::Yes.can_be_started());
        assert!(super::SwitchPosition::Manual.can_be_started());
        assert!(!super::SwitchPosition::No.can_be_started());
    }

    #[test]
    fn duplication_issue_test() {
        // Create a test rc_conf with the same pattern as the real one
        let mut items = HashMap::new();

        // Add autogen setup
        items.insert(
            "METRO_CUSTOMER_example4_AUTOGEN".to_string(),
            "YES".to_string(),
        );
        items.insert(
            "METRO_CUSTOMER_example4_ENABLED".to_string(),
            "YES".to_string(),
        );
        items.insert(
            "METRO_CUSTOMER_example4_ALIASES".to_string(),
            "example4".to_string(),
        );
        items.insert(
            "METRO_CUSTOMER_example4_INHERIT".to_string(),
            "YES".to_string(),
        );
        items.insert("VALUES_METRO".to_string(), "metros.conf".to_string());
        items.insert("VALUES_CUSTOMER".to_string(), "customers.conf".to_string());

        // Add manual enabled for the same service
        items.insert(
            "Jfk_PlanetExpress_example4_ENABLED".to_string(),
            "YES".to_string(),
        );
        items.insert(
            "Jfk_PlanetExpress_example4_FIELD1".to_string(),
            "Good News".to_string(),
        );

        // Create a mock rc_conf - this won't work directly but shows the concept
        let rc_conf = super::RcConf::parse("rc.conf").unwrap();
        let services: Vec<_> = rc_conf.list().unwrap().collect();

        // Check for duplicates
        let jfk_planet_services: Vec<_> = services
            .iter()
            .filter(|s| s.to_lowercase().contains("jfk") && s.to_lowercase().contains("planet"))
            .collect();

        println!("JFK Planet services found: {:?}", jfk_planet_services);

        // Should only appear once, either as autogen or manual, not both
        assert!(
            jfk_planet_services.len() <= 1,
            "jfk-planetexpress-example4 should appear at most once, found: {:?}",
            jfk_planet_services
        );
    }

    #[test]
    fn fragmented_services() {
        let rc_conf = super::RcConf::parse("rc.conf").unwrap();
        assert_eq!(
            vec![
                "Jfk_PlanetExpress_example4",
                "Jfk_TyrellCorp_example4",
                "Sac_Acme_example4",
                "Sfo_ApertureScience_example4",
                "Sjc_CyberDyne_example4",
                "example3",
            ],
            rc_conf.aliases(),
        );
        assert_eq!(
            vec![
                "Jfk_PlanetExpress_example4",
                "Jfk_TyrellCorp_example4",
                "Sac_Acme_example4",
                "Sfo_ApertureScience_example4",
                "Sjc_CyberDyne_example4",
                "example1",
                "example2",
                "example3",
                "rcdemo",
                "runbook1",
            ],
            rc_conf.list().unwrap().collect::<Vec<_>>()
        );
    }
}
