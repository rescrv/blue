use std::cmp::Ordering;

///////////////////////////////////////////// Centroid /////////////////////////////////////////////

#[derive(Clone, Debug, Default)]
struct Centroid {
    sum: f64,
    count: u64,
}

impl Eq for Centroid {}

impl Ord for Centroid {
    fn cmp(&self, rhs: &Self) -> Ordering {
        self.sum
            .partial_cmp(&rhs.sum)
            .unwrap_or(Ordering::Equal)
            .then(self.count.cmp(&rhs.count).reverse())
    }
}

impl PartialEq for Centroid {
    fn eq(&self, rhs: &Self) -> bool {
        self.cmp(rhs) == Ordering::Equal
    }
}

impl PartialOrd for Centroid {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}

////////////////////////////////////////////// TDigest /////////////////////////////////////////////

#[derive(Clone, Debug)]
pub struct TDigest {
    delta: u64,
    centroids: Vec<Centroid>,
    buffer: Vec<f64>,
}

impl TDigest {
    pub const fn new(delta: u64) -> Self {
        Self {
            delta,
            centroids: Vec::new(),
            buffer: Vec::new(),
        }
    }

    pub fn from_points(delta: u64, points: Vec<f64>) -> Self {
        let centroids = TDigest::centroids_from_points(points);
        Self {
            delta,
            centroids,
            buffer: Vec::new(),
        }
    }

    pub fn add(&mut self, point: f64) {
        let mut compact = Vec::new();
        self.buffer.push(point);
        if self.buffer.len() > self.delta as usize {
            std::mem::swap(&mut compact, &mut self.buffer);
        }
        if !compact.is_empty() {
            let to_merge = TDigest::centroids_from_points(compact);
            TDigest::merge_centroids(self.delta, &mut self.centroids, &to_merge);
        }
    }

    fn centroids_from_points(points: Vec<f64>) -> Vec<Centroid> {
        let mut centroids = Vec::new();
        for point in points {
            if point.is_normal() || (0.0 - f64::EPSILON < point && f64::EPSILON > point) {
                centroids.push(Centroid {
                    sum: point,
                    count: 1,
                });
            }
        }
        centroids.sort();
        centroids
    }

    #[allow(non_snake_case)]
    fn merge_centroids(delta: u64, into: &mut Vec<Centroid>, from: &[Centroid]) {
        for point in from.iter() {
            into.push(point.clone());
        }
        into.sort();
        let S: f64 = into.iter().map(|x| x.count as f64).sum();
        let mut C = Vec::new();
        let mut q0 = 0.0;
        let mut q_limit = TDigest::inverse_scale(TDigest::scale(q0, delta) + 1.0, delta);
        let mut sigma = Centroid::default();
        for point in into.iter() {
            let q = q0 + (sigma.count + point.count) as f64 / S;
            if q <= q_limit || sigma.count == 0 {
                sigma.sum += point.sum;
                sigma.count += point.count;
            } else {
                q0 += sigma.count as f64 / S;
                q_limit = TDigest::inverse_scale(TDigest::scale(q0, delta) + 1.0, delta);
                C.push(sigma);
                sigma = point.clone();
            }
        }
        std::mem::swap(&mut C, into);
    }

    // z, scale, inverse_scale are provided under this license:
    // https://github.com/tdunning/t-digest/blob/main/core/src/main/java/com/tdunning/math/stats/ScaleFunction.java
    /*
     * Licensed to Ted Dunning under one or more
     * contributor license agreements.  See the NOTICE file distributed with
     * this work for additional information regarding copyright ownership.
     * The ASF licenses this file to You under the Apache License, Version 2.0
     * (the "License"); you may not use this file except in compliance with
     * the License.  You may obtain a copy of the License at
     *
     *     http://www.apache.org/licenses/LICENSE-2.0
     *
     * Unless required by applicable law or agreed to in writing, software
     * distributed under the License is distributed on an "AS IS" BASIS,
     * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
     * See the License for the specific language governing permissions and
     * limitations under the License.
     */

    fn z(delta: u64, n: u64) -> f64 {
        4.0 * (n as f64 / delta as f64).ln() + 21.0
    }

    fn scale(q: f64, delta: u64) -> f64 {
        let q = if q < 1e-15 {
            1e-15
        } else if q >= 1.0 - 1e-15 {
            1.0 - 1e-15
        } else {
            q
        };
        if q <= 0.5 {
            let z = TDigest::z(delta, 1e10 as u64);
            delta as f64 * (2.0 * q).ln() / z
        } else {
            let z = TDigest::z(delta, 1e10 as u64);
            0.0 - delta as f64 * (2.0 * (1.0 - q)).ln() / z
        }
    }

    fn inverse_scale(q_inv: f64, delta: u64) -> f64 {
        if q_inv <= 0.0 {
            let z = TDigest::z(delta, 1e10 as u64);
            let x = q_inv * z / delta as f64;
            x.exp() / 2.0
        } else {
            1.0 - TDigest::inverse_scale(0.0 - q_inv, delta)
        }
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tan_atan() {
        assert_eq!(0.1, 0.1_f64.tan().atan());
        assert_eq!(0.2, 0.2_f64.tan().atan());
        assert_eq!(0.3, 0.3_f64.tan().atan());
        assert_eq!(0.4, 0.4_f64.tan().atan());
        assert_eq!(0.5, 0.5_f64.tan().atan());
        assert_eq!(0.6, 0.6_f64.tan().atan());
        assert_eq!(0.7, 0.7_f64.tan().atan());
        assert_eq!(0.8, 0.8_f64.tan().atan());
        assert_eq!(0.9, 0.9_f64.tan().atan());
    }

    #[test]
    fn scale_function() {
        let k = TDigest::scale(0.0, 1);
        assert!(-0.299 > k && -0.300 < k);

        let k = TDigest::scale(0.5, 1);
        assert!(0.001 > k && -0.001 < k);

        let k = TDigest::scale(1.0, 1);
        assert!(0.300 > k && 0.299 < k);
    }

    #[test]
    fn inverse_function() {
        const NOT_EPSILON: f64 = 10e-9;
        let k = TDigest::scale(0.0, 1);
        let q = TDigest::inverse_scale(k, 1);
        assert!(0.0 - NOT_EPSILON < q && 0.0 + NOT_EPSILON > q);

        let k = TDigest::scale(0.5, 1);
        let q = TDigest::inverse_scale(k, 1);
        assert!(0.5 - NOT_EPSILON < q && 0.5 + NOT_EPSILON > q);

        let k = TDigest::scale(1.0, 1);
        let q = TDigest::inverse_scale(k, 1);
        assert!(1.0 - NOT_EPSILON < q && 1.0 + NOT_EPSILON > q);
    }

    // TODO(rescrv): Test other deltas.
}
