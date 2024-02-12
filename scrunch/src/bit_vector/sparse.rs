use std::iter::zip;

use buffertk::{stack_pack, v64, Unpackable};

use crate::bit_array::BitArray;
use crate::bit_vector::BitVector as BitVectorTrait;
use crate::builder::{Builder, Helper};
use crate::Error;

///////////////////////////////////////////// Internals ////////////////////////////////////////////

#[derive(Debug, Default)]
struct Root {
    node: u64,
    levels: u8,
}

#[derive(Debug)]
struct Leaf<'a> {
    branch: usize,
    base: u64,
    bits: u8,
    words: BitArray<'a>,
}

impl<'a> Leaf<'a> {
    fn access_rank(&self, x: usize) -> Option<(bool, usize)> {
        let mut word = self.base;
        if word >= x as u64 {
            return Some((word == x as u64, 0));
        }
        for idx in 0..self.branch - 1 {
            let delta = self
                .words
                .load(idx * self.bits as usize, self.bits as usize)?;
            word += delta;
            if word >= x as u64 {
                return Some((word == x as u64, idx + 1));
            } else if delta == 0 {
                return Some((false, idx + 1));
            }
        }
        Some((false, self.branch))
    }

    fn select(&self, index: usize) -> Option<usize> {
        let mut word = self.base;
        for idx in 0..index {
            let delta = self
                .words
                .load(idx * self.bits as usize, self.bits as usize)?;
            if delta == 0 {
                return None;
            }
            word += delta;
        }
        (word + 1).try_into().ok()
    }
}

#[derive(Debug)]
struct Internal<'a> {
    branch: usize,
    divider_base: u64,
    pointer_base: u64,
    divider_bits: u8,
    pointer_bits: u8,
    dividers: BitArray<'a>,
    pointers: BitArray<'a>,
}

impl<'a> Internal<'a> {
    fn position(&self, x: usize) -> Option<(usize, u64)> {
        let mut pointer = self.pointer_base;
        let mut divider = self.divider_base;
        if divider >= x as u64 {
            return Some((0, pointer));
        }
        for i in 0..self.branch - 2 {
            let divider_delta = self
                .dividers
                .load(i * self.divider_bits as usize, self.divider_bits as usize)?;
            let pointer_delta = self
                .pointers
                .load(i * self.pointer_bits as usize, self.pointer_bits as usize)?;
            divider += divider_delta;
            pointer += pointer_delta;
            if divider >= x as u64 || divider_delta == 0 {
                return Some((i + 1, pointer));
            }
        }
        pointer += self.pointers.load(
            (self.branch - 2) * self.pointer_bits as usize,
            self.pointer_bits as usize,
        )?;
        Some((self.branch - 1, pointer))
    }

    fn pointer(&self, index: usize) -> Option<u64> {
        let mut pointer = self.pointer_base;
        for i in 0..index {
            let pointer_delta = self
                .pointers
                .load(i * self.pointer_bits as usize, self.pointer_bits as usize)?;
            if pointer_delta == 0 {
                return None;
            }
            pointer += pointer_delta;
        }
        Some(pointer)
    }
}

fn trim_prefix(bytes: &[u8], offset: usize) -> Option<&[u8]> {
    if bytes.len() >= offset {
        Some(&bytes[offset..])
    } else {
        None
    }
}

fn trim_to_length(bytes: &[u8], offset: usize) -> Option<&[u8]> {
    if bytes.len() >= offset {
        Some(&bytes[..offset])
    } else {
        None
    }
}

fn push_slice_u64(bytes: &mut Vec<u8>, branch: usize, values: &[u64]) {
    fn bits_required(values: &[u64]) -> u8 {
        let mut max = 2;
        for (lhs, rhs) in zip(values[..values.len() - 1].iter(), values[1..].iter()) {
            assert!(rhs > lhs);
            max = std::cmp::max(max, rhs - lhs + 1);
        }
        let max = max.next_power_of_two();
        let bits = max.ilog2() as u8;
        bits
    }
    if values.is_empty() {
        stack_pack(v64::from(u64::MAX)).append_to_vec(bytes);
        bytes.push(0);
        return;
    }
    let mut prev = values[0];
    stack_pack(v64::from(prev)).append_to_vec(bytes);
    let bits = bits_required(values);
    bytes.push(bits);
    let mut scratch = 0u64;
    let mut scratch_sz = 0u8;
    for value in values[1..].iter() {
        push_bits(bytes, *value - prev, bits, &mut scratch, &mut scratch_sz);
        prev = *value;
    }
    for _ in values.len()..branch {
        push_bits(bytes, 0, bits, &mut scratch, &mut scratch_sz);
    }
    while scratch_sz > 0 {
        let amt = std::cmp::min(scratch_sz, 8);
        bytes.push(scratch as u8);
        scratch >>= amt;
        scratch_sz -= amt;
    }
}

fn parse_slice_u64(branch: usize, bytes: &[u8]) -> Option<(u64, u8, BitArray<'_>, &[u8])> {
    let (base, bytes) = v64::unpack(bytes).ok()?;
    let base: u64 = base.into();
    let (bits, bytes) = u8::unpack(bytes).ok()?;
    let values_bytes = ((branch - 1) * bits as usize + 7) / 8;
    let values = trim_to_length(bytes, values_bytes)?;
    let values = BitArray::new(values);
    let bytes = trim_prefix(bytes, values_bytes)?;
    Some((base, bits, values, bytes))
}

fn push_bits(bytes: &mut Vec<u8>, value: u64, delta: u8, scratch: &mut u64, scratch_sz: &mut u8) {
    assert!(value < (1u64 << delta));
    *scratch |= value << (*scratch_sz);
    *scratch_sz += delta;
    while *scratch_sz >= 8 {
        bytes.push(*scratch as u8);
        *scratch >>= 8;
        *scratch_sz -= 8;
    }
}

///////////////////////////////////////////// BitVector ////////////////////////////////////////////

pub struct BitVector<'a> {
    length: usize,
    branch: usize,
    bytes: &'a [u8],
    root: Root,
    skip_factors: Vec<usize>
}

impl<'a> BitVector<'a> {
    pub fn new(bytes: &'a [u8]) -> Option<Self> {
        if bytes.is_empty() {
            None
        } else {
            let orig_bytes = bytes;
            let (length, bytes) = u64::unpack(bytes).ok()?;
            let length: usize = length.try_into().ok()?;
            let (branch, _) = u8::unpack(bytes).ok()?;
            let branch = branch as usize;
            let bytes = orig_bytes;
            let root = Root::default();
            let skip_factors = vec![];
            let mut this = Self {
                length,
                branch,
                bytes,
                root,
                skip_factors,
            };
            this.root = this.load_root()?;
            if this.root.levels > 0 {
                this.skip_factors = vec![this.branch.pow(this.root.levels as u32 - 1)];
                while this.skip_factors[this.skip_factors.len() - 1] > 1 {
                    let next = this.skip_factors[this.skip_factors.len() - 1] / this.branch;
                    this.skip_factors.push(next);
                }
                this.skip_factors.pop();
            }
            Some(this)
        }
    }

    pub fn from_indices<H: Helper>(
        branch: usize,
        len: usize,
        indices: &[usize],
        builder: &mut Builder<'_, H>,
    ) -> Option<()> {
        if !(4..256).contains(&branch) {
            return None;
        }
        if !indices.is_empty() {
            if len < indices[indices.len() - 1] {
                return None;
            }
            for (lhs, rhs) in zip(indices[..indices.len() - 1].iter(), indices[1..].iter()) {
                if lhs >= rhs {
                    return None;
                }
            }
        }
        let mut bytes = vec![];
        stack_pack(len as u64).append_to_vec(&mut bytes);
        bytes.push(branch as u8);
        if indices.is_empty() {
            push_slice_u64(&mut bytes, branch, &[]);
            // TODO(rescrv): Make this not copy.
            builder.append_raw(&bytes);
            return Some(());
        }
        let mut leaves = Vec::with_capacity(branch);
        let mut dividers = vec![];
        let mut pointers = vec![];
        for index in indices.iter() {
            leaves.push(*index as u64);
            if leaves.len() >= branch {
                dividers.push(leaves[leaves.len() - 1]);
                pointers.push(bytes.len() as u64);
                push_slice_u64(&mut bytes, branch, &leaves);
                leaves.clear();
            }
        }
        if !leaves.is_empty() {
            dividers.push(leaves[leaves.len() - 1]);
            pointers.push(bytes.len() as u64);
            push_slice_u64(&mut bytes, branch, &leaves);
        }
        assert_eq!(dividers.len(), pointers.len());
        let mut levels = 1u8;
        while pointers.len() > 1 {
            let mut new_dividers = vec![];
            let mut new_pointers = vec![];
            let mut idx = 0;
            while idx + branch < pointers.len() {
                new_dividers.push(dividers[idx + branch - 1]);
                new_pointers.push(bytes.len() as u64);
                push_slice_u64(&mut bytes, branch - 1, &dividers[idx..idx + branch - 1]);
                push_slice_u64(&mut bytes, branch, &pointers[idx..idx + branch]);
                idx += branch;
            }
            let amt = pointers.len() - idx;
            if amt > 0 {
                new_pointers.push(bytes.len() as u64);
                push_slice_u64(&mut bytes, branch - 1, &dividers[idx..idx + amt - 1]);
                push_slice_u64(&mut bytes, branch, &pointers[idx..idx + amt]);
            }
            dividers = new_dividers;
            pointers = new_pointers;
            levels += 1;
        }
        assert_eq!(1, pointers.len());
        stack_pack(pointers[0]).append_to_vec(&mut bytes);
        bytes.push(levels);
        // TODO(rescrv): Make this not copy.
        builder.append_raw(&bytes);
        Some(())
    }

    fn load_root(&self) -> Option<Root> {
        if self.bytes.len() < 9 {
            return None;
        }
        let bytes = &self.bytes[self.bytes.len() - 9..];
        let (node, bytes) = u64::unpack(bytes).ok()?;
        let (levels, _) = u8::unpack(bytes).ok()?;
        Some(Root { node, levels })
    }

    fn load_leaf(&self, offset: usize) -> Option<Leaf<'_>> {
        let bytes = trim_prefix(self.bytes, offset)?;
        let branch = self.branch;
        let (base, bits, words, _) = parse_slice_u64(branch, bytes)?;
        Some(Leaf {
            branch,
            base,
            bits,
            words,
        })
    }

    fn load_internal(&self, offset: usize) -> Option<Internal<'_>> {
        let bytes = trim_prefix(self.bytes, offset)?;
        let branch = self.branch;
        let (divider_base, divider_bits, dividers, bytes) = parse_slice_u64(branch - 1, bytes)?;
        let (pointer_base, pointer_bits, pointers, _) = parse_slice_u64(branch, bytes)?;
        Some(Internal {
            branch,
            divider_base,
            pointer_base,
            divider_bits,
            pointer_bits,
            dividers,
            pointers,
        })
    }

    fn access_rank(&self, x: usize) -> Option<(bool, usize)> {
        if x > self.len() {
            return None;
        }
        if self.root.levels == 0 {
            if x <= self.len() {
                Some((false, 0))
            } else {
                None
            }
        } else {
            let mut node_offset = self.root.node;
            let mut cumulative_rank = 0;
            for skip_factor in self.skip_factors.iter() {
                let node = self.load_internal(node_offset as usize)?;
                let (offset, pointer) = node.position(x)?;
                node_offset = pointer;
                cumulative_rank += offset * *skip_factor;
            }
            let leaf = self.load_leaf(node_offset as usize)?;
            let (a, r) = leaf.access_rank(x)?;
            Some((a, cumulative_rank + r))
        }
    }
}

impl<'a> BitVectorTrait for BitVector<'a> {
    type Output<'b> = BitVector<'b>;

    fn construct<H: Helper>(
        bits: &[bool],
        builder: &mut Builder<'_, H>,
    ) -> Result<(), Error> {
        let mut indices = vec![];
        for (idx, bit) in bits.iter().enumerate() {
            if *bit {
                indices.push(idx);
            }
        }
        // SAFETY(rescrv):  We uphold all the guarantees necessary for from_indices.
        let indices: &[usize] = &indices;
        Self::from_indices(16, bits.len(), indices, builder).ok_or(Error::InvalidBitVector)
    }

    fn parse<'b, 'c: 'b>(buf: &'c [u8]) -> Result<(Self::Output<'b>, &'c [u8]), Error> {
        if let Some(bv) = BitVector::new(buf) {
            Ok((bv, &[]))
        } else {
            Err(Error::InvalidBitVector)
        }
    }

    fn len(&self) -> usize {
        self.length
    }

    fn access(&self, x: usize) -> Option<bool> {
        if x >= self.len() {
            return None;
        }
        let (access, _) = self.access_rank(x)?;
        Some(access)
    }

    fn rank(&self, x: usize) -> Option<usize> {
        if x > self.len() {
            return None;
        }
        let (_, rank) = self.access_rank(x)?;
        Some(rank)
    }

    fn select(&self, mut x: usize) -> Option<usize> {
        if x == 0 {
            return Some(0);
        }
        if self.root.levels == 0 {
            None
        } else {
            x -= 1;
            let mut node_offset = self.root.node;
            for skip_factor in self.skip_factors.iter() {
                let node = self.load_internal(node_offset as usize)?;
                let mut index = 0;
                while x >= *skip_factor {
                    index += 1;
                    x -= *skip_factor;
                }
                node_offset = node.pointer(index)?;
            }
            let leaf = self.load_leaf(node_offset as usize)?;
            leaf.select(x)
        }
    }
}
