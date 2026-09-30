//! End-to-end tests of the rustrc and rustrcctl binaries.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const RUSTRC: &str = env!("CARGO_BIN_EXE_rustrc");
const RUSTRCCTL: &str = env!("CARGO_BIN_EXE_rustrcctl");

struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new(name: &str, services: &[&str]) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("rustrc-bin-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("rc.d")).unwrap();
        for service in services {
            let path = dir.join("rc.d").join(service);
            std::fs::write(
                &path,
                "#!/bin/sh\ncase \"$1\" in\nrcvar) ;;\nrun) exec sleep 1000 ;;\nesac\n",
            )
            .unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let conf = services
            .iter()
            .map(|s| format!("{s}_ENABLED=\"YES\"\n"))
            .collect::<String>();
        std::fs::write(dir.join("rc.conf"), conf + "STOP_TIMEOUT=\"5\"\n").unwrap();
        Self { dir }
    }

    fn start(&self, args: &[&str]) -> Child {
        let log = |name: &str| std::fs::File::create(self.dir.join(name)).unwrap();
        Command::new(RUSTRC)
            .args(args)
            .current_dir(&self.dir)
            .stdin(Stdio::null())
            .stdout(log("stdout"))
            .stderr(log("stderr"))
            .spawn()
            .unwrap()
    }

    fn ctl(&self, args: &[&str]) -> (bool, String) {
        let output = Command::new(RUSTRCCTL)
            .arg("--control-sock")
            .arg(self.dir.join("rc.sock"))
            .args(args)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        (
            output.status.success(),
            String::from_utf8_lossy(&output.stdout).into_owned(),
        )
    }

    /// The pid rustrcctl reports for a running service.
    fn pid(&self, service: &str) -> Option<i32> {
        let (_, status) = self.ctl(&["status", service]);
        status.lines().find_map(|line| {
            let fields = line.split_whitespace().collect::<Vec<_>>();
            (fields.first() == Some(&service) && fields.get(1) == Some(&"running"))
                .then(|| fields.get(2).and_then(|p| p.parse().ok()))
                .flatten()
        })
    }

    fn stderr(&self) -> String {
        std::fs::read_to_string(self.dir.join("stderr")).unwrap_or_default()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn wait_until(what: &str, timeout: Duration, mut f: impl FnMut() -> bool) {
    let deadline = Instant::now() + timeout;
    while !f() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn signal(pid: u32, signal: i32) {
    // SAFETY: kill observes only integer arguments.
    unsafe {
        libc::kill(pid as i32, signal);
    }
}

fn is_gone(pid: i32) -> bool {
    match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(stat) => stat
            .rsplit_once(')')
            .map(|(_, rest)| rest.trim_start().starts_with('Z'))
            .unwrap_or(true),
        Err(_) => (unsafe { libc::kill(pid, 0) }) != 0,
    }
}

fn exits_within(child: &mut Child, timeout: Duration) -> i32 {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            use std::os::unix::process::ExitStatusExt;
            return status
                .code()
                .unwrap_or_else(|| 128 + status.signal().unwrap_or(0));
        }
        assert!(Instant::now() < deadline, "rustrc did not exit");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn exists(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok()
}

#[test]
fn hup_reloads_other_signals_are_ignored_and_term_shuts_down() {
    let fx = Fixture::new("signals", &["one", "two"]);
    std::fs::write(fx.dir.join("rc.conf"), "one_ENABLED=\"YES\"\n").unwrap();
    let mut rustrc = fx.start(&[]);
    wait_until("one to run", Duration::from_secs(10), || {
        fx.pid("one").is_some()
    });
    std::fs::write(
        fx.dir.join("rc.conf"),
        "one_ENABLED=\"YES\"\ntwo_ENABLED=\"YES\"\n",
    )
    .unwrap();
    signal(rustrc.id(), libc::SIGHUP);
    wait_until("two to run after SIGHUP", Duration::from_secs(10), || {
        fx.pid("two").is_some()
    });
    for sig in [libc::SIGWINCH, libc::SIGUSR1, libc::SIGUSR2, libc::SIGHUP] {
        signal(rustrc.id(), sig);
    }
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        rustrc.try_wait().unwrap().is_none(),
        "rustrc exited on a non-shutdown signal"
    );
    let (one, two) = (fx.pid("one").unwrap(), fx.pid("two").unwrap());
    signal(rustrc.id(), libc::SIGTERM);
    assert_eq!(0, exits_within(&mut rustrc, Duration::from_secs(10)));
    assert!(is_gone(one) && is_gone(two));
    assert!(!exists(&fx.dir.join("rc.sock")));
    let log = fx.stderr();
    assert!(log.contains("\"exited\""), "{log}");
}

#[test]
fn after_kill_9_a_restart_reclaims_the_socket_and_fences_leftovers() {
    let fx = Fixture::new("kill9", &["svc"]);
    let mut first = fx.start(&[]);
    wait_until("svc to run", Duration::from_secs(10), || {
        fx.pid("svc").is_some()
    });
    let leftover = fx.pid("svc").unwrap();
    signal(first.id(), libc::SIGKILL);
    let _ = first.wait();
    assert!(
        !is_gone(leftover),
        "the service should outlive a SIGKILLed rustrc"
    );
    assert!(exists(&fx.dir.join("rc.sock")));

    let mut second = fx.start(&[]);
    wait_until("a fresh svc", Duration::from_secs(15), || {
        fx.pid("svc").is_some_and(|pid| pid != leftover)
    });
    assert!(is_gone(leftover), "leftover was not fenced");
    signal(second.id(), libc::SIGTERM);
    assert_eq!(0, exits_within(&mut second, Duration::from_secs(10)));
}

#[test]
fn a_second_rustrc_on_the_same_state_dir_refuses() {
    let fx = Fixture::new("exclusive", &["svc"]);
    let mut first = fx.start(&[]);
    wait_until("svc to run", Duration::from_secs(10), || {
        fx.pid("svc").is_some()
    });
    let pid = fx.pid("svc").unwrap();
    let mut second = Command::new(RUSTRC)
        .args(["--control-sock", "other.sock"])
        .current_dir(&fx.dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    assert_eq!(1, exits_within(&mut second, Duration::from_secs(5)));
    let mut err = String::new();
    std::io::Read::read_to_string(&mut second.stderr.take().unwrap(), &mut err).unwrap();
    assert!(err.contains("AlreadyRunning"), "{err}");
    assert!(!exists(&fx.dir.join("other.sock")));
    assert_eq!(Some(pid), fx.pid("svc"), "the first rustrc was disturbed");
    signal(first.id(), libc::SIGTERM);
    assert_eq!(0, exits_within(&mut first, Duration::from_secs(10)));
}

/// The supervisor half of `rustrc --container-init`, found as the init's only child.
#[cfg(target_os = "linux")]
fn supervisor_of(init: u32) -> i32 {
    let mut children = vec![];
    for entry in std::fs::read_dir("/proc").unwrap().flatten() {
        let Ok(stat) = std::fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        let Some((head, rest)) = stat.rsplit_once(')') else {
            continue;
        };
        let fields = rest.split_whitespace().collect::<Vec<_>>();
        if fields.get(1) == Some(&init.to_string().as_str()) && fields.first() != Some(&"Z") {
            children.push(head.split_whitespace().next().unwrap().parse().unwrap());
        }
    }
    assert_eq!(1, children.len(), "{children:?}");
    children[0]
}

#[cfg(target_os = "linux")]
#[test]
fn container_init_tears_down_what_a_dead_supervisor_left() {
    let fx = Fixture::new("init-crash", &["svc"]);
    let mut init = fx.start(&["--no-state-dir", "--container-init"]);
    wait_until("svc to run", Duration::from_secs(10), || {
        fx.pid("svc").is_some()
    });
    let service = fx.pid("svc").unwrap();
    let supervisor = supervisor_of(init.id());
    signal(supervisor as u32, libc::SIGKILL);
    // The service is reparented to the init, which stops it and reports the supervisor's death.
    assert_eq!(
        128 + libc::SIGKILL,
        exits_within(&mut init, Duration::from_secs(10))
    );
    assert!(is_gone(service), "the orphaned service survived");
}

#[cfg(target_os = "linux")]
#[test]
fn container_init_forwards_sigterm_for_a_clean_shutdown() {
    let fx = Fixture::new("init-term", &["svc"]);
    let mut init = fx.start(&["--container-init"]);
    wait_until("svc to run", Duration::from_secs(10), || {
        fx.pid("svc").is_some()
    });
    let service = fx.pid("svc").unwrap();
    signal(init.id(), libc::SIGTERM);
    assert_eq!(0, exits_within(&mut init, Duration::from_secs(10)));
    assert!(is_gone(service));
    let log = fx.stderr();
    assert!(log.contains("\"shutdown\""), "{log}");
}

/// Kill a process when dropped, so a failed assertion cannot leak it past the test.
struct KillOnDrop(i32);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        signal(self.0 as u32, libc::SIGKILL);
    }
}

fn write_stub(path: &Path, run: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(
        path,
        format!("#!/bin/sh\ncase \"$1\" in\nrcvar) ;;\nrun) {run} ;;\nesac\n"),
    )
    .unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// A rustrc nested as a service of another stops within the outer's STOP_TIMEOUT and takes its
/// services with it, even when the inner rc.conf asks for an hour:  the ceiling and init grace the
/// outer's stub passes bound the inner's shutdown from outside.  Without the ceiling the outer
/// escalates to SIGKILL and the inner's service is orphaned.
#[cfg(target_os = "linux")]
#[test]
fn a_nested_rustrc_stops_its_services_within_the_outer_budget() {
    let fx = Fixture::new("nested", &[]);
    let inner = fx.dir.join("inner");
    std::fs::create_dir_all(inner.join("rc.d")).unwrap();
    let at = |name: &str| inner.join(name).to_string_lossy().into_owned();
    write_stub(
        &inner.join("rc.d").join("stubborn"),
        &format!(
            "trap '' TERM; echo $$ > {}; while :; do sleep 0.05; done",
            at("stubborn.pid")
        ),
    );
    // The inner rc.conf is the agent's to write; the outer's budget is not.
    std::fs::write(
        inner.join("rc.conf"),
        "stubborn_ENABLED=\"YES\"\nstubborn_STOP_TIMEOUT=\"3600\"\n",
    )
    .unwrap();
    // Flags in arrrg's canonical order, which rustrc enforces.
    write_stub(
        &fx.dir.join("rc.d").join("agent_rc"),
        &format!(
            "exec {RUSTRC} --control-sock {} --rc-conf-path {} --rc-d-path {} --state-dir {} \
             --container-init --max-stop-timeout-ms 500 --init-grace-ms 500",
            at("rc.sock"),
            at("rc.conf"),
            at("rc.d"),
            at("rc.state"),
        ),
    );
    // Budget:  ceiling + init grace + the init's 5s KILL-phase reap + up to 2s of log flush.
    std::fs::write(
        fx.dir.join("rc.conf"),
        "agent_rc_ENABLED=\"YES\"\nagent_rc_STOP_TIMEOUT=\"10\"\n",
    )
    .unwrap();
    let mut outer = fx.start(&[]);
    let pid_file = inner.join("stubborn.pid");
    wait_until(
        "the inner's service to run",
        Duration::from_secs(15),
        || std::fs::read_to_string(&pid_file).is_ok_and(|s| s.ends_with('\n')),
    );
    let stubborn: i32 = std::fs::read_to_string(&pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let _cleanup = KillOnDrop(stubborn);

    let started = Instant::now();
    let (ok, out) = fx.ctl(&["services", "-S", "agent_rc"]);
    let took = started.elapsed();
    assert!(ok, "{out}");
    assert!(
        is_gone(stubborn),
        "the inner's service outlived agent_rc (stop took {took:?})"
    );
    assert!(
        took < Duration::from_secs(10),
        "the outer escalated to SIGKILL after {took:?}"
    );
    assert!(
        !exists(&inner.join("rc.sock")),
        "the inner did not shut down cleanly"
    );
    signal(outer.id(), libc::SIGTERM);
    assert_eq!(0, exits_within(&mut outer, Duration::from_secs(10)));
}
