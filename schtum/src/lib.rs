#![doc = include_str!("../README.md")]

use std::collections::BTreeMap;
use std::fmt::{Debug, Display};
use std::io::Read;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use biometrics::{Counter, Gauge};
use buffertk::{Unpacker, stack_pack};
use prototk::SError as PrototkError;
use prototk_derive::Message;
use utf8path::Path;

///////////////////////////////////////////// Constants ////////////////////////////////////////////

/// The file (or Kubernetes data key) that holds the domain header.
pub const DOMAIN_FILE: &str = "_domain";

/// The default size, in bytes, of generated key material.
pub const DEFAULT_MATERIAL_BYTES: usize = 32;

/// The `accept_until` of a version that has no successor.  Rotation bounds it.
pub const NEVER: u64 = u64::MAX;

/// The default distribution lag between writing a rotation and it taking effect.
pub const DEFAULT_LAG: u64 = 300;

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static DOMAIN_WRITTEN_AT: Gauge = Gauge::new("schtum.domain.written_at");
static STAGED_COUNT: Gauge = Gauge::new("schtum.staged.count");
static PRIMARY_FLIPS: Counter = Counter::new("schtum.primary.flips");
static RELOAD_COUNT: Counter = Counter::new("schtum.reload.count");
static RELOAD_FAILURES: Counter = Counter::new("schtum.reload.failures");
static VERIFY_UNKNOWN_KID: Counter = Counter::new("schtum.verify.unknown_kid");
static VERIFY_EXPIRED_KID: Counter = Counter::new("schtum.verify.expired_kid");

/// Register schtum's sensors.  Sensors are process-wide; a process that opens several domains
/// reports the most recently reloaded domain's `written_at`.
pub fn register_biometrics(collector: &biometrics::Collector) {
    collector.register_gauge(&DOMAIN_WRITTEN_AT);
    collector.register_gauge(&STAGED_COUNT);
    collector.register_counter(&PRIMARY_FLIPS);
    collector.register_counter(&RELOAD_COUNT);
    collector.register_counter(&RELOAD_FAILURES);
    collector.register_counter(&VERIFY_UNKNOWN_KID);
    collector.register_counter(&VERIFY_EXPIRED_KID);
}

/////////////////////////////////////////////// Error //////////////////////////////////////////////

/// The error cases schtum can encounter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    /// An I/O error against `path`.
    Io {
        /// The path being read or written.
        path: Path<'static>,
        /// The underlying error, as text.
        what: String,
    },
    /// Bytes could not be decoded as exactly one message.
    InvalidEncoding {
        /// A diagnostic description of the decoding problem.
        what: String,
    },
    /// A ring name is not of the form `[-._a-zA-Z0-9]+`, or is reserved.
    InvalidRingName {
        /// The offending name.
        name: String,
    },
    /// A ring with this name already exists in the manifest.
    DuplicateRing {
        /// The ring name.
        name: String,
    },
    /// No ring with this name exists in the manifest.
    UnknownRing {
        /// The ring name.
        name: String,
    },
    /// No version has reached its `primary_from`, or the primary is dead.
    NoPrimary {
        /// The ring name.
        ring: String,
    },
    /// A rotation is already staged; only one may be in flight (I4).
    RotationInFlight {
        /// The ring name.
        ring: String,
        /// The staged version.
        staged: VersionId,
    },
    /// The ring fails its invariants.
    Lint {
        /// The ring name.
        ring: String,
        /// Every violation found.
        violations: Vec<Violation>,
    },
    /// Secure random material could not be generated.
    RandomGenerationFailed,
    /// A secret reference could not be parsed.
    InvalidSecretRef {
        /// A diagnostic description.
        what: String,
    },
    /// A duration could not be parsed.
    InvalidDuration {
        /// The offending text.
        what: String,
    },
    /// A kid lookup failed.
    Lookup(Lookup),
}

impl Display for Error {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Error::Io { path, what } => write!(fmt, "io error on {path}: {what}"),
            Error::InvalidEncoding { what } => write!(fmt, "invalid encoding: {what}"),
            Error::InvalidRingName { name } => write!(fmt, "invalid ring name: {name:?}"),
            Error::DuplicateRing { name } => write!(fmt, "duplicate ring: {name}"),
            Error::UnknownRing { name } => write!(fmt, "unknown ring: {name}"),
            Error::NoPrimary { ring } => write!(fmt, "ring {ring} has no primary"),
            Error::RotationInFlight { ring, staged } => {
                write!(fmt, "ring {ring} already has version {staged} staged")
            }
            Error::Lint { ring, violations } => {
                write!(fmt, "ring {ring} fails lint:")?;
                for violation in violations {
                    write!(fmt, "\n  {violation}")?;
                }
                Ok(())
            }
            Error::RandomGenerationFailed => write!(fmt, "random generation failed"),
            Error::InvalidSecretRef { what } => write!(fmt, "invalid secret ref: {what}"),
            Error::InvalidDuration { what } => write!(fmt, "invalid duration: {what:?}"),
            Error::Lookup(lookup) => write!(fmt, "{lookup}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<Lookup> for Error {
    fn from(lookup: Lookup) -> Self {
        Error::Lookup(lookup)
    }
}

fn io_error(path: &Path<'_>, err: std::io::Error) -> Error {
    Error::Io {
        path: path.clone().into_owned(),
        what: err.to_string(),
    }
}

////////////////////////////////////////////// Lookup //////////////////////////////////////////////

/// Why a kid lookup failed.  The two cases alert differently: `Unknown` means this holder is behind
/// the fleet or the kid is forged; `Expired` means the artifact outlived its key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Lookup {
    /// No version with this id exists in the ring.
    Unknown(VersionId),
    /// The version exists but its `accept_until` has passed.
    Expired(VersionId),
}

impl Display for Lookup {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Lookup::Unknown(id) => write!(fmt, "unknown kid {id}"),
            Lookup::Expired(id) => write!(fmt, "expired kid {id}"),
        }
    }
}

/////////////////////////////////////////////// clock //////////////////////////////////////////////

/// The current time in unix seconds.  Wall clocks are trusted by design.
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Parse a duration such as `90s`, `5m`, `24h`, `30d`, or `2w` into seconds.  A bare integer is
/// seconds.
pub fn parse_duration(text: &str) -> Result<u64, Error> {
    let text = text.trim();
    let err = || Error::InvalidDuration {
        what: text.to_string(),
    };
    let (digits, mult) = match text.chars().last() {
        Some('s') => (&text[..text.len() - 1], 1),
        Some('m') => (&text[..text.len() - 1], 60),
        Some('h') => (&text[..text.len() - 1], 3600),
        Some('d') => (&text[..text.len() - 1], 86400),
        Some('w') => (&text[..text.len() - 1], 7 * 86400),
        Some(c) if c.is_ascii_digit() => (text, 1),
        _ => return Err(err()),
    };
    let n: u64 = digits.parse().map_err(|_| err())?;
    n.checked_mul(mult).ok_or_else(err)
}

/// Render seconds as a compact duration.
pub fn format_duration(mut secs: u64) -> String {
    if secs == 0 {
        return "0s".to_string();
    }
    let mut out = String::new();
    for (unit, size) in [
        ("w", 7 * 86400),
        ("d", 86400),
        ("h", 3600),
        ("m", 60),
        ("s", 1),
    ] {
        if secs >= size {
            out.push_str(&format!("{}{}", secs / size, unit));
            secs %= size;
        }
    }
    out
}

///////////////////////////////////////////// VersionId ////////////////////////////////////////////

/// Identifies a version within a ring.  Monotone; never reused.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VersionId(pub u64);

impl Display for VersionId {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(fmt, "v{}", self.0)
    }
}

impl From<u64> for VersionId {
    fn from(x: u64) -> Self {
        VersionId(x)
    }
}

////////////////////////////////////////////// Material ////////////////////////////////////////////

/// Key material.  Scrubbed on drop, redacted in Debug, compared in constant time, never cloned.
#[derive(Default, Message)]
pub struct Material {
    #[prototk(1, bytes)]
    bytes: Vec<u8>,
}

impl Material {
    /// Wrap existing bytes.
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    /// Generate `len` random bytes from the operating system's CSPRNG.
    pub fn random(len: usize) -> Result<Self, Error> {
        let mut bytes = vec![0u8; len];
        let mut urandom =
            std::fs::File::open("/dev/urandom").map_err(|_| Error::RandomGenerationFailed)?;
        urandom
            .read_exact(&mut bytes)
            .map_err(|_| Error::RandomGenerationFailed)?;
        Ok(Self { bytes })
    }

    /// The material.  Callers should not copy it further than they must.
    pub fn expose(&self) -> &[u8] {
        &self.bytes
    }

    /// The length of the material in bytes.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// True if the material is empty.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    fn scrub(&mut self) {
        for b in self.bytes.iter_mut() {
            // SAFETY: writing zero to a valid, exclusively borrowed u8.
            unsafe { std::ptr::write_volatile(b, 0) };
        }
        std::sync::atomic::compiler_fence(Ordering::SeqCst);
    }
}

impl Drop for Material {
    fn drop(&mut self) {
        self.scrub();
    }
}

impl Debug for Material {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(fmt, "Material([redacted; {} bytes])", self.bytes.len())
    }
}

impl Eq for Material {}

impl PartialEq for Material {
    fn eq(&self, other: &Material) -> bool {
        constant_time_eq(&self.bytes, &other.bytes)
    }
}

/// Compare two byte strings without short-circuiting on the first difference.  Length is not
/// hidden.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut acc = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        acc |= x ^ y;
    }
    acc == 0
}

/////////////////////////////////////////////// State //////////////////////////////////////////////

/// The clock-derived state of a version.  Never stored.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State {
    /// `now < primary_from`.  Accepted, not yet produced under.
    Staged,
    /// The latest version whose `primary_from <= now`.
    Primary,
    /// Superseded but still accepted: `now < accept_until`.
    Retired,
    /// `accept_until <= now`.  Not accepted; eligible for gc.
    Dead,
}

impl Display for State {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let s = match self {
            State::Staged => "staged",
            State::Primary => "primary",
            State::Retired => "retired",
            State::Dead => "dead",
        };
        fmt.write_str(s)
    }
}

////////////////////////////////////////////// Version /////////////////////////////////////////////

/// One version of a ring's material with its schedule.
#[derive(Default, Message)]
pub struct Version {
    #[prototk(1, uint64)]
    id: u64,
    #[prototk(2, message)]
    material: Material,
    #[prototk(3, uint64)]
    primary_from: u64,
    #[prototk(4, uint64)]
    accept_until: u64,
}

/// A view of a version without its material.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VersionView {
    /// The version id.
    pub id: VersionId,
    /// Unix seconds from which this version is primary.
    pub primary_from: u64,
    /// Unix seconds until which this version is accepted.  [NEVER] if unbounded.
    pub accept_until: u64,
    /// Bytes of material.
    pub material_len: usize,
}

impl VersionView {
    /// The clock-derived state, given whether this version is the ring's primary at `now`.
    fn state_at(&self, now: u64, is_primary: bool) -> State {
        if self.accept_until <= now {
            State::Dead
        } else if now < self.primary_from {
            State::Staged
        } else if is_primary {
            State::Primary
        } else {
            State::Retired
        }
    }
}

///////////////////////////////////////////// Violation ////////////////////////////////////////////

/// A ring invariant that does not hold.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Violation {
    /// The ring has no versions.
    NoVersions,
    /// The ring name is not valid.
    InvalidName {
        /// The name.
        name: String,
    },
    /// Version ids are not strictly increasing (I2).
    IdsNotIncreasing {
        /// The earlier version.
        prev: VersionId,
        /// The later version.
        next: VersionId,
    },
    /// `primary_from` is not strictly increasing with id (I2).
    PrimaryFromNotIncreasing {
        /// The earlier version.
        prev: VersionId,
        /// The later version.
        next: VersionId,
    },
    /// A version is not accepted for `max_artifact_ttl` after its successor becomes primary (I3).
    AcceptWindowTooShort {
        /// The version whose window is too short.
        id: VersionId,
        /// The minimum acceptable `accept_until`.
        required: u64,
        /// The actual `accept_until`.
        actual: u64,
    },
    /// The newest version has a bounded `accept_until`; only rotation may bound it.
    NewestBounded {
        /// The newest version.
        id: VersionId,
    },
    /// More than one version is staged (I4).
    MultipleStaged {
        /// How many.
        count: usize,
    },
    /// A version has empty material.
    EmptyMaterial {
        /// The version.
        id: VersionId,
    },
}

impl Display for Violation {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Violation::NoVersions => write!(fmt, "ring has no versions"),
            Violation::InvalidName { name } => write!(fmt, "invalid ring name {name:?}"),
            Violation::IdsNotIncreasing { prev, next } => {
                write!(fmt, "ids not increasing: {prev} then {next}")
            }
            Violation::PrimaryFromNotIncreasing { prev, next } => {
                write!(fmt, "primary_from not increasing: {prev} then {next}")
            }
            Violation::AcceptWindowTooShort {
                id,
                required,
                actual,
            } => write!(
                fmt,
                "{id} accept_until={actual} is before required {required}"
            ),
            Violation::NewestBounded { id } => {
                write!(fmt, "newest version {id} has bounded accept_until")
            }
            Violation::MultipleStaged { count } => {
                write!(fmt, "{count} versions staged; at most one allowed")
            }
            Violation::EmptyMaterial { id } => write!(fmt, "{id} has empty material"),
        }
    }
}

/// Ring names must be valid Kubernetes data keys and filenames: `[-._a-zA-Z0-9]+`, not starting
/// with `.` or `_`.
pub fn valid_ring_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && !name.starts_with('_')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

//////////////////////////////////////////////// Ring //////////////////////////////////////////////

/// A named sequence of versions.  The unit of rotation.
#[derive(Default, Message)]
pub struct Ring {
    #[prototk(1, string)]
    name: String,
    #[prototk(2, uint64)]
    max_artifact_ttl: u64,
    #[prototk(3, message)]
    versions: Vec<Version>,
}

impl Ring {
    /// Create an empty ring.  `max_artifact_ttl` is the longest anything produced under a version
    /// remains valid; it bounds how long a retired version stays accepted.
    pub fn new(name: &str, max_artifact_ttl: u64) -> Result<Self, Error> {
        if !valid_ring_name(name) {
            return Err(Error::InvalidRingName {
                name: name.to_string(),
            });
        }
        Ok(Self {
            name: name.to_string(),
            max_artifact_ttl,
            versions: Vec::new(),
        })
    }

    /// The ring name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The maximum artifact TTL in seconds.
    pub fn max_artifact_ttl(&self) -> u64 {
        self.max_artifact_ttl
    }

    /// Encode to prototk bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        stack_pack(self).to_vec()
    }

    /// Decode exactly one ring.  Trailing bytes are rejected.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        let mut unpacker = Unpacker::new(bytes);
        let ring: Ring =
            unpacker
                .unpack()
                .map_err(|error: PrototkError| Error::InvalidEncoding {
                    what: error.to_string(),
                })?;
        if !unpacker.is_empty() {
            return Err(Error::InvalidEncoding {
                what: format!("{} trailing bytes", unpacker.remain().len()),
            });
        }
        Ok(ring)
    }

    /// Views of every version, oldest first, without material.
    pub fn versions(&self) -> impl Iterator<Item = VersionView> + '_ {
        self.versions.iter().map(|v| VersionView {
            id: VersionId(v.id),
            primary_from: v.primary_from,
            accept_until: v.accept_until,
            material_len: v.material.len(),
        })
    }

    fn primary_index_at(&self, now: u64) -> Option<usize> {
        self.versions
            .iter()
            .rposition(|v| v.primary_from <= now)
            .filter(|&idx| now < self.versions[idx].accept_until)
    }

    /// The version to produce under at `now`: the latest with `primary_from <= now`.
    pub fn primary_at(&self, now: u64) -> Result<(VersionId, &Material), Error> {
        match self.primary_index_at(now) {
            Some(idx) => {
                let v = &self.versions[idx];
                Ok((VersionId(v.id), &v.material))
            }
            None => Err(Error::NoPrimary {
                ring: self.name.clone(),
            }),
        }
    }

    /// [Ring::primary_at] using the wall clock.
    pub fn primary(&self) -> Result<(VersionId, &Material), Error> {
        self.primary_at(now())
    }

    /// Material for a kid, if the version exists and is accepted at `now`.  Staged versions are
    /// accepted: a peer whose clock is slightly ahead may already produce under one.
    pub fn get_at(&self, id: VersionId, now: u64) -> Result<&Material, Lookup> {
        match self.versions.iter().find(|v| v.id == id.0) {
            None => {
                VERIFY_UNKNOWN_KID.click();
                Err(Lookup::Unknown(id))
            }
            Some(v) if v.accept_until <= now => {
                VERIFY_EXPIRED_KID.click();
                Err(Lookup::Expired(id))
            }
            Some(v) => Ok(&v.material),
        }
    }

    /// [Ring::get_at] using the wall clock.
    pub fn get(&self, id: VersionId) -> Result<&Material, Lookup> {
        self.get_at(id, now())
    }

    /// The state of a version at `now`.
    pub fn state_at(&self, id: VersionId, now: u64) -> Option<State> {
        let primary = self.primary_index_at(now);
        self.versions
            .iter()
            .enumerate()
            .find(|(_, v)| v.id == id.0)
            .map(|(idx, v)| {
                VersionView {
                    id,
                    primary_from: v.primary_from,
                    accept_until: v.accept_until,
                    material_len: v.material.len(),
                }
                .state_at(now, primary == Some(idx))
            })
    }

    /// Every version with its state at `now`.
    pub fn states_at(&self, now: u64) -> Vec<(VersionView, State)> {
        let primary = self.primary_index_at(now);
        self.versions()
            .enumerate()
            .map(|(idx, view)| (view, view.state_at(now, primary == Some(idx))))
            .collect()
    }

    /// The number of staged versions at `now`.
    pub fn staged_at(&self, now: u64) -> usize {
        self.versions
            .iter()
            .filter(|v| now < v.primary_from)
            .count()
    }

    /// Stage a new version.  In one write: the new version becomes primary at `now + lag`, and the
    /// current newest version stops being accepted `max_artifact_ttl` after that.
    pub fn rotate(&mut self, now: u64, lag: u64, material: Material) -> Result<VersionId, Error> {
        if let Some(staged) = self.versions.iter().find(|v| now < v.primary_from) {
            return Err(Error::RotationInFlight {
                ring: self.name.clone(),
                staged: VersionId(staged.id),
            });
        }
        let primary_from = now.saturating_add(lag);
        let id = self.versions.last().map(|v| v.id + 1).unwrap_or(1);
        if let Some(prev) = self.versions.last_mut() {
            prev.accept_until = primary_from.saturating_add(self.max_artifact_ttl);
        }
        self.versions.push(Version {
            id,
            material,
            primary_from,
            accept_until: NEVER,
        });
        Ok(VersionId(id))
    }

    /// Versions that are dead at `now`.  Never includes the primary.
    pub fn dead_at(&self, now: u64) -> Vec<VersionId> {
        self.versions
            .iter()
            .filter(|v| v.accept_until <= now)
            .map(|v| VersionId(v.id))
            .collect()
    }

    /// Remove dead versions.  Returns what was removed.
    pub fn gc(&mut self, now: u64) -> Vec<VersionId> {
        let dead = self.dead_at(now);
        self.versions.retain(|v| now < v.accept_until);
        dead
    }

    /// Check every invariant at `now`.  Empty means the ring is well-formed.
    pub fn lint(&self, now: u64) -> Vec<Violation> {
        let mut out = Vec::new();
        if !valid_ring_name(&self.name) {
            out.push(Violation::InvalidName {
                name: self.name.clone(),
            });
        }
        if self.versions.is_empty() {
            out.push(Violation::NoVersions);
            return out;
        }
        for w in self.versions.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            if a.id >= b.id {
                out.push(Violation::IdsNotIncreasing {
                    prev: VersionId(a.id),
                    next: VersionId(b.id),
                });
            }
            if a.primary_from >= b.primary_from {
                out.push(Violation::PrimaryFromNotIncreasing {
                    prev: VersionId(a.id),
                    next: VersionId(b.id),
                });
            }
            let required = b.primary_from.saturating_add(self.max_artifact_ttl);
            if a.accept_until < required {
                out.push(Violation::AcceptWindowTooShort {
                    id: VersionId(a.id),
                    required,
                    actual: a.accept_until,
                });
            }
        }
        if let Some(last) = self.versions.last()
            && last.accept_until != NEVER
        {
            out.push(Violation::NewestBounded {
                id: VersionId(last.id),
            });
        }
        let staged = self.staged_at(now);
        if staged > 1 {
            out.push(Violation::MultipleStaged { count: staged });
        }
        for v in &self.versions {
            if v.material.is_empty() {
                out.push(Violation::EmptyMaterial {
                    id: VersionId(v.id),
                });
            }
        }
        out
    }

    /// Return `Err(Error::Lint)` if lint finds anything.
    pub fn check(&self, now: u64) -> Result<(), Error> {
        let violations = self.lint(now);
        if violations.is_empty() {
            Ok(())
        } else {
            Err(Error::Lint {
                ring: self.name.clone(),
                violations,
            })
        }
    }
}

impl Debug for Ring {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        fmt.debug_struct("Ring")
            .field("name", &self.name)
            .field("max_artifact_ttl", &self.max_artifact_ttl)
            .field("versions", &self.versions().collect::<Vec<_>>())
            .finish()
    }
}

/////////////////////////////////////////// DomainHeader ///////////////////////////////////////////

/// The domain header, stored as [DOMAIN_FILE].  `written_at` is the fleet-wide lag reference.
#[derive(Clone, Debug, Default, Eq, Message, PartialEq)]
pub struct DomainHeader {
    #[prototk(1, uint64)]
    written_at: u64,
    #[prototk(2, string)]
    rings: Vec<String>,
}

impl DomainHeader {
    /// When the domain was written, in unix seconds.
    pub fn written_at(&self) -> u64 {
        self.written_at
    }

    /// The ring names the header claims.
    pub fn rings(&self) -> &[String] {
        &self.rings
    }

    /// Encode to prototk bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        stack_pack(self).to_vec()
    }

    /// Decode exactly one header.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        let mut unpacker = Unpacker::new(bytes);
        let header: DomainHeader =
            unpacker
                .unpack()
                .map_err(|error: PrototkError| Error::InvalidEncoding {
                    what: error.to_string(),
                })?;
        if !unpacker.is_empty() {
            return Err(Error::InvalidEncoding {
                what: format!("{} trailing bytes", unpacker.remain().len()),
            });
        }
        Ok(header)
    }
}

///////////////////////////////////////////// Manifest /////////////////////////////////////////////

/// A complete domain: header plus rings.  This is the editable form the CLI manipulates and the
/// immutable snapshot a [Domain] serves.  An image is its serialized form: a map from file name (or
/// Kubernetes data key) to bytes, which every store speaks.
#[derive(Debug, Default)]
pub struct Manifest {
    written_at: u64,
    rings: BTreeMap<String, Ring>,
}

impl Manifest {
    /// An empty manifest.
    pub fn new() -> Self {
        Self::default()
    }

    /// When this manifest was last written, in unix seconds.
    pub fn written_at(&self) -> u64 {
        self.written_at
    }

    /// Ring names, sorted.
    pub fn ring_names(&self) -> impl Iterator<Item = &str> + '_ {
        self.rings.keys().map(String::as_str)
    }

    /// A ring by name.
    pub fn ring(&self, name: &str) -> Option<&Ring> {
        self.rings.get(name)
    }

    /// A ring by name, mutably.
    pub fn ring_mut(&mut self, name: &str) -> Result<&mut Ring, Error> {
        self.rings.get_mut(name).ok_or_else(|| Error::UnknownRing {
            name: name.to_string(),
        })
    }

    /// Add a ring.  Fails on duplicates.
    pub fn add_ring(&mut self, ring: Ring) -> Result<(), Error> {
        if self.rings.contains_key(&ring.name) {
            return Err(Error::DuplicateRing { name: ring.name });
        }
        self.rings.insert(ring.name.clone(), ring);
        Ok(())
    }

    /// Remove a ring entirely.
    pub fn remove_ring(&mut self, name: &str) -> Result<Ring, Error> {
        self.rings.remove(name).ok_or_else(|| Error::UnknownRing {
            name: name.to_string(),
        })
    }

    /// Lint every ring.  Returns (ring name, violations) for rings with violations.
    pub fn lint(&self, now: u64) -> Vec<(String, Vec<Violation>)> {
        self.rings
            .values()
            .map(|r| (r.name.clone(), r.lint(now)))
            .filter(|(_, v)| !v.is_empty())
            .collect()
    }

    /// Return the first lint failure as an error.
    pub fn check(&self, now: u64) -> Result<(), Error> {
        for ring in self.rings.values() {
            ring.check(now)?;
        }
        Ok(())
    }

    /// Total staged versions across rings at `now`.
    pub fn staged_at(&self, now: u64) -> usize {
        self.rings.values().map(|r| r.staged_at(now)).sum()
    }

    /// Serialize.  Sets `written_at` to `now` and runs lint first; a manifest that fails lint is
    /// never written.
    pub fn to_image(&mut self, now: u64) -> Result<BTreeMap<String, Vec<u8>>, Error> {
        self.check(now)?;
        self.written_at = now;
        let mut image = BTreeMap::new();
        for (name, ring) in &self.rings {
            image.insert(name.clone(), ring.to_bytes());
        }
        let header = DomainHeader {
            written_at: now,
            rings: self.rings.keys().cloned().collect(),
        };
        image.insert(DOMAIN_FILE.to_string(), header.to_bytes());
        Ok(image)
    }

    /// Deserialize.  Keys not named in the header are ignored, which is how Kubernetes' `..data`
    /// bookkeeping and stray files stay harmless.
    pub fn from_image(image: &BTreeMap<String, Vec<u8>>) -> Result<Self, Error> {
        let header_bytes = image
            .get(DOMAIN_FILE)
            .ok_or_else(|| Error::InvalidEncoding {
                what: format!("missing {DOMAIN_FILE}"),
            })?;
        let header = DomainHeader::from_bytes(header_bytes)?;
        let mut rings = BTreeMap::new();
        for name in &header.rings {
            let bytes = image.get(name).ok_or_else(|| Error::InvalidEncoding {
                what: format!("header names ring {name:?} but it is absent"),
            })?;
            let ring = Ring::from_bytes(bytes)?;
            if ring.name != *name {
                return Err(Error::InvalidEncoding {
                    what: format!("ring file {name:?} contains ring {:?}", ring.name),
                });
            }
            rings.insert(name.clone(), ring);
        }
        Ok(Self {
            written_at: header.written_at,
            rings,
        })
    }

    /// Read a directory: [DOMAIN_FILE] plus one file per ring it names.
    pub fn read_dir(path: &Path<'_>) -> Result<Self, Error> {
        let header_path = path.join(DOMAIN_FILE);
        let header_bytes =
            std::fs::read(header_path.as_std_path()).map_err(|e| io_error(&header_path, e))?;
        let header = DomainHeader::from_bytes(&header_bytes)?;
        let mut image = BTreeMap::new();
        for name in &header.rings {
            if !valid_ring_name(name) {
                return Err(Error::InvalidRingName { name: name.clone() });
            }
            let ring_path = path.join(name.as_str());
            let bytes =
                std::fs::read(ring_path.as_std_path()).map_err(|e| io_error(&ring_path, e))?;
            image.insert(name.clone(), bytes);
        }
        image.insert(DOMAIN_FILE.to_string(), header_bytes);
        Self::from_image(&image)
    }

    /// Write a directory.  Ring files are written first via temp-and-rename; [DOMAIN_FILE] is
    /// written last so a watcher that sees the new header sees the new rings.
    pub fn write_dir(&mut self, path: &Path<'_>, now: u64) -> Result<(), Error> {
        let image = self.to_image(now)?;
        std::fs::create_dir_all(path.as_std_path()).map_err(|e| io_error(path, e))?;
        let write = |name: &str, bytes: &[u8]| -> Result<(), Error> {
            let tmp = path.join(format!(".{name}.tmp"));
            let dst = path.join(name);
            std::fs::write(tmp.as_std_path(), bytes).map_err(|e| io_error(&tmp, e))?;
            std::fs::rename(tmp.as_std_path(), dst.as_std_path()).map_err(|e| io_error(&dst, e))
        };
        for (name, bytes) in &image {
            if name != DOMAIN_FILE {
                write(name, bytes)?;
            }
        }
        write(DOMAIN_FILE, &image[DOMAIN_FILE])
    }
}

////////////////////////////////////////////// Domain //////////////////////////////////////////////

/// A read-only, reloadable view of a domain directory.  Producers fetch the ring per operation so a
/// reload mid-flight is a rotation, not a race.
pub struct Domain {
    path: Path<'static>,
    header_bytes: RwLock<Vec<u8>>,
    manifest: RwLock<Arc<Manifest>>,
}

impl Domain {
    /// Open a directory.
    pub fn open(path: impl Into<Path<'static>>) -> Result<Arc<Self>, Error> {
        let path = path.into();
        let manifest = Manifest::read_dir(&path)?;
        let header_bytes = std::fs::read(path.join(DOMAIN_FILE).as_std_path()).unwrap_or_default();
        DOMAIN_WRITTEN_AT.set(manifest.written_at as f64);
        Ok(Arc::new(Self {
            path,
            header_bytes: RwLock::new(header_bytes),
            manifest: RwLock::new(Arc::new(manifest)),
        }))
    }

    /// The directory.
    pub fn path(&self) -> &Path<'static> {
        &self.path
    }

    /// The current manifest.
    pub fn manifest(&self) -> Arc<Manifest> {
        Arc::clone(&self.manifest.read().unwrap())
    }

    /// A ring by name.  Holds the whole manifest alive; drop it after use.
    pub fn ring(&self, name: &str) -> Option<RingRef> {
        let manifest = self.manifest();
        if manifest.ring(name).is_some() {
            Some(RingRef {
                manifest,
                name: name.to_string(),
            })
        } else {
            None
        }
    }

    /// `written_at` of the loaded manifest.
    pub fn written_at(&self) -> u64 {
        self.manifest().written_at
    }

    /// Re-read the directory and swap the manifest.
    pub fn reload(&self) -> Result<(), Error> {
        match Manifest::read_dir(&self.path) {
            Ok(manifest) => {
                let header_bytes =
                    std::fs::read(self.path.join(DOMAIN_FILE).as_std_path()).unwrap_or_default();
                DOMAIN_WRITTEN_AT.set(manifest.written_at as f64);
                *self.manifest.write().unwrap() = Arc::new(manifest);
                *self.header_bytes.write().unwrap() = header_bytes;
                RELOAD_COUNT.click();
                Ok(())
            }
            Err(err) => {
                RELOAD_FAILURES.click();
                Err(err)
            }
        }
    }

    /// Reload only if [DOMAIN_FILE] changed.  Returns whether a reload happened.
    pub fn reload_if_changed(&self) -> Result<bool, Error> {
        let header_path = self.path.join(DOMAIN_FILE);
        let current =
            std::fs::read(header_path.as_std_path()).map_err(|e| io_error(&header_path, e))?;
        if current == *self.header_bytes.read().unwrap() {
            return Ok(false);
        }
        self.reload()?;
        Ok(true)
    }

    /// Spawn a thread that polls every `interval`, reloads on change, and keeps the staged-count
    /// gauge and primary-flip counter current.  Stops when the returned [Watcher] drops.
    pub fn watch(self: &Arc<Self>, interval: Duration) -> Watcher {
        let stop = Arc::new(AtomicBool::new(false));
        let domain = Arc::clone(self);
        let stop2 = Arc::clone(&stop);
        let handle = std::thread::spawn(move || {
            let mut primaries: BTreeMap<String, Option<VersionId>> = BTreeMap::new();
            while !stop2.load(Ordering::Relaxed) {
                let _ = domain.reload_if_changed();
                let now = now();
                let manifest = domain.manifest();
                STAGED_COUNT.set(manifest.staged_at(now) as f64);
                let mut next = BTreeMap::new();
                for (name, ring) in &manifest.rings {
                    let primary = ring.primary_at(now).ok().map(|(id, _)| id);
                    if let Some(prev) = primaries.get(name)
                        && *prev != primary
                    {
                        PRIMARY_FLIPS.click();
                    }
                    next.insert(name.clone(), primary);
                }
                primaries = next;
                std::thread::sleep(interval);
            }
        });
        Watcher {
            stop,
            handle: Some(handle),
        }
    }
}

/// A ring pinned inside a manifest snapshot.
pub struct RingRef {
    manifest: Arc<Manifest>,
    name: String,
}

impl std::ops::Deref for RingRef {
    type Target = Ring;

    fn deref(&self) -> &Ring {
        self.manifest
            .ring(&self.name)
            .expect("RingRef constructed from a present ring")
    }
}

/// Handle to a watch thread.  Dropping it stops the thread.
pub struct Watcher {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

///////////////////////////////////////////// SecretRef ////////////////////////////////////////////

/// A reference to where secrets live, for rc.conf and arrrg.  Config carries references; values
/// are resolved at runtime.
///
/// - `dir:/run/secrets/db` — a domain directory (the only form that live-reloads).
/// - `file:/etc/schtum/db.ring` — a single ring file, wrapped as a one-ring domain.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum SecretRef {
    /// Nothing specified.
    #[default]
    Unspecified,
    /// A domain directory.
    Dir(Path<'static>),
    /// A single ring file.
    File(Path<'static>),
}

impl SecretRef {
    /// Resolve to a manifest.  `Dir` reads the directory; `File` wraps the ring with `written_at`
    /// taken from the file's mtime.
    pub fn resolve(&self) -> Result<Manifest, Error> {
        match self {
            SecretRef::Unspecified => Err(Error::InvalidSecretRef {
                what: "unspecified".to_string(),
            }),
            SecretRef::Dir(path) => Manifest::read_dir(path),
            SecretRef::File(path) => {
                let bytes = std::fs::read(path.as_std_path()).map_err(|e| io_error(path, e))?;
                let ring = Ring::from_bytes(&bytes)?;
                let written_at = std::fs::metadata(path.as_std_path())
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let mut manifest = Manifest::new();
                manifest.written_at = written_at;
                manifest.add_ring(ring)?;
                Ok(manifest)
            }
        }
    }
}

impl FromStr for SecretRef {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        if s.is_empty() {
            return Ok(SecretRef::Unspecified);
        }
        if let Some(rest) = s.strip_prefix("dir:") {
            Ok(SecretRef::Dir(Path::from(rest).into_owned()))
        } else if let Some(rest) = s.strip_prefix("file:") {
            Ok(SecretRef::File(Path::from(rest).into_owned()))
        } else {
            Err(Error::InvalidSecretRef {
                what: format!("expected dir:PATH or file:PATH, got {s:?}"),
            })
        }
    }
}

impl Display for SecretRef {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            SecretRef::Unspecified => Ok(()),
            SecretRef::Dir(p) => write!(fmt, "dir:{p}"),
            SecretRef::File(p) => write!(fmt, "file:{p}"),
        }
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    fn mat(b: u8) -> Material {
        Material::from_bytes(vec![b; 32])
    }

    const TTL: u64 = 3600;
    const LAG: u64 = 300;

    fn two_version_ring() -> Ring {
        let mut ring = Ring::new("db", TTL).unwrap();
        ring.rotate(1000, LAG, mat(1)).unwrap(); // v1 primary_from 1300
        ring.rotate(2000, LAG, mat(2)).unwrap(); // v2 primary_from 2300; v1 accept_until 5900
        ring
    }

    #[test]
    fn rotation_is_one_write() {
        let ring = two_version_ring();
        let views: Vec<_> = ring.versions().collect();
        assert_eq!(views[0].id, VersionId(1));
        assert_eq!(views[0].primary_from, 1300);
        assert_eq!(views[0].accept_until, 2300 + TTL);
        assert_eq!(views[1].id, VersionId(2));
        assert_eq!(views[1].primary_from, 2300);
        assert_eq!(views[1].accept_until, NEVER);
        assert!(ring.lint(2000).is_empty());
    }

    #[test]
    fn states_flip_on_the_clock() {
        let ring = two_version_ring();
        assert!(matches!(
            ring.primary_at(1000),
            Err(Error::NoPrimary { .. })
        ));
        assert_eq!(ring.primary_at(1300).unwrap().0, VersionId(1));
        assert_eq!(ring.primary_at(2299).unwrap().0, VersionId(1));
        assert_eq!(ring.state_at(VersionId(2), 2299), Some(State::Staged));
        assert_eq!(ring.primary_at(2300).unwrap().0, VersionId(2));
        assert_eq!(ring.state_at(VersionId(1), 2300), Some(State::Retired));
        assert_eq!(ring.state_at(VersionId(1), 5899), Some(State::Retired));
        assert_eq!(ring.state_at(VersionId(1), 5900), Some(State::Dead));
    }

    #[test]
    fn staged_versions_are_accepted() {
        let ring = two_version_ring();
        assert!(ring.get_at(VersionId(2), 2000).is_ok());
        assert_eq!(
            ring.get_at(VersionId(1), 5900),
            Err(Lookup::Expired(VersionId(1)))
        );
        assert_eq!(
            ring.get_at(VersionId(9), 5900),
            Err(Lookup::Unknown(VersionId(9)))
        );
    }

    #[test]
    fn one_rotation_in_flight() {
        let mut ring = two_version_ring();
        assert!(matches!(
            ring.rotate(2100, LAG, mat(3)),
            Err(Error::RotationInFlight {
                staged: VersionId(2),
                ..
            })
        ));
        assert_eq!(ring.rotate(2300, LAG, mat(3)).unwrap(), VersionId(3));
    }

    #[test]
    fn gc_removes_dead_only() {
        let mut ring = two_version_ring();
        assert!(ring.gc(5899).is_empty());
        assert_eq!(ring.gc(5900), vec![VersionId(1)]);
        assert_eq!(ring.versions().count(), 1);
        assert_eq!(ring.primary_at(5900).unwrap().0, VersionId(2));
    }

    #[test]
    fn lint_catches_short_window() {
        let mut ring = two_version_ring();
        ring.versions[0].accept_until = 2300 + TTL - 1;
        assert_eq!(
            ring.lint(2000),
            vec![Violation::AcceptWindowTooShort {
                id: VersionId(1),
                required: 2300 + TTL,
                actual: 2300 + TTL - 1,
            }]
        );
    }

    #[test]
    fn round_trip_bytes() {
        let ring = two_version_ring();
        let bytes = ring.to_bytes();
        let back = Ring::from_bytes(&bytes).unwrap();
        assert_eq!(back.name(), "db");
        assert_eq!(back.max_artifact_ttl(), TTL);
        assert_eq!(
            back.versions().collect::<Vec<_>>(),
            ring.versions().collect::<Vec<_>>()
        );
        assert_eq!(back.get_at(VersionId(1), 2000).unwrap(), &mat(1));
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(matches!(
            Ring::from_bytes(&trailing),
            Err(Error::InvalidEncoding { .. })
        ));
    }

    #[test]
    fn manifest_image_round_trip() {
        let mut manifest = Manifest::new();
        manifest.add_ring(two_version_ring()).unwrap();
        let mut other = Ring::new("api-key", 60).unwrap();
        other.rotate(1000, 0, mat(7)).unwrap();
        manifest.add_ring(other).unwrap();
        let image = manifest.to_image(4242).unwrap();
        assert_eq!(image.len(), 3);
        let back = Manifest::from_image(&image).unwrap();
        assert_eq!(back.written_at(), 4242);
        assert_eq!(back.ring_names().collect::<Vec<_>>(), vec!["api-key", "db"]);
    }

    #[test]
    fn manifest_refuses_to_write_lint_failures() {
        let mut manifest = Manifest::new();
        manifest.add_ring(Ring::new("empty", 60).unwrap()).unwrap();
        assert!(matches!(manifest.to_image(1), Err(Error::Lint { .. })));
    }

    #[test]
    fn dir_round_trip_and_reload() {
        let tmp = std::env::temp_dir().join(format!("schtum-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let path = Path::from(tmp.to_str().unwrap()).into_owned();
        let mut manifest = Manifest::new();
        manifest.add_ring(two_version_ring()).unwrap();
        manifest.write_dir(&path, 4242).unwrap();
        let domain = Domain::open(path.clone()).unwrap();
        assert_eq!(domain.written_at(), 4242);
        assert_eq!(
            domain.ring("db").unwrap().primary_at(2300).unwrap().0,
            VersionId(2)
        );
        assert!(domain.ring("nope").is_none());
        assert!(!domain.reload_if_changed().unwrap());
        // rotate on disk; the domain sees it on reload
        let mut edited = Manifest::read_dir(&path).unwrap();
        edited
            .ring_mut("db")
            .unwrap()
            .rotate(2300, LAG, mat(3))
            .unwrap();
        edited.write_dir(&path, 4300).unwrap();
        assert!(domain.reload_if_changed().unwrap());
        assert_eq!(domain.written_at(), 4300);
        assert_eq!(domain.ring("db").unwrap().versions().count(), 3);
        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn watcher_reloads_on_header_change() {
        let tmp = std::env::temp_dir().join(format!("schtum-watch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let path = Path::from(tmp.to_str().unwrap()).into_owned();
        let mut manifest = Manifest::new();
        manifest.add_ring(two_version_ring()).unwrap();
        manifest.write_dir(&path, 4242).unwrap();
        let domain = Domain::open(path.clone()).unwrap();
        let watcher = domain.watch(Duration::from_millis(20));
        let mut edited = Manifest::read_dir(&path).unwrap();
        edited
            .ring_mut("db")
            .unwrap()
            .rotate(2300, LAG, mat(3))
            .unwrap();
        edited.write_dir(&path, 4300).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while domain.written_at() != 4300 && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(domain.written_at(), 4300);
        assert_eq!(domain.ring("db").unwrap().versions().count(), 3);
        drop(watcher);
        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn material_hygiene() {
        let m = mat(9);
        assert_eq!(format!("{m:?}"), "Material([redacted; 32 bytes])");
        assert_eq!(m, mat(9));
        assert_ne!(m, mat(8));
        assert!(Material::random(32).unwrap() != Material::random(32).unwrap());
    }

    #[test]
    fn durations() {
        assert_eq!(parse_duration("90").unwrap(), 90);
        assert_eq!(parse_duration("90s").unwrap(), 90);
        assert_eq!(parse_duration("5m").unwrap(), 300);
        assert_eq!(parse_duration("24h").unwrap(), 86400);
        assert_eq!(parse_duration("30d").unwrap(), 30 * 86400);
        assert_eq!(parse_duration("2w").unwrap(), 14 * 86400);
        assert!(parse_duration("5x").is_err());
        assert!(parse_duration("").is_err());
        assert_eq!(format_duration(0), "0s");
        assert_eq!(format_duration(3661), "1h1m1s");
        assert_eq!(format_duration(30 * 86400), "4w2d");
    }

    #[test]
    fn secret_refs() {
        assert_eq!("".parse::<SecretRef>().unwrap(), SecretRef::Unspecified);
        let r: SecretRef = "dir:/run/secrets/db".parse().unwrap();
        assert_eq!(r.to_string(), "dir:/run/secrets/db");
        let r: SecretRef = "file:/etc/x.ring".parse().unwrap();
        assert_eq!(r.to_string(), "file:/etc/x.ring");
        assert!("env:X".parse::<SecretRef>().is_err());
    }

    #[test]
    fn ring_names() {
        assert!(valid_ring_name("db"));
        assert!(valid_ring_name("api-key.v2_x"));
        assert!(!valid_ring_name(""));
        assert!(!valid_ring_name("_domain"));
        assert!(!valid_ring_name(".hidden"));
        assert!(!valid_ring_name("a/b"));
        assert!(Ring::new("a b", 1).is_err());
    }
}
