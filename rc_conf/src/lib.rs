use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::os::unix::process::CommandExt;
use std::process::Command;

use shvar::{PrefixingVariableProvider, VariableProvider};
use utf8path::Path;

///////////////////////////////////////////// constants ////////////////////////////////////////////

const RESTRICTED_VARIABLES: &[&str] = &["NAME"];

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
    FromUtf8Error(std::string::FromUtf8Error),
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

impl From<std::string::FromUtf8Error> for Error {
    fn from(err: std::string::FromUtf8Error) -> Self {
        Self::FromUtf8Error(err)
    }
}

////////////////////////////////////////// SwitchPosition //////////////////////////////////////////

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SwitchPosition {
    No,
    Yes,
    Manual,
}

impl SwitchPosition {
    pub fn from_enable<S: AsRef<str>>(s: S) -> Option<Self> {
        let s = s.as_ref();
        match s {
            "YES" => Some(SwitchPosition::Yes),
            "NO" => Some(SwitchPosition::No),
            "MANUAL" => Some(SwitchPosition::Manual),
            _ => None,
        }
    }

    pub fn is_enabled(self) -> bool {
        match self {
            Self::Yes => true,
            Self::Manual => true,
            Self::No => false,
        }
    }
}

///////////////////////////////////////////// RcScript /////////////////////////////////////////////

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RcScript {
    pub name: String,
    describe: String,
    command: String,
}

impl RcScript {
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

    pub fn describe(&self) -> &str {
        &self.describe
    }

    pub fn command(&self) -> &str {
        &self.command
    }

    pub fn rcvar(&self) -> Result<Vec<String>, Error> {
        let name = var_prefix_from_service(&self.name);
        Ok(shvar::rcvar(&self.command)?
            .into_iter()
            .map(|v| format!("{}_{}", name, v))
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
        let mut name = var_prefix_from_service(&self.name);
        name.push('_');
        let evp = EnvironmentVariableProvider::new(Some(name));
        let meta = HashMap::from([("NAME".to_string(), self.name.to_string())]);
        let exp = shvar::expand(&(&meta, &evp), &self.command)?;
        let mut cmd = shvar::split(&exp)?;
        if !args.is_empty() {
            cmd.push("--".to_string());
        }
        cmd.extend(args.iter().map(|s| s.to_string()));
        panic!(
            "could not exec {} {:?}\n{:?}",
            &cmd[0],
            args,
            Command::new(&cmd[0]).args(&cmd[1..]).exec()
        );
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

/////////////////////////////////////////////// Alias //////////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Alias {
    // The physical service stub this service aliases.
    aliases: String,
    // True if this alias inherits from what it aliases in rc.conf.
    inherit: bool,
}

////////////////////////////////////////////// RcConf //////////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RcConf {
    items: HashMap<String, String>,
    aliases: HashMap<String, Alias>,
    values: HashMap<String, RcConf>,
}

impl RcConf {
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
        for (varname, alias) in items.iter() {
            let Some(name) = varname.strip_suffix("_ALIASES") else {
                continue;
            };
            aliases.insert(
                name.to_string(),
                Alias {
                    aliases: alias.clone(),
                    inherit: false,
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
            values.insert(name.to_string(), RcConf {
                items: values_items,
                aliases: HashMap::default(),
                values: HashMap::default(),
            });
        }
        Ok(Self { items, aliases, values })
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
                Self::parse_recursive(&Path::from(source), seen, items)?;
            } else if let Some((var, val)) = line.split_once('=') {
                let split = shvar::split(val)?;
                if split.len() != 1 {
                    return Err(Error::invalid_rc_conf(path, number, line));
                }
                items.insert(var.to_string(), split[0].clone());
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
                if split.len() != 1 {
                    return Err(Error::invalid_rc_conf(path, number, line));
                }
                items.insert(var.to_string(), split[0].clone());
            } else {
                return Err(Error::invalid_rc_conf(path, number, line));
            }
        }
        Ok(())
    }

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
                    *rc_conf += &format!("# begin source {:?}\n", source);
                    seen.insert(path.clone().into_owned());
                    Self::examine_recursive(&source, seen, rc_conf)?;
                    *rc_conf += &format!("# end source {:?}\n", source);
                } else {
                    *rc_conf += &format!("# already sourced {:?}\n", source);
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

    pub fn variables(&self) -> Vec<String> {
        self.items.values().cloned().collect()
    }

    pub fn merge(&mut self, other: Self) {
        for (key, value) in other.items.into_iter() {
            self.items.insert(key, value);
        }
    }

    pub fn bind_for_invoke(
        &self,
        service: &str,
        path: &Path,
    ) -> Result<HashMap<String, String>, Error> {
        let mut bindings = HashMap::new();
        let output = Command::new(path.clone().into_std())
            .arg("rcvar")
            .env("RCVAR_ARGV0", var_prefix_from_service(service))
            .output()?;
        if !output.status.success() {
            return Err(Error::InvalidInvocation {
                message: "rcvar command failed".to_string(),
            });
        }
        let stdout = String::from_utf8(output.stdout)?;
        for var in stdout.split_whitespace() {
            if let Some(value) = self.lookup(var) {
                let value = shvar::expand(self, &value)?;
                let quoted = shvar::quote(shvar::split(&value)?);
                bindings.insert(var.to_string(), quoted);
            } else if let Some(var2) = var.strip_prefix(&(var_prefix_from_service(service) + "_")) {
                if let Some(value) = self.lookup(var2) {
                    let value = shvar::expand(self, &value)?;
                    let quoted = shvar::quote(shvar::split(&value)?);
                    bindings.insert(var.to_string(), quoted);
                }
            }
        }
        Ok(bindings)
    }

    pub fn wrapper(&self, service: &str, variable: &str) -> Result<Vec<String>, Error> {
        let mut prefix = var_prefix_from_service(service);
        prefix.push('_');
        let meta = HashMap::from([("NAME".to_string(), service.to_string())]);
        let pvp = PrefixingVariableProvider {
            prefix,
            nested: self,
        };
        let vp = (&meta, &pvp);
        let Some(wrapper) = vp.lookup(variable) else {
            return Ok(vec![]);
        };
        let wrapper = shvar::expand(&vp, &wrapper)?;
        if wrapper.trim().is_empty() {
            return Ok(vec![]);
        }
        Ok(shvar::split(&wrapper)?)
    }

    pub fn service_switch(&self, service: &str) -> SwitchPosition {
        let Some(enable) = self.lookup_suffix(service, "_ENABLED") else {
            // TODO(rescrv): biometrics.
            return SwitchPosition::No;
        };
        let Ok(split) = shvar::split(&enable) else {
            // TODO(rescrv): biometrics.
            return SwitchPosition::No;
        };
        let enable = if split.len() == 1 {
            // SAFETY(rescrv): Length is one, so pop will succeed.
            &split[0]
        } else {
            &enable
        };
        let Some(switch) = SwitchPosition::from_enable(enable) else {
            // TODO(rescrv): biometrics.
            return SwitchPosition::No;
        };
        switch
    }

    fn lookup_suffix(&self, service: &str, suffix: &str) -> Option<String> {
        let mut enabled = var_prefix_from_service(service);
        enabled += suffix;
        if let Some(enable) = self.lookup(&enabled) {
            return Some(enable);
        }
        if let Some(alias) = self.aliases.get(service) {
            if alias.inherit {
                self.lookup_suffix(&alias.aliases, suffix)
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn aliases(&self) -> Vec<String> {
        self.aliases.keys().cloned().collect()
    }

    pub fn resolve_alias<'a>(&'a self, service: &'a str) -> &'a str {
        if let Some(alias) = self.aliases.get(service) {
            self.resolve_alias(&alias.aliases)
        } else {
            service
        }
    }
}

impl shvar::VariableProvider for RcConf {
    fn lookup(&self, ident: &str) -> Option<String> {
        self.items.get(ident).cloned()
    }
}

/////////////////////////////////////////////// rc.d ///////////////////////////////////////////////

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

pub fn exec_rc(rc_conf_path: &str, rc_d_path: &str, service: &str, cmd: &[&str]) -> ! {
    let rc_conf = RcConf::parse(rc_conf_path).expect("rc_conf should parse");
    let rc_d = load_services(rc_d_path).expect("rc.d should parse");
    if !rc_conf.service_switch(service).is_enabled() {
        eprintln!("service not enabled");
        std::process::exit(132);
    }
    let mut env = HashMap::new();
    let path = if let Some(alias) = rc_conf.aliases.get(service) {
        let Some(path) = rc_d.get(&alias.aliases) else {
            eprintln!("expected alias of service to be available via --rc-d-path");
            std::process::exit(130);
        };
        env.insert("RCVAR_ARGV0".to_string(), service.to_string());
        path
    } else {
        let Some(path) = rc_d.get(service) else {
            eprintln!("expected service to be available via --rc-d-path");
            std::process::exit(130);
        };
        path
    };
    let path = match path {
        Ok(path) => path,
        Err(err) => {
            eprintln!("service encountered an error: {err:?}");
            std::process::exit(131);
        }
    };
    let mut bound = rc_conf
        .bind_for_invoke(service, path)
        .expect("bind for invoke should bind");
    bound.extend(env);
    let wrapper = rc_conf
        .wrapper(service, "WRAPPER")
        .expect("wrapper should generate");
    let err = if !wrapper.is_empty() {
        Command::new(&wrapper[0])
            .args(&wrapper[1..])
            .arg(path.as_str())
            .args(cmd)
            .envs(bound)
            .exec()
    } else {
        Command::new(path.as_str()).args(cmd).envs(bound).exec()
    };
    panic!("command unexpectedly failed: {err}");
}

///////////////////////////////////////////// rcinvoke /////////////////////////////////////////////

pub fn invoke(rc_conf_path: &str, rc_d_path: &str, service: &str, args: &[&str]) -> ! {
    let mut cmd = vec!["run"];
    cmd.extend(args);
    exec_rc(rc_conf_path, rc_d_path, service, &cmd)
}

/////////////////////////////////////////////// rcvar //////////////////////////////////////////////

pub fn rcvar(rc_conf_path: &str, rc_d_path: &str, service: &str) -> ! {
    exec_rc(rc_conf_path, rc_d_path, service, &["rcvar"])
}

///////////////////////////////////////////// utilities ////////////////////////////////////////////

/// Turn the contents of a file into numbered lines, while respecting line continuation markers.
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

pub fn var_prefix_from_service(service: &str) -> String {
    service
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect()
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
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
                ]),
                services
            );
        }
    }
}
