use std::alloc::{alloc_zeroed, dealloc, handle_alloc_error, Layout};
use std::fmt;
use std::slice::{from_raw_parts, from_raw_parts_mut};

use super::compare_bytes;

////////////////////////////////////////////// layout //////////////////////////////////////////////

fn layout(sz: usize) -> Layout {
    if sz > isize::max_value() as usize {
        panic!("cannot create buffer bigger than {}", isize::max_value());
    }
    match Layout::from_size_align(sz, 1) {
        Ok(layout) => layout,
        Err(e) => {
            panic!("cannot create layout: {}", e);
        }
    }
}

////////////////////////////////////////////// Buffer //////////////////////////////////////////////

pub struct Buffer {
    ptr: *mut u8,
    sz: usize,
}

impl Buffer {
    pub fn new(sz: usize) -> Self {
        let layout = layout(sz);
        let ptr = unsafe { alloc_zeroed(layout) };
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

impl Drop for Buffer {
    fn drop(&mut self) {
        let layout = layout(self.sz);
        unsafe {
            dealloc(self.ptr, layout);
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
        let bytes = buf.as_bytes_mut();
        for i in 0..x.len() {
            bytes[i] = x[i];
        }
        buf
    }
}

impl From<Vec<u8>> for Buffer {
    fn from(v: Vec<u8>) -> Self {
        let mut buf = Self::new(v.len());
        let bytes = buf.as_bytes_mut();
        for i in 0..v.len() {
            bytes[i] = v[i];
        }
        buf
    }
}

impl From<&Vec<u8>> for Buffer {
    fn from(v: &Vec<u8>) -> Self {
        let mut buf = Self::new(v.len());
        let bytes = buf.as_bytes_mut();
        for i in 0..v.len() {
            bytes[i] = v[i];
        }
        buf
    }
}

impl From<&str> for Buffer {
    fn from(s: &str) -> Self {
        let sbytes = s.as_bytes();
        let mut buf = Self::new(sbytes.len());
        let bytes = buf.as_bytes_mut();
        for i in 0..sbytes.len() {
            bytes[i] = sbytes[i];
        }
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

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_works() {
        let _ = layout(0);
        let _ = layout(1);
        let _ = layout(isize::max_value() as usize);
    }

    #[test]
    #[should_panic]
    fn layout_panic() {
        let sz: usize = isize::max_value() as usize + 1;
        let _ = layout(sz);
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
