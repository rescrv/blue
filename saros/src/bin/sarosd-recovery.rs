use arrrg::CommandLine;
use saros::query::QueryParams;
use saros::recovery::RecoveryBiometricsStore;
use saros::{QueryEngine, Time, Window};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, arrrg_derive::CommandLine)]
struct WindowOptions {
    #[arrrg(required, "Start of the window in seconds.")]
    start: i64,
    #[arrrg(required, "Limit of the window in seconds.  Must be start + step * N.")]
    limit: i64,
    #[arrrg(required, "Step in seconds.")]
    step: i64,
}

fn main() {
    let mut store = RecoveryBiometricsStore::new();
    for arg in std::env::args().skip(1) {
        let path = utf8path::Path::new(&arg);
        store.load(&path).expect("should be able to load file");
    }
    let ctx = rpc_pb::Context::default();
    let window = store
        .window()
        .expect("1 second is valid")
        .round_to_seconds();
    // SAFETY(rescrv):  These are known-good values.
    let mut params = QueryParams::new(window, Time::from_secs(1).expect("1 second is valid"))
        .expect("expect valid query params");
    let query_engine = QueryEngine::new(store);
    for line in std::io::stdin().lines() {
        let Ok(line) = line else {
            eprintln!("should be able to read stdin");
            continue;
        };
        if let Some(line) = line.strip_prefix("query:") {
            let serieses = match query_engine.query(&ctx, line.trim(), params) {
                Ok(serieses) => serieses,
                Err(e) => {
                    eprintln!("query failed: {e}");
                    continue;
                }
            };
            if serieses.is_empty() {
                continue;
            }
            for series in serieses.iter() {
                if series.points().len() != serieses[0].points().len() {
                    eprintln!("serieses have different lengths");
                    continue;
                }
            }
            for i in 0..serieses[0].points().len() {
                let time = params.window.start() + params.step * i as i64;
                print!("{}", time.to_rfc3339());
                for series in serieses.iter() {
                    print!(" {}", series.points()[i]);
                }
                println!();
            }
        } else if let Some(line) = line.strip_prefix("window:") {
            let args = match shvar::split(line.trim()) {
                Ok(args) => args,
                Err(_) => {
                    eprintln!("shell quote it");
                    continue;
                }
            };
            let args = args.iter().map(|s| s.as_str()).collect::<Vec<_>>();
            let (opts, free) =
                WindowOptions::from_arguments("USAGE: --start X --limit Y --step Z", &args);
            if !free.is_empty() {
                eprintln!("unexpected arguments: {free:?}");
                continue;
            }
            fn query_params(opts: WindowOptions) -> Option<QueryParams> {
                let Some(start) = Time::from_secs(opts.start) else {
                    eprintln!("invalid start");
                    return None;
                };
                let Some(limit) = Time::from_secs(opts.limit) else {
                    eprintln!("invalid limit");
                    return None;
                };
                let Some(window) = Window::new(start, limit) else {
                    eprintln!("invalid window");
                    return None;
                };
                let Some(step) = Time::from_secs(opts.step) else {
                    eprintln!("invalid step");
                    return None;
                };
                QueryParams::new(window, step)
            }
            if let Some(p) = query_params(opts) {
                params = p;
            } else {
                eprintln!("invalid window options");
            }
        } else {
            eprintln!("query: ...");
            eprintln!("window: --start S --limit L --step T");
            eprintln!("not understood: {line}");
        }
    }
}
