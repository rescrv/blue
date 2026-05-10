#![doc = include_str!("../README.md")]

use std::collections::{BTreeSet, HashMap};
use std::hash::Hash;

use serde::Deserialize;
use serde_yaml::{Deserializer, Value, from_str, to_string};
use siphasher::sip128::{Hasher128, SipHasher24};

use rc_conf::RcConf;
use shvar::VariableProvider;
use utf8path::Path;

///////////////////////////////////////////// constants ////////////////////////////////////////////

const K8SIGNORE: &str = ".k8srcignore";

const SERVICE_DEFAULT_YAML: &str = r#"apiVersion: apps/v1
kind: Deployment
metadata:
  name: ${SERVICE:?SERVICE not defined}
  namespace: ${NAMESPACE:?NAMESPACE not defined}
  labels:
    app: ${SERVICE:?SERVICE not defined}
spec:
  replicas: ${REPLICAS:?}
  selector:
    matchLabels:
      app: ${SERVICE:?SERVICE not defined}
  template:
    metadata:
      labels:
        app: ${SERVICE:?SERVICE not defined}
    spec:
      containers:
      - name: ${SERVICE:?SERVICE not defined}
        image: ${IMAGE:?IMAGE not defined}
        ports:
        - containerPort: ${PORT:?PORT not defined}
        envFrom:
        - configMapRef:
            name: ${RCVARS:?RCVARS not defined}
        env:
        - name: RCVAR_ARGV0
          value: ${SERVICE:?SERVICE not defined}
---
apiVersion: v1
kind: Service
metadata:
  name: ${SERVICE:?SERVICE not defined}
  namespace: ${NAMESPACE:?NAMESPACE not defined}
  labels:
    app: ${SERVICE:?SERVICE not defined}
spec:
  type: NodePort
  ports:
  - port: ${PORT:?PORT not defined}
    protocol: TCP
    targetPort: ${PORT:?PORT not defined}
  selector:
    app: ${SERVICE:?SERVICE not defined}
"#;

/// Return the built-in default service template.
pub fn default_service_template() -> &'static str {
    SERVICE_DEFAULT_YAML
}

/////////////////////////////////////////////// Error //////////////////////////////////////////////

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    NonUtf8Path(std::path::PathBuf),
    ParseIntError(std::num::ParseIntError),
    RcConf(rc_conf::Error),
    Shvar(shvar::Error),
    SerdeYaml(serde_yaml::Error),
    InvalidCurrentDirectory,
    ManifestsDirectoryExists,
    ManifestExists(Path<'static>),
    ManifestMissing(Path<'static>),
    BadOptions(String),
    VerificationError(Path<'static>),
    MissingImage {
        service: String,
    },
    MissingRequiredVariable {
        service: String,
        key: String,
        rc_conf_path: String,
        output: Path<'static>,
        relative: Path<'static>,
    },
    InvalidRoot {
        root: Path<'static>,
        rc_conf: Path<'static>,
    },
    NoRcConf {
        root: Path<'static>,
    },
    MultipleTerminalRcConfs {
        rc_conf_path: String,
    },
    CommandFailed {
        service: String,
        rc_command: String,
        status: std::process::ExitStatus,
        stdout: String,
        stderr: String,
    },
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<std::num::ParseIntError> for Error {
    fn from(err: std::num::ParseIntError) -> Self {
        Self::ParseIntError(err)
    }
}

impl From<rc_conf::Error> for Error {
    fn from(err: rc_conf::Error) -> Self {
        Self::RcConf(err)
    }
}

impl From<shvar::Error> for Error {
    fn from(err: shvar::Error) -> Self {
        Self::Shvar(err)
    }
}

impl From<serde_yaml::Error> for Error {
    fn from(err: serde_yaml::Error) -> Self {
        Self::SerdeYaml(err)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(err) => write!(f, "IO error: {err}"),
            Error::NonUtf8Path(path) => write!(f, "non-UTF8 path: {}", path.to_string_lossy()),
            Error::ParseIntError(err) => write!(f, "parse int error: {err}"),
            Error::RcConf(err) => write!(f, "rc.conf error: {err}"),
            Error::Shvar(err) => write!(f, "shvar error: {err}"),
            Error::SerdeYaml(err) => write!(f, "YAML error: {err}"),
            Error::InvalidCurrentDirectory => write!(f, "current directory unavailable"),
            Error::ManifestsDirectoryExists => {
                write!(
                    f,
                    "manifests output directory exists and overwrite is disabled"
                )
            }
            Error::ManifestExists(path) => {
                write!(
                    f,
                    "manifest already exists and overwrite is disabled: {path}"
                )
            }
            Error::ManifestMissing(path) => {
                write!(f, "manifest is missing for verification: {path}")
            }
            Error::BadOptions(message) => write!(f, "{message}"),
            Error::VerificationError(path) => {
                write!(f, "manifest contents differ during verification: {path}")
            }
            Error::MissingImage { service } => {
                write!(f, "missing IMAGE configuration for service {service}")
            }
            Error::MissingRequiredVariable { key, .. } => write!(f, "{key} is required"),
            Error::InvalidRoot { root, rc_conf } => {
                write!(f, "rc_conf {rc_conf} is not under root {root}")
            }
            Error::NoRcConf { root } => write!(f, "no rc.conf found under root {root}"),
            Error::MultipleTerminalRcConfs { .. } => {
                write!(
                    f,
                    "multiple terminal rc.conf files found; run from one overlay root"
                )
            }
            Error::CommandFailed {
                rc_command, status, ..
            } => write!(f, "IMAGE_RCVAR command {rc_command:?} failed with {status}"),
        }
    }
}

impl std::error::Error for Error {}

pub fn error_code(err: &Error) -> Option<&'static str> {
    Some(match err {
        Error::Io(_) => "io-error",
        Error::NonUtf8Path(_) => "non-utf8-path",
        Error::ParseIntError(_) | Error::RcConf(_) | Error::Shvar(_) | Error::SerdeYaml(_) => {
            "parse-error"
        }
        Error::InvalidCurrentDirectory | Error::InvalidRoot { .. } | Error::NoRcConf { .. } => {
            "invalid-root"
        }
        Error::ManifestsDirectoryExists | Error::ManifestExists(_) | Error::BadOptions(_) => {
            "invalid-options"
        }
        Error::ManifestMissing(_) | Error::VerificationError(_) => "verification-mismatch",
        Error::MissingImage { .. } | Error::MissingRequiredVariable { .. } => {
            "missing-required-variable"
        }
        Error::MultipleTerminalRcConfs { .. } => "invalid-options",
        Error::CommandFailed { .. } => "command-failed",
    })
}

pub fn error_message(err: &Error) -> Option<String> {
    Some(match err {
        Error::NonUtf8Path(_) => "non-UTF8 directory entry path encountered".to_string(),
        Error::MissingImage { .. } => "IMAGE is required".to_string(),
        Error::MissingRequiredVariable { key, .. } => format!("{key} is required"),
        Error::InvalidRoot { .. } => "rc_conf is not under root".to_string(),
        Error::NoRcConf { .. } => "no rc.conf found under root".to_string(),
        Error::MultipleTerminalRcConfs { .. } => {
            "multiple terminal rc.conf files found; run from one overlay root".to_string()
        }
        Error::CommandFailed { .. } => "IMAGE_RCVAR command failed".to_string(),
        _ => err.to_string(),
    })
}

pub fn error_string_field(err: &Error, name: &str) -> Option<String> {
    match (err, name) {
        (Error::NonUtf8Path(path), "path") => Some(path.to_string_lossy().into_owned()),
        (Error::ManifestExists(path), "output")
        | (Error::ManifestMissing(path), "output")
        | (Error::VerificationError(path), "output") => Some(path.as_str().to_string()),
        (Error::BadOptions(message), "message") => Some(message.clone()),
        (Error::MissingImage { service }, "service") => Some(service.clone()),
        (Error::MissingImage { .. }, "key") => Some("IMAGE".to_string()),
        (
            Error::MissingRequiredVariable {
                service,
                key,
                rc_conf_path,
                output,
                relative,
            },
            field,
        ) => match field {
            "service" => Some(service.clone()),
            "key" => Some(key.clone()),
            "rc_conf_path" => Some(rc_conf_path.clone()),
            "output" => Some(output.as_str().to_string()),
            "relative" => Some(relative.as_str().to_string()),
            _ => None,
        },
        (Error::InvalidRoot { root, .. }, "path") => Some(root.as_str().to_string()),
        (Error::InvalidRoot { rc_conf, .. }, "rc_conf_path") => Some(rc_conf.as_str().to_string()),
        (Error::NoRcConf { root }, "path") => Some(root.as_str().to_string()),
        (Error::MultipleTerminalRcConfs { rc_conf_path }, "rc_conf_path") => {
            Some(rc_conf_path.clone())
        }
        (Error::CommandFailed { service, .. }, "service") => Some(service.clone()),
        (Error::CommandFailed { rc_command, .. }, "rc_command") => Some(rc_command.clone()),
        (Error::CommandFailed { status, .. }, "exit_status") => Some(status.to_string()),
        (Error::CommandFailed { stdout, .. }, "stdout") => Some(stdout.clone()),
        (Error::CommandFailed { stderr, .. }, "stderr") => Some(stderr.clone()),
        (Error::CommandFailed { .. }, "context") => Some("running IMAGE_RCVAR command".to_string()),
        _ => None,
    }
}

////////////////////////////////////////////// rewrite /////////////////////////////////////////////

pub fn rewrite(rc_conf: &RcConf, service: &str, yaml: &str) -> Result<String, Error> {
    let vp = rc_conf.variable_provider_for(service)?;
    _rewrite(&vp, yaml)
}

fn _rewrite(vp: &impl VariableProvider, yaml: &str) -> Result<String, Error> {
    let mut docs = vec![];
    for doc in Deserializer::from_str(yaml) {
        let value = Value::deserialize(doc)?;
        docs.push(value);
    }
    let docs = docs
        .into_iter()
        .map(|y| transform(vp, y))
        .collect::<Result<Vec<_>, _>>()?;
    let mut out = String::new();
    for doc in docs.into_iter().flatten() {
        out += &to_string(&doc)?;
    }
    Ok(out)
}

fn transform(vp: &dyn VariableProvider, yaml: Value) -> Result<Option<Value>, Error> {
    fn transform_vec(vp: &dyn VariableProvider, yaml: Vec<Value>) -> Result<Option<Value>, Error> {
        let yaml = yaml
            .into_iter()
            .map(|y| transform(vp, y))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(Value::Sequence(yaml.into_iter().flatten().collect())))
    }

    fn transform_kv(
        vp: &dyn VariableProvider,
        k: Value,
        v: Value,
    ) -> Result<Option<(Value, Value)>, Error> {
        let k = transform(vp, k)?;
        let v = transform(vp, v)?;
        if let (Some(k), Some(v)) = (k, v) {
            Ok(Some((k, v)))
        } else {
            Ok(None)
        }
    }

    fn transform_hash(
        vp: &dyn VariableProvider,
        yaml: impl Iterator<Item = (Value, Value)>,
    ) -> Result<Option<Value>, Error> {
        let yaml = yaml
            .into_iter()
            .map(|(k, v)| transform_kv(vp, k, v))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(Value::Mapping(yaml.into_iter().flatten().collect())))
    }

    match yaml {
        Value::String(s) => match shvar::expand_recursive(vp, &s) {
            Ok(expanded) => {
                let pieces = shvar::split(&expanded)?;
                let quoted = shvar::quote(pieces);
                let value: Value = from_str(&quoted)?;
                Ok(Some(value))
            }
            Err(shvar::Error::Requested(msg)) => {
                if msg.is_empty() {
                    Ok(None)
                } else {
                    Err(shvar::Error::Requested(msg).into())
                }
            }
            Err(err) => Err(err.into()),
        },
        Value::Sequence(a) => transform_vec(vp, a),
        Value::Mapping(h) => transform_hash(vp, h.into_iter()),
        _ => Ok(Some(yaml)),
    }
}

///////////////////////////////////////// RegenerateOptions ////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[cfg_attr(feature = "command_line", derive(arrrg_derive::CommandLine))]
pub struct RegenerateOptions {
    #[cfg_attr(
        feature = "command_line",
        arrrg(optional, "Root of the k8src repository.")
    )]
    pub root: Option<String>,
    #[cfg_attr(feature = "command_line", arrrg(optional, "Root of the k8src output."))]
    pub output: Option<String>,
    #[cfg_attr(feature = "command_line", arrrg(flag, "Overwrite the existing files."))]
    pub overwrite: bool,
    #[cfg_attr(
        feature = "command_line",
        arrrg(flag, "Verify the existing files rather than generating.")
    )]
    pub verify: bool,
    #[cfg_attr(
        feature = "command_line",
        arrrg(flag, "Print generated manifest paths without writing files.")
    )]
    pub dry_run: bool,
    #[cfg_attr(
        feature = "command_line",
        arrrg(
            flag,
            "Print unified diffs for changed manifests without writing files."
        )
    )]
    pub diff: bool,
}

//////////////////////////////////////////// regenerate ////////////////////////////////////////////

fn line_count(text: &str) -> usize {
    if text.is_empty() {
        0
    } else {
        text.lines().count()
    }
}

fn unified_diff(path: &str, old: Option<&str>, new: &str) -> String {
    let old_path = if old.is_some() { path } else { "/dev/null" };
    let old = old.unwrap_or("");
    if old == new {
        return String::new();
    }
    let mut out = String::new();
    out += &format!("--- {old_path}\n");
    out += &format!("+++ {path}\n");
    out += &format!("@@ -1,{} +1,{} @@\n", line_count(old), line_count(new));
    for line in old.lines() {
        out += &format!("-{line}\n");
    }
    for line in new.lines() {
        out += &format!("+{line}\n");
    }
    out
}

fn write_yaml(
    options: &RegenerateOptions,
    output: Path,
    yaml: &str,
    tracking: &mut Vec<Path>,
) -> Result<(), Error> {
    if options.verify && options.overwrite {
        Err(Error::BadOptions(
            "--verify and --overwrite are mutually exclusive".to_string(),
        ))
    } else if options.verify {
        if !output.exists()? {
            return Err(Error::ManifestMissing(output.into_owned()));
        }
        let returned = yaml;
        let expected = std::fs::read_to_string(&output)?;
        if expected != returned {
            Err(Error::VerificationError(output.into_owned()))
        } else {
            tracking.push(output.into_owned());
            Ok(())
        }
    } else if options.diff {
        let expected = if output.exists()? {
            Some(std::fs::read_to_string(&output)?)
        } else {
            None
        };
        let diff = unified_diff(output.as_str(), expected.as_deref(), yaml);
        if !diff.is_empty() {
            print!("{diff}");
        }
        tracking.push(output.into_owned());
        Ok(())
    } else if options.dry_run {
        println!("would generate {}", output.as_str());
        tracking.push(output.into_owned());
        Ok(())
    } else {
        if output.exists()? && !options.overwrite {
            return Err(Error::ManifestExists(output.into_owned()));
        }
        std::fs::create_dir_all(output.dirname())?;
        std::fs::write(&output, yaml)?;
        tracking.push(output.into_owned());
        Ok(())
    }
}

pub fn regenerate(options: RegenerateOptions) -> Result<(), Error> {
    let root = if let Some(root) = options.root.as_ref() {
        Path::from(root)
    } else {
        Path::cwd().ok_or(Error::InvalidCurrentDirectory)?
    };
    let output = if let Some(output) = options.output.as_ref() {
        Path::from(output)
    } else {
        root.join("manifests")
    };
    if !options.verify
        && !options.overwrite
        && !options.dry_run
        && !options.diff
        && output.exists()?
    {
        return Err(Error::ManifestsDirectoryExists);
    }
    if options.overwrite && !options.dry_run && !options.diff && output.exists()? {
        std::fs::remove_dir_all(&output)?;
    }
    let rc_confs = restrict_to_terminals(find_rc_confs(&root)?);
    for rc_conf in rc_confs.into_iter() {
        let candidates = candidates(&root, &rc_conf)?;
        let Some(relative) = candidates[candidates.len() - 1].strip_prefix(root.as_str()) else {
            panic!("there's a logic error; this should be unreachable");
        };
        let relative = Path::from(relative.as_str().trim_start_matches('/'));
        let rc_conf_path = rc_conf_path(&candidates);
        let rc_conf = RcConf::parse(&rc_conf_path)?;
        let mut root_yaml = r#"apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization
resources:
"#
        .to_string();
        let mut extended = false;
        let mut tracking = vec![];
        for service in rc_conf.list_services()? {
            let Some(image) = rc_conf.lookup_suffix(&service, "IMAGE") else {
                return Err(Error::MissingRequiredVariable {
                    service,
                    key: "IMAGE".to_string(),
                    rc_conf_path: rc_conf_path.clone(),
                    output: output.clone().into_owned(),
                    relative: relative.clone().into_owned(),
                });
            };
            let extra =
                HashMap::from_iter([("IMAGE", image.clone()), ("RCVAR_ARGV0", service.clone())]);
            let rcvar = rc_conf.argv(&service, "IMAGE_RCVAR", &extra)?;
            let rcvars = if !rcvar.is_empty() {
                let rc_command = rcvar.join(" ");
                let rcvar = std::process::Command::new(&rcvar[0])
                    .args(&rcvar[1..])
                    .output()?;
                if !rcvar.status.success() {
                    return Err(Error::CommandFailed {
                        service,
                        rc_command,
                        status: rcvar.status,
                        stdout: String::from_utf8_lossy(&rcvar.stdout).into_owned(),
                        stderr: String::from_utf8_lossy(&rcvar.stderr).into_owned(),
                    });
                }
                let rckeys = String::from_utf8_lossy(&rcvar.stdout);
                let mut rckeys = rckeys
                    .split_whitespace()
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>();
                rckeys.sort();
                let mut rcvars = rc_conf
                    .generate_rcvars(&service, &rckeys)?
                    .into_iter()
                    .collect::<Vec<_>>();
                rcvars.sort();
                rcvars
            } else {
                vec![]
            };
            let mut yaml = match template_for_service(&candidates, &rc_conf, &service) {
                Some(template) => std::fs::read_to_string(template)?,
                None => SERVICE_DEFAULT_YAML.to_string(),
            };
            const MAGIC_KEY: &[u8; 16] = &[
                173, 14, 53, 145, 150, 207, 208, 116, 119, 25, 149, 255, 4, 53, 29, 50,
            ];
            let mut hasher = SipHasher24::new_with_key(MAGIC_KEY);
            service.hash(&mut hasher);
            rcvars.hash(&mut hasher);
            let sig = hasher.finish128().as_u128();
            let mut config_map = format!(
                r#"apiVersion: v1
kind: ConfigMap
metadata:
  name: config-map-{sig}
  namespace: ${{NAMESPACE:?NAMESPACE not defined}}
data:
"#
            );
            for (key, value) in rcvars {
                config_map += &format!("  {key}: {value:?}");
            }
            yaml += "---\n";
            yaml += &config_map;
            let rcvp = rc_conf.variable_provider_for(&service)?;
            let locals = HashMap::from_iter([
                ("SERVICE", service.clone()),
                ("RCVARS", format!("config-map-{sig}")),
            ]);
            let vp = (&locals, &rcvp);
            let yaml = _rewrite(&vp, &yaml)?;
            let output = output.join(Path::from(format!("{relative}/herd/{service}.yaml")));
            write_yaml(&options, output, &yaml, &mut tracking)?;
            root_yaml += &format!("- {service}.yaml\n");
            extended = true;
        }
        if extended {
            write_yaml(
                &options,
                output.join(Path::from(format!("{relative}/herd/kustomization.yaml"))),
                &root_yaml,
                &mut tracking,
            )?;
        }
        let mut have_pets = false;
        for candidate in candidates.iter().rev() {
            let pets = candidate.join("pets");
            if !pets.exists()? {
                continue;
            }
            fn copy_pets_from_dir(
                options: &RegenerateOptions,
                root: &Path,
                output: &Path,
                pets: &Path,
                tracking: &mut Vec<Path>,
            ) -> Result<bool, Error> {
                let mut copied = false;
                for pet in std::fs::read_dir(pets)? {
                    let pet = pet?;
                    let pet =
                        Path::try_from(pet.path()).map_err(|_| Error::NonUtf8Path(pet.path()))?;
                    if pet.join(K8SIGNORE).exists()? {
                        continue;
                    }
                    if pet.is_dir()? {
                        copied |= copy_pets_from_dir(options, root, output, &pet, tracking)?;
                        continue;
                    }
                    if !pet.as_str().ends_with(".yaml") {
                        eprintln!("skipping pet {pet:?}");
                        continue;
                    }
                    let Some(relative) = pet.strip_prefix(root.as_str()) else {
                        panic!("there's a logic error; this should be unreachable");
                    };
                    let relative = Path::from(relative.as_str().trim_start_matches('/'));
                    let source = pet.clone();
                    let output = output.join(Path::from(format!("{relative}")));
                    write_yaml(options, output, &std::fs::read_to_string(source)?, tracking)?;
                    copied = true;
                }
                Ok(copied)
            }
            have_pets |= copy_pets_from_dir(&options, &root, &output, &pets, &mut tracking)?;
        }
        if extended && have_pets {
            write_yaml(
                &options,
                output.join(Path::from(format!("{relative}/kustomization.yaml"))),
                r#"apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization
resources:
  - herd
  - pets
"#,
                &mut tracking,
            )?;
        } else if extended {
            write_yaml(
                &options,
                output.join(Path::from(format!("{relative}/kustomization.yaml"))),
                r#"apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization
resources:
  - herd
"#,
                &mut tracking,
            )?;
        } else if have_pets {
            write_yaml(
                &options,
                output.join(Path::from(format!("{relative}/kustomization.yaml"))),
                r#"apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization
resources:
  - pets
"#,
                &mut tracking,
            )?;
        }
    }
    Ok(())
}

fn find_rc_confs(root: &Path) -> Result<Vec<Path<'static>>, Error> {
    find_rc_confs_inner(root, true)
}

fn find_rc_confs_inner(root: &Path, is_root: bool) -> Result<Vec<Path<'static>>, Error> {
    let mut paths = vec![];
    if !is_root && root.join(K8SIGNORE).exists()? {
        return Ok(paths);
    }
    for dirent in std::fs::read_dir(root)? {
        let dirent = dirent?;
        let path = Path::try_from(dirent.path()).map_err(|_| Error::NonUtf8Path(dirent.path()))?;
        if dirent.file_name() == "rc.conf" {
            paths.push(path.clone());
        }
        if dirent.file_type()?.is_dir() {
            let children = find_rc_confs_inner(&path, false)?;
            paths.extend(children);
        }
    }
    Ok(paths)
}

fn restrict_to_terminals(mut rc_confs: Vec<Path<'static>>) -> Vec<Path<'static>> {
    rc_confs.sort_by_key(|rc_conf| rc_conf.as_str().len());
    let mut restricted: Vec<Path> = vec![];
    for rc_conf in rc_confs.into_iter().rev() {
        fn is_parent_rc_conf(parent: &Path, child: &Path) -> bool {
            let parent = parent.dirname();
            let mut child = child.clone();
            while child.components().count() > parent.components().count() {
                child = child.dirname().into_owned();
            }
            child == parent
        }
        if !restricted.iter().any(|r| is_parent_rc_conf(&rc_conf, r)) {
            restricted.push(rc_conf);
        }
    }
    restricted
}

pub fn candidates(root: &Path, rc_conf: &Path) -> Result<Vec<Path<'static>>, Error> {
    if !rc_conf.as_str().starts_with(root.as_str()) {
        return Err(Error::InvalidRoot {
            root: root.clone().into_owned(),
            rc_conf: rc_conf.clone().into_owned(),
        });
    }
    let mut rc_conf = rc_conf.dirname();
    let mut candidates = vec![];
    while rc_conf != *root {
        candidates.push(rc_conf.clone().into_owned());
        rc_conf = rc_conf.dirname().into_owned();
    }
    candidates.push(root.clone().into_owned());
    candidates.reverse();
    Ok(candidates)
}

pub fn rc_conf_path(candidates: &[Path]) -> String {
    let mut rc_conf_path = String::new();
    for candidate in candidates {
        if !rc_conf_path.is_empty() {
            rc_conf_path.push(':');
        }
        rc_conf_path += candidate.join("rc.conf").as_str();
    }
    rc_conf_path
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateCandidate {
    pub path: String,
    pub exists: bool,
    pub service: Option<String>,
    pub kind: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateResolution {
    pub selected: Option<String>,
    pub fallback_chain: Vec<TemplateCandidate>,
    pub uses_builtin_default: bool,
}

pub fn template_resolution(
    candidates: &[Path],
    rc_conf: &RcConf,
    service: &str,
) -> TemplateResolution {
    let mut service = service.to_string();
    let mut fallback_chain = vec![];
    loop {
        for candidate in candidates.iter().rev() {
            let candidate = candidate
                .join("rc.d")
                .join(format!("{service}.yaml.template"));
            let path = candidate.as_str().to_string();
            let exists = candidate.exists().unwrap_or(false);
            fallback_chain.push(TemplateCandidate {
                path: path.clone(),
                exists,
                service: Some(service.clone()),
                kind: "service-specific".to_string(),
            });
            if exists {
                return TemplateResolution {
                    selected: Some(path),
                    fallback_chain,
                    uses_builtin_default: false,
                };
            }
        }
        let direct_alias = rc_conf.direct_alias(&service);
        if direct_alias == service {
            break;
        } else {
            service = direct_alias.to_string();
        }
    }
    for candidate in candidates.iter().rev() {
        let candidate = candidate.join("service.yaml.template");
        let path = candidate.as_str().to_string();
        let exists = candidate.exists().unwrap_or(false);
        fallback_chain.push(TemplateCandidate {
            path: path.clone(),
            exists,
            service: None,
            kind: "default".to_string(),
        });
        if exists {
            return TemplateResolution {
                selected: Some(path),
                fallback_chain,
                uses_builtin_default: false,
            };
        }
    }
    TemplateResolution {
        selected: None,
        fallback_chain,
        uses_builtin_default: true,
    }
}

fn template_for_service(
    candidates: &[Path],
    rc_conf: &RcConf,
    service: &str,
) -> Option<Path<'static>> {
    template_resolution(candidates, rc_conf, service)
        .selected
        .map(Path::from)
}

fn explain_context(root: &Path) -> Result<(Vec<Path<'static>>, String, RcConf), Error> {
    let rc_confs = restrict_to_terminals(find_rc_confs(root)?);
    if rc_confs.is_empty() {
        return Err(Error::NoRcConf {
            root: root.clone().into_owned(),
        });
    }
    if rc_confs.len() != 1 {
        let rc_conf_path = rc_confs
            .iter()
            .map(|p| p.as_str())
            .collect::<Vec<_>>()
            .join(":");
        return Err(Error::MultipleTerminalRcConfs { rc_conf_path });
    }
    let candidates = candidates(root, &rc_confs[0])?;
    let rc_conf_chain = rc_conf_path(&candidates);
    let rc_conf = RcConf::parse(&rc_conf_chain)?;
    Ok((candidates, rc_conf_chain, rc_conf))
}

pub fn explain_template(root: Option<&str>, service: &str) -> Result<String, Error> {
    let root = if let Some(root) = root {
        Path::from(root)
    } else {
        Path::cwd().ok_or(Error::InvalidCurrentDirectory)?
    };
    let (candidates, rc_conf_chain, rc_conf) = explain_context(&root)?;
    let resolution = template_resolution(&candidates, &rc_conf, service);
    let selected = resolution
        .selected
        .as_deref()
        .unwrap_or("built-in default template");
    let mut out = String::new();
    out += &format!("service: {service}\n");
    out += &format!("rc_conf_path: {rc_conf_chain}\n");
    out += &format!("selected: {selected}\n");
    out += "fallback_chain:\n";
    for candidate in resolution.fallback_chain {
        let marker = if candidate.exists { "found" } else { "missing" };
        out += &format!("  - {marker} {} ({})\n", candidate.path, candidate.kind);
    }
    if resolution.uses_builtin_default {
        out += "  - found built-in default template\n";
    }
    Ok(out)
}

pub fn explain_vars(root: Option<&str>, service: &str) -> Result<String, Error> {
    let root = if let Some(root) = root {
        Path::from(root)
    } else {
        Path::cwd().ok_or(Error::InvalidCurrentDirectory)?
    };
    let (_candidates, rc_conf_chain, rc_conf) = explain_context(&root)?;
    let (alias_order, pre_lookup) = rc_conf.alias_lookup_order(service);
    let vp = rc_conf.variable_provider_for(service)?;
    let known_prefixes = rc_conf
        .list()?
        .map(|service| rc_conf::var_prefix_from_service(&service))
        .collect::<Vec<_>>();
    let alias_prefixes = alias_order
        .iter()
        .map(|service| rc_conf::var_prefix_from_service(service))
        .collect::<Vec<_>>();
    let mut suffixes = BTreeSet::new();
    for raw in rc_conf.variables() {
        let mut matched = false;
        for prefix in &alias_prefixes {
            if let Some(suffix) = raw.strip_prefix(prefix) {
                suffixes.insert(suffix.to_string());
                matched = true;
            }
        }
        if matched {
            continue;
        }
        if known_prefixes.iter().any(|prefix| raw.starts_with(prefix)) {
            continue;
        }
        suffixes.insert(raw);
    }
    suffixes.extend(pre_lookup.keys().cloned());
    let mut out = String::new();
    out += &format!("service: {service}\n");
    out += &format!("rc_conf_path: {rc_conf_chain}\n");
    out += &format!("alias_order: {}\n", alias_order.join(" -> "));
    out += "variables:\n";
    for suffix in suffixes {
        if let Some(value) = vp.lookup(&suffix) {
            let value = shvar::expand_recursive(&vp, &value).unwrap_or(value);
            out += &format!("  {suffix}={}\n", shvar::quote_string(&value));
        }
    }
    Ok(out)
}
