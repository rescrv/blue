use buffertk::Unpackable;
use prototk::FieldNumber;

use crate::binary_search::partition_by;
use crate::bit_vector::sparse::BitVector;
use crate::bit_vector::BitVector as BitVectorTrait;
use crate::builder::{Builder, Helper};
use crate::psi::Psi;
use crate::sigma::Sigma;
use crate::wavelet_tree::WaveletTree;
use crate::{inverse, Error};

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

#[derive(Debug)]
struct BuildContext {
    ctx: [u32; CTX_MAX],
    start: usize,
    tree: Vec<u32>,
    sums: Vec<usize>,
}

/////////////////////////////////////////////// Table //////////////////////////////////////////////

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
        const CTX_SZ: usize = 2;
        let table = compute_table(sigma, CTX_SZ, psi)?;
        let mut y_value = vec![];
        let mut y_key = vec![];
        let mut sum = 0;
        for i in 0..sigma.K() {
            for (idx, t) in table.iter().enumerate() {
                let in_cell = t.sums.get(i).copied().unwrap_or(0);
                if in_cell > 0 {
                    if sum > 0 {
                        y_key.push(sum - 1);
                    }
                    y_value.push(idx);
                    sum += in_cell;
                }
            }
        }
        for t in table.iter() {
            let mut builder = builder.sub(FieldNumber::must(CONTEXT_FIELD_NUMBER));
            builder.append_vec_u32(FieldNumber::must(1), &t.ctx[..CTX_SZ]);
            builder.append_u64(FieldNumber::must(2), t.start as u64);
            WT::construct(&t.tree, &mut builder.sub(FieldNumber::must(3)))?;
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
