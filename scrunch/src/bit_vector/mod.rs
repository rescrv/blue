pub trait BitVector {
    fn new(bv: &[bool]) -> Self;
    fn sparse(bv: &[usize]) -> Self;

    fn len(&self) -> usize;

    fn rank(&self, x: usize) -> usize;
    fn select(&self, x: usize) -> usize;
}

pub struct ReferenceBitVector {
    bv: Vec<bool>,
    ranks: Vec<usize>,
    selects: Vec<usize>,
}

impl BitVector for ReferenceBitVector {
    fn new(bv: &[bool]) -> Self {
        let mut ranks = Vec::with_capacity(bv.len() + 1);
        let mut selects = Vec::with_capacity(bv.len() + 1);
        let mut rank: usize = 0;
        selects.push(0);
        for i in 0..bv.len() {
            ranks.push(rank);
            if bv[i] {
                rank += 1;
                selects.push(i + 1);
            }
        }
        ranks.push(rank);
        Self {
            bv: bv.to_vec(),
            ranks,
            selects,
        }
    }

    fn sparse(bv: &[usize]) -> Self {
        let mut bits: Vec<bool> = Vec::new();
        for &i in bv {
            if bits.len() <= i {
                bits.resize(i + 1, false);
            }
            bits[i] = true;
        }
        return Self::new(&bits);
    }

    fn len(&self) -> usize {
        self.bv.len()
    }

    fn rank(&self, x: usize) -> usize {
        assert!(x <= self.bv.len());
        self.ranks[x]
    }

    fn select(&self, x: usize) -> usize {
        assert!(x <= self.rank(self.bv.len()));
        self.selects[x]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_evens<BV: BitVector>(new: fn(&[bool]) -> BV) {
        // try 010101
        let b = new(&[false, true, false, true, false, true]);
        assert_eq!(0, b.rank(0));
        assert_eq!(0, b.rank(1));
        assert_eq!(1, b.rank(2));
        assert_eq!(1, b.rank(3));
        assert_eq!(2, b.rank(4));
        assert_eq!(2, b.rank(5));
        assert_eq!(3, b.rank(6));

        assert_eq!(0, b.select(0));
        assert_eq!(2, b.select(1));
        assert_eq!(4, b.select(2));
        assert_eq!(6, b.select(3));
    }

    fn simple_odds<BV: BitVector>(new: fn(&[bool]) -> BV) {
        // try 101010
        let b = new(&[true, false, true, false, true, false]);
        assert_eq!(0, b.rank(0));
        assert_eq!(1, b.rank(1));
        assert_eq!(1, b.rank(2));
        assert_eq!(2, b.rank(3));
        assert_eq!(2, b.rank(4));
        assert_eq!(3, b.rank(5));
        assert_eq!(3, b.rank(6));

        assert_eq!(0, b.select(0));
        assert_eq!(1, b.select(1));
        assert_eq!(3, b.select(2));
        assert_eq!(5, b.select(3));
    }

    fn half_empty1<BV: BitVector>(new: fn(&[bool]) -> BV) {
        // try 000111
        let b = new(&[false, false, false, true, true, true]);
        assert_eq!(0, b.rank(0));
        assert_eq!(0, b.rank(1));
        assert_eq!(0, b.rank(2));
        assert_eq!(0, b.rank(3));
        assert_eq!(1, b.rank(4));
        assert_eq!(2, b.rank(5));
        assert_eq!(3, b.rank(6));

        assert_eq!(0, b.select(0));
        assert_eq!(4, b.select(1));
        assert_eq!(5, b.select(2));
        assert_eq!(6, b.select(3));
    }

    macro_rules! test_BitVector {
        ($name:ident, $BV:tt) => {
            mod $name {
                use super::*;

                #[test]
                fn simple_evens() {
                    super::simple_evens($BV::new);
                }

                #[test]
                fn simple_odds() {
                    super::simple_odds($BV::new);
                }

                #[test]
                fn half_empty1() {
                    super::half_empty1($BV::new);
                }
            }
        };
    }

    test_BitVector!(reference, ReferenceBitVector);
}
