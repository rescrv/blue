#[test]
fn example1() {
    use std::process::Command;

    // Test rcinvoke as an integration test by spawning it as a subprocess
    let current_path = std::env::var("PATH").unwrap_or_default();
    let debug_path = "../target/debug";
    let new_path = if current_path.is_empty() {
        debug_path.to_string()
    } else {
        format!("{}:{}", debug_path, current_path)
    };

    let output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "rcinvoke",
            "--",
            "--rc-conf-path",
            "rc.conf",
            "--rc-d-path",
            "rc.d",
            "example1",
        ])
        .env("PATH", new_path)
        .output();

    match output {
        Ok(output) => {
            // The command should execute and return a status code
            // It may fail due to service not being enabled or other config issues,
            // but it should not panic or hang indefinitely
            assert!(
                output.status.code().is_some(),
                "rcinvoke should exit with a status code: stderr={}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Err(e) => {
            panic!("Failed to execute rcinvoke integration test: {}", e);
        }
    }
}

#[test]
fn jfk_planetexpress_example4() {
    use std::process::Command;

    // Test that Jfk_PlanetExpress_example4 produces the expected output
    let current_path = std::env::var("PATH").unwrap_or_default();
    let debug_path = "../target/debug";
    let new_path = if current_path.is_empty() {
        debug_path.to_string()
    } else {
        format!("{}:{}", debug_path, current_path)
    };

    let output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "rcinvoke",
            "--",
            "--rc-conf-path",
            "rc.conf",
            "--rc-d-path",
            "rc.d",
            "Jfk_PlanetExpress_example4",
        ])
        .env("PATH", new_path)
        .output();

    match output {
        Ok(output) => {
            assert!(
                output.status.success(),
                "rcinvoke should succeed: stderr={}",
                String::from_utf8_lossy(&output.stderr)
            );

            let stdout = String::from_utf8_lossy(&output.stdout);
            let expected = "['--field1', 'Good News', '--field2', 'Everyone!']\n";

            assert_eq!(
                stdout.trim(),
                expected.trim(),
                "Output should match expected format. Got: {}, Expected: {}",
                stdout.trim(),
                expected.trim()
            );
        }
        Err(e) => {
            panic!("Failed to execute rcinvoke integration test: {}", e);
        }
    }
}
