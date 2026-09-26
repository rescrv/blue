//! The orphan reaper and rustrc's own helpers must not race for zombies.
//!
//! This lives in its own test binary because an orphan reaper reaps every unmanaged child of the
//! process, which would include other tests' services.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use rustrc::{Pid1, Pid1Options};

fn fixture() -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("rustrc-reaper-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("rc.d")).unwrap();
    for name in ["one", "two", "three"] {
        let path = dir.join("rc.d").join(name);
        // Answer rcvar instantly; when run, keep orphaning short-lived children.
        std::fs::write(
            &path,
            "#!/bin/sh\ncase \"$1\" in\nrcvar) echo \"${RCVAR_ARGV0}_X\" ;;\n\
             run) while :; do (sleep 0.01 &); sleep 0.02; done ;;\nesac\n",
        )
        .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    std::fs::write(
        dir.join("rc.conf"),
        "one_ENABLED=\"YES\"\ntwo_ENABLED=\"YES\"\nthree_ENABLED=\"YES\"\nSTOP_TIMEOUT=\"5\"\n",
    )
    .unwrap();
    dir
}

/// Zombies whose parent is this process.
fn zombie_children() -> Vec<i32> {
    let me = std::process::id().to_string();
    let mut zombies = vec![];
    for entry in std::fs::read_dir("/proc").unwrap().flatten() {
        let Ok(stat) = std::fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        let Some((head, rest)) = stat.rsplit_once(')') else {
            continue;
        };
        let fields = rest.split_whitespace().collect::<Vec<_>>();
        if fields.len() > 1 && fields[0] == "Z" && fields[1] == me {
            zombies.push(head.split_whitespace().next().unwrap().parse().unwrap());
        }
    }
    zombies
}

#[cfg(target_os = "linux")]
#[test]
fn reaper_reaps_orphans_but_never_helpers() {
    minimal_signals::block();
    let dir = fixture();
    let options = Pid1Options {
        rc_conf_path: dir.join("rc.conf").to_string_lossy().into_owned(),
        rc_d_path: dir.join("rc.d").to_string_lossy().into_owned(),
        reap_orphans: true,
        child_subreaper: true,
        ..Pid1Options::default()
    };
    let pid1 = Pid1::new(options).unwrap();
    // Every plan runs rcvar for each running service while orphans exit around it.
    let deadline = Instant::now() + Duration::from_secs(4);
    let mut plans = 0;
    while Instant::now() < deadline {
        let plan = pid1.plan_reload().unwrap();
        assert!(plan.errors.is_empty(), "plan {plans}: {:?}", plan.errors);
        plans += 1;
    }
    assert!(plans > 50, "only {plans} plans ran");
    let running = pid1
        .status()
        .iter()
        .filter(|s| !s.running.is_empty())
        .count();
    assert_eq!(3, running);
    // The reaper keeps up:  every zombie seen now is gone a moment later.
    let before = zombie_children();
    std::thread::sleep(Duration::from_millis(700));
    let after = zombie_children();
    let lingering = before
        .iter()
        .filter(|z| after.contains(z))
        .collect::<Vec<_>>();
    assert!(lingering.is_empty(), "unreaped orphans: {lingering:?}");
    pid1.shutdown().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}
