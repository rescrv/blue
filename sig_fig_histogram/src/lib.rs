#![doc = include_str!("../README.md")]

use std::io::{BufRead, BufReader, Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};

/////////////////////////////////////////////// Error //////////////////////////////////////////////

/// Error captures the error conditions of a histogram.
#[derive(Copy, Clone, Debug)]
pub enum Error {
    /// If the histogram has limited capacity and an observation exceeds said capacity, it will
    /// fail and return ExceedsMax.
    ExceedsMax,
    /// If an observation is negative, this error condition will result.
    IsNegative,
}

///////////////////////////////////////// SigFigBucketizer /////////////////////////////////////////

/// SigFigBucketizer provides methods for computing bucket boundaries and the bucket to which a
/// value belongs.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct SigFigBucketizer {
    sig_figs: i32,
    buckets: i32,
    offset: i32,
}

impl SigFigBucketizer {
    /// Create a new SigFigBucketizer.
    ///
    /// # Panics
    ///
    /// If sig_figs < 1 or sig_figs > 4, this function will panic.
    pub const fn new(sig_figs: i32) -> Self {
        assert!(sig_figs > 0);
        assert!(sig_figs < 5);
        let buckets = [0, 9, 90, 900, 9000][sig_figs as usize];
        let offset = [0, 1, 10, 100, 1000][sig_figs as usize];
        Self {
            sig_figs,
            buckets,
            offset,
        }
    }

    /// Compute the value all observations in bucket b round to.
    ///
    /// # Panics
    ///
    /// If the boundary b is negative.
    pub fn boundary_for(&self, b: i32) -> f64 {
        assert!(b >= 0);
        let x = b / self.buckets;
        let y = b % self.buckets;
        (y + self.offset) as f64 * 10.0_f64.powi(x - self.sig_figs + 1)
    }

    /// Compute the bucket for the value.
    ///
    /// # Panics
    ///
    /// If the observation x is negative.
    pub fn bucket_for(&self, x: f64) -> usize {
        assert!(x >= 0.0);
        let offset = self.offset as f64;
        let buckets = self.buckets as f64;
        let trunc = x.log10().trunc().round();
        let exponent = 10.0_f64.powi(trunc as i32);
        (x * offset / exponent + buckets * trunc - offset).round() as usize
    }

    /// Iterate over the value assigned to each bucket.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..i32::MAX).map(|idx| self.boundary_for(idx))
    }
}

///////////////////////////////////////////// Histogram ////////////////////////////////////////////

/// A basic Histogram type.
#[derive(Clone, Debug)]
pub struct Histogram {
    sfb: SigFigBucketizer,
    buckets: Vec<u64>,
}

impl Histogram {
    /// Create a new histogram with the specified number of sig_figs.
    ///
    /// # Panics
    ///
    /// Under the same conditions as [SigFigBucketizer::new].
    pub const fn new(sig_figs: i32) -> Self {
        let sfb = SigFigBucketizer::new(sig_figs);
        let buckets = vec![];
        Self { sfb, buckets }
    }

    /// Return the nubmer of significant figures in use for this histogram.
    pub fn sig_figs(&self) -> i32 {
        self.sfb.sig_figs
    }

    /// Observe a value x and increment the bucket for x.
    ///
    /// This will never fail with [Error::ExceedsMax] because it will resize.
    pub fn observe(&mut self, x: f64) -> Result<(), Error> {
        self.observe_n(x, 1)
    }

    /// Observe a value x and increment its bucket n times.
    ///
    /// This will never fail with [Error::ExceedsMax] because it will resize.
    pub fn observe_n(&mut self, x: f64, n: u64) -> Result<(), Error> {
        if x >= 0.0 {
            let bucket = self.sfb.bucket_for(x);
            if self.buckets.len() <= bucket {
                self.buckets.resize(bucket + 1, 0);
            }
            self.buckets[bucket] = self.buckets[bucket].wrapping_add(n);
            Ok(())
        } else {
            Err(Error::IsNegative)
        }
    }

    /// Return an iterator over this bucket.
    pub fn iter(&self) -> impl Iterator<Item = (f64, u64)> + '_ {
        std::iter::zip(self.sfb.iter(), self.buckets.iter().copied())
    }

    /// Dump a histogram to the specified writer.
    pub fn dump<W: Write>(&self, mut w: W) -> Result<(), std::io::Error> {
        writeln!(w, "{}", self.sfb.sig_figs)?;
        for (_, bucket) in self.iter() {
            writeln!(w, "{bucket}")?;
        }
        Ok(())
    }

    /// Load a histogram from the specified reader.
    pub fn load<R: Read>(r: R) -> Result<Self, std::io::Error> {
        let mut lines = BufReader::new(r).lines();
        let Ok(Some(sig_figs)) = lines.next().transpose() else {
            return Err(std::io::Error::other("missing sig figs value"));
        };
        let Ok(sig_figs) = sig_figs.parse::<i32>() else {
            return Err(std::io::Error::other("could not parse sig figs"));
        };
        if !(1..=4).contains(&sig_figs) {
            return Err(std::io::Error::other("sig figs out of bounds"));
        }
        let mut buckets = vec![];
        while let Some(bucket) = lines.next().transpose()? {
            let Ok(bucket) = bucket.parse::<u64>() else {
                return Err(std::io::Error::other("could not parse bucket"));
            };
            buckets.push(bucket);
        }
        let sfb = SigFigBucketizer::new(sig_figs);
        Ok(Histogram { sfb, buckets })
    }

    /// Create a new histogram downsampled to the specified number of significant figures.
    ///
    /// # Panics
    ///
    /// If sig_figs < 1 or sig_figs > 4 or sig_figs > self.sig_figs().
    pub fn downsample(&self, sig_figs: i32) -> Self {
        assert!(sig_figs > 0);
        assert!(sig_figs < 5);
        assert!(sig_figs <= self.sfb.sig_figs);
        let mut histogram = Self::new(sig_figs);
        for (idx, (_, count)) in self.iter().enumerate() {
            let boundary = self.sfb.boundary_for(idx as i32);
            histogram
                .observe_n(boundary, count)
                .expect("downsampling should never fail here");
        }
        histogram
    }

    /// Merge two histograms without loss of precision.
    ///
    /// # Panics
    ///
    /// If the signficant figures are different between the histograms.
    pub fn merge(one: &Self, two: &Self) -> Self {
        assert_eq!(one.sig_figs(), two.sig_figs());
        let mut three = Self {
            sfb: one.sfb,
            buckets: vec![0; std::cmp::max(one.buckets.len(), two.buckets.len())],
        };
        for (idx, (_, bucket)) in one.iter().enumerate() {
            three.buckets[idx] += bucket;
        }
        for (idx, (_, bucket)) in two.iter().enumerate() {
            three.buckets[idx] += bucket;
        }
        three
    }
}

///////////////////////////////////////// LockFreeHistogram ////////////////////////////////////////

/// A LockFreeHistogram.  This trades the ability to resize to accomodate new observations as
/// Histogram does in exchange for providing a concurrent, lock-free histogram.
pub struct LockFreeHistogram<const N: usize> {
    sfb: SigFigBucketizer,
    buckets: [AtomicU64; N],
}

impl<const N: usize> LockFreeHistogram<N> {
    /// Create a new lock-free histogram with the specified number of sig_figs.
    ///
    /// # Panics
    ///
    /// Under the same conditions as [SigFigBucketizer::new].
    pub const fn new(sig_figs: i32) -> Self {
        let sfb = SigFigBucketizer::new(sig_figs);
        let buckets = [0u64; N];
        // SAFETY(rescrv):  Everything is aligned and of the same size.
        let buckets: [AtomicU64; N] = unsafe { std::mem::transmute_copy(&buckets) };
        Self { sfb, buckets }
    }

    /// Return the nubmer of significant figures in use for this histogram.
    pub fn sig_figs(&self) -> i32 {
        self.sfb.sig_figs
    }

    /// Observe a value x and increment the bucket for x.
    ///
    /// This will fail with [Error::ExceedsMax] if the bucket exceeds template parameter N.
    pub fn observe(&self, x: f64) -> Result<(), Error> {
        self.observe_n(x, 1)
    }

    /// Observe a value x and increment its bucket n times.
    ///
    /// This will fail with [Error::ExceedsMax] if the bucket exceeds template parameter N.
    pub fn observe_n(&self, x: f64, n: u64) -> Result<(), Error> {
        if x >= 0.0 {
            let bucket = self.sfb.bucket_for(x);
            if bucket < N {
                self.buckets[bucket].fetch_add(n, Ordering::Relaxed);
                Ok(())
            } else {
                Err(Error::ExceedsMax)
            }
        } else {
            Err(Error::IsNegative)
        }
    }

    /// Return an iterator over this bucket.
    pub fn iter(&self) -> impl Iterator<Item = u64> + '_ {
        LockFreeHistogramIterator {
            hist: self,
            index: 0,
        }
    }

    /// Create a [Histogram] from this histogram.
    pub fn to_histogram(&self) -> Histogram {
        let sfb = self.sfb;
        let buckets = self.iter().collect::<Vec<u64>>();
        Histogram { sfb, buckets }
    }
}

///////////////////////////////////// LockFreeHistogramIterator ////////////////////////////////////

struct LockFreeHistogramIterator<'a, const N: usize> {
    hist: &'a LockFreeHistogram<N>,
    index: usize,
}

impl<const N: usize> Iterator for LockFreeHistogramIterator<'_, N> {
    type Item = u64;

    fn next(&mut self) -> Option<u64> {
        if self.index < N {
            let val = self.hist.buckets[self.index].load(Ordering::Relaxed);
            self.index += 1;
            Some(val)
        } else {
            None
        }
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    #[test]
    fn dump_load() {
        let mut h = super::Histogram::new(2);
        let _ = h.observe(std::f64::consts::PI);
        let mut s = Vec::new();
        h.dump(&mut s).unwrap();
        const EXPECTED: &str =
            "2\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n1\n";
        assert_eq!(EXPECTED, std::str::from_utf8(&s).unwrap());
        let got = super::Histogram::load(EXPECTED.as_bytes()).unwrap();
        assert_eq!(h.sfb, got.sfb);
        assert_eq!(h.buckets, got.buckets);
    }

    #[test]
    fn downsample() {
        let mut h = super::Histogram::new(3);
        let _ = h.observe(std::f64::consts::PI);
        let mut s = Vec::new();
        h.dump(&mut s).unwrap();
        const EXPECTED3: &str = "3\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n1\n";
        assert_eq!(EXPECTED3, std::str::from_utf8(&s).unwrap());
        // Now downsample.
        let h = h.downsample(2);
        let mut s = Vec::new();
        h.dump(&mut s).unwrap();
        const EXPECTED2: &str =
            "2\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n0\n1\n";
        assert_eq!(EXPECTED2, std::str::from_utf8(&s).unwrap());
    }
}
