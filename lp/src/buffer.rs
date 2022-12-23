use std::convert::TryFrom;
use std::alloc::{alloc_zeroed, dealloc, Layout};
use std::slice::{from_raw_parts, from_raw_parts_mut};

use super::Error;

////////////////////////////////////////////// layout //////////////////////////////////////////////

fn layout(sz: usize) -> Result<Layout, Error> {
    match Layout::from_size_align(sz, 1) {
        Ok(layout) => { Ok(layout) },
        Err(e) => { Err(Error::LogicError {
            context: format!("layout failed: {}", e),
        }) },
    }
}

////////////////////////////////////////////// Buffer //////////////////////////////////////////////

pub struct Buffer {
    ptr: *mut u8,
    sz: usize,
}

impl Buffer {
    pub fn new(sz: usize) -> Result<Self, Error> {
        let layout = layout(sz)?;
        let ptr = unsafe {
            alloc_zeroed(layout)
        };
        if ptr.is_null() {
            return Err(Error::MemoryAllocationFailed);
        }
        Ok(Buffer {
            ptr,
            sz,
        })
    }

    pub fn len(&self) -> usize {
        self.sz
    }

    pub fn as_slice(&self) -> &[u8] {
        unsafe {
            from_raw_parts(self.ptr, self.sz)
        }
    }

    pub fn as_slice_mut(&mut self) -> &mut [u8] {
        unsafe {
            from_raw_parts_mut(self.ptr, self.sz)
        }
    }
}

impl TryFrom<Vec<u8>> for Buffer {
    type Error = Error;

    fn try_from(v: Vec<u8>) -> Result<Self, Self::Error> {
        let mut buf = Self::new(v.len())?;
        let bytes = buf.as_slice_mut();
        for i in 0..v.len() {
            bytes[i] = v[i];
        }
        Ok(buf)
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        let layout = match layout(self.sz) {
            Ok(layout) => { layout },
            Err(_) => { return; },
        };
        unsafe {
            dealloc(self.ptr, layout);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty() {
        let mut buffer = Buffer::new(0).unwrap();
        for byte in buffer.as_slice_mut().iter_mut() {
            *byte = 1;
        }
        let mut sum = 0;
        for byte in buffer.as_slice().iter() {
            sum += *byte;
        }
        assert_eq!(0, sum);
    }

    #[test]
    fn forty_two() {
        let mut buffer = Buffer::new(42).unwrap();
        for byte in buffer.as_slice_mut().iter_mut() {
            *byte = 1;
        }
        let mut sum = 0;
        for byte in buffer.as_slice().iter() {
            sum += *byte;
        }
        assert_eq!(42, sum);
    }

    #[test]
    fn try_from_vec_u8() {
        let value: Vec<u8> = vec![1, 2, 3];
        let buf: Buffer  = value.try_into().unwrap();
        let bytes: &[u8] = buf.as_slice();
        assert_eq!(3, bytes.len());
        assert_eq!(1, bytes[0]);
        assert_eq!(2, bytes[1]);
        assert_eq!(3, bytes[2]);
    }
}
