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

use buffertk::Unpackable;
use indicio::{Clue, ClueVector, Value};
use mani::{Edit, Manifest, ManifestOptions};
use prototk::FieldNumber;
use scrunch::bit_vector::sparse::BitVector;
use scrunch::bit_vector::BitVector as BitVectorTrait;
use scrunch::builder::Builder;
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
        what: i64,
    },
    Scrunch {
        core: ErrorCore,
        what: scrunch::Error,
    },
    Indicio {
        core: ErrorCore,
        what: prototk::Error,
    },
    EmptyClueFile {
        core: ErrorCore,
    },
    FileTooLarge {
        core: ErrorCore,
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

impl From<prototk::Error> for Error {
    fn from(err: prototk::Error) -> Self {
        Self::Indicio {
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
            .with_info("path", path.as_ref().to_string_lossy())?;
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

    pub fn translate(&mut self, clue: Clue, text: &mut Vec<u32>) {
        let value = indicio::value!({
            file: clue.file,
            line: clue.line,
            level: clue.level,
            timestamp: clue.timestamp,
            value: clue.value,
        });
        self.translate_recursive(&value, "", text);
    }

    pub fn reverse_translate(&self, text: &[u32]) -> Option<Value> {
        // TODO(rescrv): Log the rate of failure.
        self.reverse_translate_recursive(text, "")
    }

    pub fn translate_query(&self, query: &Query) -> Vec<Vec<u32>> {
        self.translate_query_recursive(query, "")
    }

    pub fn reverse_translate_query(&self, text: &[u32]) -> Option<Value> {
        if text.is_empty() {
            return None;
        }
        let symbol = &self.reverse_lookup(text[0])?;
        let terminal = self.reverse_translate_recursive(text, &symbol[..symbol.len() - 1])?;
        fn build_from_symbol(mut symbol: &str, terminal: Value) -> Option<Value> {
            if symbol.is_empty() {
                return Some(terminal);
            }
            match &symbol[0..1] {
                "o" => {
                    if symbol.len() == 1 {
                        return Some(terminal);
                    }
                    if &symbol[1..2] != "k" {
                        return None;
                    }
                    let len: String = symbol[2..]
                        .chars()
                        .take_while(|c| c.is_ascii_digit())
                        .collect();
                    symbol = &symbol[2 + len.len()..];
                    let len = usize::from_str(&len).ok()?;
                    let key = symbol[..len].to_string();
                    let obj = build_from_symbol(&symbol[len..], terminal)?;
                    Some(Value::Object(indicio::Map::from_iter(vec![(key, obj)])))
                }
                "a" => {
                    let obj = build_from_symbol(&symbol[1..], terminal)?;
                    Some(Value::Array(indicio::Values::from(vec![obj])))
                }
                _ => Some(terminal),
            }
        }
        build_from_symbol(symbol, terminal)
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

    fn translate_recursive(&mut self, value: &Value, symbol: &str, text: &mut Vec<u32>) {
        match value {
            Value::Bool(b) => {
                let symbol = symbol.to_string() + if *b { "T" } else { "F" };
                let sigma = self.lookup_symbol(&symbol);
                text.push(sigma);
            }
            Value::I64(x) => {
                let symbol = symbol.to_string() + "i";
                let sigma = self.lookup_symbol(&symbol);
                text.push(sigma);
                for b in x.to_be_bytes() {
                    text.push(b as u32);
                }
                text.push(sigma + 1);
            }
            Value::U64(x) => {
                let symbol = symbol.to_string() + "u";
                let sigma = self.lookup_symbol(&symbol);
                text.push(sigma);
                for b in x.to_be_bytes() {
                    text.push(b as u32);
                }
                text.push(sigma + 1);
            }
            Value::F64(x) => {
                let symbol = symbol.to_string() + "f";
                let sigma = self.lookup_symbol(&symbol);
                text.push(sigma);
                for b in x.to_bits().to_be_bytes() {
                    text.push(b as u32);
                }
                text.push(sigma + 1);
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
                    self.translate_recursive(v, &symbol, text);
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
                for c in &["o", "a", "T", "F", "i", "u", "f"] {
                    if let Some(sigma) = self.symbols.get(&(symbol.clone() + c)).copied() {
                        result.push(vec![sigma])
                    }
                }
                result
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
            Query::I64(x) => {
                let isymbol = symbol.to_string() + "i";
                let usymbol = symbol.to_string() + "u";
                if let Some(sigma) = self.symbols.get(&isymbol) {
                    let mut text = Vec::new();
                    text.push(*sigma);
                    for b in x.to_be_bytes() {
                        text.push(b as u32);
                    }
                    text.push(*sigma + 1);
                    vec![text]
                } else if let Some(sigma) = self.symbols.get(&usymbol) {
                    let mut text = Vec::new();
                    text.push(*sigma);
                    for b in x.to_be_bytes() {
                        text.push(b as u32);
                    }
                    text.push(*sigma + 1);
                    vec![text]
                } else {
                    vec![]
                }
            }
            Query::U64(x) => {
                let isymbol = symbol.to_string() + "i";
                let usymbol = symbol.to_string() + "u";
                if let Some(sigma) = self.symbols.get(&usymbol) {
                    let mut text = Vec::new();
                    text.push(*sigma);
                    for b in x.to_be_bytes() {
                        text.push(b as u32);
                    }
                    text.push(*sigma + 1);
                    vec![text]
                } else if let Some(sigma) = self.symbols.get(&isymbol) {
                    let mut text = Vec::new();
                    text.push(*sigma);
                    for b in x.to_be_bytes() {
                        text.push(b as u32);
                    }
                    text.push(*sigma + 1);
                    vec![text]
                } else {
                    vec![]
                }
            }
            Query::F64(x) => {
                let symbol = symbol.to_string() + "f";
                if let Some(sigma) = self.symbols.get(&symbol) {
                    let mut text = Vec::new();
                    text.push(*sigma);
                    for b in x.to_bits().to_be_bytes() {
                        text.push(b as u32);
                    }
                    text.push(*sigma + 1);
                    vec![text]
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
                    self.translate_query_recursive(q, &symbol)
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
                panic!("do not translate disjunctions");
            }
        }
    }

    fn reverse_translate_keys(&self, mut text: &[u32], path: &str) -> Option<Value> {
        let mut map = indicio::Map::default();
        while !text.is_empty() {
            let symbol = self.reverse_lookup(text[0])?;
            let relative = symbol.strip_prefix(path)?;
            if !relative.starts_with('k') {
                return None;
            }
            let key_len: String = relative[1..]
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            let key = &relative[1 + key_len.len()..];
            let Ok(key_len) = usize::from_str(&key_len) else {
                return None;
            };
            let key = &key[..key.len() - 1];
            if key_len != key.len() {
                return None;
            }
            if symbol.ends_with('T') || symbol.ends_with('F') {
                let value =
                    self.reverse_translate_recursive(&text[..1], &symbol[..symbol.len() - 1])?;
                text = &text[1..];
                map.insert(key.to_string(), value);
            } else {
                let Some(position) = text.iter().position(|t| *t == text[0] + 1) else {
                    return None;
                };
                let value = self.reverse_translate_recursive(
                    &text[..position + 1],
                    &symbol[..symbol.len() - 1],
                )?;
                text = &text[position + 1..];
                map.insert(key.to_string(), value);
            }
        }
        Some(Value::Object(map))
    }

    fn reverse_translate_array(&self, mut text: &[u32], path: &str) -> Option<Value> {
        let mut values = vec![];
        while !text.is_empty() {
            let symbol = self.reverse_lookup(text[0])?;
            if !symbol.starts_with(path) {
                return None;
            }
            if symbol.ends_with('T') || symbol.ends_with('F') {
                let value =
                    self.reverse_translate_recursive(&text[..1], &symbol[..symbol.len() - 1])?;
                text = &text[1..];
                values.push(value);
            } else {
                let Some(position) = text.iter().position(|t| *t == text[0] + 1) else {
                    return None;
                };
                let value = self.reverse_translate_recursive(
                    &text[..position + 1],
                    &symbol[..symbol.len() - 1],
                )?;
                text = &text[position + 1..];
                values.push(value);
            }
        }
        Some(Value::from(values))
    }

    fn reverse_translate_recursive(&self, text: &[u32], path: &str) -> Option<Value> {
        if text.is_empty() {
            return None;
        }
        let symbol = self.reverse_lookup(text[0])?;
        let relative = symbol.strip_prefix(path)?;
        match relative {
            "o" => self.reverse_translate_keys(&text[1..text.len() - 1], &(path.to_string() + "o")),
            "a" => {
                self.reverse_translate_array(&text[1..text.len() - 1], &(path.to_string() + "a"))
            }
            "s" => Some(Value::String(
                text.iter().copied().flat_map(char::from_u32).collect(),
            )),
            "i" => {
                if text.len() != 10 {
                    return None;
                }
                let mut buf = [0u8; 8];
                for (b, t) in std::iter::zip(buf.iter_mut(), text[1..9].iter()) {
                    *b = *t as u8;
                }
                Some(Value::I64(i64::from_be_bytes(buf)))
            }
            "u" => {
                if text.len() != 10 {
                    return None;
                }
                let mut buf = [0u8; 8];
                for (b, t) in std::iter::zip(buf.iter_mut(), text[1..9].iter()) {
                    *b = *t as u8;
                }
                Some(Value::U64(u64::from_be_bytes(buf)))
            }
            "f" => {
                if text.len() != 10 {
                    return None;
                }
                let mut buf = [0u8; 8];
                for (b, t) in std::iter::zip(buf.iter_mut(), text[1..9].iter()) {
                    *b = *t as u8;
                }
                Some(Value::F64(f64::from_bits(u64::from_be_bytes(buf))))
            }
            "T" => Some(Value::Bool(true)),
            "F" => Some(Value::Bool(false)),
            _ => None,
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

//////////////////////////////////// convert_clues_to_analogize ////////////////////////////////////

#[allow(clippy::type_complexity)]
fn group_by_second(
    mut watermark: DateTime<Utc>,
    clues: Vec<Clue>,
) -> Result<Vec<(DateTime<Utc>, Vec<Clue>)>, Error> {
    if clues.is_empty() {
        return Err(Error::EmptyClueFile {
            core: ErrorCore::default(),
        });
    }
    watermark = watermark.duration_trunc(TimeDelta::seconds(1)).unwrap();
    let mut results = vec![];
    for clue in clues {
        let Some(ts) = DateTime::from_timestamp_millis(clue.timestamp as i64 / 1_000) else {
            return Err(Error::InvalidTimestamp {
                core: ErrorCore::default(),
                what: clue.timestamp as i64,
            });
        };
        while watermark <= ts {
            results.push((watermark, vec![]));
            watermark += TimeDelta::seconds(1);
        }
        let len = results.len() - 1;
        results[len].1.push(clue);
    }
    Ok(results)
}

#[allow(clippy::type_complexity)]
fn convert_clues_to_analogize_inner(
    sym_table: &mut SymbolTable,
    start_time: DateTime<Utc>,
    clues: Vec<Clue>,
) -> Result<(Vec<u32>, Vec<usize>, Vec<usize>), Error> {
    let mut text = vec![];
    let mut record_boundaries = vec![];
    let mut second_boundaries = vec![];
    if clues.is_empty() {
        second_boundaries.push(record_boundaries.len());
        record_boundaries.push(text.len());
        sym_table.append_dummy_record(&mut text);
    }
    if clues.is_empty() {
        return Err(Error::EmptyClueFile {
            core: ErrorCore::default(),
        });
    }
    for (_, clues) in group_by_second(start_time, clues)? {
        if clues.is_empty() {
            second_boundaries.push(record_boundaries.len());
            record_boundaries.push(text.len());
            sym_table.append_dummy_record(&mut text);
        } else {
            for clue in clues {
                record_boundaries.push(text.len());
                sym_table.translate(clue, &mut text);
            }
            second_boundaries.push(record_boundaries.len() - 1);
        }
    }
    second_boundaries.push(record_boundaries.len());
    Ok((text, record_boundaries, second_boundaries))
}

pub fn convert_clues_to_analogize<P: AsRef<Path>>(
    sym_table: &mut SymbolTable,
    start_time: DateTime<Utc>,
    clues: Vec<Clue>,
    analogize: P,
) -> Result<(), Error> {
    let (text, record_boundaries, second_boundaries) =
        convert_clues_to_analogize_inner(sym_table, start_time, clues)?;
    let mut buf = Vec::new();
    let mut builder = Builder::new(&mut buf);
    let mut sub = builder.sub(FieldNumber::must(1));
    CompressedDocument::construct(text, record_boundaries, &mut sub)?;
    drop(sub);
    let mut sub = builder.sub(FieldNumber::must(2));
    BitVector::from_indices(
        16,
        second_boundaries[second_boundaries.len() - 1] + 1,
        &second_boundaries,
        &mut sub,
    )
    .ok_or(scrunch::Error::InvalidBitVector)?;
    drop(sub);
    drop(builder);
    std::fs::write(analogize.as_ref(), buf)?;
    Ok(())
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
    #[allow(dead_code)]
    timeline: BitVector<'a>,
}

impl<'a> AnalogizeDocument<'a> {
    fn query(&self, syms: &SymbolTable, query: &Query) -> Result<HashSet<RecordOffset>, Error> {
        let records = if let Query::Or(subqueries) = query {
            let mut records = HashSet::new();
            for query in subqueries {
                for offset in self.query(syms, query)? {
                    records.insert(offset);
                }
            }
            records
        } else {
            let mut results = HashSet::new();
            let mut needles = vec![];
            for conjunction in query.clone().conjunctions() {
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
            results
        };
        Ok(records)
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

    fn conjunctions(self) -> impl Iterator<Item = Query> {
        let answer: Box<dyn Iterator<Item = Query>> = match self {
            Query::Any
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
                            .flat_map(|q| q.conjunctions())
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
        let partition = subqueries.partition_point(|x| !matches!(x, Query::Or(_)));
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
    logs: PathBuf,
    data: PathBuf,
    mani: Mutex<Manifest>,
    syms: Mutex<SymbolTable>,
    docs: Mutex<HashMap<String, Arc<DocumentMapping>>>,
}

impl State {
    fn new(
        options: AnalogizeOptions,
        logs: PathBuf,
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
            logs,
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
            s.strip_prefix("data:").map(String::from)
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
        if let Some(doc) = docs.get(doc) {
            return Ok(Arc::clone(doc));
        }
        let path = self.data.join(doc);
        let file = File::open(path)?;
        let md = file.metadata()?;
        if md.len() > usize::MAX as u64 {
            return Err(Error::FileTooLarge {
                core: ErrorCore::default(),
            });
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
            return Err(std::io::Error::last_os_error().into());
        }
        let mapping = Arc::new(DocumentMapping {
            data: mapping,
            size: md.len() as usize,
        });
        docs.insert(doc.to_string(), Arc::clone(&mapping));
        Ok(mapping)
    }

    fn try_ingest(&self) -> Result<(), Error> {
        self.log_to_ingest()?;
        self.convert_ingested_logs()?;
        Ok(())
    }

    fn log_to_ingest(&self) -> Result<(), Error> {
        let mut mani = self.mani.lock().unwrap();
        let threshold_ns = mani.info('L').unwrap_or("0");
        let threshold_ns =
            i64::from_str(threshold_ns).map_err(|_| Error::InvalidNumberLiteral {
                core: ErrorCore::default(),
                as_str: threshold_ns.to_string(),
            })?;
        let (logs_to_ingest, threshold_ns) = take_consistent_cut(&self.logs, threshold_ns)?;
        if logs_to_ingest.is_empty() {
            return Ok(());
        }
        let mut edit = Edit::default();
        edit.info('L', &format!("{}", threshold_ns))?;
        for log in logs_to_ingest.iter() {
            edit.add(&format!("log:{}", log))?;
        }
        mani.apply(edit)?;
        Ok(())
    }

    fn convert_ingested_logs(&self) -> Result<(), Error> {
        // First, read the manifest to figure out what JSON needs to be ingested.
        let (log_inputs, file_number): (Vec<String>, String) = {
            let mani = self.mani.lock().unwrap();
            fn select_logs(s: &str) -> Option<String> {
                s.strip_prefix("log:").map(String::from)
            }
            (
                mani.strs().filter_map(select_logs).collect(),
                mani.info('F').unwrap_or("0").to_string(),
            )
        };
        if log_inputs.is_empty() {
            return Ok(());
        }
        let file_number = u64::from_str(&file_number).map_err(|_| Error::InvalidNumberLiteral {
            core: ErrorCore::default(),
            as_str: file_number,
        })?;
        // Second, build the analogize file for all the files at once.
        let mut clues = vec![];
        for input in log_inputs.iter() {
            let buf = std::fs::read(self.logs.join(input))?;
            let mut cv = ClueVector::unpack(&buf)?.0;
            clues.append(&mut cv.clues);
        }
        let mut edit = Edit::default();
        edit.info('F', &(file_number + 1).to_string())?;
        if !clues.is_empty() {
            let mut syms = self.syms.lock().unwrap();
            let start_time = clues.iter().map(|x| x.timestamp).min().unwrap_or(0);
            let end_time = clues.iter().map(|x| x.timestamp).max().unwrap_or(0);
            let start_time = DateTime::from_timestamp_millis(start_time as i64 / 1_000).ok_or(
                Error::InvalidTimestamp {
                    core: ErrorCore::default(),
                    what: start_time as i64,
                },
            )?;
            let end_time = DateTime::from_timestamp_millis(end_time as i64 / 1_000).ok_or(
                Error::InvalidTimestamp {
                    core: ErrorCore::default(),
                    what: end_time as i64,
                },
            )?;
            let output_path = format!(
                "{}_{}_{}.analogize",
                date_time_to_string(start_time),
                date_time_to_string(end_time),
                file_number
            );
            convert_clues_to_analogize(
                &mut syms,
                start_time,
                clues,
                &self.data.join(&output_path),
            )?;
            let syms_tmp = format!("symbols.{}", Utc::now().timestamp());
            let syms_tmp = self.data.join(syms_tmp);
            syms.to_file(&syms_tmp)?;
            rename(syms_tmp, self.data.join("symbols"))?;
            edit.add(&format!("data:{}", output_path))?;
        }
        for input in log_inputs.iter() {
            edit.rm(&format!("log:{}", input))?;
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
    #[arrrg(required, "Path to indicio log files.")]
    logs: String,
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
            logs: "logs".to_string(),
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
        let logs = PathBuf::from(&options.logs);
        if !logs.exists() || !logs.is_dir() {
            return Err(Error::DirectoryNotFound {
                core: ErrorCore::default(),
                what: options.logs,
            });
        }
        let data = PathBuf::from(&options.data);
        let mani = Manifest::open(options.mani.clone(), data.clone())?;
        let state = Arc::new(State::new(options, logs, data, mani)?);
        let state_p = Arc::clone(&state);
        let thread = Some(std::thread::spawn(move || state_p.background()));
        let this = Self { state, thread };
        Ok(this)
    }

    pub fn query(&self, query: Query) -> Result<Vec<Value>, Error> {
        let query = query.normalize();
        let docs = self.state.get_documents()?;
        let mut values = vec![];
        for doc in docs {
            let doc = doc.doc()?;
            let syms = self.state.syms.lock().unwrap();
            let mut records: Vec<RecordOffset> = doc.query(&syms, &query)?.into_iter().collect();
            records.sort();
            for record in records.into_iter() {
                let Ok(record) = doc.document.retrieve(record) else {
                    // TODO(rescrv): report error
                    continue;
                };
                let Some(value) = syms.reverse_translate(&record) else {
                    // TODO(rescrv): report error
                    continue;
                };
                values.push(value);
            }
        }
        Ok(values)
    }

    pub fn exemplars(&self, num_results: usize) -> Result<Vec<Value>, Error> {
        let doc_ptrs = self.state.get_documents()?;
        let mut docs = vec![];
        for ptr in doc_ptrs.iter() {
            docs.push(ptr.doc()?);
        }
        let doc_refs: Vec<&CompressedDocument> = docs.iter().map(|d| &d.document).collect();
        let syms = self.state.syms.lock().unwrap();
        let markers: Vec<_> = syms.markers().collect();
        let mut values = vec![];
        for exemplar in scrunch::exemplars(&doc_refs, &markers).take(num_results) {
            if let Some(exemplar) = syms.reverse_translate_query(exemplar.text()) {
                values.push(exemplar);
            } else {
                // TODO(rescrv): report error
            }
        }
        Ok(values)
    }

    pub fn correlates(&self, query: Query, num_results: usize) -> Result<Vec<Value>, Error> {
        let doc_ptrs = self.state.get_documents()?;
        let mut docs = vec![];
        for ptr in doc_ptrs.iter() {
            docs.push(ptr.doc()?);
        }
        let doc_refs: Vec<&CompressedDocument> = docs.iter().map(|d| &d.document).collect();
        let syms = self.state.syms.lock().unwrap();
        let markers: Vec<_> = syms.markers().collect();
        let mut offsets: HashMap<usize, HashSet<RecordOffset>> = HashMap::new();
        for (idx, doc) in docs.iter().enumerate() {
            let records = doc.query(&syms, &query)?;
            offsets.insert(idx, records);
        }
        let mut values = vec![];
        for exemplar in scrunch::correlate(&doc_refs, &markers, move |idx, offset| {
            offsets
                .get(&idx)
                .map(|r| r.get(&offset))
                .map(|o| o.is_some())
                .unwrap_or(false)
        })
        .take(num_results)
        {
            if let Some(exemplar) = syms.reverse_translate_query(exemplar.text()) {
                values.push(exemplar);
            } else {
                // TODO(rescrv): report error
            }
        }
        Ok(values)
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

fn date_time_to_string(when: DateTime<Utc>) -> String {
    when.to_rfc3339_opts(chrono::format::SecondsFormat::Secs, true)
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use indicio::{value, Clue};

    use super::*;

    fn test_case(sym_table: &mut SymbolTable, value: Value) -> Vec<u32> {
        let mut translated: Vec<u32> = vec![];
        sym_table.translate_recursive(&value, "", &mut translated);
        translated
    }

    #[test]
    fn bool_true() {
        let mut sym_table = SymbolTable::default();
        let expected = vec![0x110000u32, 0x110002, 0x110001];
        let returned = test_case(&mut sym_table, value!({key: true}));
        assert_eq!(expected, returned);
        assert_eq!(
            HashMap::from([
                ("o".to_string(), 0x110000),
                ("ok3keyT".to_string(), 0x110002),
            ]),
            sym_table.symbols
        );
    }

    #[test]
    fn bool_false() {
        let mut sym_table = SymbolTable::default();
        let expected = vec![0x110000u32, 0x110002, 0x110001];
        let returned = test_case(&mut sym_table, value!({key: false}));
        assert_eq!(expected, returned);
        assert_eq!(
            HashMap::from([
                ("o".to_string(), 0x110000),
                ("ok3keyF".to_string(), 0x110002),
            ]),
            sym_table.symbols
        );
    }

    #[test]
    fn number_i64() {
        let mut sym_table = SymbolTable::default();
        let expected = vec![
            0x110000u32,
            0x110002,
            127,
            255,
            255,
            255,
            255,
            255,
            255,
            255,
            0x110003,
            0x110001,
        ];
        let returned = test_case(&mut sym_table, value!({key: i64::MAX}));
        assert_eq!(expected, returned);
        assert_eq!(
            HashMap::from([
                ("o".to_string(), 0x110000),
                ("ok3keyi".to_string(), 0x110002),
            ]),
            sym_table.symbols
        );
    }

    #[test]
    fn number_u64() {
        let mut sym_table = SymbolTable::default();
        let expected = vec![
            0x110000u32,
            0x110002,
            255,
            255,
            255,
            255,
            255,
            255,
            255,
            255,
            0x110003,
            0x110001,
        ];
        let returned = test_case(&mut sym_table, value!({key: u64::MAX}));
        assert_eq!(expected, returned);
        assert_eq!(
            HashMap::from([
                ("o".to_string(), 0x110000),
                ("ok3keyu".to_string(), 0x110002),
            ]),
            sym_table.symbols
        );
    }

    #[test]
    fn number_f64() {
        let mut sym_table = SymbolTable::default();
        let expected = vec![
            0x110000u32,
            0x110002,
            64,
            9,
            33,
            251,
            84,
            68,
            45,
            24,
            0x110003,
            0x110001,
        ];
        let returned = test_case(&mut sym_table, value!({key: std::f64::consts::PI}));
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
        let returned = test_case(&mut sym_table, value!({key: "value"}));
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
        let returned = test_case(&mut sym_table, value!({key: ["value1", "value2"]}));
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
        let returned = test_case(&mut sym_table, value!({key: {key: "value"}}));
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

    fn parse_dt(dt: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(dt).unwrap().to_utc()
    }

    fn make_clue(dt: &str, value: Value) -> Clue {
        Clue {
            file: "test_file".to_string(),
            line: 42,
            level: 0,
            timestamp: parse_dt(dt).timestamp_nanos_opt().unwrap() as u64 / 1_000,
            value,
        }
    }

    #[test]
    fn seal_empty() {
        let mut sym_table = SymbolTable::default();
        let start_time = parse_dt("2024-02-16T15:10:00Z");
        assert!(convert_clues_to_analogize_inner(&mut sym_table, start_time, vec![]).is_err());
    }

    #[test]
    fn seal_record_in_first_second() {
        let mut sym_table = SymbolTable::default();
        let start_time = parse_dt("2024-02-16T15:10:00Z");
        let clues = vec![make_clue("2024-02-16T15:10:00.01Z", value!({key: "value"}))];
        let (_, record_boundaries, second_boundaries) =
            convert_clues_to_analogize_inner(&mut sym_table, start_time, clues).unwrap();
        assert_eq!(vec![0], record_boundaries);
        assert_eq!(vec![0, 1], second_boundaries);
    }

    #[test]
    fn seal_one_record_per_second() {
        let mut sym_table = SymbolTable::default();
        let clues = vec![
            make_clue("2024-02-16T15:10:00.01Z", value!({key: "value0"})),
            make_clue("2024-02-16T15:10:01.01Z", value!({key: "value1"})),
            make_clue("2024-02-16T15:10:02.01Z", value!({key: "value2"})),
        ];
        let start_time = parse_dt("2024-02-16T15:10:00Z");
        let (_, record_boundaries, second_boundaries) =
            convert_clues_to_analogize_inner(&mut sym_table, start_time, clues).unwrap();
        assert_eq!(vec![0, 53, 106], record_boundaries);
        assert_eq!(vec![0, 1, 2, 3], second_boundaries);
    }

    #[test]
    fn seal_gap_at_beginning() {
        let mut sym_table = SymbolTable::default();
        let clues = vec![
            make_clue("2024-02-16T15:10:01.01Z", value!({key: "value1"})),
            make_clue("2024-02-16T15:10:02.01Z", value!({key: "value2"})),
            make_clue("2024-02-16T15:10:03.01Z", value!({key: "value3"})),
        ];
        let start_time = parse_dt("2024-02-16T15:10:00Z");
        let (_, record_boundaries, second_boundaries) =
            convert_clues_to_analogize_inner(&mut sym_table, start_time, clues).unwrap();
        assert_eq!(vec![0, 1, 54, 107], record_boundaries);
        assert_eq!(vec![0, 1, 2, 3, 4], second_boundaries);
    }

    #[test]
    fn seal_gap_after_first_record() {
        let mut sym_table = SymbolTable::default();
        let clues = vec![
            make_clue("2024-02-16T15:10:00.01Z", value!({key: "value0"})),
            make_clue("2024-02-16T15:10:02.01Z", value!({key: "value2"})),
            make_clue("2024-02-16T15:10:03.01Z", value!({key: "value3"})),
        ];
        let start_time = parse_dt("2024-02-16T15:10:00Z");
        let (_, record_boundaries, second_boundaries) =
            convert_clues_to_analogize_inner(&mut sym_table, start_time, clues).unwrap();
        assert_eq!(vec![0, 53, 54, 107], record_boundaries);
        assert_eq!(vec![0, 1, 2, 3, 4], second_boundaries);
    }

    #[test]
    fn seal_multiple_records_per_second() {
        let mut sym_table = SymbolTable::default();
        let clues = vec![
            make_clue("2024-02-16T15:10:00.01Z", value!({key: "value1"})),
            make_clue("2024-02-16T15:10:00.02Z", value!({key: "value2"})),
            make_clue("2024-02-16T15:10:00.03Z", value!({key: "value3"})),
            make_clue("2024-02-16T15:10:01.04Z", value!({key: "value4"})),
        ];
        let start_time = parse_dt("2024-02-16T15:10:00Z");
        let (_, record_boundaries, second_boundaries) =
            convert_clues_to_analogize_inner(&mut sym_table, start_time, clues).unwrap();
        assert_eq!(vec![0, 53, 106, 159], record_boundaries);
        assert_eq!(vec![2, 3, 4], second_boundaries);
    }

    #[test]
    fn seal_multiple_records_per_second_with_gaps() {
        let mut sym_table = SymbolTable::default();
        let clues = vec![
            make_clue("2024-02-16T15:10:01.01Z", value!({key: "value1"})),
            make_clue("2024-02-16T15:10:01.02Z", value!({key: "value2"})),
            make_clue("2024-02-16T15:10:01.03Z", value!({key: "value3"})),
            make_clue("2024-02-16T15:10:03.04Z", value!({key: "value4"})),
        ];
        let start_time = parse_dt("2024-02-16T15:10:00Z");
        let (_, record_boundaries, second_boundaries) =
            convert_clues_to_analogize_inner(&mut sym_table, start_time, clues).unwrap();
        assert_eq!(vec![0, 1, 54, 107, 160, 161], record_boundaries);
        assert_eq!(vec![0, 3, 4, 5, 6], second_boundaries);
    }

    #[test]
    fn query_no_normalization_expected() {
        assert_eq!(Query::Any, Query::Any.normalize());
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

    fn do_reverse_translate(value: Value) {
        let mut sym_table = SymbolTable::default();
        let mut text = vec![];
        sym_table.translate_recursive(&value, "", &mut text);
        assert_eq!(Some(value), sym_table.reverse_translate(&text));
    }

    #[test]
    fn reverse_translate_bool() {
        do_reverse_translate(Value::Bool(true));
        do_reverse_translate(Value::Bool(false));
    }

    #[test]
    fn reverse_translate_numbers() {
        do_reverse_translate(Value::I64(42));
        do_reverse_translate(Value::U64(42));
        do_reverse_translate(Value::F64(std::f64::consts::PI));
    }

    #[test]
    fn reverse_translate_string() {
        do_reverse_translate(value!("hello world"));
    }

    #[test]
    fn reverse_translate_array() {
        do_reverse_translate(value!([]));
        do_reverse_translate(value!(["hello world"]));
        do_reverse_translate(value!(["hello world", true]));
        do_reverse_translate(value!(["hello world", true, 42]));
    }

    #[test]
    fn reverse_translate_object() {
        do_reverse_translate(value!({}));
        do_reverse_translate(value!({greeting: "hello world"}));
        do_reverse_translate(value!({greeting: "hello world", success: true}));
        do_reverse_translate(value!({greeting: "hello world", success: true, number: 42}));
    }

    #[test]
    fn reverse_translate_nesting() {
        do_reverse_translate(
            value!({greetings: ["hi", "howdy", "hello world"], numbers: [std::f64::consts::PI, 42]}),
        );
    }
}
