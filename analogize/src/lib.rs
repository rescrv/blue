#![doc = include_str!("../README.md")]

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::fs::{metadata, read_dir, rename, File, Metadata, OpenOptions};
use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{sleep, JoinHandle};
use std::time::Duration;

use chrono::{DateTime, DurationRound, TimeDelta, Utc};
use serde_json::Value;

use buffertk::Unpackable;
use mani::{Edit, Manifest, ManifestOptions};
use prototk::FieldNumber;
use scrunch::bit_vector::sparse::BitVector;
use scrunch::bit_vector::BitVector as BitVectorTrait;
use scrunch::{CompressedDocument, Document, RecordOffset};
use zerror::{iotoz, Z};
use zerror_core::ErrorCore;

mod parser;

/////////////////////////////////////////////// Error //////////////////////////////////////////////

#[derive(zerror_derive::Z)]
pub enum Error {
    Success {
        core: ErrorCore,
    },
    System {
        core: ErrorCore,
        kind: ErrorKind,
        what: String,
    },
    DirectoryNotFound {
        core: ErrorCore,
        what: String,
    },
    DirectoryAlreadyExists {
        core: ErrorCore,
        what: String,
    },
    Json {
        core: ErrorCore,
        what: String,
    },
    NotAnObject {
        core: ErrorCore,
        json: String,
    },
    Manifest {
        core: ErrorCore,
        what: mani::Error,
    },
    InvalidNumberLiteral {
        core: ErrorCore,
        as_str: String,
    },
    Parsing {
        core: ErrorCore,
        what: String,
    },
    InvalidSymbolTable {
        core: ErrorCore,
        line: String,
    },
    InvalidPath {
        core: ErrorCore,
        what: String,
    },
    InvalidTimestamp {
        core: ErrorCore,
        nanos: i64,
    },
    MissingTimestamp {
        core: ErrorCore,
        json: String,
    },
    Scrunch {
        core: ErrorCore,
        what: scrunch::Error,
    },
}

iotoz! {Error}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::System {
            core: ErrorCore::default(),
            kind: err.kind(),
            what: err.to_string(),
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Self::Json {
            core: ErrorCore::default(),
            what: err.to_string(),
        }
    }
}

impl From<mani::Error> for Error {
    fn from(err: mani::Error) -> Self {
        Self::Manifest {
            core: ErrorCore::default(),
            what: err,
        }
    }
}

impl From<scrunch::Error> for Error {
    fn from(err: scrunch::Error) -> Self {
        Self::Scrunch {
            core: ErrorCore::default(),
            what: err,
        }
    }
}

impl From<parser::ParseError> for Error {
    fn from(err: parser::ParseError) -> Self {
        Self::Parsing {
            core: ErrorCore::default(),
            what: err.what().to_string(),
        }
    }
}

//////////////////////////////////////////// SymbolTable ///////////////////////////////////////////

#[derive(Debug)]
pub struct SymbolTable {
    symbols: HashMap<String, u32>,
    next_symbol: u32,
}

impl SymbolTable {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        if !path.as_ref().exists() {
            return Ok(Self::default());
        }
        let file = File::open(path.as_ref())
            .as_z()
            .with_info("path", &path.as_ref().to_string_lossy())?;
        Self::from_reader(file)
    }

    pub fn from_reader<R: Read>(reader: R) -> Result<Self, Error> {
        let reader = BufReader::new(reader);
        let mut syms = SymbolTable::default();
        for line in reader.lines() {
            let line = line?;
            let line = line.trim();
            let mut pieces: Vec<&str> = line.rsplitn(2, ' ').collect();
            pieces.reverse();
            if pieces.len() != 2 {
                return Err(Error::InvalidSymbolTable {
                    core: ErrorCore::default(),
                    line: line.to_string(),
                });
            }
            let mangled = pieces[0].to_string();
            let symbol = u32::from_str(pieces[1]).map_err(|_| Error::InvalidNumberLiteral {
                core: ErrorCore::default(),
                as_str: pieces[1].to_string(),
            })?;
            syms.symbols.insert(mangled, symbol);
            syms.next_symbol = std::cmp::max(syms.next_symbol, symbol + 2);
        }
        Ok(syms)
    }

    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), Error> {
        let mut output = OpenOptions::new().create(true).write(true).open(path)?;
        let mut sorted = self.symbols.iter().collect::<Vec<_>>();
        sorted.sort();
        for (mangled, symbol) in sorted.into_iter() {
            writeln!(output, "{} {}", mangled, symbol)?;
        }
        output.flush()?;
        Ok(())
    }

    pub fn append_dummy_record(&mut self, text: &mut Vec<u32>) {
        text.push(u32::MAX);
    }

    pub fn translate(&mut self, object: &Value, text: &mut Vec<u32>) {
        self.translate_recursive(&object, "", text);
    }

    pub fn translate_query(&self, query: &Query) -> Vec<Vec<u32>> {
        self.translate_query_recursive(query, "")
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, u32)> {
        self.symbols.iter().map(|(k, v)| (k.as_ref(), *v))
    }

    pub fn markers(&self) -> impl Iterator<Item = (u32, u32)> {
        let mut markers = vec![];
        for (sym, text) in self.symbols.iter() {
            if sym.ends_with('t') || sym.ends_with('f') || sym.ends_with('n') || sym.ends_with('#')
            {
                markers.push((*text, *text));
            } else {
                markers.push((*text, *text + 1));
            }
        }
        markers.into_iter()
    }

    pub fn to_query(&self, text: &[u32]) -> Option<String> {
        if text.is_empty() {
            return None;
        }
        let mut start = self.reverse_lookup(text[0])?;
        let text: String = text.iter().filter_map(|x| char::from_u32(*x)).collect();
        let mut prefix = String::new();
        let mut suffix = String::new();
        let mut terminal = false;
        while !start.is_empty() {
            if terminal {
                return None;
            }
            match start.chars().next() {
                Some('t') => {
                    start = &start[1..];
                    prefix += "true";
                    terminal = true;
                }
                Some('f') => {
                    start = &start[1..];
                    prefix += "false";
                    terminal = true;
                }
                Some('n') => {
                    start = &start[1..];
                    prefix += "null";
                    terminal = true;
                }
                Some('#') => {
                    start = &start[1..];
                    prefix += "#";
                    terminal = true;
                }
                Some('s') => {
                    start = &start[1..];
                    let mut buf = vec![];
                    serde_json::to_writer(&mut buf, &text).ok()?;
                    prefix += std::str::from_utf8(&buf).ok()?;
                    terminal = true;
                }
                Some('o') => {
                    start = &start[1..];
                    prefix += "{";
                    suffix += "}";
                }
                Some('a') => {
                    start = &start[1..];
                    prefix += "[";
                    suffix += "]";
                }
                Some('k') => {
                    let len: String = start[1..]
                        .chars()
                        .take_while(|c| c.is_ascii_digit())
                        .collect();
                    start = &start[1 + len.len()..];
                    let len = usize::from_str(&len).ok()?;
                    let key = &start[..len];
                    start = &start[len..];
                    let mut buf = vec![];
                    serde_json::to_writer(&mut buf, &key).ok()?;
                    prefix += std::str::from_utf8(&buf).ok()?;
                    prefix += ":";
                }
                _ => {
                    return None;
                }
            }
        }
        let mut suffix: Vec<char> = suffix.chars().collect();
        suffix.reverse();
        let suffix: String = suffix.iter().collect();
        Some(prefix + &suffix)
    }

    fn translate_recursive(&mut self, object: &Value, symbol: &str, text: &mut Vec<u32>) {
        match object {
            Value::Bool(b) => {
                let symbol = symbol.to_string() + if *b { "t" } else { "f" };
                let sigma = self.lookup_symbol(&symbol);
                text.push(sigma);
            }
            Value::Null => {
                let symbol = symbol.to_string() + "n";
                let sigma = self.lookup_symbol(&symbol);
                text.push(sigma);
            }
            Value::Number(_) => {
                let symbol = symbol.to_string() + "#";
                let sigma = self.lookup_symbol(&symbol);
                text.push(sigma);
            }
            Value::String(s) => {
                let symbol = symbol.to_string() + "s";
                let sigma = self.lookup_symbol(&symbol);
                text.push(sigma);
                for c in s.chars() {
                    text.push(c as u32);
                }
                text.push(sigma + 1);
            }
            Value::Array(a) => {
                let symbol = symbol.to_string() + "a";
                let sigma = self.lookup_symbol(&symbol);
                text.push(sigma);
                for e in a.iter() {
                    self.translate_recursive(e, &symbol, text);
                }
                text.push(sigma + 1);
            }
            Value::Object(o) => {
                let symbol = symbol.to_string() + "o";
                let sigma = self.lookup_symbol(&symbol);
                text.push(sigma);
                for (k, v) in o.iter() {
                    let len = k.chars().count();
                    let symbol = format!("{}k{}{}", symbol, len, k);
                    self.translate_recursive(&v, &symbol, text);
                }
                text.push(sigma + 1);
            }
        };
    }

    fn translate_query_recursive(&self, query: &Query, symbol: &str) -> Vec<Vec<u32>> {
        match query {
            Query::Any => {
                let symbol = symbol.to_string();
                let mut result = vec![];
                for c in &["o", "a", "n", "t", "f", "#"] {
                    if let Some(sigma) = self.symbols.get(&(symbol.clone() + c)).copied() {
                        result.push(vec![sigma])
                    }
                }
                result
            }
            Query::Null => {
                let symbol = symbol.to_string() + "n";
                if let Some(sigma) = self.symbols.get(&symbol).copied() {
                    vec![vec![sigma]]
                } else {
                    vec![]
                }
            }
            Query::True => {
                let symbol = symbol.to_string() + "t";
                if let Some(sigma) = self.symbols.get(&symbol).copied() {
                    vec![vec![sigma]]
                } else {
                    vec![]
                }
            }
            Query::False => {
                let symbol = symbol.to_string() + "f";
                if let Some(sigma) = self.symbols.get(&symbol).copied() {
                    vec![vec![sigma]]
                } else {
                    vec![]
                }
            }
            Query::I64(_) | Query::U64(_) | Query::F64(_) => {
                let symbol = symbol.to_string() + "#";
                if let Some(sigma) = self.symbols.get(&symbol).copied() {
                    vec![vec![sigma]]
                } else {
                    vec![]
                }
            }
            Query::String(s) => {
                let symbol = symbol.to_string() + "s";
                if let Some(sigma) = self.symbols.get(&symbol).copied() {
                    let mut text = vec![sigma];
                    for c in s.chars() {
                        text.push(c as u32);
                    }
                    text.push(sigma + 1);
                    vec![text]
                } else {
                    vec![]
                }
            }
            Query::Array(a) => {
                assert!(a.len() <= 1);
                let symbol = symbol.to_string() + "a";
                if a.len() == 1 {
                    self.translate_query_recursive(&a[0], &symbol)
                } else if let Some(sigma) = self.symbols.get(&symbol).copied() {
                    vec![vec![sigma]]
                } else {
                    vec![]
                }
            }
            Query::Object(o) => {
                assert!(o.len() <= 1);
                if o.len() == 1 {
                    let (k, q) = &o[0];
                    let len = k.chars().count();
                    let symbol = format!("{}ok{}{}", symbol, len, k);
                    self.translate_query_recursive(&q, &symbol)
                } else if o.is_empty() {
                    let symbol = symbol.to_string() + "o";
                    if let Some(sigma) = self.symbols.get(&symbol).copied() {
                        vec![vec![sigma]]
                    } else {
                        vec![]
                    }
                } else {
                    unreachable!();
                }
            }
            Query::Or(_) => {
                todo!();
            }
        }
    }

    fn lookup_symbol(&mut self, symbol: &str) -> u32 {
        match self.symbols.entry(symbol.to_string()) {
            Entry::Occupied(entry) => *entry.get(),
            Entry::Vacant(entry) => {
                let sym = self.next_symbol;
                self.next_symbol += 2;
                entry.insert(sym);
                sym
            }
        }
    }

    fn reverse_lookup(&self, sym: u32) -> Option<&str> {
        for (s, t) in self.symbols.iter() {
            if *t == sym {
                return Some(s);
            }
        }
        None
    }
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self {
            symbols: HashMap::new(),
            next_symbol: 0x110000,
        }
    }
}

////////////////////////////////////////////// Builder /////////////////////////////////////////////

pub struct Builder {
    time_field: String,
    objects: Vec<(DateTime<Utc>, Value)>,
}

impl Builder {
    // TODO(rescrv): Take a query.
    pub fn new(time_field: String) -> Self {
        let objects = vec![];
        Self {
            time_field,
            objects,
        }
    }

    pub fn append_ndjson_from_reader<R: Read>(&mut self, reader: R) -> Result<(), Error> {
        let reader = BufReader::new(reader);
        for line in reader.lines() {
            let line = line?;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let value: Value = serde_json::from_str(line)?;
            self.process_one_object(line, value)?;
        }
        Ok(())
    }

    pub fn process_one_object(&mut self, json: &str, value: Value) -> Result<(), Error> {
        if let Value::Object(_) = value {
            let ts =
                extract_timestamp(&self.time_field, &value).ok_or(Error::MissingTimestamp {
                    core: ErrorCore::default(),
                    json: json.to_string(),
                })?;
            self.objects.push((ts, value));
            Ok(())
        } else {
            let mut buf = vec![];
            serde_json::to_writer(&mut buf, &value)?;
            Err(Error::NotAnObject {
                core: ErrorCore::default(),
                json: String::from_utf8(buf).unwrap_or("<invalid JSON>".to_string()),
            })
        }
    }

    pub fn time_window(&self) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
        if let (Some(min), Some(max)) = (
            self.objects.iter().map(|o| o.0).min(),
            self.objects.iter().map(|o| o.0).max(),
        ) {
            Some((min, max))
        } else {
            None
        }
    }

    pub fn seal<P: AsRef<Path>>(
        self,
        sym_table: &mut SymbolTable,
        start_time: DateTime<Utc>,
        path: P,
    ) -> Result<(), Error> {
        let (text, record_boundaries, second_boundaries) =
            Self::seal_inner(sym_table, &self.objects, start_time);
        let mut buf = Vec::new();
        let mut builder = scrunch::builder::Builder::new(&mut buf);
        let mut sub = builder.sub(FieldNumber::must(1));
        // TODO(rescrv): remove the expect
        CompressedDocument::construct(text, record_boundaries, &mut sub)
            .expect("compressed document construction should succeed");
        drop(sub);
        let mut sub = builder.sub(FieldNumber::must(2));
        // TODO(rescrv): remove the expect
        BitVector::from_indices(
            16,
            second_boundaries[second_boundaries.len() - 1] + 1,
            &second_boundaries,
            &mut sub,
        )
        .expect("bit vector construction should succeed");
        drop(sub);
        drop(builder);
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .open(path.as_ref())
            .as_z()
            .with_info("path", path.as_ref().to_string_lossy())?;
        file.write_all(&buf)?;
        Ok(())
    }

    fn group_by_second(
        objects: &[(DateTime<Utc>, Value)],
        start_time: DateTime<Utc>,
    ) -> Vec<(DateTime<Utc>, Vec<&Value>)> {
        let mut watermark = start_time.duration_trunc(TimeDelta::seconds(1)).unwrap();
        let mut results = vec![];
        for (ts, object) in objects {
            while watermark <= *ts {
                results.push((watermark, vec![]));
                watermark += TimeDelta::seconds(1);
            }
            let len = results.len() - 1;
            results[len].1.push(object);
        }
        results
    }

    fn seal_inner(
        sym_table: &mut SymbolTable,
        objects: &[(DateTime<Utc>, Value)],
        start_time: DateTime<Utc>,
    ) -> (Vec<u32>, Vec<usize>, Vec<usize>) {
        let mut text = vec![];
        let mut record_boundaries = vec![];
        let mut second_boundaries = vec![];
        for (_, objects) in Self::group_by_second(objects, start_time) {
            if objects.is_empty() {
                second_boundaries.push(record_boundaries.len());
                record_boundaries.push(text.len());
                sym_table.append_dummy_record(&mut text);
            } else {
                for object in objects {
                    record_boundaries.push(text.len());
                    sym_table.translate(object, &mut text);
                }
                second_boundaries.push(record_boundaries.len() - 1);
            }
        }
        second_boundaries.push(record_boundaries.len());
        (text, record_boundaries, second_boundaries)
    }
}

///////////////////////////////////////// AnalogizeDocument ////////////////////////////////////////

#[derive(Clone, Debug, Default, prototk_derive::Message)]
struct AnalogizeDocumentStub<'a> {
    #[prototk(1, bytes)]
    document: &'a [u8],
    #[prototk(2, bytes)]
    timeline: &'a [u8],
}

struct AnalogizeDocument<'a> {
    document: CompressedDocument<'a>,
    timeline: BitVector<'a>,
}

impl<'a> AnalogizeDocument<'a> {
    fn query(&self, syms: &SymbolTable, query: Query) -> Result<HashSet<RecordOffset>, Error> {
        let query = query.normalize();
        if let Query::Or(subqueries) = query {
            let mut records = HashSet::new();
            for query in subqueries {
                for offset in self.query(syms, query)? {
                    records.insert(offset);
                }
            }
            Ok(records)
        } else {
            let mut results = HashSet::new();
            let mut needles = vec![];
            for conjunction in query.conjunctions() {
                needles.append(&mut syms.translate_query(&conjunction));
            }
            let mut needles = needles.into_iter();
            if let Some(needle) = needles.next() {
                for offset in self.document.search(&needle)? {
                    results.insert(self.document.lookup(offset)?);
                }
            }
            for needle in needles {
                let inner = std::mem::take(&mut results);
                for offset in self.document.search(&needle)? {
                    let offset = self.document.lookup(offset)?;
                    if inner.contains(&offset) {
                        results.insert(offset);
                    }
                }
            }
            Ok(results)
        }
    }
}

impl<'a> Unpackable<'a> for AnalogizeDocument<'a> {
    type Error = Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Self::Error> {
        let (stub, buf) = AnalogizeDocumentStub::unpack(buf).map_err(|_| Error::Scrunch {
            core: ErrorCore::default(),
            what: scrunch::Error::InvalidDocument,
        })?;
        let document = CompressedDocument::unpack(stub.document)?.0;
        let timeline = BitVector::parse(stub.timeline)?.0;
        Ok((AnalogizeDocument { document, timeline }, buf))
    }
}

/////////////////////////////////////////////// Query //////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub enum Query {
    #[default]
    Any,
    Null,
    True,
    False,
    I64(i64),
    U64(u64),
    F64(f64),
    String(String),
    Array(Vec<Query>),
    Object(Vec<(String, Query)>),
    Or(Vec<Query>),
}

impl Query {
    pub fn parse<S: AsRef<str>>(query: S) -> Result<Self, Error> {
        Ok(parser::parse_all(parser::query)(query.as_ref())?)
    }

    pub fn must<S: AsRef<str>>(query: S) -> Self {
        Query::parse(query).expect("query should parse")
    }

    pub fn normalize(self) -> Query {
        match self {
            Query::Any
            | Query::Null
            | Query::True
            | Query::False
            | Query::I64(_)
            | Query::U64(_)
            | Query::F64(_)
            | Query::String(_) => self,
            Query::Array(subqueries) => Self::normalize_array(subqueries),
            Query::Object(subqueries) => Self::normalize_object(subqueries),
            Query::Or(subqueries) => Self::normalize_or(subqueries),
        }
    }

    pub fn conjunctions(self) -> impl Iterator<Item = Query> {
        let answer: Box<dyn Iterator<Item = Query>> = match self {
            Query::Any
            | Query::Null
            | Query::True
            | Query::False
            | Query::I64(_)
            | Query::U64(_)
            | Query::F64(_)
            | Query::String(_) => Box::new(vec![self].into_iter()),
            Query::Array(subqueries) => {
                if subqueries.is_empty() {
                    Box::new(vec![Query::Array(vec![])].into_iter())
                } else {
                    Box::new(
                        subqueries
                            .into_iter()
                            .map(|q| q.conjunctions())
                            .flatten()
                            .map(|q| Query::Array(vec![q])),
                    )
                }
            }
            Query::Object(subqueries) => {
                if subqueries.is_empty() {
                    Box::new(vec![Query::Object(vec![])].into_iter())
                } else {
                    let mut results = vec![];
                    for (s, q) in subqueries.into_iter() {
                        for q in q.conjunctions() {
                            results.push(Query::Object(vec![(s.clone(), q)]));
                        }
                    }
                    Box::new(results.into_iter())
                }
            }
            Query::Or(_) => {
                // TODO(rescrv): Do better here.
                panic!("calling conjunctions on Or clause");
            }
        };
        answer
    }

    fn normalize_mut(query: &mut Query) {
        let q = std::mem::take(query);
        *query = Self::normalize(q);
    }

    fn normalize_array(subqueries: Vec<Query>) -> Query {
        if subqueries.is_empty() {
            return Query::Array(subqueries);
        }
        let mut disjunctions: Vec<Vec<Query>> = vec![vec![]];
        for subquery in subqueries.into_iter() {
            let subquery = Self::normalize(subquery);
            if let Query::Or(subqueries) = subquery {
                let inner = std::mem::take(&mut disjunctions);
                for subquery in subqueries.into_iter() {
                    for inner in inner.iter() {
                        let mut inner = inner.clone();
                        inner.push(subquery.clone());
                        disjunctions.push(inner);
                    }
                }
            } else {
                for disjunction in disjunctions.iter_mut() {
                    disjunction.push(subquery.clone());
                }
            }
        }
        if disjunctions.len() > 1 {
            Query::Or(disjunctions.into_iter().map(Query::Array).collect())
        } else if let Some(subqueries) = disjunctions.pop() {
            Query::Array(subqueries)
        } else {
            unreachable!();
        }
    }

    fn normalize_object(subqueries: Vec<(String, Query)>) -> Query {
        if subqueries.is_empty() {
            return Query::Object(subqueries);
        }
        let mut disjunctions: Vec<Vec<(String, Query)>> = vec![vec![]];
        for (key, subquery) in subqueries.into_iter() {
            let subquery = Self::normalize(subquery);
            if let Query::Or(subqueries) = subquery {
                let inner = std::mem::take(&mut disjunctions);
                for subquery in subqueries.into_iter() {
                    for inner in inner.iter() {
                        let mut inner = inner.clone();
                        inner.push((key.clone(), subquery.clone()));
                        disjunctions.push(inner);
                    }
                }
            } else {
                for disjunction in disjunctions.iter_mut() {
                    disjunction.push((key.clone(), subquery.clone()));
                }
            }
        }
        if disjunctions.len() > 1 {
            Query::Or(disjunctions.into_iter().map(Query::Object).collect())
        } else if let Some(subqueries) = disjunctions.pop() {
            Query::Object(subqueries)
        } else {
            unreachable!();
        }
    }

    fn normalize_or(mut subqueries: Vec<Query>) -> Query {
        subqueries.iter_mut().for_each(Query::normalize_mut);
        subqueries.sort_by_key(|x| if let Query::Or(_) = *x { 1 } else { 0 });
        let partition =
            subqueries.partition_point(|x| if let Query::Or(_) = *x { false } else { true });
        let disjunctions = subqueries.split_off(partition);
        for disjunction in disjunctions.into_iter() {
            if let Query::Or(mut subq) = disjunction {
                subqueries.append(&mut subq);
            } else {
                unreachable!();
            }
        }
        Query::Or(subqueries)
    }
}

impl Eq for Query {}

impl PartialEq for Query {
    fn eq(&self, query: &Query) -> bool {
        match (self, query) {
            (Query::Any, Query::Any) => true,
            (Query::Null, Query::Null) => true,
            (Query::True, Query::True) => true,
            (Query::False, Query::False) => true,
            (Query::I64(x), Query::I64(y)) => x == y,
            (Query::U64(x), Query::U64(y)) => x == y,
            (Query::F64(x), Query::F64(y)) => x.total_cmp(y).is_eq(),
            (Query::String(x), Query::String(y)) => x == y,
            (Query::Array(x), Query::Array(y)) => x == y,
            (Query::Object(x), Query::Object(y)) => x == y,
            (Query::Or(x), Query::Or(y)) => x == y,
            _ => false,
        }
    }
}

////////////////////////////////////////// DocumentMapping /////////////////////////////////////////

struct DocumentMapping {
    data: *mut c_void,
    size: usize,
}

impl DocumentMapping {
    fn doc(&self) -> Result<AnalogizeDocument, Error> {
        // SAFETY(rescrv):  We only ever refer to this region of memory as a slice of u8.
        let buf = unsafe { std::slice::from_raw_parts(self.data as *const u8, self.size) };
        Ok(AnalogizeDocument::unpack(buf)?.0)
    }
}

impl Drop for DocumentMapping {
    fn drop(&mut self) {
        // SAFETY(rescrv): It will always be a valid mapping.
        unsafe {
            libc::munmap(self.data, self.size);
        }
    }
}

unsafe impl Send for DocumentMapping {}
unsafe impl Sync for DocumentMapping {}

/////////////////////////////////////////////// State //////////////////////////////////////////////

struct State {
    options: AnalogizeOptions,
    done: AtomicBool,
    json: PathBuf,
    data: PathBuf,
    mani: Mutex<Manifest>,
    syms: Mutex<SymbolTable>,
    docs: Mutex<HashMap<String, Arc<DocumentMapping>>>,
}

impl State {
    fn new(
        options: AnalogizeOptions,
        json: PathBuf,
        data: PathBuf,
        mani: Manifest,
    ) -> Result<Self, Error> {
        let done = AtomicBool::new(false);
        let syms = if data.join("symbols").exists() {
            SymbolTable::from_file(data.join("symbols"))?
        } else {
            SymbolTable::default()
        };
        let mani = Mutex::new(mani);
        let syms = Mutex::new(syms);
        let docs = Mutex::new(HashMap::new());
        Ok(Self {
            options,
            done,
            json,
            data,
            mani,
            syms,
            docs,
        })
    }

    fn background(self: Arc<Self>) {
        let mut workers = Vec::with_capacity(self.options.threads);
        for _ in 0..self.options.threads {
            let this = Arc::clone(&self);
            workers.push(std::thread::spawn(move || this.worker()));
        }
        while !self.done.load(Ordering::Relaxed) {
            sleep(Duration::from_millis(100));
            // TODO(rescrv): Log errors from ingest in a shell-compatible way.
            self.try_ingest().expect("try ingest should never fail");
        }
        for worker in workers.into_iter() {
            let _ = worker.join();
        }
    }

    fn worker(self: Arc<Self>) {}

    fn get_documents(&self) -> Result<Vec<Arc<DocumentMapping>>, Error> {
        let mani = self.mani.lock().unwrap();
        fn select_data(s: &str) -> Option<String> {
            if s.starts_with("data:") {
                Some(s[5..].to_string())
            } else {
                None
            }
        }
        let docs: Vec<_> = mani.strs().filter_map(select_data).collect();
        let mut mappings = Vec::with_capacity(docs.len());
        for doc in docs {
            mappings.push(self.get_document(&doc)?);
        }
        Ok(mappings)
    }

    fn get_document(&self, doc: &str) -> Result<Arc<DocumentMapping>, Error> {
        let mut docs = self.docs.lock().unwrap();
        {
            if let Some(doc) = docs.get(doc) {
                return Ok(Arc::clone(doc));
            }
        }
        let path = self.data.join(doc);
        let file = File::open(path)?;
        let md = file.metadata()?;
        if md.len() > usize::MAX as u64 {
            panic!("TODO");
        }
        // SAFETY(rescrv):  We treat this mapping with respect and only unmap if it's valid.
        let mapping = unsafe {
            libc::mmap64(
                std::ptr::null_mut(),
                md.len() as usize,
                libc::PROT_READ,
                libc::MAP_SHARED | libc::MAP_POPULATE,
                file.as_raw_fd(),
                0,
            )
        };
        if mapping == libc::MAP_FAILED {
            panic!("TODO");
        }
        let mapping = Arc::new(DocumentMapping {
            data: mapping,
            size: md.len() as usize,
        });
        docs.insert(doc.to_string(), Arc::clone(&mapping));
        Ok(mapping)
    }

    fn get_boundaries(&self) -> Vec<(u32, u32)> {
        let syms = self.syms.lock().unwrap();
        syms.markers().collect()
    }

    fn try_ingest(&self) -> Result<(), Error> {
        self.log_json_to_ingest()?;
        self.convert_ingested_json()?;
        Ok(())
    }

    fn log_json_to_ingest(&self) -> Result<(), Error> {
        let mut mani = self.mani.lock().unwrap();
        let threshold_ns = mani.info('J').unwrap_or("0");
        let threshold_ns =
            i64::from_str(threshold_ns).map_err(|_| Error::InvalidNumberLiteral {
                core: ErrorCore::default(),
                as_str: threshold_ns.to_string(),
            })?;
        let (json_to_ingest, threshold_ns) = take_consistent_cut(&self.json, threshold_ns)?;
        if json_to_ingest.is_empty() {
            return Ok(());
        }
        let mut edit = Edit::default();
        edit.info('J', &format!("{}", threshold_ns))?;
        for json in json_to_ingest.iter() {
            edit.add(&format!("json:{}", json))?;
        }
        mani.apply(edit)?;
        Ok(())
    }

    fn convert_ingested_json(&self) -> Result<(), Error> {
        // First, read the manifest to figure out what JSON needs to be ingested.
        let (json_inputs, file_number): (Vec<String>, String) = {
            let mani = self.mani.lock().unwrap();
            fn select_json(s: &str) -> Option<String> {
                if s.starts_with("json:") {
                    Some(s[5..].to_string())
                } else {
                    None
                }
            }
            (
                mani.strs().filter_map(select_json).collect(),
                mani.info('F').unwrap_or("0").to_string(),
            )
        };
        if json_inputs.is_empty() {
            return Ok(());
        }
        let file_number = u64::from_str(&file_number).map_err(|_| Error::InvalidNumberLiteral {
            core: ErrorCore::default(),
            as_str: file_number,
        })?;
        // Second, build the JSON for all the files at once.
        // TODO(rescrv): push an expression into here.
        let mut builder = Builder::new("created_at".to_string());
        for input in json_inputs.iter() {
            let input = File::open(self.json.join(input))?;
            builder.append_ndjson_from_reader(input)?;
        }
        let mut edit = Edit::default();
        edit.info('F', &(file_number + 1).to_string())?;
        if let Some((start_time, end_time)) = builder.time_window() {
            let mut syms = self.syms.lock().unwrap();
            let output_path = format!(
                "{}_{}_{}.analogize",
                date_time_to_string(start_time),
                date_time_to_string(end_time),
                file_number
            );
            builder.seal(&mut syms, start_time, &self.data.join(&output_path))?;
            let syms_tmp = format!("symbols.{}", Utc::now().timestamp());
            let syms_tmp = self.data.join(syms_tmp);
            syms.to_file(&syms_tmp)?;
            rename(syms_tmp, self.data.join("symbols"))?;
            edit.add(&format!("data:{}", output_path))?;
        }
        for input in json_inputs.iter() {
            edit.rm(&format!("json:{}", input))?;
        }
        let mut mani = self.mani.lock().unwrap();
        mani.apply(edit)?;
        Ok(())
    }

    fn done(&self) {
        self.done.store(true, Ordering::Relaxed);
    }
}

///////////////////////////////////////// AnalogizeOptions /////////////////////////////////////////

#[derive(Clone, Debug, Eq, PartialEq, arrrg_derive::CommandLine)]
pub struct AnalogizeOptions {
    #[arrrg(required, "Path to newline-delimited JSON files.")]
    json: String,
    #[arrrg(required, "Path to analogize data files.")]
    data: String,
    #[arrrg(nested)]
    mani: ManifestOptions,
    #[arrrg(optional, "Number of background worker threads to spawn.")]
    threads: usize,
}

impl AnalogizeOptions {
    pub fn data(&self) -> &str {
        &self.data
    }
}

impl Default for AnalogizeOptions {
    fn default() -> Self {
        Self {
            json: "json".to_string(),
            data: "data".to_string(),
            mani: ManifestOptions::default(),
            threads: 8,
        }
    }
}

///////////////////////////////////////////// Analogize ////////////////////////////////////////////

pub struct Analogize {
    state: Arc<State>,
    thread: Option<JoinHandle<()>>,
}

impl Analogize {
    pub fn new(options: AnalogizeOptions) -> Result<Self, Error> {
        let json = PathBuf::from(&options.json);
        if !json.exists() || !json.is_dir() {
            return Err(Error::DirectoryNotFound {
                core: ErrorCore::default(),
                what: options.json,
            });
        }
        let data = PathBuf::from(&options.data);
        let mani = Manifest::open(options.mani.clone(), data.clone())?;
        let state = Arc::new(State::new(options, json, data, mani)?);
        let state_p = Arc::clone(&state);
        let thread = Some(std::thread::spawn(move || state_p.background()));
        let this = Self { state, thread };
        Ok(this)
    }

    pub fn correlate(&mut self, query: Query, num_results: usize) -> Result<Vec<String>, Error> {
        let docs_arc: Vec<Arc<DocumentMapping>> = self.state.get_documents()?;
        let mut docs: Vec<AnalogizeDocument> = Vec::with_capacity(docs_arc.len());
        for doc in docs_arc.iter() {
            docs.push(doc.doc()?);
        }
        let mut docs_ref: Vec<&CompressedDocument> = Vec::with_capacity(docs.len());
        for doc in docs.iter() {
            docs_ref.push(&doc.document);
        }
        let boundaries = self.state.get_boundaries();
        let mut records: Vec<HashSet<RecordOffset>> = vec![];
        for doc in docs.iter() {
            let syms = self.state.syms.lock().unwrap();
            records.push(doc.query(&syms, query.clone())?);
        }
        let select = move |doc: usize, record: RecordOffset| records[doc].contains(&record);
        let mut exemplars = vec![];
        for exemplar in scrunch::correlate(&docs_ref, &boundaries, select).take(num_results) {
            let syms = self.state.syms.lock().unwrap();
            if let Some(query) = syms.to_query(exemplar.text()) {
                exemplars.push(query);
            }
        }
        Ok(exemplars)
    }

    pub fn exemplars(&mut self, num_results: usize) -> Result<Vec<String>, Error> {
        let docs_arc: Vec<Arc<DocumentMapping>> = self.state.get_documents()?;
        let mut docs: Vec<AnalogizeDocument> = Vec::with_capacity(docs_arc.len());
        for doc in docs_arc.iter() {
            docs.push(doc.doc()?);
        }
        let mut docs_ref: Vec<&CompressedDocument> = Vec::with_capacity(docs.len());
        for doc in docs.iter() {
            docs_ref.push(&doc.document);
        }
        let boundaries = self.state.get_boundaries();
        let mut exemplars = vec![];
        for exemplar in scrunch::exemplars(&docs_ref, &boundaries).take(num_results) {
            let syms = self.state.syms.lock().unwrap();
            if let Some(query) = syms.to_query(exemplar.text()) {
                exemplars.push(query);
            }
        }
        Ok(exemplars)
    }
}

impl Drop for Analogize {
    fn drop(&mut self) {
        self.state.done();
        self.thread.take().map(|t| t.join());
    }
}

/////////////////////////////////////////////// utils //////////////////////////////////////////////

fn ctime(md: &Metadata) -> i64 {
    md.ctime()
        .wrapping_mul(1_000_000_000i64)
        .wrapping_add(md.ctime_nsec())
}

fn take_consistent_cut<P: AsRef<Path>>(
    dir: P,
    threshold_ns: i64,
) -> Result<(Vec<String>, i64), Error> {
    loop {
        let mut new_threshold_ns = threshold_ns;
        let md1 = metadata(dir.as_ref())?;
        let mut paths = vec![];
        for dirent in read_dir(dir.as_ref())? {
            let dirent = dirent?;
            let md = dirent.metadata()?;
            let ts_ns = ctime(&md);
            if ts_ns > threshold_ns {
                let mut path = dirent.path();
                if path.starts_with(dir.as_ref()) {
                    path = path
                        .strip_prefix(dir.as_ref())
                        .map_err(|_| Error::InvalidPath {
                            core: ErrorCore::default(),
                            what: format!("path {} prefix won't strip", path.to_string_lossy()),
                        })?
                        .to_path_buf();
                }
                let display = path.to_string_lossy().to_string();
                if PathBuf::from(&display) != path {
                    return Err(Error::InvalidPath {
                        core: ErrorCore::default(),
                        what: format!("path {} contains invalid characters", display),
                    });
                }
                paths.push(display);
                new_threshold_ns = std::cmp::max(ts_ns, new_threshold_ns);
            }
        }
        let md2 = metadata(dir.as_ref())?;
        let ctime1 = ctime(&md1);
        let ctime2 = ctime(&md2);
        if ctime1 == ctime2 {
            return Ok((paths, new_threshold_ns));
        }
    }
}

fn extract_timestamp(field: &str, value: &Value) -> Option<DateTime<Utc>> {
    let v = value.get(field);
    if let Some(Value::String(timestamp)) = &v {
        Some(DateTime::parse_from_rfc3339(timestamp).ok()?.to_utc())
    } else if let Some(Value::Number(timestamp)) = &v {
        DateTime::from_timestamp(timestamp.as_i64()?, 0)
    } else {
        None
    }
}

fn date_time_to_string(when: DateTime<Utc>) -> String {
    when.to_rfc3339_opts(chrono::format::SecondsFormat::Secs, true)
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    fn test_case(sym_table: &mut SymbolTable, json: &str) -> Vec<u32> {
        let value: Value = serde_json::from_str(json).expect("json should be valid for test");
        let mut translated: Vec<u32> = vec![];
        sym_table.translate(&value, &mut translated);
        translated
    }

    #[test]
    fn bool_null() {
        let mut sym_table = SymbolTable::default();
        let expected = vec![0x110000u32, 0x110002, 0x110001];
        let returned = test_case(&mut sym_table, "{\"key\": null}");
        assert_eq!(expected, returned);
        assert_eq!(
            HashMap::from([
                ("o".to_string(), 0x110000),
                ("ok3keyn".to_string(), 0x110002),
            ]),
            sym_table.symbols
        );
    }

    #[test]
    fn bool_true() {
        let mut sym_table = SymbolTable::default();
        let expected = vec![0x110000u32, 0x110002, 0x110001];
        let returned = test_case(&mut sym_table, "{\"key\": true}");
        assert_eq!(expected, returned);
        assert_eq!(
            HashMap::from([
                ("o".to_string(), 0x110000),
                ("ok3keyt".to_string(), 0x110002),
            ]),
            sym_table.symbols
        );
    }

    #[test]
    fn bool_false() {
        let mut sym_table = SymbolTable::default();
        let expected = vec![0x110000u32, 0x110002, 0x110001];
        let returned = test_case(&mut sym_table, "{\"key\": false}");
        assert_eq!(expected, returned);
        assert_eq!(
            HashMap::from([
                ("o".to_string(), 0x110000),
                ("ok3keyf".to_string(), 0x110002),
            ]),
            sym_table.symbols
        );
    }

    #[test]
    fn number() {
        let mut sym_table = SymbolTable::default();
        let expected = vec![0x110000u32, 0x110002, 0x110001];
        let returned = test_case(&mut sym_table, "{\"key\": 3.14}");
        assert_eq!(expected, returned);
        assert_eq!(
            HashMap::from([
                ("o".to_string(), 0x110000),
                ("ok3key#".to_string(), 0x110002),
            ]),
            sym_table.symbols
        );
    }

    #[test]
    fn string() {
        let mut sym_table = SymbolTable::default();
        let expected = vec![
            0x110000u32,
            0x110002,
            'v' as u32,
            'a' as u32,
            'l' as u32,
            'u' as u32,
            'e' as u32,
            0x110003,
            0x110001,
        ];
        let returned = test_case(&mut sym_table, "{\"key\": \"value\"}");
        assert_eq!(expected, returned);
        assert_eq!(
            HashMap::from([
                ("o".to_string(), 0x110000),
                ("ok3keys".to_string(), 0x110002),
            ]),
            sym_table.symbols
        );
    }

    #[test]
    fn array() {
        let mut sym_table = SymbolTable::default();
        let expected = vec![
            0x110000u32,
            0x110002,
            0x110004, //
            'v' as u32,
            'a' as u32,
            'l' as u32,
            'u' as u32,
            'e' as u32,
            '1' as u32, //
            0x110005,
            0x110004, //
            'v' as u32,
            'a' as u32,
            'l' as u32,
            'u' as u32,
            'e' as u32,
            '2' as u32, //
            0x110005,
            0x110003,
            0x110001, //
        ];
        let returned = test_case(&mut sym_table, "{\"key\": [\"value1\", \"value2\"]}");
        assert_eq!(expected, returned);
        assert_eq!(
            HashMap::from([
                ("o".to_string(), 0x110000),
                ("ok3keya".to_string(), 0x110002),
                ("ok3keyas".to_string(), 0x110004),
            ]),
            sym_table.symbols
        );
    }

    #[test]
    fn object() {
        let mut sym_table = SymbolTable::default();
        let expected = vec![
            0x110000u32,
            0x110002,
            0x110004,
            'v' as u32,
            'a' as u32,
            'l' as u32,
            'u' as u32,
            'e' as u32,
            0x110005,
            0x110003,
            0x110001,
        ];
        let returned = test_case(&mut sym_table, "{\"key\": {\"key\": \"value\"}}");
        assert_eq!(expected, returned);
        assert_eq!(
            HashMap::from([
                ("o".to_string(), 0x110000),
                ("ok3keyo".to_string(), 0x110002),
                ("ok3keyok3keys".to_string(), 0x110004),
            ]),
            sym_table.symbols
        );
    }

    fn datetime_object(dt: &str, json: &str) -> (DateTime<Utc>, Value) {
        (
            DateTime::parse_from_rfc3339(dt).unwrap().to_utc(),
            serde_json::from_str(json).unwrap(),
        )
    }

    #[test]
    fn seal_empty() {
        let mut sym_table = SymbolTable::default();
        let start_time = DateTime::parse_from_rfc3339("2024-02-16T15:10:00Z")
            .unwrap()
            .to_utc();
        let (output, record_boundaries, second_boundaries) =
            Builder::seal_inner(&mut sym_table, &[], start_time);
        assert!(output.is_empty());
        assert!(record_boundaries.is_empty());
        assert_eq!(vec![0], second_boundaries);
    }

    #[test]
    fn seal_record_in_first_second() {
        let mut sym_table = SymbolTable::default();
        let objects = vec![datetime_object(
            "2024-02-16T15:10:00.01Z",
            "{\"key\": \"value\"}",
        )];
        let start_time = DateTime::parse_from_rfc3339("2024-02-16T15:10:00Z")
            .unwrap()
            .to_utc();
        let (_, record_boundaries, second_boundaries) =
            Builder::seal_inner(&mut sym_table, &objects, start_time);
        assert_eq!(vec![0], record_boundaries);
        assert_eq!(vec![0, 1], second_boundaries);
    }

    #[test]
    fn seal_one_record_per_second() {
        let mut sym_table = SymbolTable::default();
        let objects = vec![
            datetime_object("2024-02-16T15:10:00.01Z", "{\"key\": \"value0\"}"),
            datetime_object("2024-02-16T15:10:01.01Z", "{\"key\": \"value1\"}"),
            datetime_object("2024-02-16T15:10:02.01Z", "{\"key\": \"value2\"}"),
        ];
        let start_time = DateTime::parse_from_rfc3339("2024-02-16T15:10:00Z")
            .unwrap()
            .to_utc();
        let (_, record_boundaries, second_boundaries) =
            Builder::seal_inner(&mut sym_table, &objects, start_time);
        assert_eq!(vec![0, 10, 20], record_boundaries);
        assert_eq!(vec![0, 1, 2, 3], second_boundaries);
    }

    #[test]
    fn seal_gap_at_beginning() {
        let mut sym_table = SymbolTable::default();
        let objects = vec![
            datetime_object("2024-02-16T15:10:01.01Z", "{\"key\": \"value1\"}"),
            datetime_object("2024-02-16T15:10:02.01Z", "{\"key\": \"value2\"}"),
            datetime_object("2024-02-16T15:10:03.01Z", "{\"key\": \"value3\"}"),
        ];
        let start_time = DateTime::parse_from_rfc3339("2024-02-16T15:10:00Z")
            .unwrap()
            .to_utc();
        let (_, record_boundaries, second_boundaries) =
            Builder::seal_inner(&mut sym_table, &objects, start_time);
        assert_eq!(vec![0, 1, 11, 21], record_boundaries);
        assert_eq!(vec![0, 1, 2, 3, 4], second_boundaries);
    }

    #[test]
    fn seal_gap_after_first_record() {
        let mut sym_table = SymbolTable::default();
        let objects = vec![
            datetime_object("2024-02-16T15:10:00.01Z", "{\"key\": \"value0\"}"),
            datetime_object("2024-02-16T15:10:02.01Z", "{\"key\": \"value2\"}"),
            datetime_object("2024-02-16T15:10:03.01Z", "{\"key\": \"value3\"}"),
        ];
        let start_time = DateTime::parse_from_rfc3339("2024-02-16T15:10:00Z")
            .unwrap()
            .to_utc();
        let (_, record_boundaries, second_boundaries) =
            Builder::seal_inner(&mut sym_table, &objects, start_time);
        assert_eq!(vec![0, 10, 11, 21], record_boundaries);
        assert_eq!(vec![0, 1, 2, 3, 4], second_boundaries);
    }

    #[test]
    fn seal_multiple_records_per_second() {
        let mut sym_table = SymbolTable::default();
        let objects = vec![
            datetime_object("2024-02-16T15:10:00.01Z", "{\"key\": \"value1\"}"),
            datetime_object("2024-02-16T15:10:00.02Z", "{\"key\": \"value2\"}"),
            datetime_object("2024-02-16T15:10:00.03Z", "{\"key\": \"value3\"}"),
            datetime_object("2024-02-16T15:10:01.04Z", "{\"key\": \"value4\"}"),
        ];
        let start_time = DateTime::parse_from_rfc3339("2024-02-16T15:10:00Z")
            .unwrap()
            .to_utc();
        let (_, record_boundaries, second_boundaries) =
            Builder::seal_inner(&mut sym_table, &objects, start_time);
        assert_eq!(vec![0, 10, 20, 30], record_boundaries);
        assert_eq!(vec![2, 3, 4], second_boundaries);
    }

    #[test]
    fn seal_multiple_records_per_second_with_gaps() {
        let mut sym_table = SymbolTable::default();
        let objects = vec![
            datetime_object("2024-02-16T15:10:01.01Z", "{\"key\": \"value1\"}"),
            datetime_object("2024-02-16T15:10:01.02Z", "{\"key\": \"value2\"}"),
            datetime_object("2024-02-16T15:10:01.03Z", "{\"key\": \"value3\"}"),
            datetime_object("2024-02-16T15:10:03.04Z", "{\"key\": \"value4\"}"),
        ];
        let start_time = DateTime::parse_from_rfc3339("2024-02-16T15:10:00Z")
            .unwrap()
            .to_utc();
        let (_, record_boundaries, second_boundaries) =
            Builder::seal_inner(&mut sym_table, &objects, start_time);
        assert_eq!(vec![0, 1, 11, 21, 31, 32], record_boundaries);
        assert_eq!(vec![0, 3, 4, 5, 6], second_boundaries);
    }

    #[test]
    fn query_no_normalization_expected() {
        assert_eq!(Query::Any, Query::Any.normalize());
        assert_eq!(Query::Null, Query::Null.normalize());
        assert_eq!(Query::True, Query::True.normalize());
        assert_eq!(Query::False, Query::False.normalize());
        assert_eq!(Query::I64(42), Query::I64(42).normalize());
        assert_eq!(Query::U64(42), Query::U64(42).normalize());
        assert_eq!(Query::F64(42.0), Query::F64(42.0).normalize());
        assert_eq!(
            Query::String("Hello World".to_string()),
            Query::String("Hello World".to_string()).normalize()
        );
    }

    #[test]
    fn query_normalize_array() {
        assert_eq!(Query::must("[]"), Query::must("[]").normalize());
        assert_eq!(
            Query::must("[true, false, \"Hello World\"]"),
            Query::must("[true, false, \"Hello World\"]").normalize()
        );
        assert_eq!(
            Query::must("[true, \"Hello World\"] or [false, \"Hello World\"]"),
            Query::must("[true or false, \"Hello World\"]").normalize()
        );
        assert_eq!(
            Query::must("[true, false] or [true, \"Hello World\"]"),
            Query::must("[true, false or \"Hello World\"]").normalize()
        );
    }

    #[test]
    fn query_normalize_object() {
        assert_eq!(Query::must("{}"), Query::must("{}").normalize());
        assert_eq!(
            Query::must("{\"a\": true, \"b\": \"Hello World\"}"),
            Query::must("{\"a\": true, \"b\": \"Hello World\"}").normalize()
        );
        assert_eq!(
            Query::must(
                "{\"a\": true, \"b\": \"Hello World\"} or {\"a\": false, \"b\": \"Hello World\"}"
            ),
            Query::must("{\"a\": true or false, \"b\": \"Hello World\"}").normalize()
        );
        assert_eq!(
            Query::must("{\"a\": true, \"b\": \"Hello World\"} or {\"a\": true, \"b\": 42}"),
            Query::must("{\"a\": true, \"b\": \"Hello World\" or 42}").normalize()
        );
    }

    #[test]
    fn query_normalize_nested() {
        assert_eq!(
            Query::must(
                "
            {\"a\": 42, \"b\": [\"Hello World\"]} or
            {\"a\": 42, \"b\": [13]} or
            {\"a\": 42, \"b\": [false]} or
            {\"a\": 42, \"b\": [[\"x\"]]} or
            {\"a\": 42, \"b\": [[\"y\"]]}
        "
                .trim()
            ),
            Query::must(
                "{\"a\": 42, \"b\": [\"Hello World\" or 13] or [false or [\"x\" or \"y\"]]}"
            )
            .normalize()
        );
    }

    #[test]
    fn query_no_conjunctions() {
        assert_eq!(
            vec![Query::Any],
            Query::Any.conjunctions().collect::<Vec<_>>()
        );
        assert_eq!(
            vec![Query::Null],
            Query::Null.conjunctions().collect::<Vec<_>>()
        );
        assert_eq!(
            vec![Query::True],
            Query::True.conjunctions().collect::<Vec<_>>()
        );
        assert_eq!(
            vec![Query::False],
            Query::False.conjunctions().collect::<Vec<_>>()
        );
        assert_eq!(
            vec![Query::I64(42)],
            Query::I64(42).conjunctions().collect::<Vec<_>>()
        );
        assert_eq!(
            vec![Query::U64(42)],
            Query::U64(42).conjunctions().collect::<Vec<_>>()
        );
        assert_eq!(
            vec![Query::F64(42.0)],
            Query::F64(42.0).conjunctions().collect::<Vec<_>>()
        );
        assert_eq!(
            vec![Query::String("Hello World".to_string())],
            Query::String("Hello World".to_string())
                .conjunctions()
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn query_conjunctions_array() {
        assert_eq!(
            vec![Query::must("[]")],
            Query::must("[]").conjunctions().collect::<Vec<_>>()
        );
        assert_eq!(
            vec![
                Query::must("[true]"),
                Query::must("[false]"),
                Query::must("[\"Hello World\"]")
            ],
            Query::must("[true, false, \"Hello World\"]")
                .conjunctions()
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn query_conjunctions_object() {
        assert_eq!(
            vec![Query::must("{}")],
            Query::must("{}").conjunctions().collect::<Vec<_>>()
        );
        assert_eq!(
            vec![
                Query::must("{\"a\": true}"),
                Query::must("{\"b\": \"Hello World\"}")
            ],
            Query::must("{\"a\": true, \"b\": \"Hello World\"}")
                .conjunctions()
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn query_conjucnions_nested() {
        assert_eq!(vec![
            Query::must("{\"a\": 42}"),
            Query::must("{\"b\": [\"Hello World\"]}"),
            Query::must("{\"b\": [\"Goodbye World\"]}"),
            Query::must("{\"c\": [false]}"),
            Query::must("{\"c\": [[\"x\"]]}"),
            Query::must("{\"c\": [[\"y\"]]}"),
        ], Query::must("{\"a\": 42, \"b\": [\"Hello World\", \"Goodbye World\"], \"c\": [false, [\"x\", \"y\"]]}").conjunctions().collect::<Vec<_>>());
    }
}
