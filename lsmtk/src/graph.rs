use std::cmp::{max, min};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt::Debug;
use std::ops::{Bound, Deref, DerefMut, Range};

use sst::SstMetadata;

use biometrics::{Collector, Counter, Moments};

use zerror::Z;

use zerror_core::ErrorCore;

use super::*;

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static SKIP_LOCKED: Counter = Counter::new("lsmtk.graph.skip_locked");
static LEVEL_EXCEEDS_MAX_LEVEL: Counter = Counter::new("lsmtk.graph.exceeds_max_level");
static OVERLAP_NOT_FOUND: Counter = Counter::new("lsmtk.graph.overlap_not_found");
static EMPTY_FINAL_LEVEL: Counter = Counter::new("lsmtk.graph.empty_final_level");
static EXCEEDS_MAX_BYTES: Counter = Counter::new("lsmtk.graph.exceeds_max_bytes");
static NOT_A_COMPACTION: Counter = Counter::new("lsmtk.graph.not_a_compaction");
static CONSIDERING: Counter = Counter::new("lsmtk.graph.considering");

static NUM_CANDIDATES: Moments = Moments::new("lsmtk.graph.candidates");

pub fn register_biometrics(collector: &Collector) {
    collector.register_counter(&SKIP_LOCKED);
    collector.register_counter(&LEVEL_EXCEEDS_MAX_LEVEL);
    collector.register_counter(&OVERLAP_NOT_FOUND);
    collector.register_counter(&EMPTY_FINAL_LEVEL);
    collector.register_counter(&EXCEEDS_MAX_BYTES);
    collector.register_counter(&NOT_A_COMPACTION);
    collector.register_counter(&CONSIDERING);
    collector.register_moments(&NUM_CANDIDATES);
}

////////////////////////////////////////////// Vertex //////////////////////////////////////////////

#[derive(Clone, Debug)]
pub struct Vertex {
    level: usize,
    color: usize,
    peers: usize,
    bytes_within_color: u64,
}

/////////////////////////////////////////// AdjacencyList //////////////////////////////////////////

#[derive(Clone, Debug, Default)]
pub struct AdjacencyList(BTreeSet<(usize, usize)>);

impl Deref for AdjacencyList {
    type Target = BTreeSet<(usize, usize)>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for AdjacencyList {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<BTreeSet<(usize, usize)>> for AdjacencyList {
    fn from(adj_list: BTreeSet<(usize, usize)>) -> Self {
        Self(adj_list)
    }
}

/////////////////////////////////////////////// Graph //////////////////////////////////////////////

#[derive(Debug)]
pub struct Graph {
    options: LsmOptions,
    metadata: Vec<SstMetadata>,
    vertices: Vec<Vertex>,
    forward_adj_list: AdjacencyList,
    reverse_adj_list: AdjacencyList,
    colors: BTreeSet<usize>,
    color_adj_list: BTreeSet<(usize, usize)>,
}

impl Graph {
    pub fn new(options: LsmOptions, metadata: Vec<SstMetadata>) -> Result<Self, Error> {
        // Create the adjacency lists.
        let (forward_adj_list, reverse_adj_list) =
            Self::construct_adj_lists(&metadata, 0..metadata.len())?;
        Self::from_adj_lists(options, metadata, forward_adj_list, reverse_adj_list)
    }

    pub fn edit(self, to_remove: HashSet<String>, to_add: Vec<SstMetadata>) -> Result<Self, Error> {
        let Self {
            options,
            mut metadata,
            forward_adj_list,
            reverse_adj_list,
            ..
        } = self;
        let mut removes: HashSet<usize> = HashSet::new();
        let mut renames: HashMap<usize, usize> = HashMap::new();
        let mut additions = Vec::new();
        let mut to_add_idx = 0;
        let mut metadata_idx = 0;
        while metadata_idx < metadata.len() {
            if to_remove.contains(&metadata[metadata_idx].setsum()) {
                removes.insert(metadata_idx);
                if to_add_idx >= to_add.len() {
                    if metadata_idx + 1 == metadata.len() {
                        metadata.pop();
                    } else {
                        renames.insert(metadata.len() - 1, metadata_idx);
                        metadata.swap_remove(metadata_idx);
                    }
                } else {
                    additions.push(metadata_idx);
                    metadata[metadata_idx] = to_add[to_add_idx].clone();
                    metadata_idx += 1;
                    to_add_idx += 1;
                }
            } else {
                metadata_idx += 1;
            }
        }
        while to_add_idx < to_add.len() {
            additions.push(metadata.len());
            // Order:  Always push to additions first.
            metadata.push(to_add[to_add_idx].clone());
        }
        let map = |(src, dst)| {
            if removes.contains(&src) {
                None
            } else if removes.contains(&dst) {
                None
            } else {
                let src = *renames.get(&src).unwrap_or(&src);
                let dst = *renames.get(&dst).unwrap_or(&dst);
                Some((src, dst))
            }
        };
        let mut forward_adj_list = AdjacencyList(
            forward_adj_list
                .iter()
                .copied()
                .filter_map(map)
                .collect::<BTreeSet<(usize, usize)>>(),
        );
        let mut reverse_adj_list = AdjacencyList(
            reverse_adj_list
                .iter()
                .copied()
                .filter_map(map)
                .collect::<BTreeSet<(usize, usize)>>(),
        );
        let metadata_len = metadata.len();
        let inner = &|_| 0..metadata_len;
        Self::expand_adj_lists(
            &metadata,
            additions.into_iter(),
            inner,
            &mut forward_adj_list,
            &mut reverse_adj_list,
        )?;
        Self::from_adj_lists(options, metadata, forward_adj_list, reverse_adj_list)
    }

    fn from_adj_lists(
        options: LsmOptions,
        metadata: Vec<SstMetadata>,
        forward_adj_list: AdjacencyList,
        reverse_adj_list: AdjacencyList,
    ) -> Result<Self, Error> {
        // Create a list of vertices.
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
        // Fill in the level for each color.
        let mut colors_stack = Vec::new();
        for color in colors.iter().copied() {
            let lower = Bound::Included((color, 0));
            let upper = Bound::Included((color, usize::max_value()));
            if color_reverse_adj_list.range((lower, upper)).count() == 0 {
                vertices[color].level = 0;
                colors_stack.push(color);
            }
        }
        // Perform a bfs across all colors to set the level to the max level based upon bfs depth.
        while let Some(color) = colors_stack.pop() {
            let lower = Bound::Included((color, usize::min_value()));
            let upper = Bound::Included((color, usize::max_value()));
            for (u, v) in color_forward_adj_list.range((lower, upper)).copied() {
                assert_eq!(color, u);
                if vertices[v].level == vertices.len() || vertices[v].level <= vertices[color].level
                {
                    vertices[v].level = vertices[color].level + 1;
                    colors_stack.push(v);
                }
            }
        }
        // Now fill in the level for every vertex, copying from the color.
        for idx in 0..vertices.len() {
            vertices[idx].level = vertices[vertices[idx].color].level;
            vertices[idx].peers = vertices[vertices[idx].color].peers;
            vertices[idx].bytes_within_color = vertices[vertices[idx].color].bytes_within_color;
        }
        Ok(Self {
            options,
            metadata,
            vertices,
            forward_adj_list,
            reverse_adj_list,
            colors,
            color_adj_list: color_forward_adj_list,
        })
    }

    fn construct_adj_lists(
        metadata: &[SstMetadata],
        outer: impl Iterator<Item = usize>,
    ) -> Result<(AdjacencyList, AdjacencyList), Error> {
        // Create the adjacency lists.
        let mut forward_adj_list = AdjacencyList::default();
        let mut reverse_adj_list = AdjacencyList::default();
        let metadata_len = metadata.len();
        let inner = &|x| (x + 1)..metadata_len;
        Self::expand_adj_lists(
            metadata,
            outer,
            inner,
            &mut forward_adj_list,
            &mut reverse_adj_list,
        )?;
        Ok((forward_adj_list, reverse_adj_list))
    }

    fn expand_adj_lists(
        metadata: &[SstMetadata],
        outer: impl Iterator<Item = usize>,
        inner: &dyn Fn(usize) -> Range<usize>,
        forward_adj_list: &mut AdjacencyList,
        reverse_adj_list: &mut AdjacencyList,
    ) -> Result<(), Error> {
        for i in outer {
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
            for j in inner(i) {
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
        Ok(())
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

impl Graph {
    pub fn compactions(&mut self) -> Vec<Compaction> {
        if self.colors.len() <= 1 {
            let mut compaction = Compaction {
                options: self.options.clone(),
                inputs: Vec::new(),
                stats: CompactionStats {
                    lower_level: 0,
                    upper_level: 0,
                    bytes_input: 0,
                    ratio: 0.0,
                },
            };
            for idx in 0..self.vertices.len() {
                compaction.stats.bytes_input += self.metadata[idx].file_size as usize;
                compaction.inputs.push(self.metadata[idx].clone());
            }
            return if compaction.inputs.len() > 2 {
                vec![compaction]
            } else {
                Vec::new()
            };
        }
        let mut max_level = 0;
        for idx in 0..self.vertices.len() {
            max_level = max(max_level, self.vertices[idx].level);
        }
        let mut compactions = Vec::new();
        // The adjacency lists, and thus the color adjacency lists are a transitive
        // closure of edges.  We make use of that here.
        for color in self.colors.iter().copied() {
            let mut overlap = vec![0u64; max_level + 1];
            if self.vertices[color].level >= max_level {
                LEVEL_EXCEEDS_MAX_LEVEL.click();
                continue;
            }
            overlap[self.vertices[color].level] = self.vertices[color].bytes_within_color;
            let lower = Bound::Included((color, usize::min_value()));
            let upper = Bound::Included((color, usize::max_value()));
            let mut found = false;
            for (u, v) in self.color_adj_list.range((lower, upper)).copied() {
                assert_eq!(color, u);
                overlap[self.vertices[v].level] += self.vertices[v].bytes_within_color;
                found = true;
            }
            if !found {
                OVERLAP_NOT_FOUND.click();
                continue;
            }
            // Select the level with the best ratio of levels N-1 to level N.
            let mut upper_level = None;
            let mut prev_ratio = 0.0;
            for to_consider in self.vertices[color].level + 1..overlap.len() {
                if overlap[to_consider] == 0 {
                    EMPTY_FINAL_LEVEL.click();
                    break;
                }
                let mut acc = 0u64;
                for level in self.vertices[color].level..to_consider {
                    acc = acc.saturating_add(overlap[level]);
                }
                let waste = overlap[to_consider];
                let ratio = acc as f64 / acc.saturating_add(waste) as f64;
                let to_n_minus_one: u64 = overlap[self.vertices[color].level..to_consider]
                    .iter()
                    .sum();
                let to_n: u64 = overlap[self.vertices[color].level..to_consider + 1]
                    .iter()
                    .sum();
                if to_n >= self.options.max_compaction_bytes as u64 {
                    EXCEEDS_MAX_BYTES.click();
                    break;
                }
                if to_n == to_n_minus_one {
                    NOT_A_COMPACTION.click();
                    continue;
                }
                if prev_ratio < ratio {
                    CONSIDERING.click();
                    upper_level = Some(to_consider);
                    prev_ratio = ratio;
                }
            }
            // See if there was a candidate for compaction.
            if let Some(upper_level) = upper_level {
                let mut colors = HashSet::new();
                colors.insert(color);
                for (u, v) in self.color_adj_list.range((lower, upper)).copied() {
                    assert_eq!(color, u);
                    if self.vertices[v].level <= upper_level {
                        colors.insert(v);
                    }
                }
                let mut compaction = Compaction {
                    options: self.options.clone(),
                    inputs: Vec::new(),
                    stats: CompactionStats {
                        lower_level: self.vertices[color].level,
                        upper_level,
                        bytes_input: 0,
                        ratio: prev_ratio,
                    },
                };
                for idx in 0..self.vertices.len() {
                    if colors.contains(&self.vertices[idx].color) {
                        compaction.stats.bytes_input += self.metadata[idx].file_size as usize;
                        compaction.inputs.push(self.metadata[idx].clone());
                    }
                }
                if compaction.inputs.len() > 2 {
                    compactions.push(compaction);
                }
            }
        }
        compactions.sort_by_key(|x| (x.stats().ratio * 1_000_000.0) as u64);
        compactions.reverse();
        NUM_CANDIDATES.add(compactions.len() as f64);
        let mut selected = Vec::with_capacity(compactions.len());
        let mut locked = HashSet::new();
        for compaction in compactions.into_iter() {
            let mut skip = false;
            for input in compaction.inputs.iter() {
                if locked.contains(&input.setsum) {
                    SKIP_LOCKED.click();
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
