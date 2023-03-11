use crate::bit_vector::BitVector;

// TODO(rescrv): this should inherit bitvector
pub trait Dictionary<V> {
    fn new(d: &[(usize, V)]) -> Self;

    fn lookup(&self, x: usize) -> &V;

    fn rank(&self, x: usize) -> usize;
    fn select(&self, x: usize) -> usize;

    fn rankup(&self, x: usize) -> (usize, &V) {
        (self.rank(x), self.lookup(x))
    }

    fn selectup(&self, s: usize) -> (usize, &V) {
        let x = self.select(s);
        (x, self.lookup(x))
    }
}

pub struct ReferenceDictionary<B, V>
where
    B: BitVector,
{
    offset: usize,
    keys: B,
    values: Vec<V>,
}

impl<B, V> Dictionary<V> for ReferenceDictionary<B, V>
where
    B: BitVector,
    V: Copy,
{
    fn new(d: &[(usize, V)]) -> Self {
        assert!(d.len() > 0);
        let offset = d[0].0;
        let mut values: Vec<V> = Vec::new();
        for (_, v) in d.iter() {
            values.push(*v);
        }
        let mut keys: Vec<usize> = Vec::new();
        for i in 1..d.len() {
            assert!(d[i - 1].0 < d[i].0);
            keys.push(d[i].0 - 1 - offset);
        }
        ReferenceDictionary {
            offset,
            keys: B::sparse(&keys),
            values: values,
        }
    }

    fn lookup(&self, x: usize) -> &V {
        assert!(self.offset <= x);
        &self.values[self.rank(x)]
    }

    fn rank(&self, x: usize) -> usize {
        assert!(self.offset <= x);
        self.keys.rank(x - self.offset)
    }

    fn select(&self, x: usize) -> usize {
        self.keys.select(x) + self.offset
    }
}

#[cfg(test)]
mod tests {
    use crate::bit_vector::ReferenceBitVector;

    use super::*;

    fn panic_if_empty<D: Dictionary<u64>>(new: fn(&[(usize, u64)]) -> D) {
        new(&[]);
    }

    fn panic_if_not_sorted<D: Dictionary<u64>>(new: fn(&[(usize, u64)]) -> D) {
        new(&[(0, 50), (2, 135), (1, 85)]);
    }

    fn panic_if_rank_too_high<D: Dictionary<u64>>(new: fn(&[(usize, u64)]) -> D) {
        let d = new(&[(1, 35)]);
        d.lookup(2);
    }

    fn panic_if_rank_too_low<D: Dictionary<u64>>(new: fn(&[(usize, u64)]) -> D) {
        let d = new(&[(1, 35)]);
        d.lookup(0);
    }

    fn simple_zero<D: Dictionary<u64>>(new: fn(&[(usize, u64)]) -> D) {
        let d = new(&[(0, 35), (1, 50), (3, 85), (6, 70), (10, 200)]);
        assert_eq!(35, *d.lookup(0));
        assert_eq!(50, *d.lookup(1));
        assert_eq!(50, *d.lookup(2));
        assert_eq!(85, *d.lookup(3));
        assert_eq!(85, *d.lookup(4));
        assert_eq!(85, *d.lookup(5));
        assert_eq!(70, *d.lookup(6));
        assert_eq!(70, *d.lookup(7));
        assert_eq!(70, *d.lookup(8));
        assert_eq!(70, *d.lookup(9));
        assert_eq!(200, *d.lookup(10));
    }

    fn simple_offset<D: Dictionary<u64>>(new: fn(&[(usize, u64)]) -> D) {
        let d = new(&[(1, 35), (2, 50), (4, 85), (8, 70), (16, 200)]);
        assert_eq!(35, *d.lookup(1));
        assert_eq!(50, *d.lookup(2));
        assert_eq!(50, *d.lookup(3));
        assert_eq!(85, *d.lookup(4));
        assert_eq!(85, *d.lookup(5));
        assert_eq!(85, *d.lookup(6));
        assert_eq!(85, *d.lookup(7));
        assert_eq!(70, *d.lookup(8));
        assert_eq!(70, *d.lookup(9));
        assert_eq!(70, *d.lookup(10));
        assert_eq!(70, *d.lookup(11));
        assert_eq!(70, *d.lookup(12));
        assert_eq!(70, *d.lookup(13));
        assert_eq!(70, *d.lookup(14));
        assert_eq!(70, *d.lookup(15));
        assert_eq!(200, *d.lookup(16));
    }

    fn bug_selectup_rankup_1<D: Dictionary<u64>>(new: fn(&[(usize, u64)]) -> D) {
        const INPUT: &[(usize, u64)] = &[
            (2, 2),
            (3, 3),
            (5, 5),
            (6, 6),
            (7, 7),
            (8, 8),
            (10, 10),
            (12, 12),
        ];
        let dict = new(INPUT);
        for i in 0..INPUT.len() {
            let (select, &expect) = dict.selectup(i);
            let expect = expect as usize;
            assert_eq!(expect, select);
            assert_eq!(INPUT[i], (select, expect as u64));
        }
    }

    macro_rules! test_Dictionary {
        ($name:ident, $D:tt) => {
            mod $name {
                use super::*;

                #[test]
                #[should_panic]
                fn panic_if_empty() {
                    super::panic_if_empty($D::new);
                }

                #[test]
                #[should_panic]
                fn panic_if_not_sorted() {
                    super::panic_if_not_sorted($D::new);
                }

                #[test]
                #[should_panic]
                fn panic_if_rank_too_high() {
                    super::panic_if_rank_too_high($D::new);
                }

                #[test]
                #[should_panic]
                fn panic_if_rank_too_low() {
                    super::panic_if_rank_too_low($D::new);
                }

                #[test]
                fn simple_zero() {
                    super::simple_zero($D::new);
                }

                #[test]
                fn simple_offset() {
                    super::simple_offset($D::new);
                }

                #[test]
                fn bug_selectup_rankup_1() {
                    super::bug_selectup_rankup_1($D::new);
                }
            }
        };
    }

    type TestReferenceDictionary = ReferenceDictionary<ReferenceBitVector, u64>;
    test_Dictionary!(reference, TestReferenceDictionary);
}
