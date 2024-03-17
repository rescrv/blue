use std::cmp::Ordering;

/// Binary search over the inclusive range [first, last] and return the index of the first element
/// for which search returns ordering of greater or equal.  Will never evaluate search(last).
pub fn binary_search_by<F: FnMut(usize) -> Ordering>(
    first: usize,
    last: usize,
    mut search: F,
) -> usize {
    let mut left = first;
    let mut right = last;
    while left < right {
        let mid = left + (right - left) / 2;
        let cmp = search(mid);
        if cmp == Ordering::Less {
            left = mid + 1;
        } else if cmp == Ordering::Greater {
            right = mid;
        } else {
            return mid;
        }
    }
    left
}

/// Return the first index in [first, last] for which the first element returns false.  Assumes
/// that the list is partitioned such that the first n elements are true and the last m are false.
pub fn partition_by<F: FnMut(usize) -> bool>(first: usize, last: usize, mut search: F) -> usize {
    binary_search_by(first, last, move |probe| {
        if search(probe) {
            Ordering::Less
        } else {
            Ordering::Greater
        }
    })
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_ranges() {
        assert_eq!(1, binary_search_by(1, 0, |_probe| Ordering::Less));
        assert_eq!(2, binary_search_by(2, 1, |_probe| Ordering::Less));
    }

    #[test]
    fn one_element_ranges() {
        assert_eq!(1, binary_search_by(1, 1, |probe| probe.cmp(&0)));
        assert_eq!(1, binary_search_by(1, 1, |probe| probe.cmp(&1)));
        assert_eq!(1, binary_search_by(1, 1, |probe| probe.cmp(&2)));
        assert_eq!(2, binary_search_by(2, 2, |probe| probe.cmp(&1)));
        assert_eq!(2, binary_search_by(2, 2, |probe| probe.cmp(&2)));
        assert_eq!(2, binary_search_by(2, 2, |probe| probe.cmp(&3)));
    }

    #[test]
    fn two_element_ranges() {
        assert_eq!(1, binary_search_by(1, 2, |probe| probe.cmp(&0)));
        assert_eq!(1, binary_search_by(1, 2, |probe| probe.cmp(&1)));
        assert_eq!(2, binary_search_by(1, 2, |probe| probe.cmp(&2)));
        assert_eq!(2, binary_search_by(1, 2, |probe| probe.cmp(&3)));
    }

    #[test]
    fn partitioning() {
        let slice = &[0, 1, 1, 1, 1, 2, 3, 5];
        assert_eq!(0, partition_by(0, 8, |x| slice[x] < 0));
        assert_eq!(1, partition_by(0, 8, |x| slice[x] <= 0));
        assert_eq!(1, partition_by(0, 8, |x| slice[x] < 1));
        assert_eq!(5, partition_by(0, 8, |x| slice[x] <= 1));
        assert_eq!(5, partition_by(0, 8, |x| slice[x] < 2));
        assert_eq!(6, partition_by(0, 8, |x| slice[x] <= 2));
        assert_eq!(6, partition_by(0, 8, |x| slice[x] < 3));
        assert_eq!(7, partition_by(0, 8, |x| slice[x] <= 3));
        assert_eq!(7, partition_by(0, 8, |x| slice[x] < 4));
        assert_eq!(7, partition_by(0, 8, |x| slice[x] <= 4));
        assert_eq!(7, partition_by(0, 8, |x| slice[x] < 5));
        assert_eq!(8, partition_by(0, 8, |x| slice[x] <= 5));
        assert_eq!(8, partition_by(0, 8, |x| slice[x] < 6));
        assert_eq!(8, partition_by(0, 8, |x| slice[x] <= 6));
    }

    proptest::prop_compose! {
        pub fn arb_elements()(mut elements in proptest::collection::vec(proptest::arbitrary::any::<usize>(), 0..64)) -> Vec<usize> {
            elements.sort();
            elements.dedup();
            elements
        }
    }

    proptest::proptest! {
        #[test]
        fn binary_search(elements in arb_elements()) {
            for i in 0..elements.len() {
                let expected = elements.partition_point(|x| *x < elements[i]);
                assert_eq!(expected, i);
                let returned = binary_search_by(0, elements.len() - 1, |x| elements[x].cmp(&elements[i]));
                assert_eq!(expected, returned, "elements={:?} needle={}", elements, elements[i]);
            }
        }
    }
}
