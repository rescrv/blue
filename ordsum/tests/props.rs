use ordsum::{Ordsum, ORDSUM_PRIME};

use num_bigint::BigUint;
use proptest::prelude::*;

fn checksum(items: &[Vec<u8>]) -> Ordsum {
    let mut c = Ordsum::default();
    for item in items {
        c.push(item);
    }
    c
}

fn items() -> impl Strategy<Value = Vec<Vec<u8>>> {
    prop::collection::vec(prop::collection::vec(any::<u8>(), 0..16), 0..24)
}

proptest! {
    // Homomorphism: checksum(A ++ B) == checksum(A).concat(checksum(B)),
    // for every split point of every stream.
    #[test]
    fn concat_is_homomorphic(stream in items(), split in any::<prop::sample::Index>()) {
        let split = split.index(stream.len() + 1);
        let whole = checksum(&stream);
        let left = checksum(&stream[..split]);
        let right = checksum(&stream[split..]);
        prop_assert_eq!(whole, left.concat(&right));
    }

    // Associativity over a three-way split (tree reduction correctness).
    #[test]
    fn concat_is_associative(stream in items(),
                             i in any::<prop::sample::Index>(),
                             j in any::<prop::sample::Index>()) {
        let mut i = i.index(stream.len() + 1);
        let mut j = j.index(stream.len() + 1);
        if i > j { std::mem::swap(&mut i, &mut j); }
        let (x, y, z) = (checksum(&stream[..i]), checksum(&stream[i..j]), checksum(&stream[j..]));
        prop_assert_eq!(x.concat(&y).concat(&z), x.concat(&y.concat(&z)));
    }

    // Identity and inverse.
    #[test]
    fn group_laws(stream in items()) {
        let c = checksum(&stream);
        let e = Ordsum::default();
        prop_assert_eq!(c, c.concat(&e));
        prop_assert_eq!(c, e.concat(&c));
        prop_assert_eq!(e, c.concat(&c.inverse()));
        prop_assert_eq!(e, c.inverse().concat(&c));
    }

    // Prefix/suffix removal recovers the counterpart.
    #[test]
    fn prefix_suffix(stream in items(), split in any::<prop::sample::Index>()) {
        let split = split.index(stream.len() + 1);
        let whole = checksum(&stream);
        let left = checksum(&stream[..split]);
        let right = checksum(&stream[split..]);
        prop_assert_eq!(right, whole.remove_prefix(&left));
        prop_assert_eq!(left, whole.remove_suffix(&right));
    }

    // Digest round-trips and stays canonical.
    #[test]
    fn digest_roundtrip(stream in items()) {
        let c = checksum(&stream);
        prop_assert_eq!(Some(c), Ordsum::from_digest(c.digest()));
        prop_assert_eq!(Some(c), Ordsum::from_hexdigest(&c.hexdigest()));
    }

    // Swapping two distinct adjacent items changes the checksum.
    #[test]
    fn adjacent_swap_detected(mut stream in prop::collection::vec(prop::collection::vec(any::<u8>(), 0..16), 2..24),
                              at in any::<prop::sample::Index>()) {
        let at = at.index(stream.len() - 1);
        prop_assume!(stream[at] != stream[at + 1]);
        let before = checksum(&stream);
        stream.swap(at, at + 1);
        prop_assert_ne!(before, checksum(&stream));
    }
}

// Differential tests of the internal field arithmetic against num-bigint,
// exercised through the public API: the B component of a two-item checksum is
// a1*b0 + b1 mod p, which covers mulmod's full input range via hash outputs.
// For direct coverage of the arithmetic we recompute a stream's state with
// BigUint from the items' hashes.
mod differential {
    use super::*;
    use sha3::{Digest, Sha3_256};

    fn p() -> BigUint {
        BigUint::from(ORDSUM_PRIME)
    }

    fn item_map(item: &[u8]) -> (BigUint, BigUint) {
        let d: [u8; 32] = Sha3_256::digest(item).into();
        let a_raw = BigUint::from(u128::from_le_bytes(d[..16].try_into().unwrap()));
        let b_raw = BigUint::from(u128::from_le_bytes(d[16..].try_into().unwrap()));
        let mut a = a_raw % p();
        if a == BigUint::from(0u8) {
            a = BigUint::from(1u8);
        }
        (a, b_raw % p())
    }

    proptest! {
        #[test]
        fn state_matches_bigint(stream in items()) {
            let mut a_ref = BigUint::from(1u8);
            let mut b_ref = BigUint::from(0u8);
            for item in &stream {
                let (a, b) = item_map(item);
                b_ref = (&a * b_ref + b) % p();
                a_ref = (&a * a_ref) % p();
            }
            let c = checksum(&stream);
            let d = c.digest();
            let a_got = BigUint::from(u128::from_le_bytes(d[..16].try_into().unwrap()));
            let b_got = BigUint::from(u128::from_le_bytes(d[16..].try_into().unwrap()));
            prop_assert_eq!(a_ref, a_got);
            prop_assert_eq!(b_ref, b_got);
        }
    }
}

mod diagnosis {
    use super::*;
    use ordsum::Diagnosis;

    proptest! {
        // Comparing a checksum with itself, or with an independently
        // recomputed checksum of the same stream, is Equal.
        #[test]
        fn identical_streams_are_equal(stream in items()) {
            let x = checksum(&stream);
            let y = checksum(&stream);
            prop_assert_eq!(Diagnosis::Equal, x.diagnose(&y));
        }

        // A genuine permutation of the stream (same items, order actually
        // changed) diagnoses as Reordered.
        #[test]
        fn permutation_is_reordered(stream in prop::collection::vec(prop::collection::vec(any::<u8>(), 0..16), 2..24),
                                    seed in any::<u64>()) {
            // Fisher-Yates with a splitmix64-ish PRNG; deterministic per seed.
            let mut permuted = stream.clone();
            let mut s = seed;
            for i in (1..permuted.len()).rev() {
                s = s.wrapping_add(0x9e3779b97f4a7c15);
                let mut z = s;
                z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
                z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
                z ^= z >> 31;
                permuted.swap(i, (z as usize) % (i + 1));
            }
            prop_assume!(permuted != stream);
            let diag = checksum(&stream).diagnose(&checksum(&permuted));
            prop_assert_eq!(Diagnosis::Reordered, diag);
        }

        // Changing the multiset -- replace an item with a different one,
        // drop an item, or duplicate an item -- diagnoses as Divergent.
        #[test]
        fn multiset_change_is_divergent(stream in prop::collection::vec(prop::collection::vec(any::<u8>(), 0..16), 1..24),
                                        at in any::<prop::sample::Index>(),
                                        replacement in prop::collection::vec(any::<u8>(), 0..16),
                                        kind in 0u8..3) {
            let at = at.index(stream.len());
            let mut mutated = stream.clone();
            match kind {
                0 => {
                    prop_assume!(stream[at] != replacement);
                    mutated[at] = replacement;
                }
                1 => { mutated.remove(at); }
                _ => { let dup = mutated[at].clone(); mutated.insert(at, dup); }
            }
            let diag = checksum(&stream).diagnose(&checksum(&mutated));
            prop_assert_eq!(Diagnosis::Divergent, diag);
        }

        // diagnose is symmetric.
        #[test]
        fn diagnose_is_symmetric(x in items(), y in items()) {
            let (x, y) = (checksum(&x), checksum(&y));
            prop_assert_eq!(x.diagnose(&y), y.diagnose(&x));
        }
    }
}
