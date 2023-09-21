use std::cmp::{max, min};
use std::collections::btree_set::BTreeSet;
use std::collections::hash_set::HashSet;
use std::fmt::Debug;
use std::ops::Bound;

use sst::SstMetadata;

use zerror::Z;

use zerror_core::ErrorCore;

use super::*;

////////////////////////////////////////////// Vertex //////////////////////////////////////////////

#[derive(Clone, Debug)]
pub struct Vertex {
    level: usize,
    color: usize,
    peers: usize,
    bytes_within_color: u64,
}

/////////////////////////////////////////////// Graph //////////////////////////////////////////////

#[derive(Debug)]
pub struct Graph<'a> {
    options: LsmOptions,
    metadata: &'a [SstMetadata],
    vertices: Vec<Vertex>,
    colors: BTreeSet<usize>,
    color_adj_list: BTreeSet<(usize, usize)>,
}

impl<'a> Graph<'a> {
    pub fn new(
        options: LsmOptions,
        metadata: &'a [SstMetadata],
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
                if vertices[v].level == vertices.len() || vertices[v].level <= vertices[color].level {
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
            colors,
            color_adj_list: color_forward_adj_list,
        })
    }

    pub fn level_for_vertex(&self, idx: usize) -> usize {
        self.vertices[idx].level
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

impl<'a> Graph<'a> {
    pub fn compactions(&mut self) -> Vec<Compaction> {
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
                continue;
            }
            // Select the level with the best ratio of levels N-1 to level N.
            let mut upper_level = None;
            let mut prev_ratio = 0.0;
            for to_consider in self.vertices[color].level + 1..overlap.len() {
                if overlap[to_consider] == 0 {
                    break;
                }
                let mut acc = 0;
                for level in self.vertices[color].level..to_consider {
                    acc += acc + overlap[level];
                }
                let waste = overlap[to_consider];
                let ratio = acc as f64 / (acc + waste) as f64;
                let to_n_minus_one: u64 = overlap[self.vertices[color].level..to_consider].iter().sum();
                let to_n: u64 = overlap[self.vertices[color].level..to_consider + 1].iter().sum();
                if to_n >= self.options.max_compaction_bytes as u64 {
                    break;
                }
                if to_n == to_n_minus_one {
                    continue;
                }
                if prev_ratio < ratio {
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
                        upper_level: upper_level,
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
