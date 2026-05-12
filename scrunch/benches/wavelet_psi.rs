use std::convert::TryInto;
use std::fs::{File, read_to_string, remove_file};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use buffertk::Unpackable;

use scrunch::builder::Builder;
use scrunch::encoder::HuffmanEncoder;
use scrunch::psi::Psi;
use scrunch::psi::compute as compute_psi;
use scrunch::psi::wavelet_tree::WaveletTreePsi;
use scrunch::sais;
use scrunch::sigma::Sigma;
use scrunch::wavelet_tree::prefix::WaveletTree;
use scrunch::Error;

type PrefixWaveletPsi<'a> = WaveletTreePsi<'a, WaveletTree<'a, HuffmanEncoder>>;

const DEFAULT_INPUT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/resources/gutenberg/Dracula.txt"
);
const DEFAULT_SIZES: &[usize] = &[65_536, 131_072, 262_144, 524_288];

#[derive(Debug, Clone)]
struct Options {
    input: PathBuf,
    sizes: Vec<usize>,
    symbols: Option<usize>,
    iterations: usize,
    warm_up: usize,
    prepare: Option<PathBuf>,
    child: Option<PathBuf>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            input: PathBuf::from(DEFAULT_INPUT),
            sizes: Vec::new(),
            symbols: None,
            iterations: 5,
            warm_up: 1,
            prepare: None,
            child: None,
        }
    }
}

#[derive(Debug)]
struct PreparedCase {
    sigma_buf: Vec<u8>,
    psi: Vec<usize>,
}

#[derive(Debug)]
struct Measurement {
    symbols: usize,
    psi_len: usize,
    alphabet: usize,
    iterations: usize,
    elapsed_ns: u128,
    output_bytes: usize,
    user_us: i64,
    system_us: i64,
    maxrss: i64,
    minflt: i64,
    majflt: i64,
    nvcsw: i64,
    nivcsw: i64,
}

fn main() {
    let options = parse_args(std::env::args().skip(1)).unwrap_or_else(|err| {
        eprintln!("{err}");
        std::process::exit(2);
    });
    if let Some(path) = options.prepare.as_ref() {
        let symbols = options
            .symbols
            .expect("--prepare requires exactly one --symbols value");
        let prepared = prepare_case(&options.input, symbols).unwrap_or_else(|err| {
            eprintln!("prepare failed: {err}");
            std::process::exit(1);
        });
        write_case(path, &prepared).unwrap_or_else(|err| {
            eprintln!("write failed: {err}");
            std::process::exit(1);
        });
        return;
    }
    if let Some(path) = options.child.as_ref() {
        let symbols = options
            .symbols
            .expect("--child requires exactly one --symbols value");
        let prepared = read_case(path).unwrap_or_else(|err| {
            eprintln!("read failed: {err}");
            std::process::exit(1);
        });
        let measurement = measure_case(symbols, &prepared, options.iterations, options.warm_up)
            .unwrap_or_else(|err| {
                eprintln!("benchmark failed: {err}");
                std::process::exit(1);
            });
        print_measurement(&measurement);
        return;
    }
    run_parent(&options).unwrap_or_else(|err| {
        eprintln!("benchmark failed: {err}");
        std::process::exit(1);
    });
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut options = Options::default();
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bench" => {}
            "--input" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--input requires a path".to_string())?;
                options.input = PathBuf::from(value);
            }
            "--sizes" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--sizes requires a comma-separated list".to_string())?;
                options.sizes = parse_sizes(&value)?;
            }
            "--symbols" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--symbols requires an integer".to_string())?;
                options.symbols = Some(parse_usize("--symbols", &value)?);
            }
            "--iterations" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--iterations requires an integer".to_string())?;
                options.iterations = parse_usize("--iterations", &value)?;
            }
            "--warm-up" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--warm-up requires an integer".to_string())?;
                options.warm_up = parse_usize("--warm-up", &value)?;
            }
            "--prepare" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--prepare requires a path".to_string())?;
                options.prepare = Some(PathBuf::from(value));
            }
            "--child" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--child requires a path".to_string())?;
                options.child = Some(PathBuf::from(value));
            }
            "--help" | "-h" => {
                return Err(usage());
            }
            other => {
                return Err(format!("unrecognized argument: {other}\n{}", usage()));
            }
        }
    }
    if options.prepare.is_some() && options.child.is_some() {
        return Err("--prepare and --child are mutually exclusive".to_string());
    }
    if options.iterations == 0 {
        return Err("--iterations must be positive".to_string());
    }
    Ok(options)
}

fn usage() -> String {
    format!(
        "USAGE: wavelet_psi [--input PATH] [--sizes N,N,...] [--iterations N] [--warm-up N]\n\
         Default input: {DEFAULT_INPUT}"
    )
}

fn parse_sizes(value: &str) -> Result<Vec<usize>, String> {
    let mut sizes = Vec::new();
    for part in value.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        sizes.push(parse_usize("--sizes", part)?);
    }
    if sizes.is_empty() {
        return Err("--sizes must provide at least one integer".to_string());
    }
    Ok(sizes)
}

fn parse_usize(flag: &str, value: &str) -> Result<usize, String> {
    value
        .parse()
        .map_err(|_| format!("{flag} must be an integer: {value}"))
}

fn run_parent(options: &Options) -> Result<(), String> {
    let text = read_to_string(&options.input)
        .map_err(|err| format!("could not read {}: {err}", options.input.display()))?;
    let total_symbols = text.chars().count();
    let sizes = sizes_for_run(options, total_symbols);
    let exe = std::env::current_exe().map_err(|err| format!("current_exe failed: {err}"))?;
    let maxrss_units = maxrss_units();
    println!(
        "symbols\tpsi_len\talphabet\titerations\telapsed_ms\tms_per_iter\toutput_bytes\tuser_ms\tsystem_ms\tmaxrss_{maxrss_units}\tminflt\tmajflt\tnvcsw\tnivcsw"
    );
    for symbols in sizes {
        let artifact = artifact_path(symbols);
        run_prepare(&exe, options, symbols, &artifact)?;
        let output = run_child(&exe, options, symbols, &artifact)?;
        print!("{output}");
        let _ = remove_file(&artifact);
    }
    Ok(())
}

fn sizes_for_run(options: &Options, total_symbols: usize) -> Vec<usize> {
    if let Some(symbols) = options.symbols {
        return vec![std::cmp::min(symbols, total_symbols)];
    }
    let mut sizes = if options.sizes.is_empty() {
        DEFAULT_SIZES
            .iter()
            .copied()
            .filter(|size| *size < total_symbols)
            .collect::<Vec<_>>()
    } else {
        options.sizes.clone()
    };
    sizes.push(total_symbols);
    sizes.sort_unstable();
    sizes.dedup();
    sizes
}

fn artifact_path(symbols: usize) -> PathBuf {
    let pid = std::process::id();
    std::env::temp_dir().join(format!("scrunch-wavelet-psi-{pid}-{symbols}.bin"))
}

fn run_prepare(exe: &Path, options: &Options, symbols: usize, artifact: &Path) -> Result<(), String> {
    let status = Command::new(exe)
        .arg("--input")
        .arg(&options.input)
        .arg("--symbols")
        .arg(symbols.to_string())
        .arg("--prepare")
        .arg(artifact)
        .status()
        .map_err(|err| format!("spawn prepare failed: {err}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("prepare exited with {status}"))
    }
}

fn run_child(exe: &Path, options: &Options, symbols: usize, artifact: &Path) -> Result<String, String> {
    let output = Command::new(exe)
        .arg("--symbols")
        .arg(symbols.to_string())
        .arg("--iterations")
        .arg(options.iterations.to_string())
        .arg("--warm-up")
        .arg(options.warm_up.to_string())
        .arg("--child")
        .arg(artifact)
        .output()
        .map_err(|err| format!("spawn child failed: {err}"))?;
    if output.status.success() {
        String::from_utf8(output.stdout).map_err(|err| format!("utf8 decode failed: {err}"))
    } else {
        Err(format!(
            "child exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

fn prepare_case(input: &Path, symbols: usize) -> Result<PreparedCase, String> {
    let text = read_to_string(input).map_err(|err| format!("read failed: {err}"))?;
    let text: Vec<u32> = text.chars().take(symbols).map(|c| c as u32).collect();
    let mut sigma_buf = Vec::new();
    let mut sigma_builder = Builder::new(&mut sigma_buf);
    Sigma::construct(text.iter().copied(), &mut sigma_builder)
        .map_err(|err| format!("sigma construct failed: {err:?}"))?;
    drop(sigma_builder);
    let sigma = Sigma::unpack(&sigma_buf)
        .map_err(|err| format!("sigma unpack failed: {err:?}"))?
        .0;
    let mut s = Vec::with_capacity(text.len() + 1);
    for t in text {
        s.push(
            sigma
                .char_to_sigma(t)
                .ok_or_else(|| "sigma translation failed".to_string())?,
        );
    }
    s.push(0);
    let mut sa = vec![0usize; s.len()];
    sais::sais(&sigma, &s, &mut sa).map_err(|err| format!("sais failed: {err:?}"))?;
    let isa = inverse(&sa);
    let psi = compute_psi(&isa);
    Ok(PreparedCase { sigma_buf, psi })
}

fn write_case(path: &Path, prepared: &PreparedCase) -> Result<(), String> {
    let mut file =
        File::create(path).map_err(|err| format!("create {} failed: {err}", path.display()))?;
    write_u64(&mut file, prepared.sigma_buf.len() as u64)?;
    file.write_all(&prepared.sigma_buf)
        .map_err(|err| format!("write sigma failed: {err}"))?;
    write_u64(&mut file, prepared.psi.len() as u64)?;
    for value in prepared.psi.iter().copied() {
        write_u64(&mut file, value as u64)?;
    }
    Ok(())
}

fn read_case(path: &Path) -> Result<PreparedCase, String> {
    let mut file =
        File::open(path).map_err(|err| format!("open {} failed: {err}", path.display()))?;
    let sigma_len = read_u64(&mut file)? as usize;
    let mut sigma_buf = vec![0u8; sigma_len];
    file.read_exact(&mut sigma_buf)
        .map_err(|err| format!("read sigma failed: {err}"))?;
    let psi_len = read_u64(&mut file)? as usize;
    let mut psi = Vec::with_capacity(psi_len);
    for _ in 0..psi_len {
        let value = read_u64(&mut file)?;
        psi.push(value.try_into().map_err(|_| "psi value does not fit usize")?);
    }
    Ok(PreparedCase { sigma_buf, psi })
}

fn write_u64(file: &mut File, value: u64) -> Result<(), String> {
    file.write_all(&value.to_le_bytes())
        .map_err(|err| format!("write failed: {err}"))
}

fn read_u64(file: &mut File) -> Result<u64, String> {
    let mut bytes = [0u8; 8];
    file.read_exact(&mut bytes)
        .map_err(|err| format!("read failed: {err}"))?;
    Ok(u64::from_le_bytes(bytes))
}

fn measure_case(
    symbols: usize,
    prepared: &PreparedCase,
    iterations: usize,
    warm_up: usize,
) -> Result<Measurement, String> {
    let sigma = Sigma::unpack(&prepared.sigma_buf)
        .map_err(|err| format!("sigma unpack failed: {err:?}"))?
        .0;
    for _ in 0..warm_up {
        construct_once(&sigma, &prepared.psi).map_err(|err| format!("warm-up failed: {err:?}"))?;
    }
    let before = getrusage().map_err(|err| format!("getrusage(before) failed: {err}"))?;
    let start = Instant::now();
    let mut output_bytes = 0usize;
    for _ in 0..iterations {
        output_bytes = construct_once(&sigma, &prepared.psi)
            .map_err(|err| format!("construct failed: {err:?}"))?;
    }
    let elapsed_ns = start.elapsed().as_nanos();
    let after = getrusage().map_err(|err| format!("getrusage(after) failed: {err}"))?;
    Ok(Measurement {
        symbols,
        psi_len: prepared.psi.len(),
        alphabet: sigma.K(),
        iterations,
        elapsed_ns,
        output_bytes,
        user_us: timeval_delta_us(after.ru_utime, before.ru_utime),
        system_us: timeval_delta_us(after.ru_stime, before.ru_stime),
        maxrss: after.ru_maxrss,
        minflt: after.ru_minflt - before.ru_minflt,
        majflt: after.ru_majflt - before.ru_majflt,
        nvcsw: after.ru_nvcsw - before.ru_nvcsw,
        nivcsw: after.ru_nivcsw - before.ru_nivcsw,
    })
}

fn construct_once(sigma: &Sigma, psi: &[usize]) -> Result<usize, Error> {
    let mut buf = Vec::new();
    let mut builder = Builder::new(&mut buf);
    <PrefixWaveletPsi<'_> as Psi>::construct(sigma, psi, &mut builder)?;
    drop(builder);
    Ok(buf.len())
}

fn print_measurement(measurement: &Measurement) {
    let elapsed_ms = measurement.elapsed_ns as f64 / 1_000_000.0;
    let per_iter_ms = elapsed_ms / measurement.iterations as f64;
    println!(
        "{}\t{}\t{}\t{}\t{elapsed_ms:.3}\t{per_iter_ms:.3}\t{}\t{:.3}\t{:.3}\t{}\t{}\t{}\t{}\t{}",
        measurement.symbols,
        measurement.psi_len,
        measurement.alphabet,
        measurement.iterations,
        measurement.output_bytes,
        measurement.user_us as f64 / 1_000.0,
        measurement.system_us as f64 / 1_000.0,
        measurement.maxrss,
        measurement.minflt,
        measurement.majflt,
        measurement.nvcsw,
        measurement.nivcsw
    );
}

fn getrusage() -> Result<libc::rusage, std::io::Error> {
    let mut usage = libc::rusage {
        ru_utime: libc::timeval {
            tv_sec: 0,
            tv_usec: 0,
        },
        ru_stime: libc::timeval {
            tv_sec: 0,
            tv_usec: 0,
        },
        ru_maxrss: 0,
        ru_ixrss: 0,
        ru_idrss: 0,
        ru_isrss: 0,
        ru_minflt: 0,
        ru_majflt: 0,
        ru_nswap: 0,
        ru_inblock: 0,
        ru_oublock: 0,
        ru_msgsnd: 0,
        ru_msgrcv: 0,
        ru_nsignals: 0,
        ru_nvcsw: 0,
        ru_nivcsw: 0,
    };
    let rc = unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) };
    if rc == 0 {
        Ok(usage)
    } else {
        Err(std::io::Error::last_os_error())
    }
}

fn timeval_delta_us(after: libc::timeval, before: libc::timeval) -> i64 {
    let after = after.tv_sec.saturating_mul(1_000_000) + i64::from(after.tv_usec);
    let before = before.tv_sec.saturating_mul(1_000_000) + i64::from(before.tv_usec);
    after - before
}

fn maxrss_units() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "bytes"
    }
    #[cfg(not(target_os = "macos"))]
    {
        "kib"
    }
}

fn inverse(values: &[usize]) -> Vec<usize> {
    let mut inverse = vec![0usize; values.len()];
    for (idx, value) in values.iter().copied().enumerate() {
        inverse[value] = idx;
    }
    inverse
}
