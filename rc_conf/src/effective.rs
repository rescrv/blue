//! The effective, fully-bound view of each service:  switch position, stub, bound environment,
//! and wrapper.  [diff_effective] compares two such views (rcdiff); [plan_invoke] computes exactly
//! what [crate::invoke] would exec (rcinvoke --dry-run).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use utf8path::Path;

use crate::{RcConf, SwitchPosition, load_services, var_name_from_service};

///////////////////////////////////////////// Invocation ///////////////////////////////////////////

/// A fully-planned exec:  the environment added to the inherited environment, the program, and
/// its arguments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Invocation {
    /// Variables added to (not replacing) the caller's environment.
    pub env: BTreeMap<String, String>,
    /// The program to exec.
    pub program: String,
    /// Arguments after the program.
    pub args: Vec<String>,
}

impl Invocation {
    /// Render as a copy-pasteable `env K=V ... program args` shell command.
    pub fn to_shell(&self) -> String {
        let mut pieces = vec!["env".to_string()];
        for (k, v) in self.env.iter() {
            pieces.push(format!("{k}={v}"));
        }
        pieces.push(self.program.clone());
        pieces.extend(self.args.iter().cloned());
        shvar::quote(pieces)
    }
}

/// Plan the exec that [crate::exec_rc] would perform for `service` with `cmd` passed to the stub.
/// On failure, returns the exit code exec_rc would use and a message.
pub fn plan_invoke(
    rc_conf_path: &str,
    rc_d_path: &str,
    service: &str,
    cmd: &[&str],
) -> Result<Invocation, (i32, String)> {
    let rc_conf =
        RcConf::parse(rc_conf_path).map_err(|e| (133, format!("failed to parse rc_conf: {e}")))?;
    let rc_d =
        load_services(rc_d_path).map_err(|e| (134, format!("failed to load services: {e}")))?;
    plan_invoke_with(&rc_conf, &rc_d, service, cmd)
}

pub(crate) fn plan_invoke_with(
    rc_conf: &RcConf,
    rc_d: &HashMap<String, Result<Path<'static>, String>>,
    service: &str,
    cmd: &[&str],
) -> Result<Invocation, (i32, String)> {
    if !rc_conf.service_switch(service).can_be_started() {
        return Err((132, "service not enabled".to_string()));
    }
    let path = if let Some(alias) = rc_conf.aliases.get(&var_name_from_service(service)) {
        rc_d.get(rc_conf.resolve_alias(&alias.aliases)).ok_or((
            130,
            "expected alias of service to be available via --rc-d-path".to_string(),
        ))?
    } else {
        rc_d.get(service).ok_or((
            130,
            "expected service to be available via --rc-d-path".to_string(),
        ))?
    };
    let path = match path {
        Ok(path) => path,
        Err(err) => return Err((131, format!("service encountered an error: {err:?}"))),
    };
    let mut env = rc_conf
        .bind_for_invoke(service, path)
        .map_err(|e| (135, format!("failed to bind variables for service: {e}")))?
        .into_iter()
        .collect::<BTreeMap<_, _>>();
    env.insert("RCVAR_ARGV0".to_string(), var_name_from_service(service));
    let wrapper = rc_conf
        .argv(service, "WRAPPER", &())
        .map_err(|e| (136, format!("failed to generate argv: {e}")))?;
    let (program, mut args) = if let Some((first, rest)) = wrapper.split_first() {
        let mut args = rest.to_vec();
        args.push(path.as_str().to_string());
        (first.clone(), args)
    } else {
        (path.as_str().to_string(), vec![])
    };
    args.extend(cmd.iter().map(|s| s.to_string()));
    Ok(Invocation { env, program, args })
}

////////////////////////////////////////////// Effective ///////////////////////////////////////////

/// Why a service's effective view could not be fully computed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EffectiveError {
    /// No executable rc.d stub for the service (or what it aliases).
    NoStub,
    /// The stub exists but is unusable (e.g. duplicated across rc.d directories).
    BadStub(String),
    /// Running the stub's rcvar or expanding a bound value failed.
    Bind(String),
    /// Expanding WRAPPER failed.
    Wrapper(String),
}

impl std::fmt::Display for EffectiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EffectiveError::NoStub => write!(f, "no rc.d stub"),
            EffectiveError::BadStub(e) => write!(f, "bad rc.d stub: {e}"),
            EffectiveError::Bind(e) => write!(f, "binding failed: {e}"),
            EffectiveError::Wrapper(e) => write!(f, "WRAPPER failed to expand: {e}"),
        }
    }
}

/// The effective configuration of one service.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Effective {
    /// The resolved `_ENABLED` switch.
    pub switch: SwitchPosition,
    /// The rc.d stub that would run.
    pub stub: Option<String>,
    /// The bound rc variables, as they'd be put in the environment.
    pub env: BTreeMap<String, String>,
    /// The expanded WRAPPER argv.
    pub wrapper: Vec<String>,
    /// Set if any part of the view could not be computed.
    pub error: Option<EffectiveError>,
}

/// Compute the effective view of every service rc_conf lists, plus every rc.d stub.  This runs
/// each stub's `rcvar` command.
pub fn effective(
    rc_conf: &RcConf,
    rc_d: &HashMap<String, Result<Path<'static>, String>>,
) -> BTreeMap<String, Effective> {
    let mut services = BTreeSet::new();
    if let Ok(list) = rc_conf.list() {
        services.extend(list);
    }
    services.extend(rc_d.keys().cloned());
    let mut out = BTreeMap::new();
    for service in services {
        let switch = rc_conf.service_switch(&service);
        let mut eff = Effective {
            switch,
            stub: None,
            env: BTreeMap::new(),
            wrapper: vec![],
            error: None,
        };
        match rc_d.get(rc_conf.resolve_alias(&service)) {
            None => eff.error = Some(EffectiveError::NoStub),
            Some(Err(e)) => eff.error = Some(EffectiveError::BadStub(e.clone())),
            Some(Ok(path)) => {
                eff.stub = Some(path.as_str().to_string());
                match rc_conf.bind_for_invoke(&service, path) {
                    Ok(env) => eff.env = env.into_iter().collect(),
                    Err(e) => eff.error = Some(EffectiveError::Bind(e.to_string())),
                }
            }
        }
        match rc_conf.argv(&service, "WRAPPER", &()) {
            Ok(w) => eff.wrapper = w,
            Err(e) => {
                if eff.error.is_none() {
                    eff.error = Some(EffectiveError::Wrapper(e.to_string()));
                }
            }
        }
        out.insert(service, eff);
    }
    out
}

/////////////////////////////////////////////// Change /////////////////////////////////////////////

/// One difference between two effective views.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Change {
    /// The service exists only in the new view.
    Added(String, Effective),
    /// The service exists only in the old view.
    Removed(String, Effective),
    /// The service exists in both and differs.
    Changed {
        /// The service.
        service: String,
        /// Switch before and after, if it changed.
        switch: Option<(SwitchPosition, SwitchPosition)>,
        /// Stub before and after, if it changed.
        stub: Option<(Option<String>, Option<String>)>,
        /// Per-variable (before, after); None means absent on that side.
        env: BTreeMap<String, (Option<String>, Option<String>)>,
        /// Wrapper before and after, if it changed.
        wrapper: Option<(Vec<String>, Vec<String>)>,
        /// Error before and after, if it changed.
        error: Option<(Option<EffectiveError>, Option<EffectiveError>)>,
    },
}

/// Diff two effective views.  Services that are disabled on both sides and otherwise unchanged
/// produce no output; services that are disabled on both sides are compared only by switch.
pub fn diff_effective(
    old: &BTreeMap<String, Effective>,
    new: &BTreeMap<String, Effective>,
) -> Vec<Change> {
    let mut changes = vec![];
    let names = old.keys().chain(new.keys()).collect::<BTreeSet<_>>();
    for name in names {
        match (old.get(name), new.get(name)) {
            (None, Some(n)) => {
                if n.switch != SwitchPosition::No {
                    changes.push(Change::Added(name.clone(), n.clone()));
                }
            }
            (Some(o), None) => {
                if o.switch != SwitchPosition::No {
                    changes.push(Change::Removed(name.clone(), o.clone()));
                }
            }
            (Some(o), Some(n)) => {
                let switch = (o.switch != n.switch).then_some((o.switch, n.switch));
                if o.switch == SwitchPosition::No && n.switch == SwitchPosition::No {
                    continue;
                }
                let stub = (o.stub != n.stub).then(|| (o.stub.clone(), n.stub.clone()));
                let mut env = BTreeMap::new();
                for k in o.env.keys().chain(n.env.keys()).collect::<BTreeSet<_>>() {
                    let (a, b) = (o.env.get(k), n.env.get(k));
                    if a != b {
                        env.insert(k.clone(), (a.cloned(), b.cloned()));
                    }
                }
                let wrapper =
                    (o.wrapper != n.wrapper).then(|| (o.wrapper.clone(), n.wrapper.clone()));
                let error = (o.error != n.error).then(|| (o.error.clone(), n.error.clone()));
                if switch.is_some()
                    || stub.is_some()
                    || !env.is_empty()
                    || wrapper.is_some()
                    || error.is_some()
                {
                    changes.push(Change::Changed {
                        service: name.clone(),
                        switch,
                        stub,
                        env,
                        wrapper,
                        error,
                    });
                }
            }
            (None, None) => unreachable!(),
        }
    }
    changes
}
