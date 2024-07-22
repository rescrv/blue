use utf8path::Path;

/// Return the cargo directory where binaries built by the application will reside.
pub fn cargo_dir() -> Path<'static> {
    if let Some(bin_path) = std::env::var_os("CARGO_BIN_PATH") {
        Path::try_from(bin_path).expect("CARGO_BIN_PATH should be UTF-8")
    } else if let Ok(mut path) = std::env::current_exe() {
        path.pop();
        if path.ends_with("deps") {
            path.pop();
        }
        Path::try_from(path).expect("current_exe should be UTF-8")
    } else {
        panic!("CARGO_BIN_PATH not set and binary not inferred");
    }
}
