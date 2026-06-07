use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;
use std::time::{Duration, Instant};

#[test]
fn with_system_metrics_exits_promptly_with_long_poll_interval() -> io::Result<()> {
    let _guard = test_lock();
    let bin = env!("CARGO_BIN_EXE_with-system-metrics");
    let temp_dir = temp_dir_for_test("with-system-metrics-quick-exit")?;
    fs::create_dir_all(&temp_dir)?;

    let started = Instant::now();
    let status = Command::new(bin)
        .args(["--poll-in-seconds", "30", "/bin/sleep", "1"])
        .current_dir(&temp_dir)
        .status()?;
    assert!(status.success(), "with-system-metrics should succeed");
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "poll interval should not block for 30 seconds"
    );

    let metric_paths = metrics_files(&temp_dir);
    assert!(
        !metric_paths.is_empty(),
        "expected at least one metrics file in {temp_dir:?}"
    );
    let mut saw_sys_metric = false;
    for path in metric_paths {
        let contents = fs::read_to_string(path)?;
        if contents.contains("biometrics_sys_utime") {
            saw_sys_metric = true;
            break;
        }
    }
    assert!(
        saw_sys_metric,
        "expected biometrics_sys metric in emitted output"
    );

    fs::remove_dir_all(&temp_dir)?;
    Ok(())
}

#[test]
fn with_system_metrics_emits_every_poll_interval() -> io::Result<()> {
    let _guard = test_lock();
    let bin = env!("CARGO_BIN_EXE_with-system-metrics");
    let temp_dir = temp_dir_for_test("with-system-metrics-polling")?;
    fs::create_dir_all(&temp_dir)?;

    let status = Command::new(bin)
        .args(["--poll-in-seconds", "1", "/bin/sleep", "3"])
        .current_dir(&temp_dir)
        .status()?;
    assert!(status.success(), "with-system-metrics should succeed");

    let mut samples = 0usize;
    for path in metrics_files(&temp_dir) {
        let contents = fs::read_to_string(path)?;
        samples += contents
            .lines()
            .filter(|line| line.starts_with("biometrics_sys_utime "))
            .count();
    }
    assert!(
        samples >= 2,
        "expected at least two periodic samples for 3-second run, got {samples}"
    );

    fs::remove_dir_all(&temp_dir)?;
    Ok(())
}

fn temp_dir_for_test(prefix: &str) -> io::Result<std::path::PathBuf> {
    let mut temp_dir = std::env::temp_dir();
    let nonce = match SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(since_epoch) => since_epoch.as_nanos(),
        Err(_) => 0,
    };
    temp_dir.push(format!("{prefix}-{}-{}", std::process::id(), nonce));
    Ok(temp_dir)
}

fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

fn metrics_files(temp_dir: &PathBuf) -> Vec<PathBuf> {
    let mut output = Vec::new();
    let entries = match fs::read_dir(temp_dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return output,
        Err(err) => panic!("failed to read temp dir: {err}"),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if let Some(name) = path.file_name() {
            let name = name.to_string_lossy();
            if name.starts_with("biometrics.") && name.ends_with(".prom") {
                output.push(path);
            }
        }
    }
    output
}
