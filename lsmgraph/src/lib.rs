use std::cmp::{max, min, Ordering, Reverse};
use std::collections::binary_heap::BinaryHeap;
use std::collections::btree_set::BTreeSet;
use std::collections::hash_set::HashSet;
use std::fmt::{Debug, Display, Formatter};
use std::fs::{create_dir, hard_link, read_dir, remove_dir, remove_file, rename};
use std::ops::Bound;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use arrrg_derive::CommandLine;

use biometrics::Counter;

use buffertk::{stack_pack, Buffer};

use one_two_eight::{generate_id, generate_id_prototk};

use setsum::Setsum;

use sst::file_manager::FileManager;
use sst::merging_cursor::MergingCursor;
use sst::{compare_bytes, Builder, Cursor, Sst, SstBuilder, SstMetadata, SstMultiBuilder, SstOptions};

use tatl::{Stationary, HeyListen};

use tuple_key::{FromIntoTupleKey, TupleKey};
use tuple_key_derive::FromIntoTupleKey;

use utilz::lockfile::Lockfile;
use utilz::time::now;

use zerror::Z;

use zerror_core::ErrorCore;

///////////////////////////////////////////// Constants ////////////////////////////////////////////

#[allow(non_snake_case)]
fn LOCKFILE<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().to_path_buf().join("LOCKFILE")
}

#[allow(non_snake_case)]
fn SST_ROOT<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().to_path_buf().join("sst")
}

#[allow(non_snake_case)]
fn SST_FILE<P: AsRef<Path>>(root: P, setsum: String) -> PathBuf {
    SST_ROOT(root).join(setsum + ".sst")
}

#[allow(non_snake_case)]
fn META_ROOT<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().to_path_buf().join("meta")
}

#[allow(non_snake_case)]
fn META_FILE<P: AsRef<Path>>(root: P, setsum: String) -> PathBuf {
    META_ROOT(root).join("meta.".to_owned() + &setsum + ".sst")
}

#[allow(non_snake_case)]
fn COMPACTION_ROOT<P: AsRef<Path>>(root: P, setsum: String) -> PathBuf {
    root.as_ref().to_path_buf().join(setsum + ".sst")
}

//////////////////////////////////////////////// IDs ///////////////////////////////////////////////

generate_id!(MetaID, "meta:");
generate_id_prototk!(MetaID);

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static LOCK_OBTAINED: Counter = Counter::new("lsmtk.lock_obtained");

static LOCK_NOT_OBTAINED: Counter = Counter::new("lsmtk.lock_not_obtained");
static LOCK_NOT_OBTAINED_MONITOR: Stationary =
    Stationary::new("lsmtk.lock_not_obtained", &LOCK_NOT_OBTAINED);

pub fn register_monitors(hey_listen: &mut HeyListen) {
    hey_listen.register_stationary(&LOCK_NOT_OBTAINED_MONITOR);
}

/////////////////////////////////////////// get_lockfile ///////////////////////////////////////////

pub fn get_lockfile(options: &LsmOptions, root: &PathBuf) -> Result<Lockfile, Error> {
    // Deal with making the root directory.
    if root.is_dir() && options.fail_if_exists {
        return Err(Error::DbExists { core: ErrorCore::default(), path: root.clone() });
    }
    if !root.is_dir() && options.fail_if_not_exist {
        return Err(Error::DbNotExist { core: ErrorCore::default(), path: root.clone() });
    } else if !root.is_dir() {
        create_dir(root)
            .map_io_err()
            .with_variable("root", root.to_string_lossy())?;
    }
    // Deal with the lockfile first.
    let lockfile = if options.fail_if_locked {
        Lockfile::lock(LOCKFILE(root))
            .map_io_err()
            .with_variable("root", root.to_string_lossy())?
    } else {
        Lockfile::wait(LOCKFILE(root))
            .map_io_err()
            .with_variable("root", root.to_string_lossy())?
    };
    if lockfile.is_none() {
        LOCK_NOT_OBTAINED.click();
        let err = Error::LockNotObtained {
            core: ErrorCore::default(),
            path: LOCKFILE(root),
        };
        return Err(err);
    }
    LOCK_OBTAINED.click();
    Ok(lockfile.unwrap())
}


/////////////////////////////////////////////// Error //////////////////////////////////////////////

#[derive(Clone, Debug)]
pub enum Error {
    KeyTooLarge {
        core: ErrorCore,
        length: usize,
        limit: usize,
    },
    ValueTooLarge {
        core: ErrorCore,
        length: usize,
        limit: usize,
    },
    SortOrder {
        core: ErrorCore,
        last_key: Vec<u8>,
        last_timestamp: u64,
        new_key: Vec<u8>,
        new_timestamp: u64,
    },
    TableFull {
        core: ErrorCore,
        size: usize,
        limit: usize,
    },
    BlockTooSmall {
        core: ErrorCore,
        length: usize,
        required: usize,
    },
    UnpackError {
        core: ErrorCore,
        error: prototk::Error,
        context: String,
    },
    Crc32cFailure {
        core: ErrorCore,
        start: u64,
        limit: u64,
        crc32c: u32,
    },
    Corruption {
        core: ErrorCore,
        context: String,
    },
    LogicError {
        core: ErrorCore,
        context: String,
    },
    SystemError {
        core: ErrorCore,
        what: String,
    },
    TooManyOpenFiles {
        core: ErrorCore,
        limit: usize,
    },
    LockNotObtained {
        core: ErrorCore,
        path: PathBuf,
    },
    DuplicateSst {
        core: ErrorCore,
        what: String,
    },
    SstNotFound {
        core: ErrorCore,
        setsum: String,
    },
    DbExists {
        core: ErrorCore,
        path: PathBuf,
    },
    DbNotExist {
        core: ErrorCore,
        path: PathBuf,
    },
    PathError {
        core: ErrorCore,
        path: PathBuf,
        what: String,
    },
}

impl Error {
    fn core(&self) -> &ErrorCore {
        match self {
            Error::KeyTooLarge { core, .. } => { core },
            Error::ValueTooLarge { core, .. } => { core } ,
            Error::SortOrder { core, .. } => { core } ,
            Error::TableFull { core, .. } => { core } ,
            Error::BlockTooSmall { core, .. } => { core } ,
            Error::UnpackError { core, .. } => { core } ,
            Error::Crc32cFailure { core, .. } => { core } ,
            Error::LockNotObtained { core, .. } => { core } ,
            Error::DuplicateSst { core, .. } => { core } ,
            Error::Corruption { core, .. } => { core } ,
            Error::LogicError { core, .. } => { core } ,
            Error::SystemError { core, .. } => { core } ,
            Error::TooManyOpenFiles { core, .. } => { core } ,
            Error::SstNotFound { core, .. } => { core } ,
            Error::DbExists { core, .. } => { core } ,
            Error::DbNotExist { core, .. } => { core } ,
            Error::PathError { core, .. } => { core } ,
        }
    }

    fn core_mut(&mut self) -> &mut ErrorCore {
        match self {
            Error::KeyTooLarge { core, .. } => { core },
            Error::ValueTooLarge { core, .. } => { core } ,
            Error::SortOrder { core, .. } => { core } ,
            Error::TableFull { core, .. } => { core } ,
            Error::BlockTooSmall { core, .. } => { core } ,
            Error::UnpackError { core, .. } => { core } ,
            Error::Crc32cFailure { core, .. } => { core } ,
            Error::LockNotObtained { core, .. } => { core } ,
            Error::DuplicateSst { core, .. } => { core } ,
            Error::Corruption { core, .. } => { core } ,
            Error::LogicError { core, .. } => { core } ,
            Error::SystemError { core, .. } => { core } ,
            Error::TooManyOpenFiles { core, .. } => { core } ,
            Error::SstNotFound { core, .. } => { core } ,
            Error::DbExists { core, .. } => { core } ,
            Error::DbNotExist { core, .. } => { core } ,
            Error::PathError { core, .. } => { core } ,
        }
    }
}

impl Z for Error {
    type Error = Self;

    fn long_form(&self) -> String {
        format!("{}", self) + "\n" + &self.core().long_form()
    }

    fn with_token(mut self, identifier: &str, value: &str) -> Self::Error {
        self.set_token(identifier, value);
        self
    }

    fn set_token(&mut self, identifier: &str, value: &str) {
        self.core_mut().set_token(identifier, value);
    }

    fn with_url(mut self, identifier: &str, url: &str) -> Self::Error {
        self.set_url(identifier, url);
        self
    }

    fn set_url(&mut self, identifier: &str, url: &str) {
        self.core_mut().set_url(identifier, url);
    }

    fn with_variable<X: Debug>(mut self, variable: &str, x: X) -> Self::Error where X: Debug {
        self.set_variable(variable, x);
        self
    }

    fn set_variable<X: Debug>(&mut self, variable: &str, x: X) {
        self.core_mut().set_variable(variable, x);
    }
}

impl Display for Error {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        // TODO(rescrv):  Make sure this isn't infinitely co-recursive with long_form
        write!(fmt, "{}", self.long_form())
    }
}

impl From<std::io::Error> for Error {
    fn from(what: std::io::Error) -> Error {
        Error::SystemError { core: ErrorCore::default(), what: what.to_string() }
    }
}

impl From<sst::Error> for Error {
    fn from(what: sst::Error) -> Error {
        match what {
            sst::Error::KeyTooLarge { core, length, limit } => Error::KeyTooLarge { core, length, limit },
            sst::Error::ValueTooLarge { core, length, limit } => Error::ValueTooLarge { core, length, limit },
            sst::Error::SortOrder { core, last_key, last_timestamp, new_key, new_timestamp } => Error::SortOrder { core, last_key, last_timestamp, new_key, new_timestamp },
            sst::Error::TableFull { core, size, limit } => Error::TableFull { core, size, limit },
            sst::Error::BlockTooSmall { core, length, required } => Error::BlockTooSmall { core, length, required },
            sst::Error::UnpackError { core, error, context } => Error::UnpackError { core, error, context },
            sst::Error::Crc32cFailure { core, start, limit, crc32c } => Error::Crc32cFailure { core, start, limit, crc32c },
            sst::Error::Corruption { core, context } => Error::Corruption { core, context },
            sst::Error::LogicError { core, context } => Error::LogicError { core, context },
            sst::Error::SystemError { core, what } => Error::SystemError { core, what },
            sst::Error::TooManyOpenFiles { core, limit } => Error::TooManyOpenFiles { core, limit },
        }
    }
}

////////////////////////////////////////////// FromIO //////////////////////////////////////////////

pub trait MapIoError {
    type Result;

    fn map_io_err(self) -> Self::Result;
}

impl<T> MapIoError for Result<T, std::io::Error> {
    type Result = Result<T, Error>;

    fn map_io_err(self) -> Self::Result {
        match self {
            Ok(x) => Ok(x),
            Err(e) => Err(Error::from(e)),
        }
    }
}

//////////////////////////////////////////// LsmOptions ////////////////////////////////////////////

#[derive(CommandLine, Clone, Debug, Eq, PartialEq)]
pub struct LsmOptions {
    #[arrrg(flag, "Create the graph if it does not exist.")]
    fail_if_not_exist: bool,
    #[arrrg(flag, "Exit with an error if the graph exists.")]
    fail_if_exists: bool,
    #[arrrg(flag, "Block waiting for the lock.")]
    fail_if_locked: bool,
    #[arrrg(optional, "Maximum number of files to open", "FILES")]
    max_open_files: usize,
    #[arrrg(nested)]
    sst: SstOptions,
    #[arrrg(required, "Root path for the lsmgraph", "PATH")]
    path: String,
    #[arrrg(optional, "Root Table's 16B identifier", "XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX")]
    meta_id: MetaID,
    #[arrrg(optional, "Maximum number of bytes permitted in a compaction", "BYTES")]
    max_compaction_bytes: usize,
}

impl LsmOptions {
    pub fn open(&self) -> Result<DB, Error> {
        let root: PathBuf = PathBuf::from(&self.path);
        let lockfile = get_lockfile(self, &root)?;
        let root: PathBuf = root
            .canonicalize()
            .map_io_err()
            .with_variable("root", root.to_string_lossy())?;
        let file_manager = Arc::new(FileManager::new(self.max_open_files));
        let file_manager_p = Arc::clone(&file_manager);
        // Create the correct directories, or at least make sure they exist.
        if !META_ROOT(&self.path).is_dir() {
            create_dir(META_ROOT(&self.path))
                .map_io_err()
                .with_variable("meta", META_ROOT(&self.path))?;
        }
        if !SST_ROOT(&self.path).is_dir() {
            create_dir(SST_ROOT(&self.path))
                .map_io_err()
                .with_variable("sst", SST_ROOT(&self.path))?;
        }
        if !SST_ROOT(&self.path).is_dir() {
            create_dir(SST_ROOT(&self.path))
                .map_io_err()
                .with_variable("sst", SST_ROOT(&self.path))?;
        }
        let metadata = Mutex::new(Metadata::new(&self.path, file_manager_p)?);
        let lsm_graph = DB {
            root,
            options: self.clone(),
            file_manager,
            metadata,
            _lockfile: lockfile,
        };
        lsm_graph.reload()?;
        Ok(lsm_graph)
    }
}

impl Default for LsmOptions {
    fn default() -> Self {
        Self {
            fail_if_not_exist: false,
            fail_if_exists: false,
            fail_if_locked: false,
            max_open_files: 1 << 20,
            sst: SstOptions::default(),
            path: "db".to_owned(),
            meta_id: MetaID::from_human_readable("meta:2482d311-f68a-4da6-bfc1-f65b2db7ca99").unwrap(),
            max_compaction_bytes: usize::max_value(),
        }
    }
}

///////////////////////////////////////////// Metadata /////////////////////////////////////////////

struct Metadata {
    root: PathBuf,
    file_manager: Arc<FileManager>,

    meta: Vec<SstMetadata>,
    data: Vec<SstMetadata>,
}

impl Metadata {
    fn new<P: AsRef<Path>>(root: P, file_manager: Arc<FileManager>) -> Result<Arc<Self>, Error> {
        let md = Self {
            root: root.as_ref().to_path_buf(),
            file_manager,
            meta: Vec::new(),
            data: Vec::new(),
        };
        md.reload()
    }

    fn reload(&self) -> Result<Arc<Self>, Error> {
        let mut md = Self {
            root: self.root.clone(),
            file_manager: Arc::clone(&self.file_manager),
            meta: Vec::new(),
            data: Vec::new(),
        };
        for file in read_dir(META_ROOT(&self.root))? {
            let file = self.file_manager.open(file?.path())?;
            let sst = Sst::from_file_handle(file)?;
            md.meta.push(sst.metadata()?);
        }
        for file in read_dir(SST_ROOT(&self.root))? {
            let file = self.file_manager.open(file?.path())?;
            let sst = Sst::from_file_handle(file)?;
            md.data.push(sst.metadata()?);
        }
        Ok(Arc::new(md))
    }
}

//////////////////////////////////////////// MetadataKey ///////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, PartialEq, FromIntoTupleKey)]
struct MetadataKey (
    #[tuple_key(1)]
    [u8; 16],
    #[tuple_key(1)]
    [u8; 32],
);

///////////////////////////////////////// key_range_overlap ////////////////////////////////////////

fn key_range_overlap(lhs: &SstMetadata, rhs: &SstMetadata) -> bool {
    compare_bytes(lhs.first_key.as_bytes(), rhs.last_key.as_bytes()) != Ordering::Greater
        && compare_bytes(rhs.first_key.as_bytes(), lhs.last_key.as_bytes()) != Ordering::Greater
}

////////////////////////////////////////////// Vertex //////////////////////////////////////////////

#[derive(Clone, Debug)]
struct Vertex {
    level: usize,
    color: usize,
    peers: usize,
    bytes_within_color: u64,
}

///////////////////////////////////// GraphRepresentation::new /////////////////////////////////////

#[derive(Debug)]
pub struct GraphRepresentation<'a> {
    options: LsmOptions,
    metadata: &'a Vec<SstMetadata>,
    vertices: Vec<Vertex>,
    colors: BTreeSet<usize>,
    color_adj_list: BTreeSet<(usize, usize)>,
}

impl<'a> GraphRepresentation<'a> {
    pub fn new(
        options: LsmOptions,
        metadata: &'a Vec<SstMetadata>,
    ) -> Result<Self, Error> {
        let mut vertices = Vec::with_capacity(metadata.len());
        vertices.resize(
            metadata.len(),
            Vertex {
                color: metadata.len(),
                level: metadata.len(),
                peers: 0,
                bytes_within_color: 0,
            },
        );
        // Create the adjacency lists.
        let mut forward_adj_list = BTreeSet::new();
        let mut reverse_adj_list = BTreeSet::new();
        for i in 0..metadata.len() {
            if metadata[i].smallest_timestamp > metadata[i].biggest_timestamp {
                let err = Error::Corruption {
                    core: ErrorCore::default(),
                    context: "metadata timestamps not in order".to_string(),
                }
                .with_variable("SST", metadata[i].setsum())
                .with_variable("smallest_timestamp", metadata[i].smallest_timestamp)
                .with_variable("biggest_timestamp", metadata[i].biggest_timestamp);
                return Err(err);
            }
            for j in i + 1..metadata.len() {
                if !key_range_overlap(&metadata[i], &metadata[j]) {
                    continue;
                }
                // NOTE(rescrv):  This will establish a link in the adjacency list between every
                // SST that has an overlap in the keyspace.  We use this fact elsewhere.
                if metadata[i].biggest_timestamp < metadata[j].smallest_timestamp {
                    forward_adj_list.insert((j, i));
                    reverse_adj_list.insert((i, j));
                } else if metadata[j].biggest_timestamp < metadata[i].smallest_timestamp {
                    forward_adj_list.insert((i, j));
                    reverse_adj_list.insert((j, i));
                } else {
                    forward_adj_list.insert((i, j));
                    forward_adj_list.insert((j, i));
                    reverse_adj_list.insert((i, j));
                    reverse_adj_list.insert((j, i));
                }
            }
        }
        // Compute the strongly-connected components.
        tarjan_scc(&mut vertices, &forward_adj_list);
        // Find the number of peers for each vertex.  First do a pass to calculate the peers, then
        // do a pass to set the number of peers for each vertex.
        let mut colors = BTreeSet::new();
        for idx in 0..vertices.len() {
            let color = vertices[idx].color;
            colors.insert(color);
            vertices[color].peers += 1;
            vertices[color].bytes_within_color += metadata[idx].file_size;
        }
        for idx in 0..vertices.len() {
            vertices[idx].peers = vertices[vertices[idx].color].peers;
            vertices[idx].bytes_within_color = vertices[vertices[idx].color].bytes_within_color;
        }
        // Use the strongly connected components to make a graph of the links between components.
        // An edge (u, v) in this graph means there's at least one edge from color u to color v.
        //
        // Unlink forward_adj_list, color_forward_adj_list is a DAG.
        //
        // If it had a cycle it would mean we could create a new strongly-connected component that
        // would have been found by tarjan_scc and the colors would be the same.
        let mut color_forward_adj_list = BTreeSet::new();
        let mut color_reverse_adj_list = BTreeSet::new();
        for (src, dst) in forward_adj_list.iter() {
            let src: usize = *src;
            let dst: usize = *dst;
            if vertices[src].color != vertices[dst].color {
                color_forward_adj_list.insert((vertices[src].color, vertices[dst].color));
                color_reverse_adj_list.insert((vertices[dst].color, vertices[src].color));
            }
        }
        // Find colors with no incoming edges from other colors.
        // Put colors in level 1 when the file has no peers (a single SST); else 0.
        for color in colors.iter() {
            let color: usize = *color;
            let lower = Bound::Included((color, 0));
            let upper = Bound::Included((color, usize::max_value()));
            if color_reverse_adj_list.range((lower, upper)).count() == 0 {
                if vertices[color].peers == 1 {
                    vertices[color].level = 1;
                } else {
                    vertices[color].level = 0;
                }
                let mut heap: BinaryHeap<Reverse<(usize, usize)>> = BinaryHeap::with_capacity(vertices.len());
                heap.push(Reverse((vertices[color].level, color)));
                while let Some(Reverse((level, color))) = heap.pop() {
                    if vertices[color].level < level {
                        vertices[color].level = level;
                        let lower = Bound::Included((color, 0));
                        let upper = Bound::Included((color, usize::max_value()));
                        for (u, v) in color_forward_adj_list.range((lower, upper)) {
                            assert_eq!(color, *u);
                            heap.push(Reverse((level + 1, *v)));
                        }
                    }
                }
            }
        }
        // Fill in the level by copying from color.
        for idx in 0..vertices.len() {
            vertices[idx].level = vertices[vertices[idx].color].level;
        }
        Ok(Self {
            options,
            metadata,
            vertices,
            colors,
            color_adj_list: color_forward_adj_list,
        })
    }
}

/////////////////////////////// Tarjan Strongly Connected Components ///////////////////////////////

#[derive(Clone)]
struct TarjanVertex {
    index: usize,
    lowlink: usize,
    on_stack: bool,
}

// The Tarjan strongly connected components algorithm unrolled to not be recursive.
fn tarjan_scc(vertices: &mut Vec<Vertex>, adj_list: &BTreeSet<(usize, usize)>) {
    let mut state = Vec::with_capacity(vertices.len());
    state.resize(
        vertices.len(),
        TarjanVertex {
            index: vertices.len(),
            lowlink: vertices.len(),
            on_stack: false,
        },
    );
    let mut index = 0;
    let mut stack = Vec::new();

    for idx in 0..vertices.len() {
        if state[idx].index != vertices.len() {
            continue;
        }
        let lower = Bound::Included((idx, usize::min_value()));
        let upper = Bound::Included((idx, usize::max_value()));
        let iter = adj_list.range((lower, upper));
        let mut recursion = Vec::new();
        // strongconnect
        recursion.push((idx, iter));
        state[idx].index = index;
        state[idx].lowlink = index;
        index += 1;
        stack.push(idx);
        state[idx].on_stack = true;
        while !recursion.is_empty() {
            let recursion_idx = recursion.len() - 1;
            let (v, iter) = &mut recursion[recursion_idx];
            if let Some((src, w)) = iter.next() {
                assert_eq!(*v, *src);
                if state[*w].index == vertices.len() {
                    let lower = Bound::Included((*w, 0));
                    let upper = Bound::Included((*w, usize::max_value()));
                    let iter = adj_list.range((lower, upper));
                    // strongconnect
                    recursion.push((*w, iter));
                    state[*w].index = index;
                    state[*w].lowlink = index;
                    index += 1;
                    stack.push(*w);
                    state[*w].on_stack = true;
                } else if state[*w].on_stack {
                    state[*v].lowlink = min(state[*v].lowlink, state[*w].index);
                }
            } else {
                if state[*v].lowlink == state[*v].index {
                    vertices[*v].color = *v;
                    while !stack.is_empty() && stack[stack.len() - 1] != *v {
                        let w = stack.pop().unwrap();
                        state[w].on_stack = false;
                        vertices[w].color = *v;
                    }
                    assert!(!stack.is_empty() && stack[stack.len() - 1] == *v);
                    stack.pop();
                    state[*v].on_stack = false;
                }
                let prev_w: usize = *v;
                recursion.pop();
                if !recursion.is_empty() {
                    let prev_v = recursion[recursion.len() - 1].0;
                    state[prev_v].lowlink = min(state[prev_v].lowlink, state[prev_w].lowlink);
                }
            }
        }
    }
}

///////////////////////////////// GraphRepresentation::compactions /////////////////////////////////

impl<'a> GraphRepresentation<'a> {
    pub fn compactions(&self) -> Vec<Compaction> {
        let mut max_level = 0;
        for idx in 0..self.vertices.len() {
            max_level = max(max_level, self.vertices[idx].level);
        }
        let mut compactions = Vec::new();
        // There are two bad patterns that come from overlapping keyspaces within imported SSTs:
        // - Overlapping keyspaces with overlapping timestamps.  Each such overlap is one color.
        // - Overlapping keyspaces with disjoint timestamps.  This creates many colors/levels.
        //
        // The first case will be handled explicitly in the event that the algorithm below does not
        // handle it.
        //
        // NOTE(rescrv):  The adjacency lists, and thus the color adjacency lists are a transitive
        // closure of edges.  We make use of that here.
        for color in self.colors.iter() {
            let level = self.vertices[*color].level;
            let mut overlap = vec![0u64; max_level + 1];
            overlap[level] = self.vertices[*color].bytes_within_color;
            let lower = Bound::Included((*color, 0));
            let upper = Bound::Included((*color, usize::max_value()));
            for (u, v) in self.color_adj_list.range((lower, upper)) {
                assert_eq!(*color, *u);
                overlap[self.vertices[*v].level] += self.vertices[*v].bytes_within_color;
            }
            // Select the level with the best ratio of levels N-1 to level N.
            let mut upper_level = None;
            for to_consider in level + 1..overlap.len() {
                let to_n_minus_one: u64 = overlap[level..to_consider].iter().sum();
                let to_n: u64 = overlap[level..to_consider + 1].iter().sum();
                if to_n > self.options.max_compaction_bytes as u64 {
                    continue;
                }
                let ratio = (to_n_minus_one as f64) / (to_n as f64);
                if upper_level.is_none() {
                    upper_level = Some((to_consider, ratio));
                }
                if let Some((_, prev_ratio)) = upper_level {
                    if prev_ratio <= ratio {
                        upper_level = Some((to_consider, ratio));
                    }
                }
            }
            if upper_level.is_none() && self.vertices[*color].peers > 1 {
                upper_level = Some((level, 0.0));
            }
            // See if there was a candidate for compaction.
            if let Some((upper_level, _)) = upper_level {
                let mut colors = HashSet::new();
                colors.insert(*color);
                for (u, v) in self.color_adj_list.range((lower, upper)) {
                    assert_eq!(*color, *u);
                    if self.vertices[*v].level <= upper_level {
                        colors.insert(*v);
                    }
                }
                let mut compaction = Compaction::default();
                for idx in 0..self.vertices.len() {
                    if colors.contains(&self.vertices[idx].color) {
                        compaction.inputs.push(self.metadata[idx].clone());
                    }
                }
                if compaction.inputs.len() > 1 {
                    compactions.push(compaction);
                }
            }
        }
        compactions.sort_by_key(|x| (x.stats().ratio * 1_000_000.0) as u64);
        compactions.reverse();
        let mut selected = Vec::with_capacity(compactions.len());
        let mut locked = HashSet::new();
        for compaction in compactions.into_iter() {
            let mut skip = false;
            for input in compaction.inputs.iter() {
                if locked.contains(&input.setsum) {
                    skip = true;
                }
            }
            if skip {
                continue;
            }
            for input in compaction.inputs.iter() {
                locked.insert(input.setsum);
            }
            selected.push(compaction);
        }
        selected
    }
}

//////////////////////////////////////////// Compaction ////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct Compaction {
    pub options: LsmOptions,
    pub inputs: Vec<SstMetadata>,
}

impl Compaction {
    pub fn stats(&self) -> CompactionStats {
        let graph = GraphRepresentation::new(self.options.clone(), &self.inputs).unwrap();
        let mut stats = CompactionStats::default();
        stats.num_inputs += self.inputs.len();
        if stats.num_inputs > 0 {
            stats.lower_level = usize::max_value();
        }
        for input in self.inputs.iter() {
            stats.bytes_input += input.file_size as usize;
        }
        for vertex in graph.vertices.iter() {
            stats.lower_level = min(stats.lower_level, vertex.level);
            stats.upper_level = max(stats.upper_level, vertex.level);
        }
        let mut lower = 0;
        let mut upper = 0;
        for idx in 0..self.inputs.len() {
            upper += self.inputs[idx].file_size;
            if graph.vertices[idx].level < stats.upper_level {
                lower += self.inputs[idx].file_size;
            }
        }
        if upper > 0 {
            stats.ratio = (lower as f64) / (upper as f64);
        }
        stats
    }

    pub fn perform(&self, file_manager: &FileManager) -> Result<(), Error> {
        let mut digests: Vec<(Setsum, Option<Buffer>)> = Vec::new();
        let mut cursors: Vec<Box<dyn Cursor + 'static>> = Vec::new();
        let mut acc_setsum = Setsum::default();
        for sst_metadata in &self.inputs {
            let sst_setsum = Setsum::from_digest(sst_metadata.setsum);
            acc_setsum = acc_setsum + sst_setsum.clone();
            let file = file_manager.open(SST_FILE(&self.options.path, sst_setsum.hexdigest()))?;
            digests.push((sst_setsum, None));
            let sst = Sst::from_file_handle(file)?;
            cursors.push(Box::new(sst.cursor()));
        }
        let mut cursor = MergingCursor::new(cursors)?;
        cursor.seek_to_first()?;
        let prefix = COMPACTION_ROOT(&self.options.path, acc_setsum.hexdigest());
        create_dir(prefix.clone())?;
        let mut sstmb = SstMultiBuilder::new(prefix.clone(), ".sst".to_string(), self.options.sst.clone());
        loop {
            cursor.next()?;
            let kvr = match cursor.value() {
                Some(v) => { v },
                None => { break; },
            };
            match kvr.value {
                Some(v) => { sstmb.put(kvr.key, kvr.timestamp, v)?; }
                None => { sstmb.del(kvr.key, kvr.timestamp)?; }
            }
        }
        let paths = sstmb.seal()?;
        for path in paths.iter() {
            let file = file_manager.open(path.clone())?;
            let sst = Sst::from_file_handle(file)?;
            let sst_setsum = sst.setsum();
            digests.push((Setsum::from_digest(sst_setsum.digest()), Some(stack_pack(sst.metadata()?).to_buffer())));
            let new_path = SST_FILE(&self.options.path, sst_setsum.hexdigest());
            hard_link(path, new_path)?;
        }
        digests.sort_by_key(|x| x.0.hexdigest());
        let meta_now = now::millis();
        let meta_file_final = META_FILE(&self.options.path, acc_setsum.hexdigest());
        let meta_file = format!("tmp-{}-{}.sst", acc_setsum.hexdigest(), meta_now);
        let mut meta = SstBuilder::new(&meta_file, self.options.sst.clone())?;
        for (digest, buf) in digests.into_iter() {
            let key = MetadataKey(self.options.meta_id.id, digest.digest());
            let tuple_key = key.into_tuple_key();
            match buf {
                Some(value) => {
                    meta.put(tuple_key.as_bytes(), meta_now, value.as_bytes())?;
                },
                None => {
                    meta.del(tuple_key.as_bytes(), meta_now)?;
                },
            }
        }
        meta.seal()?;
        rename(meta_file, meta_file_final)?;
        for path in paths.into_iter() {
            remove_file(path)?;
        }
        remove_dir(prefix)?;
        Ok(())
    }
}

////////////////////////////////////////// CompactionStats /////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct CompactionStats {
    pub num_inputs: usize,
    pub bytes_input: usize,
    pub lower_level: usize,
    pub upper_level: usize,
    pub ratio: f64,
}

//////////////////////////////////////////////// DB ////////////////////////////////////////////////

pub struct DB {
    root: PathBuf,
    options: LsmOptions,
    file_manager: Arc<FileManager>,
    metadata: Mutex<Arc<Metadata>>,
    _lockfile: Lockfile,
}

impl DB {
    pub fn ingest(&self, sst_paths: &[PathBuf]) -> Result<(), Error> {
        // For each SST, hardlink it into the ingest root.
        let mut ssts = Vec::new();
        let mut acc = Setsum::default();
        for sst_path in sst_paths {
            let file = self.file_manager.open(sst_path.clone())?;
            let sst = Sst::from_file_handle(file)?;
            // Update the setsum for the ingest.
            let setsum = sst.setsum();
            acc = acc + Setsum::from_digest(setsum.digest());
            // Hard-link the file into place.
            let target = SST_FILE(&self.root, setsum.hexdigest());
            if target.is_file() {
                return Err(Error::DuplicateSst {
                    core: ErrorCore::default(),
                    what: target.to_string_lossy().to_string(),
                });
            }
            hard_link(sst_path, target).map_io_err()?;
            // Extract the metadata.
            let metadata = sst.metadata()?;
            ssts.push(metadata);
        }
        ssts.sort_by(|lhs, rhs| compare_bytes(&lhs.setsum, &rhs.setsum));
        // Create one file that will be linked into meta.  Swizzling this file is what gives us a
        // form of atomicity.
        let meta_file_final = META_FILE(&self.root, acc.hexdigest());
        let meta_file = format!("tmp-{}-{}.sst", acc.hexdigest(), now::millis());
        let mut meta = SstBuilder::new(&meta_file, self.options.sst.clone())?;
        for metadata in ssts.iter() {
            let key = MetadataKey(self.options.meta_id.id, metadata.setsum);
            let tuple_key = key.into_tuple_key();
            let ts = std::fs::metadata(SST_FILE(&self.root, metadata.setsum()))?.modified()?.duration_since(std::time::UNIX_EPOCH).map_err(|err| {
                Error::SystemError {
                    core: ErrorCore::default(),
                    what: err.to_string(),
                }
            })?.as_secs();
            let value = stack_pack(metadata).to_buffer();
            meta.put(tuple_key.as_bytes(), ts, value.as_bytes())?;
        }
        meta.seal()?;
        rename(meta_file, meta_file_final)?;
        Ok(())
    }

    pub fn compactions(&self) -> Result<Vec<Compaction>, Error> {
        let state = self.get_state();
        let graph = GraphRepresentation::new(self.options.clone(), &state.data)?;
        let mut compactions = graph.compactions();
        compactions.sort_by_key(|x| (x.stats().ratio * 1_000_000.0) as u64);
        compactions.reverse();
        Ok(compactions)
    }

    pub fn compact(&self, ssts: &[String]) -> Result<(), Error> {
        let mut compaction = Compaction {
            options: self.options.clone(),
            inputs: Vec::new(),
        };
        for sst_setsum in ssts {
            let file = self.file_manager.open(SST_FILE(self.options.path.clone(), sst_setsum.to_string()))?;
            let sst = Sst::from_file_handle(file)?;
            compaction.inputs.push(sst.metadata()?);
        }
        compaction.perform(&self.file_manager)?;
        self.reload()?;
        Ok(())
    }

    fn reload(&self) -> Result<(), Error> {
        // We will hold the lock for the entirety of this call to synchronize all calls to the lsm
        // tree.  Everything else should grab the state and then grab the tree behind the Arc.
        let mut guard = self.metadata.lock().unwrap();
        *guard = guard.reload()?;
        Ok(())
    }

    fn get_state(&self) -> Arc<Metadata> {
        Arc::clone(&self.metadata.lock().unwrap())
    }
}
