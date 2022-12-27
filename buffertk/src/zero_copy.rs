use std::fmt::Debug;
use std::marker::{PhantomData, PhantomPinned};
use std::mem::transmute;
use std::pin::Pin;
use std::ptr::NonNull;
use std::rc::Rc;

use super::{Buffer, Unpackable, Unpacker};

////////////////////////////////////////////// Backing /////////////////////////////////////////////

pub trait Backing: Debug {
    fn as_bytes(&self) -> &[u8];
}

impl Backing for &[u8] {
    fn as_bytes(&self) -> &[u8] {
        self
    }
}

impl Backing for Vec<u8> {
    fn as_bytes(&self) -> &[u8] {
        self
    }
}

impl Backing for Rc<Vec<u8>> {
    fn as_bytes(&self) -> &[u8] {
        self.as_ref()
    }
}

impl Backing for Buffer {
    fn as_bytes(&self) -> &[u8] {
        Buffer::as_bytes(self)
    }
}

impl Backing for Rc<Buffer> {
    fn as_bytes(&self) -> &[u8] {
        Buffer::as_bytes(self.as_ref())
    }
}

///////////////////////////////////////// ZeroCopyUnpacker /////////////////////////////////////////

#[derive(Debug)]
pub struct ZeroCopyUnpacker<'a, B, E, V>
where
    B: Backing,
    V: Unpackable<'a, Error=E> + 'a,
{
    value: NonNull<V>,
    backing: B,
    _pinned: PhantomPinned,
    _data: PhantomData<&'a mut u8>,
}

impl<'a, B, E, V> ZeroCopyUnpacker<'a, B, E, V>
where
    B: Backing,
    V: Unpackable<'a, Error=E> + 'a,
{
    pub fn new(backing: B) -> Result<Pin<Box<Self>>, E> {
        let zcu = ZeroCopyUnpacker {
            value: NonNull::dangling(),
            backing,
            _pinned: PhantomPinned,
            _data: PhantomData,
        };
        let mut boxed = Box::pin(zcu);
        let buf: &[u8] = boxed.backing.as_bytes();
        let buf: &'a [u8] = unsafe { transmute(buf) };
        let mut up: Unpacker<'a> = Unpacker::new(buf);
        let value: V = up.unpack()?;
        unsafe {
            boxed.as_mut().get_unchecked_mut().value =
                NonNull::new(Box::into_raw(Box::new(value))).unwrap();
        }
        Ok(boxed)
    }

    pub fn wrapped_value(self: Pin<&Self>) -> &'a V {
        unsafe {
            &*self.get_ref().value.as_ptr()
        }
    }
}

impl<'a, B, E, V> Drop for ZeroCopyUnpacker<'a, B, E, V>
where
    B: Backing,
    V: Unpackable<'a, Error=E> + 'a,
{
    fn drop(&mut self) {
        unsafe {
            if self.value != NonNull::dangling() {
                drop(Box::from_raw(NonNull::as_ptr(self.value)));
            }
        }
        self.value = NonNull::dangling();
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::super::{stack_pack, Error, Packable, Unpackable, Unpacker};
    use super::*;

    struct TwoSlices<'a> {
        x: &'a [u8],
        y: &'a [u8],
    }

    impl<'a> Packable for TwoSlices<'a> {
        fn pack_sz(&self) -> usize {
            self.x.pack_sz() + self.y.pack_sz()
        }

        fn pack(&self, out: &mut [u8]) {
            let div = self.x.pack_sz();
            self.x.pack(&mut out[..div]);
            self.y.pack(&mut out[div..]);
        }
    }

    impl<'a> Unpackable<'a> for TwoSlices<'a> {
        type Error = Error;

        fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
            let mut up = Unpacker::new(buf);
            let x: &[u8] = up.unpack()?;
            let y: &[u8] = up.unpack()?;
            Ok((Self { x, y }, up.remain()))
        }
    }

    const X: &[u8] = &[0, 1, 2, 3, 4, 5, 6, 7];
    const Y: &[u8] = &[8, 9, 10, 11, 12, 13, 14, 15];

    #[test]
    fn two_slices_struct() {
        let exp: &[u8] = &[8, 0, 1, 2, 3, 4, 5, 6, 7, 8, 8, 9, 10, 11, 12, 13, 14, 15];
        let got: &[u8] = &stack_pack(TwoSlices { x: X, y: Y }).to_vec();
        assert_eq!(exp, got);
        let mut up = Unpacker::new(exp);
        let ts: TwoSlices = up.unpack().expect("unpack TwoSlices");
        assert_eq!(X, ts.x);
        assert_eq!(Y, ts.y);
    }

    #[test]
    fn two_slices() {
        let exp: &[u8] = &[8, 0, 1, 2, 3, 4, 5, 6, 7, 8, 8, 9, 10, 11, 12, 13, 14, 15];
        let zcu: Pin<Box<ZeroCopyUnpacker<&[u8], Error, TwoSlices>>> = ZeroCopyUnpacker::new(exp).unwrap();
        let zcu1: &TwoSlices = zcu.as_ref().wrapped_value();
        assert_eq!(X, zcu1.x);
        assert_eq!(Y, zcu1.y);
        let zcu2: &TwoSlices = zcu.as_ref().wrapped_value();
        assert_eq!(X, zcu2.x);
        assert_eq!(Y, zcu2.y);
    }
}
