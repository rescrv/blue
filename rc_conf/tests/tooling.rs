mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use common::cargo_dir;

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("rc_conf_tooling_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("rc.d")).unwrap();
        Self(dir)
    }

    fn write(&self, name: &str, contents: &str) {
        std::fs::write(self.0.join(name), contents).unwrap();
    }

    fn stub(&self, name: &str, command: &str) {
        use std::os::unix::fs::PermissionsExt;
        let path = self.0.join("rc.d").join(name);
        std::fs::write(
            &path,
            format!("#!/usr/bin/env rcscript\nDESCRIBE={name}\nCOMMAND={command}\n"),
        )
        .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn run(&self, bin: &str, args: &[&str]) -> Output {
        let cargo_dir = cargo_dir();
        let cargo_dir = cargo_dir.into_std();
        let path = format!(
            "{}:{}",
            cargo_dir.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        Command::new(cargo_dir.join(bin))
            .args(args)
            .current_dir(Path::new(&self.0))
            .env("PATH", path)
            .output()
            .expect("binary should spawn")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

#[test]
fn rclint_reports_the_silent_failures() {
    let s = Scratch::new("lint");
    s.stub("memcached", "echo ${PORT} ${THREADS}");
    s.stub("orphan", "echo hi");
    s.write(
        "rc.conf",
        "memcached_ENABLED=\"YES\"\nmemcached_PORT=\"1\"\nmemcahced_PORT=\"2\"\nmemcached_PORT=\"3\"\n\
         two_ALIASES=\"memcached\"\ntwo_ENABLED=\"yes\"\nredis_ENABLED=\"YES\"\nmemcached_IMAGE=\"x\"\n",
    );
    let out = s.run("rclint", &[]);
    let text = stdout(&out);
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(
        text.contains("rc.conf:6: error: two: _ENABLED is \"yes\""),
        "{text}"
    );
    assert!(
        text.contains("rc.conf:7: error: redis: enabled but there is no executable rc.d stub"),
        "{text}"
    );
    assert!(
        text.contains("rc.conf:3: warning: memcahced_PORT is never read"),
        "{text}"
    );
    assert!(
        text.contains("rc.conf:4: warning: memcached_PORT assigned more than once"),
        "{text}"
    );
    assert!(text.contains("memcached_IMAGE is never read"), "{text}");
    assert!(
        text.contains("note: memcached: stub reads THREADS"),
        "{text}"
    );
    assert!(
        text.contains("note: orphan: rc.d stub is never enabled"),
        "{text}"
    );
    let allowed = stdout(&s.run("rclint", &["--allow", "IMAGE"]));
    assert!(!allowed.contains("memcached_IMAGE"), "{allowed}");
}

#[test]
fn rclint_clean_tree_exits_zero() {
    let s = Scratch::new("clean");
    s.stub("svc", "echo ${PORT}");
    s.write("rc.conf", "svc_ENABLED=\"YES\"\nsvc_PORT=\"1\"\n");
    let out = s.run("rclint", &["--strict"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
}

#[test]
fn rcwhy_attributes_across_files() {
    let s = Scratch::new("why");
    s.stub("svc", "echo ${PORT} ${HOST}");
    s.write(
        "rc.conf",
        "svc_ENABLED=\"YES\"\nsvc_PORT=\"1\"\nHOST=\"${DOMAIN}\"\nDOMAIN=\"d\"\n",
    );
    s.write("rc.conf.local", "svc_PORT=\"2\"\n");
    let out = s.run("rcwhy", &["--rc-conf-path", "rc.conf:rc.conf.local", "svc"]);
    let text = stdout(&out);
    assert!(out.status.success(), "{text}");
    assert!(text.contains("svc ENABLED = YES"), "{text}");
    assert!(text.contains("svc PORT = \"2\""), "{text}");
    assert!(text.contains("rc.conf.local:1"), "{text}");
    assert!(text.contains("overrides \"1\"  rc.conf:2"), "{text}");
    assert!(text.contains("svc DOMAIN = \"d\""), "{text}");
}

#[test]
fn rcdiff_reports_binding_changes() {
    let s = Scratch::new("diff");
    s.stub("svc", "echo ${PORT}");
    s.write("old.conf", "svc_ENABLED=\"YES\"\nsvc_PORT=\"1\"\n");
    s.write("new.conf", "svc_ENABLED=\"YES\"\nsvc_PORT=\"2\"\n");
    let out = s.run("rcdiff", &["old.conf", "new.conf"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stdout(&out).contains("svc_PORT: 1 -> 2"),
        "{}",
        stdout(&out)
    );
    let same = s.run("rcdiff", &["old.conf", "old.conf"]);
    assert_eq!(same.status.code(), Some(0));
    assert!(stdout(&same).is_empty());
}

#[test]
fn rcinvoke_dry_run_prints_the_exec() {
    let s = Scratch::new("dry");
    s.stub("svc", "echo ${PORT}");
    s.write(
        "rc.conf",
        "svc_ENABLED=\"YES\"\nsvc_PORT=\"1\"\nsvc_WRAPPER=\"nice -n 5\"\n",
    );
    let out = s.run("rcinvoke", &["--dry-run", "svc", "extra"]);
    let text = stdout(&out);
    assert!(out.status.success(), "{text}");
    assert_eq!(
        text.trim(),
        "env RCVAR_ARGV0=svc svc_PORT=1 nice -n 5 rc.d/svc run extra"
    );
}
