use std::sync::atomic::{AtomicU64, Ordering};

use biometrics::click;

////////////////////////////////////////////// Vector //////////////////////////////////////////////

pub struct Vector {
    elem_bits: usize,
    num_elems: usize,
    backing: Vec<AtomicU64>,
}

impl Vector {
    pub fn new(elem_bits: usize, num_elems: usize) -> Self {
        assert!(elem_bits <= 63);
        let total_bits = num_elems
            .checked_mul(elem_bits)
            .expect("tried to create too large a vector");
        let num_u64s = total_bits / 64 + 2;
        let mut backing = Vec::with_capacity(num_u64s);
        backing.resize_with(num_u64s, AtomicU64::default);
        Self {
            elem_bits,
            num_elems,
            backing,
        }
    }

    pub fn len(&self) -> usize {
        self.num_elems
    }

    pub fn load(&self, idx: usize) -> u64 {
        let (index, lower_shift, lower_mask, upper_mask, upper_shift) = self.shifts_and_masks(idx);
        let lower = self.backing[index].load(Ordering::Relaxed);
        let upper = self.backing[index + 1].load(Ordering::Relaxed);
        ((lower >> lower_shift) & lower_mask) | ((upper & upper_mask) << upper_shift)
    }

    pub fn increment(&self, idx: usize) {
        let (index, lower_shift, _, _, _) = self.shifts_and_masks(idx);
        loop {
            let lower = self.backing[index].load(Ordering::Relaxed);
            let upper = self.backing[index + 1].load(Ordering::Relaxed);
            let new_lower = lower.wrapping_add(1u64 << lower_shift);
            let new_upper = if new_lower < lower {
                upper.wrapping_add(1)
            } else {
                upper
            };
            if !self.compare_and_swap(index, lower, new_lower) {
                continue;
            }
            if upper == new_upper {
                break;
            }
            if !self.compare_and_swap(index + 1, upper, new_upper) {
                click!("tiny_lfu.increment_collision");
            }
            break;
        }
    }

    pub fn divide_two(&self, idx: usize) {
        let (index, lower_shift, lower_mask, upper_mask, _) = self.shifts_and_masks(idx);
        let lower_shifted_mask = lower_mask << lower_shift;
        loop {
            let lower = self.backing[index].load(Ordering::Relaxed);
            let upper = self.backing[index + 1].load(Ordering::Relaxed);
            let new_lower = (((lower & lower_shifted_mask) >> 1) & lower_shifted_mask)
                | (lower & !lower_shifted_mask)
                | ((upper & 0x1 & upper_mask) << 63);
            let new_upper = ((upper & upper_mask) >> 1) | (upper & !upper_mask);
            if !self.compare_and_swap(index + 1, upper, new_upper) {
                continue;
            }
            if !self.compare_and_swap(index, lower, new_lower) {
                click!("tiny_lfu.divide_two_collision");
            }
            break;
        }
    }

    #[inline(always)]
    fn shifts_and_masks(&self, index: usize) -> (usize, u64, u64, u64, u64) {
        let bit_index = index * self.elem_bits;
        let u64_index = bit_index / 64;
        let lower_shift = (bit_index - u64_index * 64) as u64;
        let lower_bits = if 64 - lower_shift < self.elem_bits as u64 {
            64 - lower_shift
        } else {
            self.elem_bits as u64
        };
        let lower_mask = (1u64 << lower_bits) - 1;
        let upper_mask = (1u64 << (self.elem_bits as u64 - lower_bits)) - 1;
        let upper_shift = if lower_bits == self.elem_bits as u64 {
            0
        } else {
            lower_bits
        };
        (u64_index, lower_shift, lower_mask, upper_mask, upper_shift)
    }

    #[inline(always)]
    fn compare_and_swap(&self, index: usize, current: u64, new: u64) -> bool {
        let ordering = Ordering::Relaxed;
        match self.backing[index].compare_exchange(current, new, ordering, ordering) {
            Ok(x) => x == current,
            Err(x) => x == current,
        }
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new() {
        let v = Vector::new(24, 47);
        assert_eq!(24, v.elem_bits);
        assert_eq!(47, v.num_elems);
        assert!(v.backing.len() >= 18);
    }

    #[test]
    fn load() {
        let v = Vector::new(24, 5);
        v.backing[0].store(0xdeadbeefcafe1eafu64, Ordering::Relaxed);
        v.backing[1].store(0xc0ffeef00dc0ffeeu64, Ordering::Relaxed);
        assert_eq!(0xfe1eafu64, v.load(0));
        assert_eq!(0xbeefcau64, v.load(1));
        assert_eq!(0xeedeadu64, v.load(2));
        assert_eq!(0x0dc0ffu64, v.load(3));
        assert_eq!(0xffeef0u64, v.load(4));
    }

    #[test]
    fn increment() {
        let v = Vector::new(24, 5);
        v.backing[0].store(0xdeadbeefcafe1eafu64, Ordering::Relaxed);
        v.backing[1].store(0xc0ffeef00dc0ffeeu64, Ordering::Relaxed);
        v.increment(0);
        assert_eq!(0xdeadbeefcafe1eb0u64, v.backing[0].load(Ordering::Relaxed));
        assert_eq!(0xc0ffeef00dc0ffeeu64, v.backing[1].load(Ordering::Relaxed));
        v.increment(1);
        assert_eq!(0xdeadbeefcbfe1eb0u64, v.backing[0].load(Ordering::Relaxed));
        assert_eq!(0xc0ffeef00dc0ffeeu64, v.backing[1].load(Ordering::Relaxed));
        v.increment(2);
        assert_eq!(0xdeaebeefcbfe1eb0u64, v.backing[0].load(Ordering::Relaxed));
        assert_eq!(0xc0ffeef00dc0ffeeu64, v.backing[1].load(Ordering::Relaxed));
        v.increment(3);
        assert_eq!(0xdeaebeefcbfe1eb0u64, v.backing[0].load(Ordering::Relaxed));
        assert_eq!(0xc0ffeef00dc100eeu64, v.backing[1].load(Ordering::Relaxed));
        v.increment(4);
        assert_eq!(0xdeaebeefcbfe1eb0u64, v.backing[0].load(Ordering::Relaxed));
        assert_eq!(0xc0ffeef10dc100eeu64, v.backing[1].load(Ordering::Relaxed));
    }

    #[test]
    fn divide_two() {
        let v = Vector::new(24, 5);
        v.backing[0].store(0xdeadbeefcafe1eafu64, Ordering::Relaxed);
        v.backing[1].store(0xc0ffeef00dc0ffeeu64, Ordering::Relaxed);
        v.divide_two(0);
        assert_eq!(0xdeadbeefca7f0f57u64, v.backing[0].load(Ordering::Relaxed));
        assert_eq!(0xc0ffeef00dc0ffeeu64, v.backing[1].load(Ordering::Relaxed));
        v.divide_two(1);
        assert_eq!(0xdead5f77e57f0f57u64, v.backing[0].load(Ordering::Relaxed));
        assert_eq!(0xc0ffeef00dc0ffeeu64, v.backing[1].load(Ordering::Relaxed));
        v.divide_two(2);
        assert_eq!(0x6f565f77e57f0f57u64, v.backing[0].load(Ordering::Relaxed));
        assert_eq!(0xc0ffeef00dc0ff77u64, v.backing[1].load(Ordering::Relaxed));
        v.divide_two(3);
        assert_eq!(0x6f565f77e57f0f57u64, v.backing[0].load(Ordering::Relaxed));
        assert_eq!(0xc0ffeef006e07f77u64, v.backing[1].load(Ordering::Relaxed));
        v.divide_two(4);
        assert_eq!(0x6f565f77e57f0f57u64, v.backing[0].load(Ordering::Relaxed));
        assert_eq!(0xc07ff77806e07f77u64, v.backing[1].load(Ordering::Relaxed));
    }

    #[test]
    fn shifts_and_masks() {
        let v = Vector::new(24, 5);
        v.backing[0].store(0xdeadbeefcafe1eafu64, Ordering::Relaxed);
        v.backing[1].store(0xc0ffeef00dc0ffeeu64, Ordering::Relaxed);
        let (index, lower_shift, lower_mask, upper_mask, upper_shift) = v.shifts_and_masks(0);
        assert_eq!(0, index);
        assert_eq!(0, lower_shift);
        assert_eq!(0xffffffu64, lower_mask);
        assert_eq!(0, upper_mask);
        assert_eq!(0, upper_shift);
        let (index, lower_shift, lower_mask, upper_mask, upper_shift) = v.shifts_and_masks(1);
        assert_eq!(0, index);
        assert_eq!(24, lower_shift);
        assert_eq!(0xffffffu64, lower_mask);
        assert_eq!(0, upper_mask);
        assert_eq!(0, upper_shift);
        let (index, lower_shift, lower_mask, upper_mask, upper_shift) = v.shifts_and_masks(2);
        assert_eq!(0, index);
        assert_eq!(48, lower_shift);
        assert_eq!(0xffffu64, lower_mask);
        assert_eq!(0xffu64, upper_mask);
        assert_eq!(16, upper_shift);
        let (index, lower_shift, lower_mask, upper_mask, upper_shift) = v.shifts_and_masks(3);
        assert_eq!(1, index);
        assert_eq!(8, lower_shift);
        assert_eq!(0xffffffu64, lower_mask);
        assert_eq!(0, upper_mask);
        assert_eq!(0, upper_shift);
        let (index, lower_shift, lower_mask, upper_mask, upper_shift) = v.shifts_and_masks(4);
        assert_eq!(1, index);
        assert_eq!(32, lower_shift);
        assert_eq!(0xffffffu64, lower_mask);
        assert_eq!(0, upper_mask);
        assert_eq!(0, upper_shift);
    }
}
