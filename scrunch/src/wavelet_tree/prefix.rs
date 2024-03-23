//! A WaveletTree that works with prefix codes.

use buffertk::{Packable, Unpackable};
use prototk::{FieldNumber, Tag, WireType};

use crate::bit_vector::rrr::BitVector;
use crate::bit_vector::BitVector as BitVectorTrait;
use crate::builder::{parse_one_field_bytes, Builder, Helper};
use crate::encoder::Encoder;
use crate::Error;

use super::WaveletTree as WaveletTreeTrait;

///////////////////////////////////////////// constants ////////////////////////////////////////////

/// Each wavelet tree is wrapped in a CONTAINER_TAG field so that Unpackable can parse properly and
/// not overrun the buffer.
const CONTAINER_TAG: u32 = 1;

/// Each Node object is wrapped in a NODE_TAG field so that Unpackable can parse properly and not
/// overrun the buffer.
const NODE_TAG: u32 = 2;

///////////////////////////////////////////// internals ////////////////////////////////////////////

/// A capstone that reports the offset of the root.
#[derive(Clone, Debug, Default, prototk_derive::Message)]
struct Capstone {
    /// Offset of the root in the tree.
    #[prototk(1, fixed64)]
    root_offset: u64,
}

/// The root of the WaveletTree.
#[derive(Clone, Debug, Default, prototk_derive::Message)]
struct Root {
    /// Offset of the encoder in the tree.
    #[prototk(3, uint64)]
    encoder_start: u64,
    #[prototk(4, uint64)]
    encoder_limit: u64,
    /// Number of symbols in the wavelet tree.
    #[prototk(5, uint64)]
    length: u64,
    /// Offset of the root node of the tree.
    #[prototk(6, uint64)]
    tree: u64,
}

/// A Node in the WaveletTree.
#[derive(Clone, Debug, Default, prototk_derive::Message)]
struct Node {
    /// Number of symbols in this node.
    #[prototk(7, uint64)]
    length: u64,
    /// Start of this node's tree.
    #[prototk(8, uint64)]
    start: u64,
    /// One past the last byte of this node's tree.
    #[prototk(9, uint64)]
    limit: u64,
    /// Index of the left node of this tree.  0 if there is no left node.
    #[prototk(10, uint64)]
    left: u64,
    /// Index of the right node of this tree.  0 if there is no right node.
    #[prototk(11, uint64)]
    right: u64,
}

fn all_sized(mut iter: impl Iterator<Item = (u32, u8)>) -> bool {
    iter.all(|s| s.1 > 0)
}

fn all_zero(mut iter: impl Iterator<Item = (u32, u8)>) -> bool {
    iter.all(|s| s.1 == 0)
}

//////////////////////////////////////////// WaveletTree ///////////////////////////////////////////

pub struct WaveletTree<'a, E: Encoder> {
    encoder: E,
    tree: &'a [u8],
}

impl<'a, E: Encoder> WaveletTree<'a, E> {
    fn load_root(tree: &[u8]) -> Option<Root> {
        if tree.len() < 9 {
            return None;
        }
        let capstone = Capstone::unpack(&tree[tree.len() - 9..]).ok()?.0;
        if (tree.len() as u64) < capstone.root_offset {
            return None;
        }
        let root_offset: usize = capstone.root_offset.try_into().ok()?;
        Some(Root::unpack(&tree[root_offset..tree.len() - 9]).ok()?.0)
    }

    fn load_node(&self, offset: u64) -> Option<Node> {
        if offset >= self.tree.len() as u64 {
            return None;
        }
        let offset: usize = offset.try_into().ok()?;
        let (tag, value, _) = parse_one_field_bytes(&self.tree[offset..])?;
        if tag
            != (Tag {
                field_number: FieldNumber::must(NODE_TAG),
                wire_type: WireType::LengthDelimited,
            })
        {
            return None;
        }
        Some(Node::unpack(value).ok()?.0)
    }

    fn construct_from_iter<H: Helper>(
        builder: &mut Builder<H>,
        intermediate: &mut Vec<(u32, u8)>,
        iter: impl Iterator<Item = (u32, u8)> + Clone,
    ) -> Result<u64, Error> {
        if iter.clone().next().is_some() && all_sized(iter.clone()) {
            intermediate.clear();
            for x in iter {
                intermediate.push(x);
            }
            Self::construct_recursive(builder, intermediate)
        } else if all_zero(iter) {
            Ok(0)
        } else {
            Err(Error::LogicError(
                "wavelet tree should be all zero or all sized",
            ))
        }
    }

    fn construct_recursive<H: Helper>(
        builder: &mut Builder<H>,
        symbols: &[(u32, u8)],
    ) -> Result<u64, Error> {
        let (left, right) = if !all_zero(symbols.iter().copied()) {
            let lhs_iter = symbols
                .iter()
                .filter(|s| s.0 & 1 == 0)
                .map(|s| (s.0 >> 1, s.1 - 1));
            let rhs_iter = symbols
                .iter()
                .filter(|s| s.0 & 1 == 1)
                .map(|s| (s.0 >> 1, s.1 - 1));
            let mut intermediate = Vec::with_capacity(symbols.len());
            let left = Self::construct_from_iter(builder, &mut intermediate, lhs_iter)?;
            let right = Self::construct_from_iter(builder, &mut intermediate, rhs_iter)?;
            (left, right)
        } else {
            (0, 0)
        };
        let this: Vec<bool> = symbols.iter().map(|s| s.0 & 1 == 1).collect();
        let length: u64 = symbols.len() as u64;
        let start: u64 = builder.relative_len() as u64;
        BitVector::construct(&this, builder)?;
        let limit: u64 = builder.relative_len() as u64;
        let node = Node {
            length,
            start,
            limit,
            left,
            right,
        };
        builder.append_packable(FieldNumber::must(NODE_TAG), &node);
        Ok(limit)
    }

    fn recursive_access(&self, mut e: u32, mut sz: u8, node_offset: u64, x: usize) -> Option<u32> {
        if node_offset == 0 {
            self.encoder.decode(e, sz)
        } else {
            let node = self.load_node(node_offset)?;
            if node.start > node.limit || node.limit > self.tree.len() as u64 {
                return None;
            }
            let start: usize = node.start.try_into().ok()?;
            let limit: usize = node.limit.try_into().ok()?;
            let bv = BitVector::parse(&self.tree[start..limit]).ok()?.0;
            let bit = bv.access(x)?;
            let (x, node_offset) = if bit {
                e |= 1 << sz;
                (bv.rank(x)?, node.right)
            } else {
                (bv.rank0(x)?, node.left)
            };
            sz += 1;
            self.recursive_access(e, sz, node_offset, x)
        }
    }

    fn recursive_rank(&self, e: u32, sz: u8, node: Node, x: usize) -> Option<usize> {
        if sz == 0 {
            return None;
        }
        if node.start > node.limit || node.limit > self.tree.len() as u64 {
            return None;
        }
        let start: usize = node.start.try_into().ok()?;
        let limit: usize = node.limit.try_into().ok()?;
        let bv = BitVector::parse(&self.tree[start..limit]).ok()?.0;
        let (this_rank, next_node_offset) = if e & 1 != 0 {
            (bv.rank(x)?, node.right)
        } else {
            ((x - bv.rank(x)?), node.left)
        };
        if sz == 1 {
            Some(this_rank)
        } else if next_node_offset != 0 {
            let node = self.load_node(next_node_offset)?;
            self.recursive_rank(e >> 1, sz - 1, node, this_rank)
        } else {
            None
        }
    }

    fn recursive_select(&self, e: u32, sz: u8, node: Node, x: usize) -> Option<usize> {
        if sz == 0 {
            return None;
        }
        let x = if sz > 1 {
            let node_offset = if e & 1 != 0 { node.right } else { node.left };
            let inner = self.load_node(node_offset)?;
            self.recursive_select(e >> 1, sz - 1, inner, x)?
        } else {
            x
        };
        let start: usize = node.start.try_into().ok()?;
        let limit: usize = node.limit.try_into().ok()?;
        if start > limit || limit > self.tree.len() {
            return None;
        }
        let bv = BitVector::parse(&self.tree[start..limit]).ok()?.0;
        if e & 1 != 0 {
            bv.select(x)
        } else {
            bv.select0(x)
        }
    }
}

impl<'a, E: Encoder + Packable> WaveletTreeTrait for WaveletTree<'a, E> {
    fn construct<H: Helper>(symbols: &[u32], builder: &mut Builder<'_, H>) -> Result<(), Error> {
        let mut builder = builder.sub(FieldNumber::must(CONTAINER_TAG));
        // Construct an encoder.
        let enc = E::construct(symbols);
        let encoder_start = builder.relative_len() as u64;
        builder.append_raw_packable(&enc);
        let encoder_limit = builder.relative_len() as u64;
        // Translate the text.
        let mut encoded: Vec<(u32, u8)> = Vec::with_capacity(symbols.len());
        for sym in symbols.iter() {
            encoded.push(enc.encode(*sym).ok_or(Error::InvalidEncoder)?);
        }
        let length = encoded.len() as u64;
        drop(enc);
        // Recursively construct the tree.
        let tree = Self::construct_recursive(&mut builder, &encoded)?;
        // Append the root node.
        let root = Root {
            encoder_start,
            encoder_limit,
            length,
            tree,
        };
        let root_offset: u64 = builder.relative_len() as u64;
        builder.append_raw_packable(&root);
        // Capstone must come immediately after the root.
        let capstone = Capstone { root_offset };
        builder.append_raw_packable(&capstone);
        Ok(())
    }

    fn len(&self) -> usize {
        if let Some(root) = Self::load_root(self.tree) {
            root.length as usize
        } else {
            0
        }
    }

    fn access(&self, x: usize) -> Option<u32> {
        let root = Self::load_root(self.tree)?;
        self.recursive_access(0, 0, root.tree, x)
    }

    fn rank_q(&self, q: u32, x: usize) -> Option<usize> {
        let root = Self::load_root(self.tree)?;
        let node = self.load_node(root.tree)?;
        let (e, sz) = self.encoder.encode(q)?;
        self.recursive_rank(e, sz, node, x)
    }

    fn select_q(&self, q: u32, x: usize) -> Option<usize> {
        let root = Self::load_root(self.tree)?;
        let node = self.load_node(root.tree)?;
        let (e, sz) = self.encoder.encode(q)?;
        self.recursive_select(e, sz, node, x)
    }
}

impl<'a, E: Encoder + std::fmt::Debug> std::fmt::Debug for WaveletTree<'a, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.debug_struct("WaveletTree")
            .field("encoder", &self.encoder.symbols())
            .field("tree", &self.tree.len())
            .finish()
    }
}

impl<'a, E: Encoder + Unpackable<'a>> Unpackable<'a> for WaveletTree<'a, E> {
    type Error = Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Self::Error> {
        let (tag, value, remain) = parse_one_field_bytes(buf).ok_or(Error::InvalidWaveletTree)?;
        if tag
            != (Tag {
                field_number: FieldNumber::must(CONTAINER_TAG),
                wire_type: WireType::LengthDelimited,
            })
        {
            return Err(Error::InvalidWaveletTree);
        }
        let root = Self::load_root(value).ok_or(Error::InvalidWaveletTree)?;
        if root.encoder_start > root.encoder_limit || root.encoder_limit > value.len() as u64 {
            return Err(Error::InvalidWaveletTree);
        }
        let encoder_start: usize = root.encoder_start.try_into()?;
        let encoder_limit: usize = root.encoder_limit.try_into()?;
        let encoder = E::unpack(&value[encoder_start..encoder_limit])
            .map_err(|_| Error::InvalidEncoder)?
            .0;
        let tree = value;
        Ok((WaveletTree { encoder, tree }, remain))
    }
}
