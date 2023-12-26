//! This module implements a garbage-collecting cursor that compacts according to a
//! garbage-collection policy.
//!
//! This module is intended to be used within lsmtk where a KeyValueStore performs the act of
//! rewriting data according to a garbage-collection policy, and a second, separate, process is
//! responsible for unlinking the old files that served as inputs to the garbage collection after
//! verifying that the policy is upheld and no extra data is thrown away.
//!
//! This is intended to make sure the garbage collection policy is specified in two places.  By
//! specifying it twice, we can verify that the garbage collection mechanism doesn't break from one
//! release to the next.  The verifier or key-value store can be updated independently and skew
//! across releases of the code.  Thus, if there's an update to this code, it can be compared
//! against the old code.
//!
//! Consequently, we need some rules that allow us to garbage collect safely.o
//!
//! We start with the observation that just because garbage collection _can_ throw something away,
//! doesn't mean that it _will_ throw something away.  This gives rise to three rules:
//!
//! - When updating to a policy that retains more data, update the writer first.  The verifier will
//!   allow for the extra rows to be retained.
//! - When updating to a policy that deletes more data, update the verifier first.  The key-value
//!   store will retain excess data, but the verifier will allow that.
//! - When updating policies, there must always be an incremental path that gets followed, or else
//!   both policies must be updated together.

use std::fmt::{Display, Formatter, Write};
use std::num::NonZeroU64;

use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{digit1, multispace0},
    combinator::{all_consuming, cut, map, map_res, opt, recognize},
    error::{context, VerboseError, VerboseErrorKind},
    multi::separated_list0,
    sequence::{terminated, tuple},
    IResult, Offset,
};

use keyvalint::{compare_bytes, Cursor, KeyRef, KeyValuePair};

use super::Error;

////////////////////////////////////// GarbageCollectionPolicy /////////////////////////////////////

/// A GarbageCollectionPolicy specifies which data to retain and which data to compact-away.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GarbageCollectionPolicy {
    /// Retain at least this many versions of the data.
    ///
    /// Versions are defined as follows:
    ///
    /// - A non-tombstone value.
    /// - The oldest tombstone (lowest timestamp) in a sequence of tombstones.
    ///
    /// A sequence of `[TOMBSTONE@3, TOMBSTONE@2, TOMBSTONE@1, VALUE@0]` with `number == 2` will
    /// become `[TOMBSTONE@1, VALUE@0]`.
    ///
    /// A sequence of `[VALUE@3, TOMBSTONE@2, TOMBSTONE@1, TOMBSTONE@0]` with `number == 2` will
    /// become `[VALUE@3]` and the tombstones will be dropped.
    Versions {
        /// The minimum number of versions to retain.  After this many versions are retained, data
        /// may be thrown away.
        number: NonZeroU64,
    },
    /// Retain data fresher than this expiration threshold.
    ///
    /// A sequence of `[VALUE@3, TOMBSTONE@2, TOMBSTONE@1, TOMBSTONE@0]` with `now() - micros = 1`
    /// will become `[VALUE@3]` and the tombstones will be dropped.
    ///
    /// A sequence of `[TOMBSTONE@3, TOMBSTONE@2, TOMBSTONE@1, VALUE@0]` with `now() - micros = 1`
    /// will retain nothing.
    Expires {
        /// The number of microseconds in the past that specifies the threshold for data retention.
        micros: NonZeroU64,
    },
    /// Retain data when any of these predicates would retain data.
    Any(Vec<GarbageCollectionPolicy>),
    /// Retain data only when all of these predicates would retain data.
    All(Vec<GarbageCollectionPolicy>),
}

impl GarbageCollectionPolicy {
    /// Take a cursor _positioned at the first key to be considered for garbage collection_ and
    /// return a garbage collector that will run the cursor until it returns key() == None.
    pub fn collector<C: Cursor<Error = Error> + 'static>(
        &self,
        cursor: C,
        now_micros: u64,
    ) -> Result<GarbageCollector, Error> {
        let cursor: Box<dyn Cursor<Error = Error>> = Box::new(cursor) as _;
        let determiner = self.determiner(now_micros);
        let key_backing = if let Some(key) = cursor.key() {
            key.key.to_vec()
        } else {
            vec![]
        };
        let key_return = None;
        Ok(GarbageCollector {
            cursor,
            determiner,
            key_backing,
            key_return,
        })
    }

    fn determiner(&self, now_micros: u64) -> Box<dyn Determiner> {
        match self {
            Self::Versions { number } => Box::new(VersionsDeterminer::new(*number)),
            Self::Expires { micros } => {
                let threshold = now_micros.saturating_sub(micros.get());
                Box::new(ExpiresDeterminer::new(threshold))
            }
            Self::Any(any) => {
                let any = any
                    .iter()
                    .map(|p| p.determiner(now_micros))
                    .collect::<Vec<_>>();
                Box::new(AnyDeterminer::new(any))
            }
            Self::All(all) => {
                let all = all
                    .iter()
                    .map(|p| p.determiner(now_micros))
                    .collect::<Vec<_>>();
                Box::new(AllDeterminer::new(all))
            }
        }
    }
}

impl TryFrom<&str> for GarbageCollectionPolicy {
    type Error = ParseError;

    fn try_from(input: &str) -> Result<GarbageCollectionPolicy, ParseError> {
        parse_all(gc_policy)(input)
    }
}

impl std::str::FromStr for GarbageCollectionPolicy {
    type Err = ParseError;

    fn from_str(input: &str) -> Result<GarbageCollectionPolicy, ParseError> {
        GarbageCollectionPolicy::try_from(input)
    }
}

impl Display for GarbageCollectionPolicy {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Self::Versions { number } => {
                write!(fmt, "versions = {number}")
            }
            Self::Expires { micros } => {
                write!(fmt, "ttl_micros = {micros}")
            }
            Self::Any(any) => {
                write!(
                    fmt,
                    "any({})",
                    any.iter()
                        .map(|gc| gc.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            Self::All(all) => {
                write!(
                    fmt,
                    "all({})",
                    all.iter()
                        .map(|gc| gc.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        }
    }
}

///////////////////////////////////////// GarbageCollector /////////////////////////////////////////

/// Determine which keys in the constructed cursor should be retained.
/// Can only be built by the `collector` method on a policy.
pub struct GarbageCollector {
    cursor: Box<dyn Cursor<Error = Error>>,
    determiner: Box<dyn Determiner>,
    key_backing: Vec<u8>,
    key_return: Option<u64>,
}

impl GarbageCollector {
    /// Return the next key to be retained from garbage collection.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Result<Option<KeyRef>, Error> {
        if let Some(ts) = self.key_return.take() {
            return Ok(Some(KeyRef {
                key: &self.key_backing,
                timestamp: ts,
            }));
        }
        'iterating: loop {
            let mut tombstones = vec![];
            let mut kvp = match self.cursor.key_value() {
                Some(kvr) => KeyValuePair::from(kvr),
                None => {
                    break 'iterating;
                }
            };
            while compare_bytes(&self.key_backing, &kvp.key).is_eq() {
                if kvp.value.is_some() {
                    self.cursor.next()?;
                    if self.determiner.retain(&kvp.key, &tombstones, kvp.timestamp) {
                        return self.return_key(kvp, tombstones);
                    } else {
                        continue 'iterating;
                    }
                }
                tombstones.push(kvp.timestamp);
                self.cursor.next()?;
                kvp = match self.cursor.key_value() {
                    Some(kvr) => KeyValuePair::from(kvr),
                    None => {
                        break 'iterating;
                    }
                };
            }
            // The only way to get here is to have a different key than self.key_backing.
            // Copy the key to the key backing and go around the loop again.
            // Do not advance the cursor.
            // That will happen in the while loop above on the next iteration.
            self.key_backing.resize(kvp.key.len(), 0);
            self.key_backing.copy_from_slice(&kvp.key);
        }
        Ok(None)
    }

    fn return_key(
        &mut self,
        kvp: KeyValuePair,
        tombstones: Vec<u64>,
    ) -> Result<Option<KeyRef>, Error> {
        if !tombstones.is_empty() {
            // TODO(rescrv):  Possibly set key_backing?
            self.key_return = Some(kvp.timestamp);
            Ok(Some(KeyRef {
                key: &self.key_backing,
                timestamp: tombstones[tombstones.len() - 1],
            }))
        } else {
            self.key_return = None;
            Ok(Some(KeyRef {
                key: &self.key_backing,
                timestamp: kvp.timestamp,
            }))
        }
    }
}

/////////////////////////////////////////// nom, nom, nom //////////////////////////////////////////

type ParseResult<'a, T> = IResult<&'a str, T, VerboseError<&'a str>>;

/// An error when parsing the textual representation of a garbage collection policy.
#[derive(Clone, Eq, PartialEq)]
pub struct ParseError {
    string: String,
}

impl std::fmt::Debug for ParseError {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        writeln!(fmt, "{}", self.string)
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        writeln!(fmt, "{}", self.string)
    }
}

impl From<String> for ParseError {
    fn from(string: String) -> Self {
        Self { string }
    }
}

fn interpret_verbose_error(input: &'_ str, err: VerboseError<&'_ str>) -> ParseError {
    let mut result = String::new();
    let mut index = 0;
    for (substring, kind) in err.errors.iter() {
        let offset = input.offset(substring);
        let prefix = &input.as_bytes()[..offset];
        // Count the number of newlines in the first `offset` bytes of input
        let line_number = prefix.iter().filter(|&&b| b == b'\n').count() + 1;
        // Find the line that includes the subslice:
        // Find the *last* newline before the substring starts
        let line_begin = prefix
            .iter()
            .rev()
            .position(|&b| b == b'\n')
            .map(|pos| offset - pos)
            .unwrap_or(0);
        // Find the full line after that newline
        let line = input[line_begin..]
            .lines()
            .next()
            .unwrap_or(&input[line_begin..])
            .trim_end();
        // The (1-indexed) column number is the offset of our substring into that line
        let column_number = line.offset(substring) + 1;
        match kind {
            VerboseErrorKind::Char(c) => {
                if let Some(actual) = substring.chars().next() {
                    write!(
                        &mut result,
                        "{index}: at line {line_number}:\n\
                 {line}\n\
                 {caret:>column$}\n\
                 expected '{expected}', found {actual}\n\n",
                        index = index,
                        line_number = line_number,
                        line = line,
                        caret = '^',
                        column = column_number,
                        expected = c,
                        actual = actual,
                    )
                    .unwrap();
                } else {
                    write!(
                        &mut result,
                        "{index}: at line {line_number}:\n\
                 {line}\n\
                 {caret:>column$}\n\
                 expected '{expected}', got end of input\n\n",
                        index = index,
                        line_number = line_number,
                        line = line,
                        caret = '^',
                        column = column_number,
                        expected = c,
                    )
                    .unwrap();
                }
                index += 1;
            }
            VerboseErrorKind::Context(s) => {
                write!(
                    &mut result,
                    "{index}: at line {line_number}, in {context}:\n\
               {line}\n\
               {caret:>column$}\n\n",
                    index = index,
                    line_number = line_number,
                    context = s,
                    line = line,
                    caret = '^',
                    column = column_number,
                )
                .unwrap();
                index += 1;
            }
            // Swallow these.   They are ugly.
            VerboseErrorKind::Nom(_) => {}
        };
    }
    ParseError {
        string: result.trim().to_string(),
    }
}

fn parse_all<T, F: Fn(&str) -> ParseResult<T> + Copy>(
    f: F,
) -> impl Fn(&str) -> Result<T, ParseError> {
    move |input| {
        let (rem, t) = match all_consuming(f)(input) {
            Ok((rem, t)) => (rem, t),
            Err(err) => match err {
                nom::Err::Incomplete(_) => {
                    panic!("all_consuming combinator should be all consuming");
                }
                nom::Err::Error(err) | nom::Err::Failure(err) => {
                    return Err(interpret_verbose_error(input, err));
                }
            },
        };
        if rem.is_empty() {
            Ok(t)
        } else {
            panic!("all_consuming combinator should be all consuming");
        }
    }
}

fn ws0(input: &str) -> ParseResult<()> {
    map(multispace0, |_| ())(input)
}

fn parse_number(input: &str) -> Result<NonZeroU64, &'static str> {
    if let Ok(x) = str::parse::<u64>(input) {
        if let Some(x) = NonZeroU64::new(x) {
            Ok(x)
        } else {
            Err("must have non-zero number of versions")
        }
    } else {
        Err("invalid number")
    }
}

fn number_literal(input: &str) -> ParseResult<NonZeroU64> {
    context(
        "number literal",
        map_res(recognize(tuple((opt(tag("-")), digit1))), parse_number),
    )(input)
}

fn versions(input: &str) -> ParseResult<GarbageCollectionPolicy> {
    context(
        "versions",
        map(
            tuple((
                ws0,
                tag("versions"),
                cut(ws0),
                cut(tag("=")),
                cut(ws0),
                cut(number_literal),
                cut(ws0),
            )),
            |(_, _, _, _, _, number, _)| GarbageCollectionPolicy::Versions { number },
        ),
    )(input)
}

fn expires(input: &str) -> ParseResult<GarbageCollectionPolicy> {
    context(
        "expires",
        map(
            tuple((
                ws0,
                tag("ttl_micros"),
                cut(ws0),
                cut(tag("=")),
                cut(ws0),
                cut(number_literal),
                cut(ws0),
            )),
            |(_, _, _, _, _, micros, _)| GarbageCollectionPolicy::Expires { micros },
        ),
    )(input)
}

fn any(input: &str) -> ParseResult<GarbageCollectionPolicy> {
    context(
        "any",
        map(
            tuple((
                ws0,
                tag("any"),
                cut(ws0),
                cut(tag("(")),
                cut(ws0),
                terminated(separated_list0(tag(","), gc_policy), opt(tag(","))),
                cut(ws0),
                cut(tag(")")),
                cut(ws0),
            )),
            |(_, _, _, _, _, any, _, _, _)| GarbageCollectionPolicy::Any(any),
        ),
    )(input)
}

fn all(input: &str) -> ParseResult<GarbageCollectionPolicy> {
    context(
        "all",
        map(
            tuple((
                ws0,
                tag("all"),
                cut(ws0),
                cut(tag("(")),
                cut(ws0),
                terminated(separated_list0(tag(","), gc_policy), opt(tag(","))),
                cut(ws0),
                cut(tag(")")),
                cut(ws0),
            )),
            |(_, _, _, _, _, all, _, _, _)| GarbageCollectionPolicy::All(all),
        ),
    )(input)
}

fn gc_policy(input: &str) -> ParseResult<GarbageCollectionPolicy> {
    context(
        "garbage collection policy",
        alt((versions, expires, any, all)),
    )(input)
}

//////////////////////////////////////////// Determiner ////////////////////////////////////////////

/// Given a stream of sorted keys, indicate whether a key should be retained.
///
/// # Panics
///
/// Panics when the stream of keys is not sorted according to Sst sorting rules.
pub trait Determiner {
    /// Returns true iff the key, tombstones, and present value should be retained.
    fn retain(&mut self, key: &[u8], tombstones: &[u64], exists: u64) -> bool;
}

//////////////////////////////////////// VersionsDeterminer ////////////////////////////////////////

/// Determine when the specified number of versions has been retained.  Drop every subsequent key.
#[derive(Debug)]
struct VersionsDeterminer {
    // TODO(rescrv): NonZero number
    number: NonZeroU64,
    key: Vec<u8>,
    count: u64,
}

impl VersionsDeterminer {
    fn new(number: NonZeroU64) -> Self {
        Self {
            number,
            key: vec![],
            count: 0,
        }
    }
}

impl Determiner for VersionsDeterminer {
    fn retain(&mut self, key: &[u8], tombstones: &[u64], _: u64) -> bool {
        if !compare_bytes(&self.key, key).is_eq() {
            self.key.resize(key.len(), 0);
            self.key.copy_from_slice(key);
            if tombstones.is_empty() {
                self.count = 1;
                true
            } else {
                self.count = 2;
                self.count <= self.number.get()
            }
        } else {
            if tombstones.is_empty() {
                self.count += 1;
            } else {
                self.count += 2;
            }
            self.count <= self.number.get()
        }
    }
}

///////////////////////////////////////// ExpiresDeterminer ////////////////////////////////////////

/// Determine when data is older than an expiration threshold.  Drop every subsequent key.
struct ExpiresDeterminer {
    threshold: u64,
}

impl ExpiresDeterminer {
    fn new(threshold: u64) -> Self {
        Self { threshold }
    }
}

impl Determiner for ExpiresDeterminer {
    fn retain(&mut self, _: &[u8], _: &[u64], exists: u64) -> bool {
        exists >= self.threshold
    }
}

/////////////////////////////////////////// AnyDeterminer //////////////////////////////////////////

/// Determine when any of the determiners would keep a key.  Drop every subsequent key.
struct AnyDeterminer {
    any: Vec<Box<dyn Determiner>>,
}

impl AnyDeterminer {
    fn new(any: Vec<Box<dyn Determiner>>) -> Self {
        Self { any }
    }
}

impl Determiner for AnyDeterminer {
    fn retain(&mut self, key: &[u8], tombstones: &[u64], exists: u64) -> bool {
        let mut retain = false;
        for d in self.any.iter_mut() {
            retain |= d.retain(key, tombstones, exists);
        }
        retain
    }
}

/////////////////////////////////////////// AllDeterminer //////////////////////////////////////////

/// Determine when all of the determiners would keep a key.  Drop every subsequent key.
struct AllDeterminer {
    all: Vec<Box<dyn Determiner>>,
}

impl AllDeterminer {
    fn new(all: Vec<Box<dyn Determiner>>) -> Self {
        Self { all }
    }
}

impl Determiner for AllDeterminer {
    fn retain(&mut self, key: &[u8], tombstones: &[u64], exists: u64) -> bool {
        let mut retain = true;
        for d in self.all.iter_mut() {
            retain &= d.retain(key, tombstones, exists);
        }
        retain
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use keyvalint::KeyValuePair;

    use super::*;

    mod policy {
        use super::*;

        #[test]
        fn versions0() {
            assert!(GarbageCollectionPolicy::try_from("versions = 0").is_err());
        }

        #[test]
        fn versions1() {
            const POLICY: &str = "versions = 1";
            assert_eq!(
                POLICY,
                GarbageCollectionPolicy::try_from(POLICY)
                    .unwrap()
                    .to_string()
            );
            assert_eq!(
                GarbageCollectionPolicy::Versions {
                    number: NonZeroU64::new(1).unwrap()
                },
                GarbageCollectionPolicy::try_from(POLICY).unwrap()
            );
        }

        #[test]
        fn versions42() {
            const POLICY: &str = "versions = 42";
            assert_eq!(
                GarbageCollectionPolicy::Versions {
                    number: NonZeroU64::new(42).unwrap()
                },
                GarbageCollectionPolicy::try_from(POLICY).unwrap()
            );
        }

        #[test]
        fn expires0() {
            assert!(GarbageCollectionPolicy::try_from("ttl_micros = 0").is_err());
        }

        #[test]
        fn expires1() {
            const POLICY: &str = "ttl_micros = 1";
            assert_eq!(
                POLICY,
                GarbageCollectionPolicy::try_from(POLICY)
                    .unwrap()
                    .to_string()
            );
            assert_eq!(
                GarbageCollectionPolicy::Expires {
                    micros: NonZeroU64::new(1).unwrap()
                },
                GarbageCollectionPolicy::try_from(POLICY).unwrap()
            );
        }

        #[test]
        fn expires42() {
            const POLICY: &str = "ttl_micros = 42";
            assert_eq!(
                POLICY,
                GarbageCollectionPolicy::try_from(POLICY)
                    .unwrap()
                    .to_string()
            );
            assert_eq!(
                GarbageCollectionPolicy::Expires {
                    micros: NonZeroU64::new(42).unwrap()
                },
                GarbageCollectionPolicy::try_from(POLICY).unwrap()
            );
        }

        #[test]
        fn any() {
            const POLICY: &str = "any(versions = 1, ttl_micros = 42)";
            assert_eq!(
                POLICY,
                GarbageCollectionPolicy::try_from(POLICY)
                    .unwrap()
                    .to_string()
            );
            let policy = GarbageCollectionPolicy::Any(vec![
                GarbageCollectionPolicy::Versions {
                    number: NonZeroU64::new(1).unwrap(),
                },
                GarbageCollectionPolicy::Expires {
                    micros: NonZeroU64::new(42).unwrap(),
                },
            ]);
            assert_eq!(policy, GarbageCollectionPolicy::try_from(POLICY).unwrap());
        }

        #[test]
        fn all() {
            const POLICY: &str = "all(versions = 1, ttl_micros = 42)";
            assert_eq!(
                POLICY,
                GarbageCollectionPolicy::try_from(POLICY)
                    .unwrap()
                    .to_string()
            );
            let policy = GarbageCollectionPolicy::All(vec![
                GarbageCollectionPolicy::Versions {
                    number: NonZeroU64::new(1).unwrap(),
                },
                GarbageCollectionPolicy::Expires {
                    micros: NonZeroU64::new(42).unwrap(),
                },
            ]);
            assert_eq!(policy, GarbageCollectionPolicy::try_from(POLICY).unwrap());
        }
    }

    #[derive(Debug, Default)]
    struct SampleCursor {
        entries: Vec<KeyValuePair>,
        index: usize,
    }

    impl Cursor for SampleCursor {
        type Error = Error;

        fn next(&mut self) -> Result<(), Error> {
            if self.index < self.entries.len() {
                self.index += 1;
            }
            Ok(())
        }

        fn key(&self) -> Option<KeyRef<'_>> {
            if self.index < self.entries.len() {
                Some(KeyRef::from(&self.entries[self.index]))
            } else {
                None
            }
        }

        fn value(&self) -> Option<&[u8]> {
            if self.index < self.entries.len() {
                self.entries[self.index].value.as_deref()
            } else {
                None
            }
        }

        fn seek_to_first(&mut self) -> Result<(), Error> {
            unimplemented!()
        }

        fn seek_to_last(&mut self) -> Result<(), Error> {
            unimplemented!()
        }

        fn seek(&mut self, _: &[u8]) -> Result<(), Error> {
            unimplemented!()
        }

        fn prev(&mut self) -> Result<(), Error> {
            unimplemented!()
        }
    }

    macro_rules! sample_cursor {
        () => {
            SampleCursor::default()
        };
        ($($key:literal @ $ts:literal => $val:expr,)*) => {
            {
                let mut cursor = SampleCursor::default();
                $(
                    let v: Option::<&[u8]> = $val;
                    cursor.entries.push(KeyValuePair {
                        key: $key.to_vec(),
                        timestamp: $ts,
                        value: v.map(|v| v.to_vec()),
                    });
                )*
                cursor
            }
        };
    }

    fn test_expectation(
        keys: SampleCursor,
        mut expect: SampleCursor,
        policy: &str,
        now_micros: u64,
    ) {
        let policy = GarbageCollectionPolicy::try_from(policy).unwrap();
        let mut collector = policy.collector(keys, now_micros).unwrap();
        loop {
            let exp = expect.key();
            let got = collector.next().unwrap();
            match (&exp, &got) {
                (Some(exp), Some(got)) => {
                    assert_eq!(exp, got);
                }
                (None, None) => {
                    break;
                }
                (Some(exp), None) => {
                    panic!("dropped too much data: {exp:?}");
                }
                (None, Some(got)) => {
                    panic!("retained too much data: {got:?}");
                }
            }
            expect.next().unwrap();
        }
    }

    #[test]
    fn versions_example1() {
        let cursor = sample_cursor! {
            b"key" @ 4 => None,
            b"key" @ 3 => None,
            b"key" @ 2 => None,
            b"key" @ 1 => Some(b"value"),
        };
        let expectation = sample_cursor! {
            b"key" @ 2 => None,
            b"key" @ 1 => Some(b"value"),
        };
        let policy = "versions = 2";
        test_expectation(cursor, expectation, policy, 4);
    }

    #[test]
    fn versions_example2() {
        let cursor = sample_cursor! {
            b"key" @ 4 => Some(b"value"),
            b"key" @ 3 => None,
            b"key" @ 2 => None,
            b"key" @ 1 => None,
        };
        let expectation = sample_cursor! {
            b"key" @ 4 => Some(b"value"),
        };
        let policy = "versions = 2";
        test_expectation(cursor, expectation, policy, 4);
    }

    #[test]
    fn expires_example1() {
        let cursor = sample_cursor! {
            b"key" @ 4 => Some(b"value"),
            b"key" @ 3 => None,
            b"key" @ 2 => None,
            b"key" @ 1 => Some(b"drop"),
        };
        let expectation = sample_cursor! {
            b"key" @ 4 => Some(b"value"),
        };
        let policy = "ttl_micros = 2";
        test_expectation(cursor, expectation, policy, 4);
    }

    #[test]
    fn expires_example2() {
        let cursor = sample_cursor! {
            b"key" @ 4 => None,
            b"key" @ 3 => None,
            b"key" @ 2 => None,
            b"key" @ 1 => Some(b"drop"),
        };
        let expectation = sample_cursor! {};
        let policy = "ttl_micros = 2";
        test_expectation(cursor, expectation, policy, 4);
    }
}
