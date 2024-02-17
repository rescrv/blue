use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::num::TryFromIntError;

use buffertk::Unpackable;
use prototk::FieldNumber;

pub mod binary_search;
pub mod bit_array;
pub mod bit_vector;
pub mod builder;
pub mod encoder;
pub mod isa;
pub mod psi;
pub mod sa;
pub mod sais;
pub mod sampled;
pub mod sigma;
pub mod wavelet_tree;

use crate::bit_vector::BitVector;
use crate::builder::{Builder, Helper};

/////////////////////////////////////////////// Error //////////////////////////////////////////////

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    IntoUsize,
    Unparseable,
    TextTooLong,
    BadSearch,
    BadTextOffset,
    BadRecordOffset,
    CouldNotConstructBitVector,
    InvalidEncoder,
    InvalidBitVector,
    InvalidWaveletTree,
    InvalidSigma,
    InvalidSuffixArray,
    InvalidInverseSuffixArray,
    InvalidPsi,
    InvalidDocument,
    BadRank(usize),
    BadSelect(usize),
    BadIndex(usize),
    LogicError(&'static str),
}

impl From<TryFromIntError> for Error {
    fn from(_: TryFromIntError) -> Self {
        Self::IntoUsize
    }
}

////////////////////////////////////////////// Offset //////////////////////////////////////////////

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct TextOffset(pub usize);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct RecordOffset(pub usize);

///////////////////////////////////////////// Document /////////////////////////////////////////////

pub trait Document {
    type Search: Iterator<Item = TextOffset>;

    fn construct<H: Helper>(
        text: Vec<u32>,
        record_boundaries: Vec<usize>,
        builder: &mut Builder<H>,
    ) -> Result<(), Error>;

    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn records(&self) -> usize;

    fn search(&self, needle: &[u32]) -> Result<Self::Search, Error>;
    fn count(&self, needle: &[u32]) -> Result<usize, Error>;

    fn lookup(&self, offset: TextOffset) -> Result<RecordOffset, Error>;
    fn retrieve(&self, record: RecordOffset) -> Result<Vec<u32>, Error>;
    fn offset_of(&self, record: RecordOffset) -> Result<TextOffset, Error>;
}

/////////////////////////////////////// ReferenceDocumentStub //////////////////////////////////////

#[derive(Clone, Debug, Default, prototk_derive::Message)]
struct ReferenceDocumentStub {
    #[prototk(1, uint32)]
    text: Vec<u32>,
    #[prototk(2, uint64)]
    record_boundaries: Vec<u64>,
}

impl From<ReferenceDocument> for ReferenceDocumentStub {
    fn from(rd: ReferenceDocument) -> Self {
        let ReferenceDocument {
            text,
            record_boundaries,
        } = rd;
        let record_boundaries = record_boundaries.into_iter().map(|x| x as u64).collect();
        Self {
            text,
            record_boundaries,
        }
    }
}

impl TryFrom<ReferenceDocumentStub> for ReferenceDocument {
    type Error = Error;

    fn try_from(rds: ReferenceDocumentStub) -> Result<Self, Self::Error> {
        let ReferenceDocumentStub {
            text,
            record_boundaries,
        } = rds;
        if record_boundaries.iter().any(|x| *x > usize::MAX as u64) {
            return Err(Error::IntoUsize);
        }
        let record_boundaries = record_boundaries.into_iter().map(|x| x as usize).collect();
        Ok(ReferenceDocument {
            text,
            record_boundaries,
        })
    }
}

///////////////////////////////////////// ReferenceDocument ////////////////////////////////////////

pub struct ReferenceDocument {
    text: Vec<u32>,
    record_boundaries: Vec<usize>,
}

impl Document for ReferenceDocument {
    type Search = std::vec::IntoIter<TextOffset>;

    fn construct<H: Helper>(
        text: Vec<u32>,
        record_boundaries: Vec<usize>,
        builder: &mut Builder<H>,
    ) -> Result<(), Error> {
        check_record_boundaries(&text, &record_boundaries)?;
        builder.append_vec_u32(FieldNumber::must(1), &text);
        builder.append_vec_usize(FieldNumber::must(2), &record_boundaries);
        Ok(())
    }

    fn len(&self) -> usize {
        self.text.len()
    }

    fn records(&self) -> usize {
        self.record_boundaries.len()
    }

    fn search(&self, needle: &[u32]) -> Result<Self::Search, Error> {
        let offsets: Vec<TextOffset> = if needle.is_empty() {
            (0..self.len()).map(TextOffset).collect()
        } else {
            let mut offsets = vec![];
            for (idx, candidate) in self.text.windows(needle.len()).enumerate() {
                if candidate == needle {
                    offsets.push(TextOffset(idx));
                }
            }
            offsets
        };
        Ok(offsets.into_iter())
    }

    fn count(&self, needle: &[u32]) -> Result<usize, Error> {
        Ok(self.search(needle)?.count())
    }

    fn lookup(&self, offset: TextOffset) -> Result<RecordOffset, Error> {
        let partition_point = self.record_boundaries.partition_point(|b| *b < offset.0);
        if partition_point >= self.record_boundaries.len() && !self.record_boundaries.is_empty() {
            Ok(RecordOffset(self.record_boundaries.len() - 1))
        } else if partition_point >= self.record_boundaries.len() {
            Err(Error::BadTextOffset)
        } else if self.record_boundaries[partition_point] <= offset.0 {
            Ok(RecordOffset(partition_point))
        } else {
            Ok(RecordOffset(partition_point - 1))
        }
    }

    fn retrieve(&self, record: RecordOffset) -> Result<Vec<u32>, Error> {
        if record.0 >= self.record_boundaries.len() {
            return Err(Error::BadRecordOffset);
        }
        let record_start = self.record_boundaries[record.0];
        let record_limit = if record.0 + 1 >= self.record_boundaries.len() {
            self.text.len()
        } else {
            self.record_boundaries[record.0 + 1]
        };
        Ok(Vec::from(&self.text[record_start..record_limit]))
    }

    fn offset_of(&self, record: RecordOffset) -> Result<TextOffset, Error> {
        if record.0 >= self.record_boundaries.len() {
            return Err(Error::BadRecordOffset);
        }
        Ok(TextOffset(self.record_boundaries[record.0]))
    }
}

impl<'a> Unpackable<'a> for ReferenceDocument {
    type Error = Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Self::Error> {
        let (rds, buf) = <ReferenceDocumentStub as Unpackable>::unpack(buf)
            .map_err(|_| Error::InvalidDocument)?;
        Ok((rds.try_into()?, buf))
    }
}

////////////////////////////////////////// PsiDocumentStub /////////////////////////////////////////

#[derive(Clone, Debug, Default, prototk_derive::Message)]
struct PsiDocumentStub<'a> {
    #[prototk(1, bytes)]
    record_boundaries: &'a [u8],
    #[prototk(2, bytes)]
    sigma: &'a [u8],
    #[prototk(3, bytes)]
    sa: &'a [u8],
    #[prototk(4, bytes)]
    isa: &'a [u8],
    #[prototk(5, bytes)]
    psi: &'a [u8],
}

//////////////////////////////////////////// PsiDocument ///////////////////////////////////////////

pub struct PsiDocument<'a, SA, ISA, PSI>
where
    SA: sa::SuffixArray,
    ISA: isa::InverseSuffixArray,
    PSI: psi::Psi,
{
    // Record boundaries, where select(i) returns the offset of the i'th record in text.
    record_boundaries: bit_vector::sparse::BitVector<'a>,

    // Translate indices to characters.
    sigma: sigma::Sigma<'a>,

    // Sorted array of all suffixes of the original text.
    //
    // Each entry sa[i] in the suffix array indicates the offset in the original text at which the
    // i'th prefix begins.  Compactly: i<j => text[sa[i]..] < text[sa[j]..].
    sa: SA,

    // Inverse of `sa`.  This maps a location in the text to its location in the suffix array.
    isa: ISA,

    // Psi is the successor function applied to the suffix array.
    // Using psi, you can traverse the suffix array because sa, isa, psi are all related:
    //
    // psi[idx] = isa[sa[idx] + 1]
    //
    // See [DGA] for a more thorough understanding.
    psi: PSI,
}

impl<'a, SA, ISA, PSI> PsiDocument<'a, SA, ISA, PSI>
where
    SA: sa::SuffixArray,
    ISA: isa::InverseSuffixArray,
    PSI: psi::Psi,
{
    fn backwards_search(&self, needle: &[u32]) -> Result<(usize, usize), Error> {
        let mut needle = needle.iter().rev();
        let mut range = if let Some(t) = needle.next() {
            self.sigma.sa_range_for(*t)?
        } else {
            // If there's no needle, we should return everything except the artificial end marker.
            return Ok((1usize, self.psi.len() - 1));
        };
        // This performs backwards search.
        for t in needle {
            range = self
                .psi
                .constrain(&self.sigma, self.sigma.sa_range_for(*t)?, range)?;
        }
        Ok(range)
    }
}

impl<'a, SA, ISA, PSI> Document for PsiDocument<'a, SA, ISA, PSI>
where
    SA: sa::SuffixArray,
    ISA: isa::InverseSuffixArray,
    PSI: psi::Psi,
{
    type Search = std::vec::IntoIter<TextOffset>;

    fn construct<H: Helper>(
        text: Vec<u32>,
        record_boundaries: Vec<usize>,
        builder: &mut Builder<H>,
    ) -> Result<(), Error> {
        check_record_boundaries(&text, &record_boundaries)?;
        // compute the record boundaries
        let sparse_boundaries: Vec<usize> = record_boundaries[1..].iter().map(|rb| rb - 1).collect();
        bit_vector::sparse::BitVector::from_indices(
            64,
            text.len(),
            &sparse_boundaries,
            &mut builder.sub(FieldNumber::must(1)),
        )
        .ok_or(Error::InvalidBitVector)?;
        // compute sigma
        let mut sigma_builder = builder.sub(FieldNumber::must(2));
        sigma::Sigma::construct(text.iter().copied(), &mut sigma_builder)?;
        let sigma_buf = sigma_builder.relative_bytes(0).to_vec();
        let sigma = sigma::Sigma::unpack(&sigma_buf)?.0;
        drop(sigma_builder);
        // convert the text into a compacted alphabet
        let mut s = Vec::with_capacity(text.len() + 1);
        for t in text.iter().copied() {
            s.push(sigma.char_to_sigma(t).ok_or(Error::LogicError(
                "freshly constructed sigma cannot translate text",
            ))?);
        }
        s.push(0);
        // compute the suffix array
        let mut sa = vec![0usize; s.len()];
        sais::sais(&sigma, &s, &mut sa)?;
        // compute the inverse suffix array
        let isa = inverse(&sa);
        // compute the successor array, psi
        let psi = psi::compute(&isa);
        // Now build them.
        // TODO(rescrv): parameterize
        SA::construct(3, &sa, &mut builder.sub(FieldNumber::must(3)))?;
        ISA::construct(&isa, &record_boundaries, &mut builder.sub(FieldNumber::must(4)))?;
        PSI::construct(&sigma, &psi, &mut builder.sub(FieldNumber::must(5)))?;
        Ok(())
    }

    fn len(&self) -> usize {
        // psi always includes an implicit end character that we strip
        self.psi.len() - 1
    }

    fn records(&self) -> usize {
        // TODO(rescrv): better error handling; requires modifying Document trait.
        self.record_boundaries
            .rank(self.record_boundaries.len())
            .unwrap_or(0)
            + 1
    }

    fn search(&self, needle: &[u32]) -> Result<Self::Search, Error> {
        let range = self.backwards_search(needle)?;
        if range.0 > range.1 {
            Ok(vec![].into_iter())
        } else {
            let mut result = Vec::with_capacity(range.1 - range.0 + 1);
            for offset in range.0..=range.1 {
                result.push(TextOffset(self.sa.lookup(&self.sigma, &self.psi, offset)?));
            }
            result.sort();
            Ok(result.into_iter())
        }
    }

    fn count(&self, needle: &[u32]) -> Result<usize, Error> {
        let range = self.backwards_search(needle)?;
        if range.0 > range.1 {
            Ok(0)
        } else {
            Ok(range.1 - range.0 + 1)
        }
    }

    fn lookup(&self, offset: TextOffset) -> Result<RecordOffset, Error> {
        Ok(RecordOffset(
            self.record_boundaries
                .rank(offset.0)
                .ok_or(Error::BadRank(offset.0))?,
        ))
    }

    fn offset_of(&self, record: RecordOffset) -> Result<TextOffset, Error> {
        Ok(TextOffset(
            self.record_boundaries
                .select(record.0)
                .ok_or(Error::BadSelect(record.0))?,
        ))
    }

    fn retrieve(&self, record: RecordOffset) -> Result<Vec<u32>, Error> {
        let start: usize = self
            .record_boundaries
            .select(record.0)
            .ok_or(Error::BadSelect(record.0))?;
        // TODO(rescrv):  This treats an error as if it's end.  Need to change the bit_vector API.
        let limit: usize = self
            .record_boundaries
            .select(record.0 + 1)
            .unwrap_or(self.len());
        if start > limit {
            return Err(Error::BadRecordOffset);
        }
        let mut idx = self.isa.lookup(start)?;
        let mut result = Vec::with_capacity(limit - start);
        for _ in start..limit {
            result.push(
                self.sigma
                    .sa_index_to_t(idx)
                    .ok_or(Error::InvalidSigma)?,
            );
            idx = self.psi.lookup(&self.sigma, idx)?;
        }
        Ok(result)
    }
}

impl<'a, SA, ISA, PSI> Unpackable<'a> for PsiDocument<'a, SA, ISA, PSI>
where
    SA: sa::SuffixArray + Unpackable<'a>,
    ISA: isa::InverseSuffixArray + Unpackable<'a>,
    PSI: psi::Psi + Unpackable<'a>,
    Error: From<<SA as Unpackable<'a>>::Error> + From<<ISA as Unpackable<'a>>::Error> + From<<PSI as Unpackable<'a>>::Error>,
{
    type Error = Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Self::Error> {
        let (stub, buf) = <PsiDocumentStub as Unpackable>::unpack(buf)
            .map_err(|_| Error::InvalidBitVector)?;
        let record_boundaries = bit_vector::sparse::BitVector::new(stub.record_boundaries)
            .ok_or(Error::InvalidBitVector)?;
        let sigma = sigma::Sigma::unpack(stub.sigma)?.0;
        let sa = SA::unpack(stub.sa)?.0;
        let isa = ISA::unpack(stub.isa)?.0;
        let psi = PSI::unpack(stub.psi)?.0;
        Ok((PsiDocument {
            record_boundaries,
            sigma,
            sa,
            isa,
            psi,
        }, buf))
    }
}

pub type CompressedDocument<'a> = PsiDocument::<'a, sa::SampledSuffixArray<'a>, isa::SampledInverseSuffixArray<'a>, psi::wavelet_tree::WaveletTreePsi<'a, wavelet_tree::prefix::WaveletTree<'a, encoder::HuffmanEncoder>>>;

///////////////////////////////////////////// Correlate ////////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct Exemplar {
    count: usize,
    needle: Vec<u32>,
}

impl Exemplar {
    pub fn count(&self) -> usize {
        self.count
    }

    pub fn text(&self) -> &[u32] {
        &self.needle
    }
}

impl From<CorrelateState> for Exemplar {
    fn from(mut cs: CorrelateState) -> Self {
        cs.needle.reverse();
        Self {
            count: cs.docs.iter().map(|(_, d)| d.numer).fold(0, usize::saturating_add),
            needle: cs.needle,
        }
    }
}

impl From<ExemplarState> for Exemplar {
    fn from(mut es: ExemplarState) -> Self {
        es.needle.reverse();
        Self {
            count: es.docs.iter().map(|(_, d)| d.numer).fold(0, usize::saturating_add),
            needle: es.needle,
        }
    }
}

#[derive(Debug, Default, Eq, PartialEq)]
struct CorrelateDocState {
    numer: usize,
    range: (usize, usize),
}

#[derive(Debug, Default, Eq, PartialEq)]
struct CorrelateState {
    terminal: u32,
    needle: Vec<u32>,
    docs: HashMap<usize, CorrelateDocState>,
}

impl Ord for CorrelateState {
    fn cmp(&self, other: &CorrelateState) -> Ordering {
        if self == other {
            Ordering::Equal
        } else {
            let numer_lhs = self.docs.iter().map(|(_, d)| d.numer).fold(0, usize::saturating_add);
            let numer_rhs = other.docs.iter().map(|(_, d)| d.numer).fold(0, usize::saturating_add);
            let denom_lhs = self.docs.iter().map(|(_, d)| d.range.1 + 1 - d.range.0).fold(0, usize::saturating_add);
            let denom_rhs = other.docs.iter().map(|(_, d)| d.range.1 + 1 - d.range.0).fold(0, usize::saturating_add);
            if denom_lhs == 0 && denom_rhs == 0 {
                Ordering::Equal
            } else if denom_lhs == 0 {
                Ordering::Less
            } else if denom_rhs == 0 {
                Ordering::Greater
            } else {
                (numer_lhs as f64 / denom_lhs as f64).total_cmp(&(numer_rhs as f64 / denom_rhs as f64))
            }
        }
    }
}

impl PartialOrd for CorrelateState {
    fn partial_cmp(&self, other: &CorrelateState) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub fn correlate<'a, SA, ISA, PSI, F>(docs: &'a [&'a PsiDocument<'a, SA, ISA, PSI>], boundaries: &[(u32, u32)], select: F) -> Correlate<'a, SA, ISA, PSI, F>
where
    SA: sa::SuffixArray,
    ISA: isa::InverseSuffixArray,
    PSI: psi::Psi,
    F: Fn(usize, RecordOffset) -> bool,
{
    let mut heap = BinaryHeap::new();
    for boundary in boundaries.iter() {
        let mut state = CorrelateState {
            // the starting boundary because we do backwards search.
            terminal: boundary.0,
            needle: vec![boundary.1],
            docs: HashMap::new(),
        };
        for (idx, doc) in docs.iter().enumerate() {
            let Ok(range) = doc.sigma.sa_range_for(boundary.1) else {
                continue;
            };
            if range.0 > range.1 {
                continue;
            }
            let mut numer = 0;
            for offset in range.0..=range.1 {
                let Ok(text_offset) = doc.sa.lookup(&doc.sigma, &doc.psi, offset) else {
                    // TODO(rescrv): Metrics because this should never happen.
                    continue;
                };
                let Ok(record_offset) = doc.lookup(TextOffset(text_offset)) else {
                    // TODO(rescrv): Metrics because this should never happen.
                    continue;
                };
                if select(idx, record_offset) {
                    numer += 1;
                }
            }
            if numer > 0 {
                state.docs.insert(idx, CorrelateDocState {
                    numer,
                    range,
                });
            }
        }
        if !state.docs.is_empty() {
            heap.push(state);
        }
    }
    Correlate {
        docs,
        heap,
        select,
    }
}

pub struct Correlate<'a, SA, ISA, PSI, F>
where
    SA: sa::SuffixArray,
    ISA: isa::InverseSuffixArray,
    PSI: psi::Psi,
    F: Fn(usize, RecordOffset) -> bool,
{
    docs: &'a [&'a PsiDocument<'a, SA, ISA, PSI>],
    heap: BinaryHeap<CorrelateState>,
    select: F,
}

impl<'a, SA, ISA, PSI, F> Iterator for Correlate<'a, SA, ISA, PSI, F>
where
    SA: sa::SuffixArray,
    ISA: isa::InverseSuffixArray,
    PSI: psi::Psi,
    F: Fn(usize, RecordOffset) -> bool,
{
    type Item = Exemplar;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(correlate) = self.heap.pop() {
            if correlate.terminal == correlate.needle[correlate.needle.len() - 1] {
                return Some(Exemplar::from(correlate));
            }
            let mut new_correlates: HashMap<u32, CorrelateState> = HashMap::new();
            for (idx, doc_state) in correlate.docs.iter() {
                let doc = &self.docs[*idx];
                for i in 1..=doc.sigma.K() as u32 {
                    let Some(t) = doc.sigma.sigma_to_char(i) else {
                        // TODO(rescrv): Metrics because this should never happen.
                        continue;
                    };
                    let new_correlate = new_correlates.entry(t).or_insert_with(|| {
                        let mut needle = correlate.needle.clone();
                        needle.push(t);
                        CorrelateState {
                            terminal: correlate.terminal,
                            needle,
                            docs: HashMap::new(),
                        }
                    });
                    let Ok(range) = doc.sigma.sa_range_for_sigma(i) else {
                        // TODO(rescrv): Metrics because this should never happen.
                        continue;
                    };
                    if range.0 > range.1 {
                        // TODO(rescrv): Metrics because this should never happen.
                        continue;
                    }
                    let Ok(range) = doc
                        .psi
                        .constrain(&doc.sigma, range, doc_state.range) else {
                        // TODO(rescrv): Metrics because this should never happen.
                        continue;
                    };
                    if range.0 > range.1 {
                        continue;
                    }
                    let mut numer = 0;
                    for offset in range.0..=range.1 {
                        let Ok(text_offset) = doc.sa.lookup(&doc.sigma, &doc.psi, offset) else {
                            // TODO(rescrv): Metrics because this should never happen.
                            continue;
                        };
                        let Ok(record_offset) = doc.lookup(TextOffset(text_offset)) else {
                            // TODO(rescrv): Metrics because this should never happen.
                            continue;
                        };
                        if (self.select)(*idx, record_offset) {
                            numer += 1;
                        }
                    }
                    if numer > 0 {
                        new_correlate.docs.insert(*idx, CorrelateDocState {
                            numer,
                            range,
                        });
                    }
                }
            }
            for (_, new_correlate) in new_correlates {
                if !new_correlate.docs.is_empty() {
                    self.heap.push(new_correlate);
                }
            }
        }
        None
    }
}

#[derive(Debug, Default, Eq, PartialEq)]
struct ExemplarState {
    terminal: u32,
    needle: Vec<u32>,
    docs: HashMap<usize, CorrelateDocState>,
}

impl Ord for ExemplarState {
    fn cmp(&self, other: &ExemplarState) -> Ordering {
        if self == other {
            Ordering::Equal
        } else {
            let numer_lhs = self.docs.iter().map(|(_, d)| d.numer).fold(0, usize::saturating_add);
            let numer_rhs = other.docs.iter().map(|(_, d)| d.numer).fold(0, usize::saturating_add);
            numer_lhs.cmp(&numer_rhs)
        }
    }
}

impl PartialOrd for ExemplarState {
    fn partial_cmp(&self, other: &ExemplarState) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub fn exemplars<'a, SA, ISA, PSI>(docs: &'a [&'a PsiDocument<'a, SA, ISA, PSI>], boundaries: &[(u32, u32)]) -> Exemplars<'a, SA, ISA, PSI>
where
    SA: sa::SuffixArray,
    ISA: isa::InverseSuffixArray,
    PSI: psi::Psi,
{
    let mut heap = BinaryHeap::new();
    for boundary in boundaries.iter() {
        let mut state = ExemplarState {
            // the starting boundary because we do backwards search.
            terminal: boundary.0,
            needle: vec![boundary.1],
            docs: HashMap::new(),
        };
        for (idx, doc) in docs.iter().enumerate() {
            let Ok(range) = doc.sigma.sa_range_for(boundary.1) else {
                continue;
            };
            if range.0 > range.1 {
                continue;
            }
            let numer = range.1 - range.0 + 1;
            state.docs.insert(idx, CorrelateDocState {
                numer,
                range,
            });
        }
        if !state.docs.is_empty() {
            heap.push(state);
        }
    }
    Exemplars {
        docs,
        heap,
    }
}

pub struct Exemplars<'a, SA, ISA, PSI>
where
    SA: sa::SuffixArray,
    ISA: isa::InverseSuffixArray,
    PSI: psi::Psi,
{
    docs: &'a [&'a PsiDocument<'a, SA, ISA, PSI>],
    heap: BinaryHeap<ExemplarState>,
}

impl<'a, SA, ISA, PSI> Iterator for Exemplars<'a, SA, ISA, PSI>
where
    SA: sa::SuffixArray,
    ISA: isa::InverseSuffixArray,
    PSI: psi::Psi,
{
    type Item = Exemplar;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(correlate) = self.heap.pop() {
            if correlate.terminal == correlate.needle[correlate.needle.len() - 1] {
                return Some(Exemplar::from(correlate));
            }
            let mut new_correlates: HashMap<u32, ExemplarState> = HashMap::new();
            for (idx, doc_state) in correlate.docs.iter() {
                let doc = &self.docs[*idx];
                for i in 1..=doc.sigma.K() as u32 {
                    let Some(t) = doc.sigma.sigma_to_char(i) else {
                        // TODO(rescrv): Metrics because this should never happen.
                        continue;
                    };
                    let new_correlate = new_correlates.entry(t).or_insert_with(|| {
                        let mut needle = correlate.needle.clone();
                        needle.push(t);
                        ExemplarState {
                            terminal: correlate.terminal,
                            needle,
                            docs: HashMap::new(),
                        }
                    });
                    let Ok(range) = doc.sigma.sa_range_for_sigma(i) else {
                        // TODO(rescrv): Metrics because this should never happen.
                        continue;
                    };
                    if range.0 > range.1 {
                        // TODO(rescrv): Metrics because this should never happen.
                        continue;
                    }
                    let Ok(range) = doc
                        .psi
                        .constrain(&doc.sigma, range, doc_state.range) else {
                        // TODO(rescrv): Metrics because this should never happen.
                        continue;
                    };
                    if range.0 > range.1 {
                        continue;
                    }
                    let numer = range.1 - range.0 + 1;
                    new_correlate.docs.insert(*idx, CorrelateDocState {
                        numer,
                        range,
                    });
                }
            }
            for (_, new_correlate) in new_correlates {
                if !new_correlate.docs.is_empty() {
                    self.heap.push(new_correlate);
                }
            }
        }
        None
    }
}

////////////////////////////////////////////// inverse /////////////////////////////////////////////

fn inverse(x: &[usize]) -> Vec<usize> {
    let mut ix = vec![0usize; x.len()];
    for i in 0..x.len() {
        ix[x[i]] = i
    }
    ix
}

////////////////////////////////////// check_record_boundaries /////////////////////////////////////

fn check_record_boundaries(text: &[u32], record_boundaries: &[usize]) -> Result<(), Error> {
    if record_boundaries.is_empty() {
        return Err(Error::BadRecordOffset);
    }
    for (lhs, rhs) in std::iter::zip(record_boundaries.iter(), record_boundaries[1..].iter()) {
        if lhs >= rhs {
            return Err(Error::BadRecordOffset);
        }
    }
    if record_boundaries[0] != 0 {
        return Err(Error::BadRecordOffset);
    }
    if record_boundaries[record_boundaries.len() - 1] >= text.len() {
        return Err(Error::BadRecordOffset);
    }
    Ok(())
}

///////////////////////////////////////////// test_util ////////////////////////////////////////////

#[cfg(test)]
pub mod test_util {
    use crate::builder::Builder;
    use crate::sigma::Sigma;

    use super::*;

    #[macro_export]
    macro_rules! assert_eq_with_ctx {
        (@inner [$($elems:tt)*] , $($rem:tt)*) => {
            format!("{} = {:?}; {}", stringify!($($elems)*), $($elems)*, assert_eq_with_ctx!(@inner [] $($rem)*))
        };

        (@inner [$($elems:tt)*] $e:tt $($rem:tt)*) => {
            assert_eq_with_ctx!(@inner [$($elems)* $e] $($rem)*)
        };

        (@inner [$($elems:tt)*]) => {
            format!("{} = {:?}", stringify!($($elems)*), $($elems)*)
        };

        ($lhs:expr, $rhs:expr, $($rem:expr),+) => {
            assert_eq!($lhs, $rhs, "{} == {}; {}", stringify!($lhs), stringify!($rhs), assert_eq_with_ctx!(@inner [] $($rem),*));
        };

        ($lhs:expr, $rhs:expr) => {
            assert_eq!($lhs, $rhs, "{} == {}", stringify!($lhs), stringify!($rhs));
        };
    }

    pub(crate) use assert_eq_with_ctx;

    // NOTE(rescrv):  Verbose, possibly duplicating information, so that there's a canonical place
    // to look for the examples of different representations.  Also serves as a human cross-check.
    // Static, possibly to a point of inconvenience, to make the most easy thing to do to create
    // top-level constants.  These are both done to serve human readers well and force human
    // writers to think about the test case and generate it by hand.
    #[allow(non_snake_case)]
    pub struct TestCase {
        pub text: &'static str,
        pub sigma2text: &'static [char],
        pub boundaries: &'static [usize],
        pub not_in_str: &'static [char],
        pub S: &'static [u32],
        pub SA: &'static [usize],
        pub ISA: &'static [usize],
        pub PSI: &'static [usize],
        pub bucket_starts: &'static [usize],
        pub bucket_limits: &'static [usize],
        pub deref_SA: &'static [u32],
        pub lstype: &'static str,
        pub lmspos: &'static str,
        pub searches: &'static [(&'static str, &'static [usize])],
        pub table: &'static str,
        #[allow(clippy::type_complexity)]
        pub constrain: &'static [((usize, usize), (usize, usize), (usize, usize))],
    }

    impl TestCase {
        pub fn sigma(&self) -> Vec<u8> {
            let mut buf = Vec::new();
            let mut builder = Builder::new(&mut buf);
            Sigma::construct(self.text.chars().map(|c| c as u32), &mut builder)
                .expect("test case should construct");
            drop(builder);
            buf
        }
    }

    pub const BANANA: &TestCase = &TestCase {
        text: "BANANA",
        sigma2text: &['A', 'B', 'N'],
        boundaries: &[1, 4, 5, 7],
        not_in_str: &['C', 'D', 'E'],
        S: &[2, 1, 3, 1, 3, 1, 0],
        SA: &[6, 5, 3, 1, 0, 4, 2],
        ISA: &[4, 3, 6, 2, 5, 1, 0],
        PSI: &[4, 0, 5, 6, 3, 1, 2],
        bucket_starts: &[0, 1, 4, 5],
        bucket_limits: &[1, 4, 5, 7],
        deref_SA: &[0, 1, 1, 1, 2, 3, 3],
        lstype: "LSLSLLS",
        lmspos: " * *  *",
        searches: &[("AN", &[1, 3]), ("NA", &[2, 4])],
        table: "
    A      B   N
[]  [0]    []  [] 
[]  []     []  [1]
[]  []     [3] [2]
[4] []     []  [] 
[]  [5, 6] []  [] 
",
        constrain: &[
            // Backwards search of AN
            ((5, 6), (0, 7), (5, 6)),
            ((1, 3), (5, 6), (2, 3)),

            // Backwards search of BANA
            ((1, 3), (0, 7), (1, 3)),
            ((5, 6), (1, 3), (5, 6)),
            ((1, 3), (5, 6), (2, 3)),
            ((4, 4), (2, 3), (4, 4)),
        ],
    };

    pub const MISSISSIPPI: &TestCase = &TestCase {
        text: "MISSISSIPPI",
        sigma2text: &['I', 'M', 'P', 'S'],
        boundaries: &[1, 5, 6, 8, 12],
        not_in_str: &['A', 'B', 'N'],
        S: &[2, 1, 4, 4, 1, 4, 4, 1, 3, 3, 1, 0],
        SA: &[11, 10, 7, 4, 1, 0, 9, 8, 6, 3, 5, 2],
        ISA: &[5, 4, 11, 9, 3, 10, 8, 2, 7, 6, 1, 0],
        PSI: &[5, 0, 7, 10, 11, 4, 1, 6, 2, 3, 8, 9],
        bucket_starts: &[0, 1, 5, 6, 8],
        bucket_limits: &[1, 5, 6, 8, 12],
        deref_SA: &[0, 1, 1, 1, 1, 2, 3, 3, 4, 4, 4, 4],
        lstype: "LSLLSLLSLLLS",
        lmspos: " *  *  *   *",
        searches: &[("ISS", &[1, 4])],
        table: "
    I        M   P   S
[]  [0]      []  []  []
[]  []       []  [1] []
[]  []       []  []  [2]
[]  []       [4] []  [3]
[5] []       []  []  []
[]  []       []  [6] []
[]  [7]      []  []  []
[]  []       []  []  [8, 9]
[]  [10, 11] []  []  []
",
        constrain: &[
            ((8, 11), (8, 11), (10, 11)),
            ((8, 11), (1, 4), (8, 9)),
            ((1, 4), (8, 11), (3, 4)),
            ((1, 4), (0, 0), (1, 1)),
            ((1, 4), (6, 7), (2, 2)),
            ((5, 5), (1, 4), (5, 5)),
            ((6, 7), (6, 7), (7, 7)),
        ],
    };

    pub const MISSISSIPPI_BANANA: &TestCase = &TestCase {
        text: "MISSISSIPPIBANANA",
        sigma2text: &['A', 'B', 'I', 'M', 'N', 'P', 'S'],
        boundaries: &[1, 4, 5, 9, 10, 12, 14, 18],
        not_in_str: &['C', 'D', 'E'],
        S: &[4, 3, 7, 7, 3, 7, 7, 3, 6, 6, 3, 2, 1, 5, 1, 5, 1, 0],
        SA: &[17, 16, 14, 12, 11, 10, 7, 4, 1, 0, 15, 13, 9, 8, 6, 3, 5, 2],
        ISA: &[9, 8, 17, 15, 7, 16, 14, 6, 13, 12, 5, 4, 3, 11, 2, 10, 1, 0],
        PSI: &[9, 0, 10, 11, 3, 4, 13, 16, 17, 8, 1, 2, 5, 12, 6, 7, 14, 15],
        bucket_starts: &[0, 1, 4, 5, 9, 10, 12, 14],
        bucket_limits: &[1, 4, 5, 9, 10, 12, 14, 18],
        deref_SA: &[0, 1, 1, 1, 2, 3, 3, 3, 3, 4, 5, 5, 6, 6, 7, 7, 7, 7],
        lstype: "LSLLSLLSLLLLSLSLLS",
        lmspos: " *  *  *    * *  *",
        searches: &[("ISS", &[1, 4])],
        table: "
    A        B   I        M   N   P    S
[]  [0]      []  []       []  []  []   []
[]  []       []  []       []  [1] []   []
[]  []       [3] []       []  [2] []   []
[]  []       []  [4]      []  []  []   []
[]  []       []  []       []  []  [5]  []
[]  []       []  []       []  []  []   [6]
[]  []       []  []       [8] []  []   [7]
[9] []       []  []       []  []  []   []
[]  [10, 11] []  []       []  []  []   []
[]  []       []  []       []  []  [12] []
[]  []       []  [13]     []  []  []   []
[]  []       []  []       []  []  []   [14, 15]
[]  []       []  [16, 17] []  []  []   []
",
        constrain: &[],
    };

    pub const MIISSISSISSIPPI: &TestCase = &TestCase {
        text: "MIISSISSISSIPPI",
        sigma2text: &['I', 'M', 'P', 'S'],
        boundaries: &[1, 7, 8, 10, 16],
        not_in_str: &['A', 'B', 'N'],
        S: &[2, 1, 1, 4, 4, 1, 4, 4, 1, 4, 4, 1, 3, 3, 1, 0],
        SA: &[15, 14, 1, 11, 8, 5, 2, 0, 13, 12, 10, 7, 4, 9, 6, 3],
        ISA: &[7, 2, 6, 15, 12, 5, 14, 11, 4, 13, 10, 3, 9, 8, 1, 0],
        PSI: &[7, 0, 6, 9, 13, 14, 15, 2, 1, 8, 3, 4, 5, 10, 11, 12],
        bucket_starts: &[0, 1, 7, 8, 10],
        bucket_limits: &[1, 7, 8, 10, 16],
        deref_SA: &[0, 1, 1, 1, 1, 1, 1, 2, 3, 3, 4, 4, 4, 4, 4, 4],
        lstype: "LSSLLSLLSLLSLLLS",
        lmspos: " *   *  *  *   *",
        searches: &[("ISS", &[2, 5, 8])],
        table: "
    I            M   P   S
[]  [0]          []  []  []
[]  []           []  [1] []
[]  []           [2] []  []
[]  []           []  []  [3]
[]  [6]          []  []  [4, 5]
[7] []           []  []  []
[]  []           []  [8] []
[]  [9]          []  []  []
[]  []           []  []  [10, 11, 12]
[]  [13, 14, 15] []  []  []
",
        constrain: &[],
    };

    // Thist test case was found in early prototype of sais.  It has survived a language rewrite
    // and really only exists because it makes me laugh.  I'd love to replace it with a minimal
    // test case demonstrating all of the following:
    // - identical S+LS blocks ordered in SA the way they are ordered in the input
    // - identical S+LS blocks reversed in SA from the way they are ordered in the input
    // - S+LS block that whose S+L prefix is shared by multiple S+L strings, also forward/reverse
    pub const MUTANT_BANANA: &TestCase = &TestCase {
        text: "ANBNANANCANBNCNANANCNA",
        sigma2text: &['A', 'B', 'C', 'N'],
        boundaries: &[1, 8, 10, 13, 23],
        not_in_str: &['D', 'E', 'F'],
        S: &[
            1, 4, 2, 4, 1, 4, 1, 4, 3, 1, 4, 2, 4, 3, 4, 1, 4, 1, 4, 3, 4, 1, 0,
        ],
        SA: &[
            22, 21, 4, 15, 0, 9, 6, 17, 2, 11, 8, 19, 13, 20, 3, 14, 5, 16, 1, 10, 7, 18, 12,
        ],
        ISA: &[
            4, 18, 8, 14, 2, 16, 6, 20, 10, 5, 19, 9, 22, 12, 15, 3, 17, 7, 21, 11, 13, 1, 0,
        ],
        PSI: &[
            4, 0, 16, 17, 18, 19, 20, 21, 14, 22, 5, 13, 15, 1, 2, 3, 6, 7, 8, 9, 10, 11, 12,
        ],
        bucket_starts: &[0, 1, 8, 10, 13],
        bucket_limits: &[1, 8, 10, 13, 23],
        deref_SA: &[
            0, 1, 1, 1, 1, 1, 1, 1, 2, 2, 3, 3, 3, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        ],
        lstype: "SLSLSLSLLSLSLSLSLSLSLLS",
        lmspos: "  * * *  * * * * * *  *",
        searches: &[("AN", &[0, 4, 6, 9, 15, 17])],
        table: "
    A        B    C        N
[]  [0]      []   []       []
[]  []       []   []       [1]
[4] []       []   [5]      [2, 3, 6, 7]
[]  []       []   []       [8, 9]
[]  []       []   []       [10]
[]  []       []   []       [11, 12]
[]  [16, 17] [14] [13, 15] []
[]  [18, 19] []   []       []
[]  [20, 21] [22] []       []
",
        constrain: &[],
    };

    pub const SAIS_EXAMPLE: &TestCase = &TestCase {
        text: "baabababbab",
        sigma2text: &['a', 'b'],
        boundaries: &[1, 6, 12],
        not_in_str: &['c', 'd', 'e'],
        S: &[2, 1, 1, 2, 1, 2, 1, 2, 2, 1, 2, 0],
        SA: &[11, 1, 9, 2, 4, 6, 10, 0, 8, 3, 5, 7],
        ISA: &[7, 1, 3, 9, 4, 10, 5, 11, 8, 2, 6, 0],
        PSI: &[7, 3, 6, 9, 10, 11, 0, 1, 2, 4, 5, 8],
        bucket_starts: &[0, 1, 6],
        bucket_limits: &[1, 6, 12],
        deref_SA: &[0, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2],
        lstype: "LSSLSLSLLSLS",
        lmspos: " *  * *  * *",
        searches: &[("b", &[0, 3, 5, 7, 8, 10])],
        table: "
    a       b
[]  []      [0]
[]  []      [1]
[]  [3]     [2, 4, 5]
[]  [6]     []
[7] [9, 10] [8]
[]  [11]    []
",
        constrain: &[],
    };

    pub const BAD_CASE_1: &TestCase = &TestCase {
        text: "1111111111112111",
        sigma2text: &['1', '2'],
        boundaries: &[1, 16, 17],
        not_in_str: &['3'],
        S: &[1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 1, 1, 1, 0],
        SA: &[16, 15, 14, 13, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
        ISA: &[4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 3, 2, 1, 0],
        PSI: &[4, 0, 1, 2, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 3],
        bucket_starts: &[0, 1, 16],
        bucket_limits: &[1, 16, 17],
        deref_SA: &[0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2],
        lstype: "SSSSSSSSSSSSLLLLS",
        lmspos: "                *",
        searches: &[],
        table: "
    1                                      2
[]  [0]                                    []
[]  [1]                                    []
[4] [2, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14] [3]
[]  [15]                                   []
[]  [16]                                   []
",
        constrain: &[
            ((1, 15), (16, 16), (15, 15)),
        ],
    };

    pub const BAD_CASE_2: &TestCase = &TestCase {
        text: "2121221211111132",
        sigma2text: &['1', '2', '3'],
        boundaries: &[1, 10, 16, 17],
        not_in_str: &['4'],
        S: &[2, 1, 2, 1, 2, 2, 1, 2, 1, 1, 1, 1, 1, 1, 3, 2, 0],
        SA: &[16, 8, 9, 10, 11, 12, 6, 1, 3, 13, 15, 7, 5, 0, 2, 4, 14],
        ISA: &[13, 7, 14, 8, 15, 12, 6, 11, 1, 2, 3, 4, 5, 9, 16, 10, 0],
        PSI: &[13, 2, 3, 4, 5, 9, 11, 14, 15, 16, 0, 1, 6, 7, 8, 12, 10],
        bucket_starts: &[0, 1, 10, 16],
        bucket_limits: &[1, 10, 16, 17],
        deref_SA: &[0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 3],
        lstype: "LSLSLLSLSSSSSSLLS",
        lmspos: " * *  * *       *",
        searches: &[],
        table: "
     1            2         3
[]   []           [0]       []
[]   [2, 3, 4, 5] [1]       []
[]   []           [6, 7, 8] []
[]   [9]          []        []
[]   []           []        [10]
[13] [11, 14]     [12]      []
[]   [15]         []        []
[]   [16]         []        []
",
        constrain: &[
            ((16, 16), (1, 9), (16, 15)),
        ],
    };

    pub const BAD_CASE_3: &TestCase = &TestCase {
        text: "2121221212121222",
        sigma2text: &['1', '2'],
        boundaries: &[1, 7, 17],
        not_in_str: &['3'],
        S: &[2, 1, 2, 1, 2, 2, 1, 2, 1, 2, 1, 2, 1, 2, 2, 2, 0],
        SA: &[16, 6, 8, 1, 10, 3, 12, 15, 5, 7, 0, 9, 2, 11, 14, 4, 13],
        ISA: &[10, 3, 12, 5, 15, 8, 1, 9, 2, 11, 4, 13, 6, 16, 14, 7, 0],
        PSI: &[10, 9, 11, 12, 13, 15, 16, 0, 1, 2, 3, 4, 5, 6, 7, 8, 14],
        bucket_starts: &[0, 1, 7],
        bucket_limits: &[1, 7, 17],
        deref_SA: &[0, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2],
        lstype: "LSLSLLSLSLSLSLLLS",
        lmspos: " * *  * * * *   *",
        searches: &[],
        table: "
     1               2
[]   []              [0]
[]   []              [1, 2, 3, 4, 5, 6]
[]   []              [7]
[10] [9, 11, 12, 13] [8]
[]   [15, 16]        [14]
",
        constrain: &[
            ((1, 6), (1, 6), (1, 0)),
        ],
    };

    pub const BAD_CASE_4: &TestCase = &TestCase {
        text: "12224122231222312222122221222212221122211222112221",
        sigma2text: &['1', '2', '3', '4'],
        boundaries: &[1, 15, 48, 50, 51],
        not_in_str: &['5'],
        S: &[1, 2, 2, 2, 4, 1, 2, 2, 2, 3, 1, 2, 2, 2, 3, 1, 2, 2, 2, 2, 1, 2, 2, 2, 2, 1, 2, 2, 2, 2, 1, 2, 2, 2, 1, 1, 2, 2, 2, 1, 1, 2, 2, 2, 1, 1, 2, 2, 2, 1, 0],
        SA: &[50, 49, 44, 39, 34, 45, 40, 35, 30, 25, 20, 15, 10, 5, 0, 48, 43, 38, 33, 29, 24, 19, 47, 42, 37, 32, 28, 23, 18, 46, 41, 36, 31, 27, 22, 17, 26, 21, 16, 11, 6, 1, 12, 7, 2, 13, 8, 3, 14, 9, 4],
        ISA: &[14, 41, 44, 47, 50, 13, 40, 43, 46, 49, 12, 39, 42, 45, 48, 11, 38, 35, 28, 21, 10, 37, 34, 27, 20, 9, 36, 33, 26, 19, 8, 32, 25, 18, 4, 7, 31, 24, 17, 3, 6, 30, 23, 16, 2, 5, 29, 22, 15, 1, 0],
        PSI: &[14, 0, 5, 6, 7, 29, 30, 31, 32, 36, 37, 38, 39, 40, 41, 1, 2, 3, 4, 8, 9, 10, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 33, 34, 35, 42, 43, 44, 45, 46, 47, 48, 49, 50, 11, 12, 13],
        bucket_starts: &[0, 1, 15, 48, 50],
        bucket_limits: &[1, 15, 48, 50, 51],
        deref_SA: &[0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 3, 3, 4],
        lstype: "SSSSLSSSSLSSSSLSLLLLSLLLLSLLLLSLLLSSLLLSSLLLSSLLLLS",
        lmspos: "     *    *    *    *    *    *   *    *    *     *",
        searches: &[],
        table: "
     1                                        2                                                    3        4
[]   [0]                                      []                                                   []       []
[]   []                                       [1]                                                  []       []
[]   []                                       [2, 3, 4]                                            []       []
[14] [5, 6, 7]                                [8, 9, 10]                                           [11, 12] [13]
[]   []                                       [15, 16, 17, 18, 19, 20, 21]                         []       []
[]   [29, 30, 31, 32, 36, 37, 38, 39, 40, 41] [22, 23, 24, 25, 26, 27, 28, 33, 34, 35, 42, 43, 44] []       []
[]   []                                       [45, 46]                                             []       []
[]   []                                       [47]                                                 []       []
[]   []                                       [48, 49]                                             []       []
[]   []                                       [50]                                                 []       []
",
        constrain: &[
            ((15, 47), (48, 49), (45, 46)),
            ((15, 47), (45, 46), (42, 43)),
            ((15, 47), (42, 43), (39, 40)),
            ((15, 47), (50, 50), (47, 47)),
            ((15, 47), (47, 47), (44, 44)),
        ],
    };

    #[macro_export]
    macro_rules! test_cases_for {
        ($name:ident, $check:path) => {
            mod $name {
                #[test]
                fn banana() {
                    $check($crate::test_util::BANANA);
                }

                #[test]
                fn mississippi() {
                    $check($crate::test_util::MISSISSIPPI);
                }

                #[test]
                fn mississippi_banana() {
                    $check($crate::test_util::MISSISSIPPI_BANANA);
                }

                #[test]
                fn miissississippi() {
                    $check($crate::test_util::MIISSISSISSIPPI);
                }

                #[test]
                fn mutant_banana() {
                    $check($crate::test_util::MUTANT_BANANA);
                }

                #[test]
                fn sais_example() {
                    $check($crate::test_util::SAIS_EXAMPLE);
                }

                #[test]
                fn bad_case1() {
                    $check($crate::test_util::BAD_CASE_1);
                }

                #[test]
                fn bad_case2() {
                    $check($crate::test_util::BAD_CASE_2);
                }

                #[test]
                fn bad_case3() {
                    $check($crate::test_util::BAD_CASE_3);
                }

                #[test]
                fn bad_case4() {
                    $check($crate::test_util::BAD_CASE_4);
                }
            }
        };
    }

    pub(crate) use test_cases_for;

    #[macro_export]
    macro_rules! search_cases_for {
        ($name:ident, $check:path) => {
            mod $name {
                use super::*;

                fn build(t: &TestCase) -> Vec<u8> {
                    let text: Vec<u32> = t.text.chars().map(|c| c as u32).collect();
                    let record_boundaries = vec![0usize];
                    let mut buf = Vec::new();
                    let mut builder = Builder::new(&mut buf);
                    <$check>::construct(text, record_boundaries, &mut builder)
                        .expect("test case should index");
                    drop(builder);
                    buf
                }

                fn check_len(t: &TestCase) {
                    let doc = build(t);
                    let (doc, _) =
                        <$check>::unpack(&doc).expect("freshly created document should unpack");
                    assert_eq!(t.text.chars().count(), doc.len());
                }

                test_cases_for! {len, super::check_len}

                fn check_retrieve(t: &TestCase) {
                    let text: Vec<u32> = t.text.chars().map(|c| c as u32).collect();
                    let doc = build(t);
                    let (doc, _) =
                        <$check>::unpack(&doc).expect("freshly created document should unpack");
                    assert_eq!(
                        text,
                        doc.retrieve(RecordOffset(0))
                            .expect("retrieve should succeed")
                    );
                }

                test_cases_for! {retrieve, super::check_retrieve}

                fn check_search(t: &TestCase) {
                    let doc = build(t);
                    let (doc, _) =
                        <$check>::unpack(&doc).expect("freshly created document should unpack");
                    for (needle, offsets) in t.searches.iter() {
                        let expected: Vec<TextOffset> =
                            offsets.iter().map(|x| TextOffset(*x)).collect();
                        let needle_u32: Vec<u32> = needle.chars().map(|c| c as u32).collect();
                        let mut returned: Vec<TextOffset> = doc
                            .search(&needle_u32)
                            .expect("search should succeed")
                            .collect();
                        returned.sort();
                        assert_eq_with_ctx!(expected, returned, needle);
                    }
                    // TODO(rescrv): Check empty search.
                }

                test_cases_for! {search, super::check_search}

                fn check_count(t: &TestCase) {
                    let doc = build(t);
                    let (doc, _) =
                        <$check>::unpack(&doc).expect("freshly created document should unpack");
                    for (needle, offsets) in t.searches.iter() {
                        let needle_u32: Vec<u32> = needle.chars().map(|c| c as u32).collect();
                        let count: usize = doc.count(&needle_u32).expect("count should succeed");
                        assert_eq_with_ctx!(offsets.len(), count, needle);
                    }
                }

                test_cases_for! {count, super::check_count}
            }
        };
    }

    pub(crate) use search_cases_for;
}

#[cfg(test)]
mod tests {
    use crate::sa::SuffixArray;
    use crate::isa::InverseSuffixArray;

    use super::psi::Psi;
    use super::test_util::*;
    use super::*;

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn u32_as_usize() {
        assert!(u32::BITS <= usize::BITS);
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn usize_as_u64() {
        assert!(usize::BITS <= u64::BITS);
    }

    #[test]
    fn inverse() {
        let x = &[8, 6, 9, 5, 0, 3, 1, 2, 7, 4];
        let ix = &[4, 6, 7, 5, 9, 3, 1, 8, 0, 2];
        let returned: &[usize] = &super::inverse(x);
        assert_eq!(ix, returned);
        let returned: &[usize] = &super::inverse(returned);
        assert_eq!(x, returned);
    }

    fn check_string(t: &TestCase) {
        let sigma = t.sigma();
        let sigma = sigma::Sigma::unpack(&sigma).expect("test should unpack").0;
        let mut s: Vec<u32> = t.text.chars().map(|x| sigma.char_to_sigma(x as u32).unwrap()).collect();
        s.push(0);
        assert_eq!(t.S, s);
    }

    test_cases_for! {string, super::check_string}

    fn check_isa(t: &TestCase) {
        let returned = super::inverse(t.SA);
        assert_eq_with_ctx!(t.ISA, returned);
    }

    test_cases_for! {isa, super::check_isa}

    fn check_unpack_psi_document(t: &TestCase) {
        let text: Vec<u32> = t.text.chars().map(|c| c as u32).collect();
        let record_boundaries = vec![0usize];
        let mut buf = Vec::new();
        let mut builder = Builder::new(&mut buf);
        type Document<'a> = super::PsiDocument::<'a, super::sa::ReferenceSuffixArray, super::isa::ReferenceInverseSuffixArray, super::psi::ReferencePsi>;
        Document::construct(text, record_boundaries, &mut builder)
            .expect("test case should index");
        drop(builder);
        let doc = Document::unpack(&buf).expect("document should unpack").0;

        for (idx, sa) in t.SA.iter().enumerate() {
            assert_eq_with_ctx!(*sa, doc.sa.lookup(&doc.sigma, &doc.psi, idx).unwrap(), idx);
        }

        for (idx, isa) in t.ISA.iter().enumerate() {
            assert_eq_with_ctx!(*isa, doc.isa.lookup(idx).unwrap(), idx);
        }

        assert_eq!(t.PSI.len(), doc.psi.len());
        for (idx, psi) in t.PSI.iter().enumerate() {
            assert_eq_with_ctx!(*psi, doc.psi.lookup(&doc.sigma, idx).unwrap(), idx);
        }
    }

    test_cases_for!{unpack_psi_document, super::check_unpack_psi_document}

    search_cases_for! {reference, crate::ReferenceDocument}
    search_cases_for! {psi_with_all_reference, crate::PsiDocument::<crate::sa::ReferenceSuffixArray, crate::isa::ReferenceInverseSuffixArray, psi::ReferencePsi>}
    search_cases_for! {psi_with_wavelet_psi, crate::PsiDocument::<crate::sa::ReferenceSuffixArray, crate::isa::ReferenceInverseSuffixArray, psi::wavelet_tree::WaveletTreePsi<wavelet_tree::ReferenceWaveletTree>>}
    search_cases_for! {compressed_document, crate::CompressedDocument}
}
