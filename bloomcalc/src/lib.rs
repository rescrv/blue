#![doc = include_str!("../README.md")]

pub trait Parameter {
    fn new(x: f64) -> Self;
}

const LN2_2: f64 = 0.4804530139182014; // ln^2 2

/// N is the number of items expected to be inserted into the bloom filter.
#[derive(Clone, Copy)]
pub struct N(pub f64);

impl Parameter for N {
    fn new(x: f64) -> N {
        N(x)
    }
}

/// P is the probability of false positives.
#[derive(Clone, Copy)]
pub struct P(pub f64);

impl Parameter for P {
    fn new(x: f64) -> P {
        P(x)
    }
}

/// M is the number of bits to use in the bloom filter.
#[derive(Clone, Copy)]
pub struct M(pub f64);

impl Parameter for M {
    fn new(x: f64) -> M {
        M(x)
    }
}

/// K is the number of keys to insert/hash.
#[derive(Clone, Copy)]
pub struct K(pub f64);

impl Parameter for K {
    fn new(x: f64) -> K {
        K(x)
    }
}

/////////////////////////////////////////// Calculations ///////////////////////////////////////////

/// Given the probability of false positive, compute the necessary number of keys.
pub fn calc_keys_given_probability(p: P) -> K {
    K(0.0f64 - p.0.log2())
}

/// Given the probability of false positive and number of elements, compute the number of bits.
pub fn calc_m_given_p_n(p: P, n: N) -> M {
    M(0.0 - n.0 * p.0.ln() / LN2_2)
}

/// Given the number of elements and number of bits, compute the probability of false positive.
pub fn calc_p_given_n_m(n: N, m: M) -> P {
    let e = std::f64::consts::E;
    P(e.powf(0.0 - LN2_2 * m.0 / n.0))
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    struct TestParams {
        n: N,
        p: P,
        m: M,
        k: K,
    }

    const TEST_PARAMS: &[TestParams] = &[
        TestParams {
            n: N(100.0),
            p: P(0.01),
            m: M(958.505837736744),
            k: K(6.643856189774724),
        },
        TestParams {
            n: N(1000.0),
            p: P(0.001),
            m: M(14377.58756605116),
            k: K(9.965784284662087),
        },
        TestParams {
            n: N(2718281.0),
            p: P(0.0314159),
            m: M(19578296.19763294),
            k: K(4.99236127889529),
        },
    ];

    fn approximately_correct(expected: f64, returned: f64) -> bool {
        returned * 0.99 < expected && returned * 1.01 > expected
    }

    #[test]
    fn ln2_2() {
        let expect = 2.0f64.ln().powf(2.0);
        assert!(approximately_correct(expect, LN2_2));
    }

    #[test]
    fn test_keys_given_probability() {
        for param in TEST_PARAMS.iter() {
            let k = calc_keys_given_probability(param.p);
            assert!(approximately_correct(param.k.0, k.0));
        }
    }

    #[test]
    fn test_m_given_p_n() {
        for param in TEST_PARAMS.iter() {
            let m = calc_m_given_p_n(param.p, param.n);
            assert!(approximately_correct(param.m.0, m.0));
        }
    }

    #[test]
    fn test_p_given_n_m() {
        for param in TEST_PARAMS.iter() {
            let p = calc_p_given_n_m(param.n, param.m);
            assert!(approximately_correct(param.p.0, p.0));
        }
    }
}
