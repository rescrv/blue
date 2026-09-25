//! ```
//! USAGE: rcdiff [OPTIONS] <old-rc-conf-path> <new-rc-conf-path>
//! ```
//!
//! Compare the effective configuration of every service between two rc_conf paths:  switch, stub,
//! bound environment, and WRAPPER.  Services disabled on both sides are skipped.  Like diff(1),
//! exits 0 when there are no differences, 1 when there are, and 2 on error.
//!
//! Values are printed; pass --keys-only when the configuration carries secrets.

use arrrg::CommandLine;

use rc_conf::{Change, Effective, RcConf, SwitchPosition};

#[derive(Clone, Debug, Eq, PartialEq, arrrg_derive::CommandLine)]
struct Options {
    #[arrrg(
        optional,
        "A colon-separated PATH-like list of rc.d directories used for the old side."
    )]
    rc_d_path: String,
    #[arrrg(optional, "rc.d path for the new side (defaults to --rc-d-path).")]
    new_rc_d_path: String,
    #[arrrg(flag, "Print which variables changed, not their values.")]
    keys_only: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            rc_d_path: "rc.d".to_string(),
            new_rc_d_path: String::new(),
            keys_only: false,
        }
    }
}

fn switch(s: SwitchPosition) -> &'static str {
    match s {
        SwitchPosition::Yes => "YES",
        SwitchPosition::No => "NO",
        SwitchPosition::Manual => "MANUAL",
    }
}

fn load(rc_conf_path: &str, rc_d_path: &str) -> std::collections::BTreeMap<String, Effective> {
    let rc_conf = RcConf::parse(rc_conf_path).unwrap_or_else(|e| {
        eprintln!("failed to parse {rc_conf_path}: {e}");
        std::process::exit(2);
    });
    let rc_d = rc_conf::load_services(rc_d_path).unwrap_or_else(|e| {
        eprintln!("failed to load services from {rc_d_path}: {e}");
        std::process::exit(2);
    });
    rc_conf::effective(&rc_conf, &rc_d)
}

fn main() {
    let (options, argv) = Options::from_command_line_relaxed(
        "USAGE: rcdiff [OPTIONS] <old-rc-conf-path> <new-rc-conf-path>",
    );
    if argv.len() != 2 {
        eprintln!("expected exactly two rc_conf paths");
        std::process::exit(2);
    }
    let new_rc_d = if options.new_rc_d_path.is_empty() {
        options.rc_d_path.as_str()
    } else {
        options.new_rc_d_path.as_str()
    };
    let old = load(&argv[0], &options.rc_d_path);
    let new = load(&argv[1], new_rc_d);
    let changes = rc_conf::diff_effective(&old, &new);
    let show = |v: &Option<String>| -> String {
        match v {
            None => "(absent)".to_string(),
            Some(_) if options.keys_only => "(set)".to_string(),
            Some(v) => v.clone(),
        }
    };
    for change in changes.iter() {
        match change {
            Change::Added(name, eff) => {
                println!("+ {name} ({})", switch(eff.switch));
                if !options.keys_only {
                    for (k, v) in eff.env.iter() {
                        println!("    {k}={v}");
                    }
                }
                if let Some(err) = &eff.error {
                    println!("    ! {err}");
                }
            }
            Change::Removed(name, eff) => {
                println!("- {name} (was {})", switch(eff.switch));
            }
            Change::Changed {
                service,
                switch: sw,
                stub,
                env,
                wrapper,
                error,
            } => {
                println!("~ {service}");
                if let Some((a, b)) = sw {
                    println!("    ENABLED: {} -> {}", switch(*a), switch(*b));
                }
                if let Some((a, b)) = stub {
                    println!("    stub: {} -> {}", show(a), show(b));
                }
                for (k, (a, b)) in env.iter() {
                    println!("    {k}: {} -> {}", show(a), show(b));
                }
                if let Some((a, b)) = wrapper {
                    println!(
                        "    WRAPPER: {} -> {}",
                        shvar::quote(a.clone()),
                        shvar::quote(b.clone())
                    );
                }
                if let Some((a, b)) = error {
                    let fmt = |e: &Option<rc_conf::EffectiveError>| {
                        e.as_ref()
                            .map(|e| e.to_string())
                            .unwrap_or_else(|| "ok".to_string())
                    };
                    println!("    status: {} -> {}", fmt(a), fmt(b));
                }
            }
        }
    }
    if !changes.is_empty() {
        std::process::exit(1);
    }
}
