use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::process::{Command, Stdio};

use arrrg::CommandLine;
use arrrg_derive::CommandLine;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use utf8path::Path;

use schtum::{
    DEFAULT_LAG, DEFAULT_MATERIAL_BYTES, DOMAIN_FILE, Manifest, Material, NEVER, Ring, State,
    format_duration, parse_duration,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn top_level_help() {
    println!(
        "USAGE:
  schtum init     <store>
  schtum add-ring --ttl DUR [--lag DUR] [--material-file PATH|-] [--material-bytes N] <store> <ring>
  schtum rotate   [--lag DUR] [--material-file PATH|-] [--material-bytes N] <store> <ring>...
  schtum gc       [--dry-run] <store>
  schtum show     <store>
  schtum lint     <store>

Options precede the store and ring arguments (arrrg's canonical order).

STORES:
  dir:PATH                 A domain directory.
  k8s:NAMESPACE/NAME       A Kubernetes Secret, via kubectl, read-CAS-write on resourceVersion.
  k8s:NAME                 Same, in the current namespace.

DURATIONS:  90s  5m  24h  30d  2w  (bare integers are seconds)

A rotation is one write.  The new version becomes primary at now+lag on every holder at once; the
old version stays accepted for the ring's max_artifact_ttl after that, then gc may remove it.
schtum writes and exits; whether the fleet loads the write before primary_from is an alert.
"
    );
}

//////////////////////////////////////////////// Store /////////////////////////////////////////////

/// An image is a domain serialized as file name -> bytes; the token is an opaque version for CAS.
type Image = BTreeMap<String, Vec<u8>>;
type Token = Option<String>;

/// A store round-trips an image and its token.
trait Store {
    fn load(&self) -> Result<(Image, Token)>;
    fn save(&self, image: Image, token: Token) -> Result<()>;
    fn create(&self, image: Image) -> Result<()>;
}

fn parse_store(s: &str) -> Result<Box<dyn Store>> {
    if let Some(path) = s.strip_prefix("dir:") {
        Ok(Box::new(DirStore {
            path: Path::from(path).into_owned(),
        }))
    } else if let Some(rest) = s.strip_prefix("k8s:") {
        let (namespace, name) = match rest.split_once('/') {
            Some((ns, name)) => (Some(ns.to_string()), name.to_string()),
            None => (None, rest.to_string()),
        };
        if name.is_empty() {
            return Err("k8s: store needs a secret name".into());
        }
        Ok(Box::new(KubectlStore { namespace, name }))
    } else {
        Err(format!("unknown store {s:?}; expected dir:PATH or k8s:[NAMESPACE/]NAME").into())
    }
}

struct DirStore {
    path: Path<'static>,
}

impl Store for DirStore {
    fn load(&self) -> Result<(Image, Token)> {
        let mut manifest = Manifest::read_dir(&self.path)?;
        // read_dir validated the image; re-serialize without touching written_at.
        let now = manifest.written_at();
        Ok((manifest.to_image(now)?, None))
    }

    fn save(&self, image: Image, _: Token) -> Result<()> {
        let mut manifest = Manifest::from_image(&image)?;
        manifest.write_dir(&self.path, manifest.written_at())?;
        Ok(())
    }

    fn create(&self, image: Image) -> Result<()> {
        if self.path.join(DOMAIN_FILE).exists()? {
            return Err(format!("{} already exists", self.path).into());
        }
        self.save(image, None)
    }
}

struct KubectlStore {
    namespace: Option<String>,
    name: String,
}

impl KubectlStore {
    fn kubectl(&self, args: &[&str], stdin: Option<&[u8]>) -> Result<Vec<u8>> {
        let mut cmd = Command::new("kubectl");
        if let Some(ns) = &self.namespace {
            cmd.arg("-n").arg(ns);
        }
        cmd.args(args);
        cmd.stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        });
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| format!("spawn kubectl: {e}"))?;
        if let Some(bytes) = stdin {
            child.stdin.take().unwrap().write_all(bytes)?;
        }
        let out = child.wait_with_output()?;
        if !out.status.success() {
            return Err(format!(
                "kubectl {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            )
            .into());
        }
        Ok(out.stdout)
    }
}

/// Merge an image into a Secret's `data`: overwrite managed keys, drop rings the old header named
/// that the new one does not, and leave unrelated keys alone.
fn merge_secret_data(
    data: &mut serde_json::Map<String, serde_json::Value>,
    image: &BTreeMap<String, Vec<u8>>,
) -> Result<()> {
    if let Some(b64) = data.get(DOMAIN_FILE).and_then(|v| v.as_str()) {
        let header = schtum::DomainHeader::from_bytes(&BASE64.decode(b64)?)?;
        for old in header.rings() {
            if !image.contains_key(old) {
                data.remove(old);
            }
        }
    }
    for (k, v) in image {
        data.insert(k.clone(), serde_json::Value::String(BASE64.encode(v)));
    }
    Ok(())
}

impl Store for KubectlStore {
    fn load(&self) -> Result<(Image, Token)> {
        let raw = self.kubectl(&["get", "secret", &self.name, "-o", "json"], None)?;
        let obj: serde_json::Value = serde_json::from_slice(&raw)?;
        let token = obj
            .pointer("/metadata/resourceVersion")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let mut image = BTreeMap::new();
        if let Some(data) = obj.get("data").and_then(|d| d.as_object()) {
            for (k, v) in data {
                if let Some(b64) = v.as_str() {
                    image.insert(k.clone(), BASE64.decode(b64)?);
                }
            }
        }
        // Validate as a domain; unrelated keys are ignored by from_image.
        Manifest::from_image(&image)?;
        Ok((image, token))
    }

    fn save(&self, image: Image, token: Token) -> Result<()> {
        let raw = self.kubectl(&["get", "secret", &self.name, "-o", "json"], None)?;
        let mut obj: serde_json::Value = serde_json::from_slice(&raw)?;
        let current = obj
            .pointer("/metadata/resourceVersion")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        if token.is_some() && current != token {
            return Err("secret changed since it was read; re-run".into());
        }
        if let Some(meta) = obj.get_mut("metadata").and_then(|m| m.as_object_mut()) {
            meta.remove("managedFields");
        }
        let data = obj
            .as_object_mut()
            .ok_or("secret is not an object")?
            .entry("data")
            .or_insert_with(|| serde_json::Value::Object(Default::default()));
        let data = data.as_object_mut().ok_or("secret data is not an object")?;
        merge_secret_data(data, &image)?;
        let body = serde_json::to_vec(&obj)?;
        // replace with resourceVersion set is a compare-and-swap at the apiserver.
        self.kubectl(&["replace", "-f", "-"], Some(&body))?;
        Ok(())
    }

    fn create(&self, image: Image) -> Result<()> {
        let mut data = serde_json::Map::new();
        merge_secret_data(&mut data, &image)?;
        let mut metadata = serde_json::Map::new();
        metadata.insert("name".into(), self.name.clone().into());
        if let Some(ns) = &self.namespace {
            metadata.insert("namespace".into(), ns.clone().into());
        }
        let obj = serde_json::json!({
            "apiVersion": "v1",
            "kind": "Secret",
            "type": "Opaque",
            "metadata": metadata,
            "data": data,
        });
        self.kubectl(&["create", "-f", "-"], Some(&serde_json::to_vec(&obj)?))?;
        Ok(())
    }
}

//////////////////////////////////////////// subcommands ///////////////////////////////////////////

#[derive(CommandLine, Debug, Default, Eq, PartialEq)]
struct TopOptions {}

#[derive(CommandLine, Debug, Default, Eq, PartialEq)]
struct InitOptions {}

#[derive(CommandLine, Debug, Default, Eq, PartialEq)]
struct AddRingOptions {
    #[arrrg(
        required,
        "Longest lifetime of anything produced under a version.",
        "DUR"
    )]
    ttl: String,
    #[arrrg(
        optional,
        "Delay before the first version becomes primary (default 5m).",
        "DUR"
    )]
    lag: String,
    #[arrrg(
        optional,
        "Read material from PATH (- for stdin) instead of generating it.",
        "PATH"
    )]
    material_file: String,
    #[arrrg(optional, "Bytes of material to generate (default 32).", "N")]
    material_bytes: String,
}

#[derive(CommandLine, Debug, Default, Eq, PartialEq)]
struct RotateOptions {
    #[arrrg(
        optional,
        "Delay before the new version becomes primary (default 5m).",
        "DUR"
    )]
    lag: String,
    #[arrrg(
        optional,
        "Read material from PATH (- for stdin) instead of generating it.",
        "PATH"
    )]
    material_file: String,
    #[arrrg(optional, "Bytes of material to generate (default 32).", "N")]
    material_bytes: String,
}

#[derive(CommandLine, Debug, Default, Eq, PartialEq)]
struct GcOptions {
    #[arrrg(flag, "Report what would be removed without writing.")]
    dry_run: bool,
}

#[derive(CommandLine, Debug, Default, Eq, PartialEq)]
struct ShowOptions {}

#[derive(CommandLine, Debug, Default, Eq, PartialEq)]
struct LintOptions {}

enum Cmd {
    Init(Vec<String>),
    AddRing(AddRingOptions, Vec<String>),
    Rotate(RotateOptions, Vec<String>),
    Gc(GcOptions, Vec<String>),
    Show(Vec<String>),
    Lint(Vec<String>),
}

fn lag_of(s: &str) -> Result<u64> {
    if s.is_empty() {
        Ok(DEFAULT_LAG)
    } else {
        Ok(parse_duration(s)?)
    }
}

fn material_of(file: &str, bytes: &str, count: usize) -> Result<Material> {
    if !file.is_empty() {
        if count > 0 {
            return Err("--material-file reads one material; use it for a single ring".into());
        }
        let mut buf = Vec::new();
        if file == "-" {
            std::io::stdin().read_to_end(&mut buf)?;
        } else {
            buf = std::fs::read(file)?;
        }
        if buf.is_empty() {
            return Err("material is empty".into());
        }
        return Ok(Material::from_bytes(buf));
    }
    let n = if bytes.is_empty() {
        DEFAULT_MATERIAL_BYTES
    } else {
        bytes.parse::<usize>()?
    };
    if n == 0 {
        return Err("--material-bytes must be positive".into());
    }
    Ok(Material::random(n)?)
}

fn load(store: &dyn Store) -> Result<(Manifest, Token)> {
    let (image, token) = store.load()?;
    Ok((Manifest::from_image(&image)?, token))
}

fn save(store: &dyn Store, mut manifest: Manifest, token: Token, now: u64) -> Result<()> {
    let image = manifest.to_image(now)?;
    store.save(image, token)
}

fn cmd_init(free: Vec<String>) -> Result<()> {
    let [store] = free.as_slice() else {
        return Err("init takes exactly one store".into());
    };
    let store = parse_store(store)?;
    let mut manifest = Manifest::new();
    store.create(manifest.to_image(schtum::now())?)
}

fn cmd_add_ring(opts: AddRingOptions, free: Vec<String>) -> Result<()> {
    let [store, ring] = free.as_slice() else {
        return Err("add-ring takes a store and a ring name".into());
    };
    let store = parse_store(store)?;
    let now = schtum::now();
    let (mut manifest, token) = load(store.as_ref())?;
    let mut r = Ring::new(ring, parse_duration(&opts.ttl)?)?;
    let id = r.rotate(
        now,
        lag_of(&opts.lag)?,
        material_of(&opts.material_file, &opts.material_bytes, 0)?,
    )?;
    manifest.add_ring(r)?;
    save(store.as_ref(), manifest, token, now)?;
    println!(
        "{ring}: added; {id} primary at +{}",
        format_duration(lag_of(&opts.lag)?)
    );
    Ok(())
}

fn cmd_rotate(opts: RotateOptions, free: Vec<String>) -> Result<()> {
    let Some((store, rings)) = free.split_first() else {
        return Err("rotate takes a store and one or more ring names".into());
    };
    if rings.is_empty() {
        return Err("rotate takes one or more ring names".into());
    }
    let store = parse_store(store)?;
    let now = schtum::now();
    let lag = lag_of(&opts.lag)?;
    let (mut manifest, token) = load(store.as_ref())?;
    let mut report = Vec::new();
    for (idx, name) in rings.iter().enumerate() {
        let material = material_of(&opts.material_file, &opts.material_bytes, idx)?;
        let ring = manifest.ring_mut(name)?;
        let ttl = ring.max_artifact_ttl();
        let id = ring.rotate(now, lag, material)?;
        report.push(format!(
            "{name}: {id} primary at {} (+{}); predecessor accepted until {} (+{})",
            now + lag,
            format_duration(lag),
            now + lag + ttl,
            format_duration(lag + ttl),
        ));
    }
    save(store.as_ref(), manifest, token, now)?;
    for line in report {
        println!("{line}");
    }
    Ok(())
}

fn cmd_gc(opts: GcOptions, free: Vec<String>) -> Result<()> {
    let [store] = free.as_slice() else {
        return Err("gc takes exactly one store".into());
    };
    let store = parse_store(store)?;
    let now = schtum::now();
    let (mut manifest, token) = load(store.as_ref())?;
    let names: Vec<String> = manifest.ring_names().map(str::to_string).collect();
    let mut removed = 0;
    for name in &names {
        let ring = manifest.ring_mut(name)?;
        let dead = if opts.dry_run {
            ring.dead_at(now)
        } else {
            ring.gc(now)
        };
        for id in dead {
            removed += 1;
            println!(
                "{name}: {id} dead{}",
                if opts.dry_run {
                    " (would remove; safe to revoke externally)"
                } else {
                    " (removed; safe to revoke externally)"
                }
            );
        }
    }
    if removed == 0 {
        println!("nothing dead");
    } else if !opts.dry_run {
        save(store.as_ref(), manifest, token, now)?;
    }
    Ok(())
}

fn cmd_show(free: Vec<String>) -> Result<()> {
    let [store] = free.as_slice() else {
        return Err("show takes exactly one store".into());
    };
    let store = parse_store(store)?;
    let now = schtum::now();
    let (manifest, token) = load(store.as_ref())?;
    println!(
        "written_at {} ({} ago){}",
        manifest.written_at(),
        format_duration(now.saturating_sub(manifest.written_at())),
        token
            .map(|t| format!(", resourceVersion {t}"))
            .unwrap_or_default()
    );
    for name in manifest.ring_names() {
        let ring = manifest.ring(name).unwrap();
        println!(
            "ring {name}  max_artifact_ttl {}",
            format_duration(ring.max_artifact_ttl())
        );
        for (view, state) in ring.states_at(now) {
            let when = |t: u64| -> String {
                if t == NEVER {
                    "never".to_string()
                } else if t > now {
                    format!("{t} (in {})", format_duration(t - now))
                } else {
                    format!("{t} ({} ago)", format_duration(now - t))
                }
            };
            let marker = match state {
                State::Primary => "*",
                _ => " ",
            };
            println!(
                "  {marker} {:<4} {:<8} {} bytes  primary_from {}  accept_until {}",
                view.id.to_string(),
                state.to_string(),
                view.material_len,
                when(view.primary_from),
                when(view.accept_until),
            );
        }
        let violations = ring.lint(now);
        for v in violations {
            println!("  ! {v}");
        }
    }
    Ok(())
}

fn cmd_lint(free: Vec<String>) -> Result<()> {
    let [store] = free.as_slice() else {
        return Err("lint takes exactly one store".into());
    };
    let store = parse_store(store)?;
    let (manifest, _) = load(store.as_ref())?;
    let failures = manifest.lint(schtum::now());
    for (name, violations) in &failures {
        for v in violations {
            println!("{name}: {v}");
        }
    }
    if failures.is_empty() {
        println!("ok");
        Ok(())
    } else {
        Err(format!("{} ring(s) fail lint", failures.len()).into())
    }
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("help") || std::env::args().len() == 1 {
        top_level_help();
        std::process::exit(if std::env::args().len() == 1 { 254 } else { 0 });
    }
    if let Err(err) = run() {
        eprintln!("schtum: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let (_top, free) = TopOptions::from_command_line_relaxed(
        "Usage: schtum <init|add-ring|rotate|gc|show|lint> [OPTIONS] <store> ...",
    );
    let cmd = arrrg::dispatch_subcommands_relaxed!(free, {
        "init" => InitOptions as _opts, rest => { Ok(Cmd::Init(rest)) },
        "add-ring" => AddRingOptions as opts, rest => { Ok(Cmd::AddRing(opts, rest)) },
        "rotate" => RotateOptions as opts, rest => { Ok(Cmd::Rotate(opts, rest)) },
        "gc" => GcOptions as opts, rest => { Ok(Cmd::Gc(opts, rest)) },
        "show" => ShowOptions as _opts, rest => { Ok(Cmd::Show(rest)) },
        "lint" => LintOptions as _opts, rest => { Ok(Cmd::Lint(rest)) },
    })
    .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    match cmd {
        Cmd::Init(free) => cmd_init(free),
        Cmd::AddRing(opts, free) => cmd_add_ring(opts, free),
        Cmd::Rotate(opts, free) => cmd_rotate(opts, free),
        Cmd::Gc(opts, free) => cmd_gc(opts, free),
        Cmd::Show(free) => cmd_show(free),
        Cmd::Lint(free) => cmd_lint(free),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_keeps_unrelated_keys_and_drops_removed_rings() {
        // old secret: header names ring "a" and "b"; unrelated key "tls.crt"
        let mut old = Manifest::new();
        for name in ["a", "b"] {
            let mut r = Ring::new(name, 60).unwrap();
            r.rotate(100, 0, Material::from_bytes(vec![1; 8])).unwrap();
            old.add_ring(r).unwrap();
        }
        let old_image = old.to_image(100).unwrap();
        let mut data = serde_json::Map::new();
        merge_secret_data(&mut data, &old_image).unwrap();
        data.insert(
            "tls.crt".into(),
            serde_json::Value::String(BASE64.encode(b"cert")),
        );
        assert_eq!(data.len(), 4);
        // new image drops "b"
        let mut new = Manifest::from_image(&old_image).unwrap();
        new.remove_ring("b").unwrap();
        let new_image = new.to_image(200).unwrap();
        merge_secret_data(&mut data, &new_image).unwrap();
        assert!(data.contains_key("a"));
        assert!(!data.contains_key("b"));
        assert!(data.contains_key("tls.crt"));
        assert!(data.contains_key(DOMAIN_FILE));
        assert_eq!(data.len(), 3);
    }
}
