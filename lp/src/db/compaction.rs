use std::cmp::{max, min, Ordering, Reverse};
use std::collections::binary_heap::BinaryHeap;
use std::collections::btree_set::BTreeSet;
use std::collections::hash_set::HashSet;
use std::fs::{create_dir, hard_link, read_dir, read_to_string, write};
use std::ops::Bound;
use std::path::{Path, PathBuf};

use prototk::field_types::*;

use util::time::now;

use zerror::{FromIOError, ZError, ZErrorResult};

use super::super::file_manager::open_without_manager;
use super::super::merging_cursor::MergingCursor;
use super::super::options::CompactionOptions;
use super::super::setsum::Setsum;
use super::super::sst::{SST, SSTBuilderOptions, SSTMetadata, SSTMultiBuilder};
use super::super::{compare_bytes, Builder, Cursor, Error};

/////////////////////////////////////////////// util ///////////////////////////////////////////////

fn key_range_overlap(lhs: &SSTMetadata, rhs: &SSTMetadata) -> bool {
    compare_bytes(lhs.first_key.as_bytes(), rhs.last_key.as_bytes()) != Ordering::Greater
        && compare_bytes(rhs.first_key.as_bytes(), lhs.last_key.as_bytes()) != Ordering::Greater
}

//////////////////////////////////////////// Graph::new ////////////////////////////////////////////

#[derive(Clone, Debug)]
struct Vertex {
    level: usize,
    color: usize,
    peers: usize,
    bytes_within_color: u64,
}

#[derive(Debug)]
pub struct Graph<'a> {
    options: CompactionOptions,
    metadata: &'a Vec<SSTMetadata>,
    vertices: Vec<Vertex>,
    colors: BTreeSet<usize>,
    color_adj_list: BTreeSet<(usize, usize)>,
}

impl<'a> Graph<'a> {
    pub fn new(
        options: CompactionOptions,
        metadata: &'a Vec<SSTMetadata>,
    ) -> Result<Self, ZError<Error>> {
        let mut vertices = Vec::with_capacity(metadata.len());
        vertices.resize(
            metadata.len(),
            Vertex {
                color: metadata.len(),
                level: 0,
                peers: 0,
                bytes_within_color: 0,
            },
        );
        // Create the adjacency lists.
        let mut forward_adj_list = BTreeSet::new();
        let mut reverse_adj_list = BTreeSet::new();
        for i in 0..metadata.len() {
            if metadata[i].smallest_timestamp > metadata[i].biggest_timestamp {
                let zerr = ZError::new(Error::Corruption {
                    context: "metadata timestamps not in order".to_string(),
                })
                .with_context::<string, 1>("SST", &metadata[i].setsum())
                .with_context::<uint64, 2>("smallest timestamp", metadata[i].smallest_timestamp)
                .with_context::<uint64, 3>("biggest timestamp", metadata[i].biggest_timestamp);
                return Err(zerr);
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
                bfs_level(&mut vertices, &color_forward_adj_list, color);
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
        let lower = Bound::Included((idx, 0));
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

//////////////////////////////////////////////// bfs ///////////////////////////////////////////////

// Run BFS to fill in the level.  Don't keep a visited list as it's assumed adj_list is a DAG.
fn bfs_level(vertices: &mut Vec<Vertex>, adj_list: &BTreeSet<(usize, usize)>, vertex: usize) {
    let mut heap: BinaryHeap<Reverse<(usize, usize)>> = BinaryHeap::with_capacity(vertices.len());
    heap.push(Reverse((vertices[vertex].level, vertex)));
    while let Some(Reverse((level, vertex))) = heap.pop() {
        vertices[vertex].level = max(vertices[vertex].level, level);
        let lower = Bound::Included((vertex, 0));
        let upper = Bound::Included((vertex, usize::max_value()));
        for (u, v) in adj_list.range((lower, upper)) {
            assert_eq!(vertex, *u);
            heap.push(Reverse((vertices[vertex].level + 1, *v)));
        }
    }
}

//////////////////////////////////////// Graph::compactions ////////////////////////////////////////

impl<'a> Graph<'a> {
    fn max_level(&self) -> usize {
        let mut max_level = 0;
        for idx in 0..self.vertices.len() {
            max_level = max(max_level, self.vertices[idx].level);
        }
        max_level
    }

    pub fn compactions(&self) -> Vec<Compaction> {
        let max_level = self.max_level();
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
                if to_n > self.options.max_compaction_bytes {
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
                compactions.push(compaction);
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
    inputs: Vec<SSTMetadata>,
    options: CompactionOptions,
    smallest_snapshot: u64,
}

#[derive(Clone, Debug, Default)]
pub struct CompactionStats {
    pub num_inputs: usize,
    pub bytes_input: usize,
    pub lower_level: usize,
    pub upper_level: usize,
    pub ratio: f64,
}

impl Compaction {
    pub fn from_inputs(options: CompactionOptions, smallest_snapshot: u64, inputs: Vec<SSTMetadata>) -> Self {
        Self {
            inputs,
            options,
            smallest_snapshot,
        }
    }

    pub fn load<P: AsRef<Path>>(options: CompactionOptions, compaction_root: P) -> Result<Self, ZError<Error>> {
        let mut inputs = Vec::new();
        for input in read_dir(compaction_root.as_ref().to_path_buf().join("inputs")).from_io()? {
            let path = input.from_io()?.path();
            let file = open_without_manager(path)?;
            let sst = SST::from_file_handle(file)?;
            inputs.push(sst.metadata()?);
        }
        let snapshot = read_to_string(compaction_root.as_ref().to_path_buf().join("SNAPSHOT")).from_io()?;
        let smallest_snapshot = snapshot.parse::<u64>().map_err(|e| ZError::new(Error::Corruption {
            context: "could not parse snapshot file".to_string(),
        }).with_context::<string, 1>("root", &compaction_root.as_ref().to_string_lossy()))?;
        Ok(Self {
            inputs,
            options,
            smallest_snapshot,
        })
    }

    pub fn inputs(&self) -> &[SSTMetadata] {
        &self.inputs
    }

    pub fn stats(&self) -> CompactionStats {
        let graph = Graph::new(self.options.clone(), &self.inputs).unwrap();
        let mut stats = CompactionStats::default();
        stats.num_inputs += graph.vertices.len();
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
        stats.ratio = (lower as f64) / (upper as f64);
        stats
    }

    pub fn is_same(&self, compaction: &Compaction) -> bool {
        let mut lhs_metadata = BTreeSet::new();
        let mut rhs_metadata = BTreeSet::new();
        for input in self.inputs().iter() {
            lhs_metadata.insert(input);
        }
        for input in compaction.inputs().iter() {
            rhs_metadata.insert(input);
        }
        lhs_metadata == rhs_metadata
    }

    pub fn setup<P: AsRef<Path>>(&self, root: P) -> Result<String, ZError<Error>> {
        let compaction_time = now::millis();
        let compaction_base = format!("compaction:{}", compaction_time);
        let compaction_root = root.as_ref().to_path_buf().join(&compaction_base);
        create_dir(&compaction_root)
            .from_io()
            .with_context::<string, 1>("root", &compaction_root.to_string_lossy())?;
        create_dir(&compaction_root.join("inputs"))
            .from_io()
            .with_context::<string, 1>("root", &compaction_root.to_string_lossy())?;
        create_dir(&compaction_root.join("outputs"))
            .from_io()
            .with_context::<string, 1>("root", &compaction_root.to_string_lossy())?;
        write(&compaction_root.join("SNAPSHOT"), format!("{}", self.smallest_snapshot));
        for sst in self.inputs.iter() {
            let target = &root.as_ref().join("sst").join(sst.setsum() + ".sst");
            let link_name = &compaction_root.join("inputs").join(sst.setsum() + ".sst");
            hard_link(target, link_name)
                .from_io()
                .with_context::<string, 1>("target", &target.to_string_lossy())
                .with_context::<string, 1>("link_name", &link_name.to_string_lossy())?;
        }
        let compaction = Compaction::load(self.options.clone(), root.as_ref().to_path_buf().join(&compaction_base))?;
        if !self.is_same(&compaction) {
            return Err(ZError::new(Error::LogicError {
                context: "compaction does not load the same from disk".to_string(),
            })
            .with_context::<string, 1>("root", &root.as_ref().to_string_lossy()));
        }
        Ok(compaction_base)
    }
}

///////////////////////////////////////// LosslessCompactor ////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct LosslessCompactor {
    inputs: Vec<PathBuf>,
    prefix: String,
    options: SSTBuilderOptions,
}

impl LosslessCompactor {
    pub fn new(prefix: String, options: SSTBuilderOptions) -> Self {
        Self {
            inputs: Vec::new(),
            prefix,
            options,
        }
    }

    pub fn add_input<P: AsRef<Path>>(&mut self, path: P) {
        self.inputs.push(path.as_ref().to_path_buf());
    }

    pub fn compact(self) -> Result<(), ZError<Error>> {
        let mut ssts: Vec<Box<dyn Cursor>> = Vec::new();
        for sst in self.inputs.iter() {
            ssts.push(Box::new(SST::new(sst)?.cursor()));
        }
        let mut cursor = MergingCursor::new(ssts)?;
        cursor.seek_to_first()?;
        let mut sstmb = SSTMultiBuilder::new(self.prefix, ".sst".to_string(), self.options.clone());
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
        sstmb.seal()
    }
}
