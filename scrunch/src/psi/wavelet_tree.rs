use crate::psi::Psi;
use std::hash::Hash;

const CTX: usize = 1;

pub struct WaveletTreePsi<D, WT>
where
    D: crate::dictionary::Dictionary<(usize, usize)>,
    WT: crate::wavelet_tree::WaveletTree,
{
    cells: D,
    table: Vec<Context<WT>>,
}

struct Context<WT>
where
    WT: crate::wavelet_tree::WaveletTree,
{
    _ctx: Vec<usize>,
    start: usize,
    tree: WT,
}

impl<D, WT> super::Psi for WaveletTreePsi<D, WT>
where
    D: crate::dictionary::Dictionary<(usize, usize)>,
    WT: crate::wavelet_tree::WaveletTree,
{
    fn new<T, B>(sigma: &crate::Sigma<T, B>, psi: &[usize]) -> Self
    where
        T: Copy + Clone + Eq + Hash + Ord,
        B: crate::bit_vector::OldBitVector,
    {
        let mut ctx: [usize; CTX] = [0; CTX];
        // compute the inverse of psi so that we can bounce around the columns in order
        let ipsi = crate::inverse(psi);
        // rows in the column/row breakdown of psi
        let mut table: Vec<Context<WT>> = Vec::new();
        // string for the wavelet tree
        let mut wt: Vec<usize> = Vec::new();
        // track the index into psi where the current context began
        let mut start = 0;
        // now iterate
        for i in 0..ipsi.len() {
            // This was not immediately intuitive to me and took awhile to discover.
            //
            // We are going to use psi to figure out the contex for the point and use ipsi to
            // figure out the character for the wavelet tree.
            let mut tmp = ctx.clone();
            let mut idx = i;
            for j in 0..CTX {
                tmp[j] = sigma.columns().rank(idx);
                idx = psi[idx];
            }
            // if this is the start of a new context
            if ctx != tmp {
                // on the first iteration of the loop, there's definitely no context to push
                //
                // skipping here allows one initialization point
                if i > 0 {
                    table.push(Context {
                        _ctx: ctx.to_vec(),
                        tree: WT::new(&wt),
                        start,
                    });
                }
                // reset for next row
                wt.clear();
                ctx = tmp;
                start = i;
            }
            // use ipsi to figure out which character is at this position in the string
            let s = match sigma.sa_index_to_sigma(ipsi[i]) {
                Some(x) => x,
                None => panic!("XXX"),
            };
            wt.push(s);
        }
        // push one last context
        table.push(Context {
            _ctx: ctx.to_vec(),
            tree: WT::new(&wt),
            start,
        });
        // cleanup and make the pieces we care about immutable
        let table = table;
        // now form the rest of what we need to be able to lookup in the wt
        // starting with a bit vector tracking non-empty cells
        let mut cells: Vec<(usize, (usize, usize))> = Vec::new();
        let mut sum = 0;
        // for each column
        for i in 0..sigma.K() {
            // for each row
            for j in 0..table.len() {
                // figure out the total number of characters in this cell
                let in_cell = table[j].tree.rank_q(table[j].tree.len(), i);
                if in_cell > 0 {
                    cells.push((sum, (sum, j)));
                }
                sum += in_cell;
            }
        }
        cells.push((psi.len(), (0, 0)));
        // contexts
        let mut contexts: Vec<usize> = Vec::new();
        let mut sum = 0;
        // for each context
        for i in 0..table.len() {
            contexts.push(sum);
            // figure out the total number of characters in this tree
            sum += table[i].tree.len();
        }
        contexts.push(sum);
        Self {
            table,
            cells: D::new(&cells),
        }
    }

    fn len(&self) -> usize {
        if self.table.len() == 0 {
            0
        } else {
            let last = &self.table[self.table.len() - 1];
            last.start + last.tree.len()
        }
    }

    fn lookup<T, B>(&self, sigma: &crate::Sigma<T, B>, idx: usize) -> usize
    where
        T: Copy + Clone + Eq + Hash + Ord,
        B: crate::bit_vector::OldBitVector,
    {
        assert!(idx == 0 || self.table.len() > 0);
        // empty table case
        if self.table.len() == 0 {
            return 0;
        }
        // use the cells dictionary to figure out the right context
        let (start_of_cell, context) = *self.cells.lookup(idx);
        let column: usize = sigma.sa_index_to_sigma(idx).unwrap();
        // answer
        self.table[context].start
            + self.table[context]
                .tree
                .select_q(idx - start_of_cell, column)
    }

    fn constrain<T, B>(
        &self,
        sigma: &crate::Sigma<T, B>,
        range: (usize, usize),
        into: (usize, usize),
    ) -> (usize, usize)
    where
        T: Copy + Clone + Eq + Hash + Ord,
        B: crate::bit_vector::OldBitVector,
    {
        assert!(range.0 <= range.1);
        assert!(range.1 <= self.len());
        assert!(into.0 <= into.1);
        assert!(into.1 <= self.len());
        // empty table case
        if self.table.len() == 0 {
            // XXX test
            return (0, 0);
        }
        // empty range case
        if into.0 >= into.1 {
            return into;
        }
        let lower = self.binary_search(sigma, into.0, range);
        let upper = self.binary_search(sigma, into.1, range);
        (lower, upper)
    }
}

impl<D, WT> WaveletTreePsi<D, WT>
where
    D: crate::dictionary::Dictionary<(usize, usize)>,
    WT: crate::wavelet_tree::WaveletTree,
{
    // find the highest index of psi in the range [into.0, into.1) s.t. psi[idx] <= point
    fn binary_search<T, B>(
        &self,
        sigma: &crate::Sigma<T, B>,
        point: usize,
        into: (usize, usize),
    ) -> usize
    where
        T: Copy + Clone + Eq + Hash + Ord,
        B: crate::bit_vector::OldBitVector,
    {
        assert!(self.table.len() > 0);
        assert!(into.0 <= into.1);
        assert!(into.1 <= self.len());
        // empty range case
        if into.0 >= into.1 {
            return into.0;
        }
        // this transforms from [) to ambiguous [)/[] intervals
        let mut cells = (self.cells.rank(into.0), self.cells.rank(into.1));
        // correct it to a closed [] interval
        if self.cells.select(cells.1) == into.1 {
            cells.1 -= 1;
        }
        // find the cell that contains the answer
        let cell = loop {
            let mid = cells.0 + (cells.1 - cells.0) / 2;
            // [] interval of psi that corresponds to the cell `mid`
            // closed interval is necessary to cover case when mid is last cell of column
            let psi_mid_lower = self.lookup(sigma, self.cells.select(mid));
            let psi_mid_upper = self.lookup(sigma, self.cells.select(mid + 1) - 1);
            if psi_mid_lower > point && mid < cells.1 {
                cells.1 = mid - 1;
            } else if psi_mid_upper < point && cells.0 < mid {
                cells.0 = mid + 1;
            } else {
                break mid;
            }
        };
        let (x, (start_of_cell, context)) = self.cells.selectup(cell);
        assert_eq!(x, *start_of_cell);
        let column: usize = sigma.sa_index_to_sigma(*start_of_cell).unwrap();
        let wt = &self.table[*context].tree;
        start_of_cell + wt.rank_q(point - self.table[*context].start, column)
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

// TODO(rescrv): Uncomment
//#[cfg(test)]
//super::tests::test_Psi!(
//    tests,
//    super::WaveletTreePsi::<
//        ReferenceDictionary<ReferenceOldBitVector, (usize, usize)>,
//        ReferenceWaveletTree,
//    >
//);
