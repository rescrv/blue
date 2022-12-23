use std::alloc::{alloc_zeroed, dealloc, handle_alloc_error, Layout};
use std::slice::{from_raw_parts, from_raw_parts_mut};

////////////////////////////////////////////// layout //////////////////////////////////////////////

fn layout(sz: usize) -> Layout {
    if sz > isize::max_value() as usize {
        panic!("cannot create buffer bigger than {}", isize::max_value());
    }
    match Layout::from_size_align(sz, 1) {
        Ok(layout) => { layout },
        Err(e) => {
            panic!("cannot create layout: {}", e);
        },
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
        let ptr = unsafe {
            alloc_zeroed(layout)
        };
        if ptr.is_null() {
            handle_alloc_error(layout);
        }
        Buffer {
            ptr,
            sz,
        }
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

impl Drop for Buffer {
    fn drop(&mut self) {
        let layout = layout(self.sz);
        unsafe {
            dealloc(self.ptr, layout);
        }
    }
}

impl From<Vec<u8>> for Buffer {
    fn from(v: Vec<u8>) -> Self {
        let mut buf = Self::new(v.len());
        let bytes = buf.as_slice_mut();
        for i in 0..v.len() {
            bytes[i] = v[i];
        }
        buf
    }
}

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
        let mut buffer = Buffer::new(42);
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
    fn from_vec_u8() {
        let value: Vec<u8> = vec![1, 2, 3];
        let buf: Buffer  = value.into();
        let bytes: &[u8] = buf.as_slice();
        assert_eq!(3, bytes.len());
        assert_eq!(1, bytes[0]);
        assert_eq!(2, bytes[1]);
        assert_eq!(3, bytes[2]);
    }
}
