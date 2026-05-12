use buffertk::Unpackable;
use prototk::FieldNumber;

use crate::binary_search::partition_by;
use crate::bit_vector::BitVector as BitVectorTrait;
use crate::bit_vector::sparse::BitVector;
use crate::builder::{Builder, Helper};
use crate::psi::Psi;
use crate::sigma::Sigma;
use crate::wavelet_tree::WaveletTree;
use crate::{Error, inverse};

///////////////////////////////////////////// Constants ////////////////////////////////////////////

const CTX_MAX: usize = 2;

const CONTEXT_FIELD_NUMBER: u32 = 2;
const Y_KEY_FIELD_NUMBER: u32 = 3;
const Y_VALUE_FIELD_NUMBER: u32 = 4;

//////////////////////////////////////////// ContextStub ///////////////////////////////////////////

#[derive(Clone, Debug, Default, prototk_derive::Message)]
struct ContextStub<'a> {
    #[prototk(1, uint32)]
    ctx: Vec<u32>,
    #[prototk(2, uint64)]
    start: u64,
    #[prototk(3, bytes)]
    tree: &'a [u8],
}

impl<'a, WT> TryFrom<ContextStub<'a>> for Context<WT>
where
    WT: WaveletTree + Unpackable<'a, Error = Error>,
{
    type Error = Error;

    fn try_from(stub: ContextStub<'a>) -> Result<Self, Self::Error> {
        let ContextStub { ctx, start, tree } = stub;
        if ctx.len() > CTX_MAX {
            return Err(Error::InvalidWaveletTree);
        }
        if start > usize::MAX as u64 {
            return Err(Error::IntoUsize);
        }
        let start = start as usize;
        let ctx_vec = ctx;
        let mut ctx = [0u32; CTX_MAX];
        ctx[..ctx_vec.len()].copy_from_slice(&ctx_vec);
        let tree = WT::unpack(tree)?.0;
        Ok(Context { ctx, start, tree })
    }
}

////////////////////////////////////////////// Context /////////////////////////////////////////////

#[derive(Debug)]
struct Context<WT>
where
    WT: WaveletTree,
{
    #[allow(dead_code)]
    ctx: [u32; CTX_MAX],
    start: usize,
    tree: WT,
}

impl<WT: WaveletTree> Context<WT> {
    fn lookup(&self, sigma: u32, idx: usize) -> Result<usize, Error> {
        if idx >= self.tree.len() {
            return Err(Error::BadIndex(idx));
        }
        let to_select = idx + 1;
        let select = self
            .tree
            .select_q(sigma, to_select)
            .ok_or(Error::BadIndex(idx))?
            - 1;
        Ok(self.start + select)
    }
}

/////////////////////////////////////////// BuildContext ///////////////////////////////////////////

#[cfg(test)]
#[allow(dead_code)]
#[derive(Debug)]
struct BuildContext {
    ctx: [u32; CTX_MAX],
    start: usize,
    tree: Vec<u32>,
    sums: Vec<usize>,
}

#[derive(Clone, Copy, Debug)]
struct CellSummary {
    row: usize,
    count: usize,
}

trait PsiIndex: Copy {
    fn to_usize(self) -> usize;
}

impl PsiIndex for usize {
    fn to_usize(self) -> usize {
        self
    }
}

impl PsiIndex for u32 {
    fn to_usize(self) -> usize {
        self as usize
    }
}

enum SaToSigma {
    U8(Vec<u8>),
    U16(Vec<u16>),
    U32(Vec<u32>),
}

impl SaToSigma {
    fn new(k: usize, len: usize, bucket_limits: &[usize]) -> Result<Self, Error> {
        if k <= u8::MAX as usize + 1 {
            Ok(Self::U8(Self::build(k, len, bucket_limits)?))
        } else if k <= u16::MAX as usize + 1 {
            Ok(Self::U16(Self::build(k, len, bucket_limits)?))
        } else {
            Ok(Self::U32(Self::build(k, len, bucket_limits)?))
        }
    }

    fn build<T: Copy + Default + TryFrom<usize>>(
        k: usize,
        len: usize,
        bucket_limits: &[usize],
    ) -> Result<Vec<T>, Error> {
        if bucket_limits.len() != k {
            return Err(Error::InvalidSigma);
        }
        let mut values = vec![T::default(); len];
        let mut previous_limit = 0usize;
        for (symbol, limit) in bucket_limits.iter().copied().enumerate() {
            if limit < previous_limit || limit > len {
                return Err(Error::InvalidSigma);
            }
            let symbol = T::try_from(symbol).map_err(|_| Error::IntoUsize)?;
            values[previous_limit..limit].fill(symbol);
            previous_limit = limit;
        }
        if previous_limit != len {
            return Err(Error::InvalidSigma);
        }
        Ok(values)
    }
}

trait SigmaMap {
    unsafe fn get_unchecked_usize(&self, idx: usize) -> usize;
}

impl SigmaMap for [u8] {
    #[inline(always)]
    unsafe fn get_unchecked_usize(&self, idx: usize) -> usize {
        // SAFETY(codex): this unsafe trait method requires callers to prove idx < self.len(); the
        // slice contains initialized u8 values, and the shared read creates no aliasing violation
        // under the Rustonomicon's unchecked indexing rules.
        unsafe { *self.get_unchecked(idx) as usize }
    }
}

impl SigmaMap for [u16] {
    #[inline(always)]
    unsafe fn get_unchecked_usize(&self, idx: usize) -> usize {
        // SAFETY(codex): callers uphold the method contract that idx is in-bounds; reading an
        // initialized u16 through a shared slice reference preserves Rustonomicon aliasing and
        // lifetime requirements.
        unsafe { *self.get_unchecked(idx) as usize }
    }
}

impl SigmaMap for [u32] {
    #[inline(always)]
    unsafe fn get_unchecked_usize(&self, idx: usize) -> usize {
        // SAFETY(codex): the unsafe method contract requires idx < self.len(); the u32 element is
        // initialized and read immutably, so unchecked access satisfies Rustonomicon bounds,
        // provenance, and aliasing rules.
        unsafe { *self.get_unchecked(idx) as usize }
    }
}

fn flush_context<WT: WaveletTree, H: Helper>(
    builder: &mut Builder<H>,
    ctx: [u32; CTX_MAX],
    ctx_sz: usize,
    start: usize,
    tree: &mut Vec<u32>,
    counts: &mut [usize],
    active: &mut Vec<usize>,
    cells_by_sigma: &mut [Vec<CellSummary>],
    row: usize,
) -> Result<(), Error> {
    for symbol in active.iter().copied() {
        let count = std::mem::take(&mut counts[symbol]);
        cells_by_sigma[symbol].push(CellSummary { row, count });
    }
    active.clear();
    let mut builder = builder.sub(FieldNumber::must(CONTEXT_FIELD_NUMBER));
    builder.append_vec_u32(FieldNumber::must(1), &ctx[..ctx_sz]);
    builder.append_u64(FieldNumber::must(2), start as u64);
    WT::construct(tree, &mut builder.sub(FieldNumber::must(3)))?;
    tree.clear();
    Ok(())
}

fn construct_streaming<WT: WaveletTree, H: Helper, I: PsiIndex>(
    sigma: &Sigma,
    len: usize,
    builder: &mut Builder<H>,
    ipsi: impl IntoIterator<Item = I>,
    psi_lookup: impl FnMut(usize) -> usize,
) -> Result<(), Error> {
    let mut bucket_limits = Vec::new();
    sigma.bucket_limits(&mut bucket_limits)?;
    let sa_to_sigma = SaToSigma::new(sigma.K(), len, &bucket_limits)?;
    match sa_to_sigma {
        SaToSigma::U8(values) => construct_streaming_mapped::<WT, H, I, _, _>(
            sigma,
            len,
            builder,
            ipsi,
            psi_lookup,
            values.as_slice(),
        ),
        SaToSigma::U16(values) => construct_streaming_mapped::<WT, H, I, _, _>(
            sigma,
            len,
            builder,
            ipsi,
            psi_lookup,
            values.as_slice(),
        ),
        SaToSigma::U32(values) => construct_streaming_mapped::<WT, H, I, _, _>(
            sigma,
            len,
            builder,
            ipsi,
            psi_lookup,
            values.as_slice(),
        ),
    }
}

fn construct_streaming_mapped<WT, H, I, F, M>(
    sigma: &Sigma,
    len: usize,
    builder: &mut Builder<H>,
    ipsi: impl IntoIterator<Item = I>,
    mut psi_lookup: F,
    sa_to_sigma: &M,
) -> Result<(), Error>
where
    WT: WaveletTree,
    H: Helper,
    I: PsiIndex,
    F: FnMut(usize) -> usize,
    M: SigmaMap + ?Sized,
{
    const CTX_SZ: usize = 2;
    let mut ctx = [0u32; CTX_MAX];
    let mut tree: Vec<u32> = Vec::new();
    let mut counts = vec![0usize; sigma.K()];
    let mut active: Vec<usize> = Vec::new();
    let mut cells_by_sigma: Vec<Vec<CellSummary>> = vec![Vec::new(); sigma.K()];
    let mut start = 0usize;
    let mut row = 0usize;
    let mut seen = 0usize;
    for (i, ipsi) in ipsi.into_iter().enumerate() {
        if i >= len {
            return Err(Error::InvalidPsi);
        }
        seen += 1;
        let mut tmp = ctx;
        // SAFETY(codex): i is checked against len above, and SaToSigma::new builds sa_to_sigma with
        // exactly len initialized entries; the unsafe trait method's in-bounds precondition is met.
        tmp[0] = unsafe { sa_to_sigma.get_unchecked_usize(i) as u32 };
        let mut idx = i;
        for t in tmp.iter_mut().take(CTX_SZ).skip(1) {
            idx = psi_lookup(idx);
            if idx >= len {
                return Err(Error::InvalidSigma);
            }
            // SAFETY(codex): idx was checked to be less than len, and sa_to_sigma has len
            // initialized entries; this shared read is in-bounds and preserves Rustonomicon
            // aliasing guarantees.
            *t = unsafe { sa_to_sigma.get_unchecked_usize(idx) as u32 };
        }
        if ctx != tmp {
            if i > 0 {
                flush_context::<WT, H>(
                    builder,
                    ctx,
                    CTX_SZ,
                    start,
                    &mut tree,
                    &mut counts,
                    &mut active,
                    &mut cells_by_sigma,
                    row,
                )?;
                row += 1;
            }
            ctx = tmp;
            start = i;
        }
        let ipsi = ipsi.to_usize();
        if ipsi >= len {
            return Err(Error::InvalidSigma);
        }
        // SAFETY(codex): ipsi was checked against len, and SaToSigma's backing map has exactly len
        // initialized symbols; the unchecked read therefore meets Rustonomicon bounds and aliasing
        // requirements.
        let symbol = unsafe { sa_to_sigma.get_unchecked_usize(ipsi) };
        tree.push(symbol as u32);
        // SAFETY(codex): sa_to_sigma values are built from bucket symbols in 0..sigma.K(), and
        // counts.len() == sigma.K(); this immutable initialized read is in-bounds and alias-safe.
        if unsafe { *counts.get_unchecked(symbol) } == 0 {
            active.push(symbol);
        }
        // SAFETY(codex): the same symbol-domain proof gives symbol < counts.len(); counts is
        // uniquely borrowed mutably here, so the initialized increment follows Rustonomicon
        // exclusivity and bounds rules.
        unsafe {
            *counts.get_unchecked_mut(symbol) += 1;
        }
    }
    if seen != len {
        return Err(Error::InvalidPsi);
    }
    flush_context::<WT, H>(
        builder,
        ctx,
        CTX_SZ,
        start,
        &mut tree,
        &mut counts,
        &mut active,
        &mut cells_by_sigma,
        row,
    )?;
    let mut y_value = vec![];
    let mut y_key = vec![];
    let mut sum = 0usize;
    for cells in cells_by_sigma {
        for cell in cells {
            if sum > 0 {
                y_key.push(sum - 1);
            }
            y_value.push(cell.row);
            sum += cell.count;
        }
    }
    y_key.push(sum - 1);
    BitVector::from_indices(
        128,
        sum,
        &y_key,
        &mut builder.sub(FieldNumber::must(Y_KEY_FIELD_NUMBER)),
    )
    .ok_or(Error::InvalidBitVector)?;
    builder.append_vec_usize(FieldNumber::must(Y_VALUE_FIELD_NUMBER), &y_value);
    Ok(())
}

/////////////////////////////////////////////// Table //////////////////////////////////////////////

#[cfg(test)]
fn compute_table(sigma: &Sigma, ctx_sz: usize, psi: &[usize]) -> Result<Vec<BuildContext>, Error> {
    let mut ctx = [0u32; CTX_MAX];
    // compute the inverse of psi so that we can bounce around the columns in order
    let ipsi = inverse(psi);
    // rows in the column/row breakdown of psi
    let mut table: Vec<BuildContext> = Vec::new();
    // string for the wavelet tree
    let mut tree: Vec<u32> = Vec::new();
    let mut sums: Vec<usize> = Vec::new();
    // track the index into psi where the current context began
    let mut start = 0;
    // now iterate
    for (i, ipsi) in ipsi.iter().enumerate() {
        // This was not immediately intuitive to me and took awhile to discover.
        //
        // We are going to use psi to figure out the contex for the point and use ipsi to
        // figure out the character for the wavelet tree.
        let mut tmp = ctx;
        let mut idx = i;
        for t in tmp.iter_mut().take(ctx_sz) {
            *t = sigma.sa_index_to_sigma(idx).ok_or(Error::InvalidSigma)?;
            idx = psi[idx];
        }
        // if this is the start of a new context
        if ctx != tmp {
            // on the first iteration of the loop, there's definitely no context to push
            //
            // skipping here allows one initialization point
            if i > 0 {
                let tree = std::mem::take(&mut tree);
                let sums = std::mem::take(&mut sums);
                table.push(BuildContext {
                    ctx,
                    start,
                    tree,
                    sums,
                });
            }
            // reset for next row
            tree.clear();
            ctx = tmp;
            start = i;
        }
        // use ipsi to figure out which character is at this position in the string
        let s = sigma.sa_index_to_sigma(*ipsi).ok_or(Error::InvalidSigma)?;
        tree.push(s);
        if sums.len() <= s as usize {
            sums.resize(s as usize + 1, 0);
        }
        sums[s as usize] += 1;
    }
    // push one last context
    let tree = std::mem::take(&mut tree);
    let sums = std::mem::take(&mut sums);
    table.push(BuildContext {
        ctx,
        start,
        tree,
        sums,
    });
    Ok(table)
}

#[cfg(test)]
pub fn draw_table(sigma: &Sigma, psi: &[usize]) -> String {
    let table = compute_table(sigma, 2, psi).expect("table should construct");
    let mut printed = "".to_string();
    let mut rows = vec![];
    for row in table.iter() {
        let mut columns = vec![vec![]; sigma.K()];
        for (idx, c) in row.tree.iter().enumerate() {
            columns[*c as usize].push(row.start + idx);
        }
        let mut row = vec![];
        for column in columns.into_iter() {
            row.push(format!("{column:?}"));
        }
        rows.push(row);
    }
    let mut widths = vec![0usize; sigma.K()];
    for row in rows.iter() {
        for (idx, cell) in row.iter().enumerate() {
            widths[idx] = std::cmp::max(widths[idx], cell.len() + 1);
        }
    }
    for (idx, width) in widths.iter().enumerate() {
        let c = sigma.sigma_to_char(idx as u32 + 1);
        let c = char::from_u32(c.unwrap_or(0)).unwrap_or(' ');
        printed += &format!("{c:<width$}");
    }
    printed += "\n";
    for row in rows.iter() {
        for (idx, cell) in row.iter().enumerate() {
            let width = widths[idx];
            printed += &format!("{cell:<width$}");
        }
        printed += "\n";
    }
    printed
}

//////////////////////////////////////// WaveletTreePsiStub ////////////////////////////////////////

#[derive(Clone, Debug, Default, prototk_derive::Message)]
pub struct WaveletTreePsiStub<'a> {
    #[prototk(2, message)]
    table: Vec<ContextStub<'a>>,
    #[prototk(3, bytes)]
    y_key: &'a [u8],
    #[prototk(4, uint64)]
    y_value: Vec<u64>,
}

////////////////////////////////////////// WaveletTreePsi //////////////////////////////////////////

pub struct WaveletTreePsi<'a, WT>
where
    WT: WaveletTree,
{
    table: Vec<Context<WT>>,
    y_key: BitVector<'a>,
    y_value: Vec<usize>,
}

impl<WT: WaveletTree> WaveletTreePsi<'_, WT> {
    fn row_for_index(&self, idx: usize) -> Result<usize, Error> {
        let row = self.table.partition_point(|row| row.start <= idx);
        row.checked_sub(1).ok_or(Error::BadIndex(idx))
    }

    fn cell_start_for_symbol_row(
        &self,
        sigma: &Sigma,
        symbol: u32,
        row: usize,
    ) -> Result<Option<usize>, Error> {
        let range = sigma.sa_range_for_sigma(symbol)?;
        if range.0 > range.1 {
            return Ok(None);
        }
        let first_cell = self.y_key.rank(range.0).ok_or(Error::BadRank(range.0))?;
        let last_cell = self.y_key.rank(range.1).ok_or(Error::BadRank(range.1))?;
        let cells = &self.y_value[first_cell..=last_cell];
        let offset = cells.partition_point(|candidate| *candidate < row);
        if offset >= cells.len() || cells[offset] != row {
            return Ok(None);
        }
        let cell = first_cell + offset;
        self.y_key
            .select(cell)
            .ok_or(Error::BadSelect(cell))
            .map(Some)
    }

    // Find the lowest index of psi in the range [into.0, into.1] s.t. psi[idx] >= point.
    //
    // Requires that into be a closed range.
    fn lower_bound(
        &self,
        sigma: &Sigma,
        point: usize,
        into: (usize, usize),
    ) -> Result<usize, Error> {
        assert!(!self.table.is_empty());
        assert!(into.0 <= into.1);
        assert!(into.1 <= self.len());
        // Empty range case.
        if into.0 > into.1 {
            return Ok(into.0);
        }
        // Empty character case
        if into.0 == 0 {
            return Err(Error::BadSearch);
        }
        // This transforms from [) to ambiguous [)/[] intervals.
        let first_cell = self.y_key.rank(into.0).ok_or(Error::BadRank(into.0))?;
        let last_cell = self.y_key.rank(into.1).ok_or(Error::BadRank(into.1))?;
        let mut cell = partition_by(first_cell, last_cell, |cell| {
            self.table[self.y_value[cell]].start < point
        });
        if cell > first_cell && self.table[self.y_value[cell]].start > point {
            cell -= 1;
        }
        let start_of_cell = self.y_key.select(cell).ok_or(Error::BadSelect(cell))?;
        let end_of_cell = self
            .y_key
            .select(cell + 1)
            .ok_or(Error::BadSelect(cell + 1))?
            - 1;
        let column = sigma
            .sa_index_to_sigma(start_of_cell)
            .ok_or(Error::InvalidSigma)?;
        if point >= self.table[self.y_value[cell]].start {
            Ok(self.table[self.y_value[cell]]
                .tree
                .rank_q(column, point - self.table[self.y_value[cell]].start)
                .unwrap_or(end_of_cell - start_of_cell + 1)
                + start_of_cell)
        } else {
            Ok(start_of_cell)
        }
    }

    // Find the highest index of psi in the range [into.0, into.1] s.t. psi[idx] <= point.
    //
    // Requires that into be a closed range.
    fn upper_bound(
        &self,
        sigma: &Sigma,
        point: usize,
        into: (usize, usize),
    ) -> Result<usize, Error> {
        assert!(!self.table.is_empty());
        assert!(into.0 <= into.1);
        assert!(into.1 <= self.len());
        // Empty range case.
        if into.0 > into.1 {
            return Ok(into.0);
        }
        // Empty character case
        if into.0 == 0 {
            return Err(Error::BadSearch);
        }
        // This transforms from [) to ambiguous [)/[] intervals.
        let first_cell = self.y_key.rank(into.0).ok_or(Error::BadRank(into.0))?;
        let last_cell = self.y_key.rank(into.1).ok_or(Error::BadRank(into.1))?;
        let mut cell = partition_by(first_cell, last_cell, |cell| {
            self.table[self.y_value[cell]].start < point
        });
        if cell > first_cell && self.table[self.y_value[cell]].start > point {
            cell -= 1;
        }
        let start_of_cell = self.y_key.select(cell).ok_or(Error::BadSelect(cell))?;
        let end_of_cell = self
            .y_key
            .select(cell + 1)
            .ok_or(Error::BadSelect(cell + 1))?
            - 1;
        let column = sigma
            .sa_index_to_sigma(start_of_cell)
            .ok_or(Error::InvalidSigma)?;
        if point >= self.table[self.y_value[cell]].start {
            if let Some(rank) = self.table[self.y_value[cell]]
                .tree
                .rank_q(column, point - self.table[self.y_value[cell]].start)
            {
                if self.table[self.y_value[cell]]
                    .lookup(column, rank)
                    .unwrap_or(point + 1)
                    > point
                {
                    Ok(rank + start_of_cell - 1)
                } else {
                    Ok(rank + start_of_cell)
                }
            } else {
                Ok(end_of_cell)
            }
        } else {
            Ok(start_of_cell - 1)
        }
    }
}

impl<WT: WaveletTree + std::fmt::Debug> std::fmt::Debug for WaveletTreePsi<'_, WT> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.debug_struct("WaveletTreePsi")
            .field("table", &self.table)
            .field("y_key", &self.y_key)
            .field("y_value", &self.y_value.len())
            .finish()
    }
}

impl<WT: WaveletTree> super::Psi for WaveletTreePsi<'_, WT> {
    fn construct<H: Helper>(
        sigma: &Sigma,
        psi: &[usize],
        builder: &mut Builder<H>,
    ) -> Result<(), Error> {
        if u32::try_from(psi.len()).is_ok() {
            let mut ipsi = vec![0u32; psi.len()];
            for (idx, value) in psi.iter().copied().enumerate() {
                ipsi[value] = idx as u32;
            }
            construct_streaming::<WT, H, _>(sigma, psi.len(), builder, ipsi, |idx| psi[idx])
        } else {
            construct_streaming::<WT, H, _>(sigma, psi.len(), builder, inverse(psi), |idx| psi[idx])
        }
    }

    fn construct_u32<H: Helper>(
        sigma: &Sigma,
        psi: &[u32],
        builder: &mut Builder<H>,
    ) -> Result<(), Error> {
        let ipsi = inverse_psi_u32(psi);
        construct_streaming::<WT, H, _>(sigma, psi.len(), builder, ipsi, |idx| psi[idx] as usize)
    }

    fn construct_from_sa_isa_u32<H: Helper>(
        sigma: &Sigma,
        sa: &[u32],
        isa: &[u32],
        builder: &mut Builder<H>,
    ) -> Result<(), Error> {
        if sa.len() != isa.len() {
            return Err(Error::InvalidPsi);
        }
        let ipsi = sa.iter().copied().map(|pos| {
            let pos = pos as usize;
            if pos == 0 {
                isa[isa.len() - 1]
            } else {
                isa[pos - 1]
            }
        });
        construct_streaming::<WT, H, _>(sigma, sa.len(), builder, ipsi, |idx| {
            let pos = sa[idx] as usize;
            if pos + 1 == isa.len() {
                isa[0] as usize
            } else {
                isa[pos + 1] as usize
            }
        })
    }

    fn len(&self) -> usize {
        if self.table.is_empty() {
            0
        } else {
            let last = &self.table[self.table.len() - 1];
            last.start + last.tree.len()
        }
    }

    fn lookup(&self, sigma: &Sigma, idx: usize) -> Result<usize, Error> {
        let y_rank = self.y_key.rank(idx).ok_or(Error::BadRank(idx))?;
        let y = self.y_value[y_rank];
        let start_of_cell = self.y_key.select(y_rank).ok_or(Error::BadSelect(idx))?;
        let sigma = sigma.sa_index_to_sigma(idx).ok_or(Error::InvalidSigma)?;
        self.table[y].lookup(sigma, idx - start_of_cell)
    }

    fn constrain(
        &self,
        sigma: &Sigma,
        range: (usize, usize),
        into: (usize, usize),
    ) -> Result<(usize, usize), Error> {
        if range.0 > range.1 {
            return Ok(range);
        }
        if into.0 > into.1 {
            return Ok((range.0, range.0 - 1));
        }
        // empty table case
        if self.table.is_empty() {
            return Ok((1, 0));
        }
        // empty range case
        if into.0 > into.1 {
            return Ok(into);
        }
        let lower = self.lower_bound(sigma, into.0, range)?;
        let upper = self.upper_bound(sigma, into.1, range)?;
        Ok((lower, upper))
    }

    fn predecessor_sigma_symbols(
        &self,
        sigma: &Sigma,
        range: (usize, usize),
        symbols: &mut Vec<u32>,
    ) -> Result<bool, Error> {
        symbols.clear();
        if range.0 > range.1 || self.table.is_empty() {
            return Ok(true);
        }
        let len = range.1 - range.0 + 1;
        if len.saturating_mul(32) >= sigma.K().saturating_sub(1) {
            return Ok(false);
        }
        for idx in range.0..=range.1 {
            let row = self.table.partition_point(|row| row.start <= idx);
            let Some(row) = row.checked_sub(1) else {
                return Err(Error::BadIndex(idx));
            };
            let row = &self.table[row];
            let Some(symbol) = row.tree.access(idx - row.start) else {
                return Err(Error::BadIndex(idx));
            };
            if symbol != 0 && !symbols.contains(&symbol) {
                symbols.push(symbol);
            }
        }
        Ok(true)
    }

    fn predecessor_sigma_ranges(
        &self,
        sigma: &Sigma,
        range: (usize, usize),
        ranges: &mut Vec<(u32, (usize, usize))>,
    ) -> Result<bool, Error> {
        ranges.clear();
        if range.0 > range.1 || self.table.is_empty() {
            return Ok(true);
        }
        let first_row = self.row_for_index(range.0)?;
        let last_row = self.row_for_index(range.1)?;
        if first_row == last_row {
            let row = &self.table[first_row];
            let lower = range.0 - row.start;
            let upper = range.1 - row.start + 1;
            row.tree
                .symbol_rank_ranges(lower, upper, ranges)
                .ok_or(Error::BadIndex(upper))?;
            let mut write = 0;
            for read in 0..ranges.len() {
                let (symbol, (lower_rank, upper_rank)) = ranges[read];
                if symbol == 0 {
                    continue;
                }
                let Some(cell_start) = self.cell_start_for_symbol_row(sigma, symbol, first_row)?
                else {
                    return Err(Error::BadIndex(first_row));
                };
                ranges[write] = (
                    symbol,
                    (cell_start + lower_rank, cell_start + upper_rank - 1),
                );
                write += 1;
            }
            ranges.truncate(write);
            return Ok(true);
        }
        let mut symbols = Vec::new();
        if !self.predecessor_sigma_symbols(sigma, range, &mut symbols)? {
            return Ok(false);
        }
        for symbol in symbols {
            let symbol_range = sigma.sa_range_for_sigma(symbol)?;
            let constrained = self.constrain(sigma, symbol_range, range)?;
            if constrained.0 <= constrained.1 {
                ranges.push((symbol, constrained));
            }
        }
        Ok(true)
    }
}

fn inverse_psi_u32(psi: &[u32]) -> Vec<u32> {
    let mut ipsi = vec![0u32; psi.len()];
    for (idx, value) in psi.iter().copied().enumerate() {
        ipsi[value as usize] = idx as u32;
    }
    ipsi
}

impl<'a, WT> Unpackable<'a> for WaveletTreePsi<'a, WT>
where
    WT: WaveletTree + Unpackable<'a, Error = Error>,
{
    type Error = Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Self::Error> {
        let (
            WaveletTreePsiStub {
                table: contexts,
                y_key,
                y_value,
            },
            buf,
        ) = WaveletTreePsiStub::unpack(buf).map_err(|_| Error::InvalidBitVector)?;
        let mut table: Vec<Context<WT>> = vec![];
        for t in contexts.into_iter() {
            table.push(t.try_into()?);
        }
        let y_key = BitVector::parse(y_key)?.0;
        // TODO(rescrv): error if doesn't fit
        let y_value: Vec<usize> = y_value.iter().map(|x| *x as usize).collect();
        Ok((
            Self {
                table,
                y_key,
                y_value,
            },
            buf,
        ))
    }
}
