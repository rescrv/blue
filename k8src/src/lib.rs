use std::collections::HashMap;

use yaml_rust::{Yaml, YamlEmitter, YamlLoader};

use rc_conf::{RcConf, SwitchPosition};
use shvar::VariableProvider;
use utf8path::Path;

///////////////////////////////////////////// constants ////////////////////////////////////////////

const SERVICE_DEFAULT_YAML: &str = r#"
"#;

/////////////////////////////////////////////// Error //////////////////////////////////////////////

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    NonUtf8Path(std::path::PathBuf),
    ParseIntError(std::num::ParseIntError),
    RcConf(rc_conf::Error),
    Shvar(shvar::Error),
    YamlScan(yaml_rust::ScanError),
    YamlEmit(yaml_rust::EmitError),
    InvalidCurrentDirectory,
    ManifestsDirectoryExists,
    ManifestExists(Path<'static>),
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

impl From<yaml_rust::ScanError> for Error {
    fn from(err: yaml_rust::ScanError) -> Self {
        Self::YamlScan(err)
    }
}

impl From<yaml_rust::EmitError> for Error {
    fn from(err: yaml_rust::EmitError) -> Self {
        Self::YamlEmit(err)
    }
}

////////////////////////////////////////////// rewrite /////////////////////////////////////////////

pub fn rewrite(rc_conf: &RcConf, service: &str, yaml: &str) -> Result<String, Error> {
    let vp = rc_conf.variable_provider_for(service)?;
    _rewrite(&vp, yaml)
}

fn _rewrite(vp: &impl VariableProvider, yaml: &str) -> Result<String, Error> {
    let yaml = YamlLoader::load_from_str(yaml)?;
    let yaml = yaml
        .into_iter()
        .map(|y| transform(&vp, y))
        .collect::<Result<Vec<_>, _>>()?;
    let yaml = yaml.into_iter().flatten().collect::<Vec<_>>();
    let mut out = String::new();
    for obj in yaml {
        let mut emitter = YamlEmitter::new(&mut out);
        emitter.dump(&obj)?;
        if !out.ends_with('\n') {
            out.push('\n');
        }
    }
    Ok(out)
}

fn transform(vp: &dyn VariableProvider, yaml: Yaml) -> Result<Option<Yaml>, Error> {
    fn transform_vec(vp: &dyn VariableProvider, yaml: Vec<Yaml>) -> Result<Option<Yaml>, Error> {
        let yaml = yaml
            .into_iter()
            .map(|y| transform(vp, y))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(Yaml::Array(yaml.into_iter().flatten().collect())))
    }

    fn transform_kv(
        vp: &dyn VariableProvider,
        k: Yaml,
        v: Yaml,
    ) -> Result<Option<(Yaml, Yaml)>, Error> {
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
        yaml: impl Iterator<Item = (Yaml, Yaml)>,
    ) -> Result<Option<Yaml>, Error> {
        let yaml = yaml
            .into_iter()
            .map(|(k, v)| transform_kv(vp, k, v))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(Yaml::Hash(yaml.into_iter().flatten().collect())))
    }

    match yaml {
        Yaml::String(s) => match shvar::expand(vp, &s) {
            Ok(expanded) => Ok(Some(Yaml::from_str(&expanded))),
            Err(shvar::Error::Requested(msg)) => {
                if msg.is_empty() {
                    Ok(None)
                } else {
                    Err(shvar::Error::Requested(msg).into())
                }
            }
            Err(err) => Err(err.into()),
        },
        Yaml::Array(a) => transform_vec(vp, a),
        Yaml::Hash(h) => transform_hash(vp, h.into_iter()),
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
    root: Option<String>,
}

//////////////////////////////////////////// regenerate ////////////////////////////////////////////

pub fn regenerate(options: RegenerateOptions) -> Result<(), Error> {
    let root = if let Some(root) = options.root.as_ref() {
        Path::from(root)
    } else {
        Path::cwd().ok_or(Error::InvalidCurrentDirectory)?
    };
    if Path::from("manifests").exists() {
        return Err(Error::ManifestsDirectoryExists);
    }
    let rc_confs = restrict_to_terminals(find_rc_confs(&root)?);
    for rc_conf in rc_confs.into_iter() {
        let candidates = candidates(&root, &rc_conf);
        let rc_conf_path = rc_conf_path(&candidates);
        let rc_conf = RcConf::parse(&rc_conf_path)?;
        for var in rc_conf.variables() {
            if let Some(service) = var.strip_suffix("_ENABLED") {
                let service = rc_conf::service_from_var_name(service);
                match rc_conf.service_switch(&service) {
                    SwitchPosition::Yes => {}
                    SwitchPosition::No => {
                        eprintln!("{service:?} disabled");
                        continue;
                    }
                    SwitchPosition::Manual => {
                        todo!();
                    }
                };
                let yaml = match template_for_service(&candidates, &service) {
                    Some(template) => std::fs::read_to_string(template)?,
                    None => SERVICE_DEFAULT_YAML.to_string(),
                };
                let rcvp = rc_conf.variable_provider_for(&service)?;
                let locals = HashMap::from_iter([("SERVICE", &service)]);
                let vp = (&locals, &rcvp);
                let yaml = _rewrite(&vp, &yaml)?;
                let Some(relative) = candidates[candidates.len() - 1].strip_prefix(root.as_str())
                else {
                    panic!("there's a logic error; this should be unreachable");
                };
                let output = Path::from(format!("manifests/{relative}/herd/{service}.yaml",));
                if output.exists() {
                    return Err(Error::ManifestExists(output.into_owned()));
                }
                std::fs::create_dir_all(output.dirname())?;
                std::fs::write(output, yaml)?;
            }
        }
        for candidate in candidates.iter().rev() {
            let pets = candidate.join("pets");
            if !pets.exists() {
                eprintln!("skipping pets in {pets:?}");
                continue;
            }
            fn copy_pets_from_dir(root: &Path, pets: &Path) -> Result<(), Error> {
                for pet in std::fs::read_dir(pets)? {
                    let pet = pet?;
                    let pet =
                        Path::try_from(pet.path()).map_err(|_| Error::NonUtf8Path(pet.path()))?;
                    if pet.is_dir() {
                        copy_pets_from_dir(root, &pet)?;
                        continue;
                    }
                    if !pet.as_str().ends_with(".yaml") {
                        eprintln!("skipping pet {pet:?}");
                        continue;
                    }
                    let Some(relative) = pet.strip_prefix(root.as_str()) else {
                        panic!("there's a logic error; this should be unreachable");
                    };
                    let source = pet.clone();
                    let Some(pet) = pet.strip_prefix(root.as_str()) else {
                        panic!("there's a logic error; this should be unreachable");
                    };
                    let output = Path::from(format!("manifests/{relative}/{pet}",));
                    if output.exists() {
                        return Err(Error::ManifestExists(output.into_owned()));
                    }
                    std::fs::create_dir_all(output.dirname())?;
                    std::fs::copy(source, output)?;
                }
                Ok(())
            }
            copy_pets_from_dir(&root, &pets)?;
        }
    }
    Ok(())
}

fn find_rc_confs(root: &Path) -> Result<Vec<Path<'static>>, Error> {
    let mut paths = vec![];
    for dirent in std::fs::read_dir(root)? {
        let dirent = dirent?;
        let path = Path::try_from(dirent.path()).map_err(|_| Error::NonUtf8Path(dirent.path()))?;
        if dirent.file_name() == "rc.conf" {
            paths.push(path.clone());
        }
        if dirent.file_type()?.is_dir() {
            let children = find_rc_confs(&path)?;
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

fn candidates(root: &Path, rc_conf: &Path) -> Vec<Path<'static>> {
    assert!(rc_conf.as_str().starts_with(root.as_str()));
    let mut rc_conf = rc_conf.dirname();
    let mut candidates = vec![];
    while rc_conf != *root {
        candidates.push(rc_conf.clone().into_owned());
        rc_conf = rc_conf.dirname().into_owned();
    }
    candidates.push(root.clone().into_owned());
    candidates.reverse();
    candidates
}

fn rc_conf_path(candidates: &[Path]) -> String {
    let mut rc_conf_path = String::new();
    for candidate in candidates {
        if !rc_conf_path.is_empty() {
            rc_conf_path.push(':');
        }
        rc_conf_path += candidate.join("rc.conf").as_str();
    }
    rc_conf_path
}

fn template_for_service(candidates: &[Path], service: &str) -> Option<Path<'static>> {
    for candidate in candidates.iter().rev() {
        let candidate = candidate
            .join("templates/rc.d")
            .join(format!("{service}.yaml.template"));
        if candidate.exists() {
            return Some(candidate.into_owned());
        }
    }
    for candidate in candidates.iter().rev() {
        let candidate = candidate.join("templates/service.yaml.template");
        if candidate.exists() {
            return Some(candidate.into_owned());
        }
    }
    None
}
