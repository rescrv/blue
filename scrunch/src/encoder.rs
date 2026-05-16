use std::cmp::{Ordering, Reverse};
use std::collections::{BinaryHeap, HashMap};

use buffertk::{Packable, Unpackable, stack_pack};

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
        if let Some(lhs) = self.lhs.as_ref()
            && !lhs.append_symbols(depth + 1, symbols)
        {
            return false;
        }
        if let Some(rhs) = self.rhs.as_ref()
            && !rhs.append_symbols(depth + 1, symbols)
        {
            return false;
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
    code_book: Vec<CodeBookEntry>,
    encode_dense: Option<Vec<(u32, u8)>>,
    decode: Vec<(u32, u32)>,
}

impl HuffmanEncoder {
    fn dense_frequencies(text: &[u32]) -> Option<Vec<(u32, u64)>> {
        let max_symbol = text.iter().copied().max()? as usize;
        if max_symbol > (1 << 20) || max_symbol > text.len().saturating_mul(4) {
            return None;
        }
        let mut counts = vec![0u64; max_symbol + 1];
        for &t in text {
            counts[t as usize] += 1;
        }
        let mut frequencies = Vec::new();
        for (symbol, count) in counts.into_iter().enumerate() {
            if count > 0 {
                frequencies.push((symbol as u32, count));
            }
        }
        Some(frequencies)
    }

    fn sparse_frequencies(text: &[u32]) -> Vec<(u32, u64)> {
        let mut probabilities = HashMap::new();
        for &t in text {
            *probabilities.entry(t).or_insert(0u64) += 1;
        }
        let mut probabilities: Vec<(u32, u64)> = probabilities.into_iter().collect();
        probabilities.sort_unstable_by_key(|(symbol, _)| *symbol);
        probabilities
    }

    fn build_code_book(symbols: Vec<(u8, u32)>) -> Vec<CodeBookEntry> {
        let mut code_book = Vec::with_capacity(symbols.len());
        let mut code = 0u32;
        let mut prev_len = 1u8;
        for (len, sym) in symbols {
            code <<= len - prev_len;
            let flipped = code.reverse_bits() >> (32 - len);
            code_book.push(CodeBookEntry {
                symbol: sym,
                code: flipped,
                len: len as u32,
            });
            code += 1;
            prev_len = len;
        }
        code_book.sort_unstable_by_key(|entry| entry.symbol);
        code_book
    }

    fn build_dense_encode(code_book: &[CodeBookEntry]) -> Option<Vec<(u32, u8)>> {
        let max_symbol = code_book.last()?.symbol as usize;
        if max_symbol > code_book.len().saturating_mul(64).max(1024) {
            return None;
        }
        let mut encode_dense = vec![(0u32, 0u8); max_symbol + 1];
        for entry in code_book {
            encode_dense[entry.symbol as usize] = (entry.code, entry.len as u8);
        }
        Some(encode_dense)
    }

    fn from_code_book(mut code_book: Vec<CodeBookEntry>) -> Self {
        code_book.sort_unstable_by_key(|entry| entry.symbol);
        let encode_dense = Self::build_dense_encode(&code_book);
        let mut decode: Vec<(u32, u32)> = code_book
            .iter()
            .map(|entry| (entry.code, entry.symbol))
            .collect();
        decode.sort_unstable_by_key(|(code, _)| *code);
        Self {
            code_book,
            encode_dense,
            decode,
        }
    }
}

impl Encoder for HuffmanEncoder {
    fn construct(text: &[u32]) -> Self {
        if text.is_empty() {
            return Self::default();
        }
        let probabilities =
            Self::dense_frequencies(text).unwrap_or_else(|| Self::sparse_frequencies(text));
        let mut heap = BinaryHeap::new();
        for (sym, prob) in probabilities {
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
        let code_book = Self::build_code_book(symbols);
        Self::from_code_book(code_book)
    }

    fn encode(&self, t: u32) -> Option<(u32, u8)> {
        if let Some(encode_dense) = self.encode_dense.as_ref() {
            let &(code, len) = encode_dense.get(t as usize)?;
            if len > 0 {
                return Some((code, len));
            }
        }
        let idx = self
            .code_book
            .binary_search_by_key(&t, |entry| entry.symbol)
            .ok()?;
        let entry = &self.code_book[idx];
        Some((entry.code, entry.len as u8))
    }

    fn decode(&self, v: u32, _: u8) -> Option<u32> {
        let idx = self
            .decode
            .binary_search_by_key(&v, |(code, _)| *code)
            .ok()?;
        Some(self.decode[idx].1)
    }

    fn symbols(&self) -> usize {
        self.code_book.len()
    }
}

impl Packable for HuffmanEncoder {
    fn pack_sz(&self) -> usize {
        let code_book = CodeBook {
            code_book: self.code_book.clone(),
        };
        stack_pack(code_book).pack_sz()
    }

    fn pack(&self, buf: &mut [u8]) {
        let code_book = CodeBook {
            code_book: self.code_book.clone(),
        };
        stack_pack(code_book).into_slice(buf);
    }
}

impl<'a> Unpackable<'a> for HuffmanEncoder {
    type Error = Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Self::Error> {
        let (code_book, buf) = CodeBook::unpack(buf).map_err(|_| Error::InvalidEncoder)?;
        let mut entries = code_book.code_book;
        entries.sort_unstable_by_key(|entry| entry.symbol);
        let mut prev_symbol = None;
        let mut decode: Vec<(u32, u32)> = Vec::with_capacity(entries.len());
        for cbe in entries.iter() {
            if prev_symbol == Some(cbe.symbol) {
                return Err(Error::InvalidEncoder);
            }
            prev_symbol = Some(cbe.symbol);
            if cbe.len == 0 || cbe.len > u8::MAX as u32 {
                return Err(Error::InvalidEncoder);
            }
            decode.push((cbe.code, cbe.symbol));
        }
        decode.sort_unstable_by_key(|(code, _)| *code);
        for pair in decode.windows(2) {
            if pair[0].0 == pair[1].0 {
                return Err(Error::InvalidEncoder);
            }
        }
        let this = Self::from_code_book(entries);
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
