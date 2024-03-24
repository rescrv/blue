use std::cmp::{Ordering, Reverse};
use std::collections::{BinaryHeap, HashMap};

use buffertk::{stack_pack, Packable, Unpackable};

use crate::Error;

////////////////////////////////////////////// Encoder /////////////////////////////////////////////

/// Encoder captures prefix-free codes from a text document.  It is necessary that codes be
/// prefix-free starting with the least significant bit.
pub trait Encoder {
    /// Construct a new encoder, returning Err if the text is not amenable to encoding (usually
    /// because there are too many symbols in the text).
    fn construct(text: &[u32]) -> Self;

    /// Encode symbol `t`, returning None if t is not a symbol of the text.
    fn encode(&self, t: u32) -> Option<(u32, u8)>;
    /// Decode the symbol `e` that is of size `s` bits.
    fn decode(&self, e: u32, s: u8) -> Option<u32>;

    /// The number of symbols in the encoder.
    fn symbols(&self) -> usize;
}

///////////////////////////////////////// FixedWidthEncoder ////////////////////////////////////////

/// FixedWidthEncoder maps the text to characters on [0, log(n)).
#[derive(Clone, Debug, Default, prototk_derive::Message)]
pub struct FixedWidthEncoder {
    #[prototk(1, uint32)]
    chars: Vec<u32>,
}

impl Encoder for FixedWidthEncoder {
    fn construct(text: &[u32]) -> Self {
        let mut chars = text.to_vec();
        chars.sort();
        chars.dedup();
        chars.shrink_to_fit();
        // SAFETY(rescrv): there are at most u32::MAX unique symbols in the input.
        assert!(chars.len() <= u32::MAX as usize);
        Self { chars }
    }

    fn encode(&self, t: u32) -> Option<(u32, u8)> {
        let position: u32 = self.chars.binary_search(&t).ok()?.try_into().ok()?;
        let bits = std::cmp::max(self.chars.len(), 2)
            .next_power_of_two()
            .ilog2()
            .try_into()
            .ok()?;
        Some((position, bits))
    }

    fn decode(&self, v: u32, _: u8) -> Option<u32> {
        let v: usize = v.try_into().ok()?;
        self.chars.get(v).copied()
    }

    fn symbols(&self) -> usize {
        self.chars.len()
    }
}

////////////////////////////////////////// HuffmanEncoder //////////////////////////////////////////

struct Node {
    prob: f64,
    sym: Option<u32>,
    lhs: Option<Box<Node>>,
    rhs: Option<Box<Node>>,
}

impl Node {
    fn append_symbols(&self, depth: u8, symbols: &mut Vec<(u8, u32)>) -> bool {
        if depth == u8::MAX {
            return false;
        }
        if let Some(sym) = self.sym.as_ref() {
            symbols.push((depth, *sym));
        }
        if let Some(lhs) = self.lhs.as_ref() {
            if !lhs.append_symbols(depth + 1, symbols) {
                return false;
            }
        }
        if let Some(rhs) = self.rhs.as_ref() {
            if !rhs.append_symbols(depth + 1, symbols) {
                return false;
            }
        }
        true
    }
}

impl Eq for Node {}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.prob.total_cmp(&other.prob).is_eq()
    }
}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        self.prob.total_cmp(&other.prob)
    }
}

impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Debug, Default, prototk_derive::Message)]
pub struct CodeBookEntry {
    #[prototk(1, uint32)]
    symbol: u32,
    #[prototk(2, uint32)]
    code: u32,
    #[prototk(3, uint32)]
    len: u32,
}

#[derive(Clone, Debug, Default, prototk_derive::Message)]
pub struct CodeBook {
    #[prototk(1, message)]
    code_book: Vec<CodeBookEntry>,
}

#[derive(Clone, Debug, Default)]
pub struct HuffmanEncoder {
    encode: HashMap<u32, (u32, u8)>,
    decode: HashMap<u32, u32>,
}

impl HuffmanEncoder {
    fn code_book(&self) -> CodeBook {
        let mut code_book = CodeBook::default();
        for (symbol, (code, len)) in self.encode.iter() {
            code_book.code_book.push(CodeBookEntry {
                symbol: *symbol,
                code: *code,
                len: *len as u32,
            });
        }
        code_book
    }
}

impl Encoder for HuffmanEncoder {
    fn construct(text: &[u32]) -> Self {
        if text.is_empty() {
            return Self {
                encode: HashMap::new(),
                decode: HashMap::new(),
            };
        }
        let mut probabilities = HashMap::new();
        for t in text.iter() {
            *probabilities.entry(*t).or_insert(0) += 1;
        }
        let mut heap = BinaryHeap::new();
        for (sym, prob) in probabilities.into_iter() {
            heap.push(Reverse(Node {
                prob: prob as f64,
                sym: Some(sym),
                lhs: None,
                rhs: None,
            }));
        }
        while heap.len() >= 2 {
            // SAFETY(rescrv): loop invariant means both pops will succeed.
            let lhs = heap.pop().unwrap().0;
            let rhs = heap.pop().unwrap().0;
            heap.push(Reverse(Node {
                prob: lhs.prob + rhs.prob,
                sym: None,
                lhs: Some(Box::new(lhs)),
                rhs: Some(Box::new(rhs)),
            }));
        }
        // SAFETY(rescrv): There's at least one symbol in the text, so there's at least one node.
        assert_eq!(1, heap.len());
        let tree = heap.pop().unwrap().0;
        let mut symbols = vec![];
        if let Some(sym) = tree.sym.as_ref() {
            symbols.push((1u8, *sym));
        } else {
            tree.append_symbols(0, &mut symbols);
        }
        symbols.sort();
        // Make the canonical code book.
        let mut encode = HashMap::new();
        let mut decode = HashMap::new();
        let mut code = 0u32;
        let mut prev_len = 1u8;
        for (len, sym) in symbols.into_iter() {
            code <<= len - prev_len;
            let flipped = code.reverse_bits() >> (32 - len);
            encode.insert(sym, (flipped, len));
            decode.insert(flipped, sym);
            code += 1;
            prev_len = len;
        }
        Self { encode, decode }
    }

    fn encode(&self, t: u32) -> Option<(u32, u8)> {
        self.encode.get(&t).copied()
    }

    fn decode(&self, v: u32, _: u8) -> Option<u32> {
        self.decode.get(&v).copied()
    }

    fn symbols(&self) -> usize {
        self.encode.len()
    }
}

impl Packable for HuffmanEncoder {
    fn pack_sz(&self) -> usize {
        let code_book = self.code_book();
        stack_pack(code_book).pack_sz()
    }

    fn pack(&self, buf: &mut [u8]) {
        let code_book = self.code_book();
        stack_pack(code_book).into_slice(buf);
    }
}

impl<'a> Unpackable<'a> for HuffmanEncoder {
    type Error = Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Self::Error> {
        let (code_book, buf) = CodeBook::unpack(buf).map_err(|_| Error::InvalidEncoder)?;
        let mut encode = HashMap::new();
        let mut decode = HashMap::new();
        for cbe in code_book.code_book.into_iter() {
            if encode.contains_key(&cbe.symbol) {
                return Err(Error::InvalidEncoder);
            }
            if cbe.len > u8::MAX as u32 {
                return Err(Error::InvalidEncoder);
            }
            let len = cbe.len as u8;
            if decode.contains_key(&cbe.code) {
                return Err(Error::InvalidEncoder);
            }
            encode.insert(cbe.symbol, (cbe.code, len));
            decode.insert(cbe.code, cbe.symbol);
        }
        let this = HuffmanEncoder { encode, decode };
        Ok((this, buf))
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::{Encoder, FixedWidthEncoder, HuffmanEncoder};

    #[test]
    fn fixed_width_empty() {
        let chars = vec![];
        let encoder = FixedWidthEncoder::construct(&chars);
        assert_eq!(None, encoder.encode(0u32));
    }

    #[test]
    fn fixed_width_0() {
        let chars = vec![0u32];
        let encoder = FixedWidthEncoder::construct(&chars);
        assert_eq!((0, 1), encoder.encode(0u32).unwrap());
        assert_eq!(None, encoder.encode(1u32));
    }

    #[test]
    fn fixed_width_0_1() {
        let chars = vec![0u32, 1u32];
        let encoder = FixedWidthEncoder::construct(&chars);
        assert_eq!((0, 1), encoder.encode(0u32).unwrap());
        assert_eq!((1, 1), encoder.encode(1u32).unwrap());
    }

    #[test]
    fn fixed_width_chars() {
        let chars: Vec<u32> = "AaBbCcDdEeFfNnBananaMississippi"
            .chars()
            .map(|c| c as u32)
            .collect();
        let encoder = FixedWidthEncoder::construct(&chars);
        assert_eq!((0, 5), encoder.encode('A' as u32).unwrap());
        assert_eq!((1, 5), encoder.encode('B' as u32).unwrap());
        assert_eq!((2, 5), encoder.encode('C' as u32).unwrap());
        assert_eq!((8, 5), encoder.encode('a' as u32).unwrap());
        assert_eq!((9, 5), encoder.encode('b' as u32).unwrap());
        assert_eq!((10, 5), encoder.encode('c' as u32).unwrap());
        for c in chars.iter().copied() {
            let (v, s) = encoder.encode(c).unwrap();
            assert_eq!(Some(c), encoder.decode(v, s));
        }
        assert_eq!(None, encoder.encode('q' as u32));
        assert_eq!(None, encoder.encode('z' as u32));
    }

    #[test]
    fn huffman_chars() {
        let chars: Vec<u32> = "BananaMississippi".chars().map(|c| c as u32).collect();
        let encoder = HuffmanEncoder::construct(&chars);
        assert_eq!((0, 2), encoder.encode('i' as u32).unwrap());
        assert_eq!((2, 2), encoder.encode('s' as u32).unwrap());
        assert_eq!((1, 3), encoder.encode('a' as u32).unwrap());
        assert_eq!((5, 3), encoder.encode('n' as u32).unwrap());
        assert_eq!((3, 3), encoder.encode('p' as u32).unwrap());
        assert_eq!((7, 4), encoder.encode('B' as u32).unwrap());
        assert_eq!((15, 4), encoder.encode('M' as u32).unwrap());
        for c in chars.iter().copied() {
            let (v, s) = encoder.encode(c).unwrap();
            assert_eq!(Some(c), encoder.decode(v, s));
        }
        assert_eq!(None, encoder.encode('q' as u32));
        assert_eq!(None, encoder.encode('z' as u32));
    }
}
