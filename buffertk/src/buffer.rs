#![allow(clippy::len_without_is_empty)]

use std::alloc::{alloc, dealloc, handle_alloc_error, Layout};
use std::cmp;
use std::fmt;
use std::slice::{from_raw_parts, from_raw_parts_mut};

////////////////////////////////////////////// Buffer //////////////////////////////////////////////

pub struct Buffer {
    ptr: *mut u8,
    sz: usize,
}

impl Buffer {
    fn layout(sz: usize) -> Layout {
        assert!(sz <= isize::max_value() as usize);
        Layout::from_size_align(sz, 1).expect("invalid layout?")
    }

    pub fn new(sz: usize) -> Self {
        let layout = Buffer::layout(sz);
        let ptr = unsafe { alloc(layout) };
        if ptr.is_null() {
            handle_alloc_error(layout);
        }
        Buffer { ptr, sz }
    }

    pub fn len(&self) -> usize {
        self.sz
    }

    pub fn as_bytes(&self) -> &[u8] {
        unsafe { from_raw_parts(self.ptr, self.sz) }
    }

    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        unsafe { from_raw_parts_mut(self.ptr, self.sz) }
    }
}

impl Clone for Buffer {
    fn clone(&self) -> Self {
        self.as_ref().into()
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new(0)
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe {
            dealloc(self.ptr, Buffer::layout(self.sz));
        }
    }
}

impl fmt::Debug for Buffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.as_bytes())
    }
}

//////////////////////////////////////////// AsRef/AsMut ///////////////////////////////////////////

impl AsRef<[u8]> for Buffer {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl AsMut<[u8]> for Buffer {
    fn as_mut(&mut self) -> &mut [u8] {
        self.as_bytes_mut()
    }
}

///////////////////////////////////////////// From/Into ////////////////////////////////////////////

impl From<&[u8]> for Buffer {
    fn from(x: &[u8]) -> Self {
        let mut buf = Self::new(x.len());
        buf.as_bytes_mut().copy_from_slice(x);
        buf
    }
}

impl From<Vec<u8>> for Buffer {
    fn from(v: Vec<u8>) -> Self {
        let mut buf = Self::new(v.len());
        buf.as_bytes_mut().copy_from_slice(&v);
        buf
    }
}

impl From<&Vec<u8>> for Buffer {
    fn from(v: &Vec<u8>) -> Self {
        let mut buf = Self::new(v.len());
        buf.as_bytes_mut().copy_from_slice(v);
        buf
    }
}

impl From<&str> for Buffer {
    fn from(s: &str) -> Self {
        let sbytes = s.as_bytes();
        let mut buf = Self::new(sbytes.len());
        buf.as_bytes_mut().copy_from_slice(sbytes);
        buf
    }
}

//////////////////////////////////////////// Comparisons ///////////////////////////////////////////

impl Eq for Buffer {}

impl PartialEq for Buffer {
    fn eq(&self, rhs: &Buffer) -> bool {
        self.cmp(rhs) == std::cmp::Ordering::Equal
    }
}

impl Ord for Buffer {
    fn cmp(&self, rhs: &Buffer) -> std::cmp::Ordering {
        let buf_lhs: &[u8] = self.as_bytes();
        let buf_rhs: &[u8] = rhs.as_bytes();
        compare_bytes(buf_lhs, buf_rhs)
    }
}

impl PartialOrd for Buffer {
    fn partial_cmp(&self, rhs: &Buffer) -> Option<std::cmp::Ordering> {
        Some(self.cmp(rhs))
    }
}

/////////////////////////////////////////// compare_bytes //////////////////////////////////////////

// Content under CC By-Sa.  I just use as is, as can you.
// https://codereview.stackexchange.com/questions/233872/writing-slice-compare-in-a-more-compact-way
pub fn compare_bytes(a: &[u8], b: &[u8]) -> cmp::Ordering {
    for (ai, bi) in a.iter().zip(b.iter()) {
        match ai.cmp(bi) {
            cmp::Ordering::Equal => continue,
            ord => return ord,
        }
    }

    /* if every single element was equal, compare length */
    a.len().cmp(&b.len())
}
// End borrowed code

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_works() {
        let _ = Buffer::layout(0);
        let _ = Buffer::layout(1);
        let _ = Buffer::layout(isize::max_value() as usize);
    }

    #[test]
    #[should_panic]
    fn layout_panic() {
        let sz: usize = isize::max_value() as usize + 1;
        let _ = Buffer::layout(sz);
    }

    #[test]
    fn empty() {
        let mut buffer = Buffer::new(0);
        for byte in buffer.as_bytes_mut().iter_mut() {
            *byte = 1;
        }
        let mut sum = 0;
        for byte in buffer.as_bytes().iter() {
            sum += *byte;
        }
        assert_eq!(0, sum);
    }

    #[test]
    fn forty_two() {
        let mut buffer = Buffer::new(42);
        for byte in buffer.as_bytes_mut().iter_mut() {
            *byte = 1;
        }
        let mut sum = 0;
        for byte in buffer.as_bytes().iter() {
            sum += *byte;
        }
        assert_eq!(42, sum);
    }

    #[test]
    fn from_vec_u8() {
        let value: Vec<u8> = vec![1, 2, 3];
        let buf: Buffer = value.into();
        let bytes: &[u8] = buf.as_bytes();
        assert_eq!(3, bytes.len());
        assert_eq!(1, bytes[0]);
        assert_eq!(2, bytes[1]);
        assert_eq!(3, bytes[2]);
    }

    #[test]
    fn from_ref_vec_u8() {
        let value: &Vec<u8> = &vec![1, 2, 3];
        let buf: Buffer = value.into();
        let bytes: &[u8] = buf.as_bytes();
        assert_eq!(3, bytes.len());
        assert_eq!(1, bytes[0]);
        assert_eq!(2, bytes[1]);
        assert_eq!(3, bytes[2]);
    }

    #[test]
    fn from_ref_str() {
        let value: &str = "123";
        let buf: Buffer = value.into();
        let bytes: &[u8] = buf.as_bytes();
        assert_eq!(3, bytes.len());
        assert_eq!('1' as u8, bytes[0]);
        assert_eq!('2' as u8, bytes[1]);
        assert_eq!('3' as u8, bytes[2]);
    }
}
