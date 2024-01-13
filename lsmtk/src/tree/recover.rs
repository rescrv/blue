use std::cmp::min;
use std::collections::BTreeSet;
use std::fmt::Debug;
use std::ops::{Bound, Deref, DerefMut};

use sst::SstMetadata;

use zerror::Z;

use zerror_core::ErrorCore;

use super::*;

///////////////////////////////////////// key_range_overlap ////////////////////////////////////////

fn key_range_overlap(lhs: &SstMetadata, rhs: &SstMetadata) -> bool {
    compare_bytes(&lhs.first_key, &rhs.last_key) != Ordering::Greater
        && compare_bytes(&rhs.first_key, &lhs.last_key) != Ordering::Greater
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

////////////////////////////////////////////// recover /////////////////////////////////////////////

pub fn recover(options: LsmtkOptions, metadata: Vec<SstMetadata>) -> Result<Version, Error> {
    // Create a forward adjacency list.
    let forward_adj_list = construct_adj_list(&metadata)?;
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
    // Adjust levels downward so that max_level == NUM_LEVELS.
    let max_level = vertices
        .iter()
        .map(|x| x.level)
        .max()
        .unwrap_or(NUM_LEVELS - 1);
    if max_level >= NUM_LEVELS {
        let delta = max_level - NUM_LEVELS + 1;
        for v in vertices.iter_mut() {
            if v.level < delta {
                v.level = 0;
            } else {
                v.level -= delta;
            }
        }
    }
    // Create the tree from the provided levels.
    let mut levels = vec![Level::default(); NUM_LEVELS];
    for (v, m) in std::iter::zip(vertices.into_iter(), metadata.into_iter()) {
        levels[v.level].ssts.push(Arc::new(m));
    }
    levels[0]
        .ssts
        .sort_by(|lhs, rhs| lhs.smallest_timestamp.cmp(&rhs.smallest_timestamp));
    for level in levels[1..].iter_mut() {
        // NOTE(rescrv):  This is a little sloppy.
        // It assumes the graph algorithm is correct, so comparison by smallest key is sufficient.
        level.ssts.sort_by(|lhs, rhs| {
            keyvalint::compare_key(
                &lhs.first_key,
                lhs.smallest_timestamp,
                &rhs.first_key,
                rhs.smallest_timestamp,
            )
        });
    }
    // Return a new tree.
    let ongoing = Arc::new(Mutex::default());
    Ok(Version {
        options,
        levels,
        ongoing,
    })
}

fn construct_adj_list(metadata: &[SstMetadata]) -> Result<AdjacencyList, Error> {
    // Create the adjacency lists.
    let mut forward_adj_list = AdjacencyList::default();
    for i in 0..metadata.len() {
        if metadata[i].smallest_timestamp > metadata[i].biggest_timestamp {
            let err = Error::Corruption {
                core: ErrorCore::default(),
                context: "metadata timestamps not in order".to_string(),
            }
            .with_variable("SST", metadata[i].setsum)
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
            } else if metadata[j].biggest_timestamp < metadata[i].smallest_timestamp {
                forward_adj_list.insert((i, j));
            } else {
                forward_adj_list.insert((i, j));
                forward_adj_list.insert((j, i));
            }
        }
    }
    Ok(forward_adj_list)
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
