#![doc = include_str!("../README.md")]

use std::fmt::Debug;

mod varint;

pub use varint::v64;

/////////////////////////////////////////////// Error //////////////////////////////////////////////

/// All Error conditions within `buffertk`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    /// BufferTooShort indicates that there was a need to pack or unpack more bytes than were
    /// available in the underlying memory.
    BufferTooShort {
        /// Number of bytes required to read the buffer.
        required: usize,
        /// Number of bytes available to read.
        had: usize,
    },
    /// VarintOverflow indicates that a varint field did not terminate with a number < 128.
    VarintOverflow {
        /// Number of bytes in the varint.
        bytes: usize,
    },
    /// UnsignedOverflow indicates that a value will not fit its intended (unsigned) target.
    UnsignedOverflow {
        /// Value that would overflow (typically a u32).
        value: u64,
    },
    /// SignedOverflow indicates that a value will not fit its intended (signed) target.
    SignedOverflow {
        /// Value that would overflow (typically an i32).
        value: i64,
    },
    /// TagTooLarge indicates the tag would overflow a 32-bit number.
    TagTooLarge {
        /// Value that's too large for a tag.
        tag: u64,
    },
    /// UnknownDiscriminant indicates a variant that is not understood by this code.
    UnknownDiscriminant {
        /// Discriminant that's not known.
        discriminant: u32,
    },
    /// NotAChar indicates that the prescribed value was tried to unpack as a char, but it's not a
    /// char.
    NotAChar {
        /// Value that's not a char.
        value: u32,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::BufferTooShort { required, had } => fmt
                .debug_struct("BufferTooShort")
                .field("required", required)
                .field("had", had)
                .finish(),
            Error::VarintOverflow { bytes } => fmt
                .debug_struct("VarintOverflow")
                .field("bytes", bytes)
                .finish(),
            Error::UnsignedOverflow { value } => fmt
                .debug_struct("UnsignedOverflow")
                .field("value", value)
                .finish(),
            Error::SignedOverflow { value } => fmt
                .debug_struct("SignedOverflow")
                .field("value", value)
                .finish(),
            Error::TagTooLarge { tag } => {
                fmt.debug_struct("TagTooLarge").field("tag", tag).finish()
            }
            Error::UnknownDiscriminant { discriminant } => fmt
                .debug_struct("UnknownDiscriminant")
                .field("discriminant", discriminant)
                .finish(),
            Error::NotAChar { value } => {
                fmt.debug_struct("NotAChar").field("value", value).finish()
            }
        }
    }
}

///////////////////////////////////////////// Packable /////////////////////////////////////////////

/// Packable objects can be serialized into an `&mut [u8]`.
///
/// The actual serialized form of the object is left unspecified by the Packable trait.  
///
/// Packable objects should avoid interior mutability to the extent necessary to ensure that anyone
/// holding a immutable reference can assume the packed output will not change for the duration of
/// the reference.
pub trait Packable {
    /// `pack_sz` returns the number of bytes required to serialize the Packable object.
    fn pack_sz(&self) -> usize;
    /// `pack` fills in the buffer `out` with the packed binary representation of the Packable
    /// object.  The implementor is responsible to ensure that `out` is exactly `pack_sz()` bytes
    /// and implementations are encouraged to assert this.
    ///
    /// The call to pack should never fail.  Good Rust practices dictate that objects should use
    /// the type system to enforce their well-formed-ness.  Consequently, we will assume here that
    /// any well-formed Packable that can be represented will serialize successfully.  If there is
    /// a need to represent a state that cannot exist, it should be done using a different type
    /// that does not implement Packable.
    ///
    /// # Panics
    ///
    /// - When `out.len() != self.pack_sz()`
    fn pack(&self, out: &mut [u8]);
    /// `stream` writes the object to the provided writer using the same representation that would
    /// be used in a call to `pack`.  The implementor is responsible for making sure that the
    /// number of bytes written is exactly equal to the number of required bytes.
    ///
    /// A default implementation is provided that will pack a vector and then write said vector to
    /// the file with `write_all`.
    fn stream<W: std::io::Write>(&self, writer: &mut W) -> Result<usize, std::io::Error>
    where
        Self: std::marker::Sized,
        for<'a> &'a Self: Packable,
    {
        let pa = stack_pack(self);
        let vec: Vec<u8> = pa.to_vec();
        match writer.write_all(&vec) {
            Ok(_) => Ok(vec.len()),
            Err(x) => Err(x),
        }
    }
}

//////////////////////////////////////////// Unpackable ////////////////////////////////////////////

/// Unpackable objects can be deserialized from an `&[u8]`.
///
/// The format understood by `T:Unpackable` must correspond to the format serialized by
/// `T:Packable`.
pub trait Unpackable<'a>: Sized {
    /// Type of error this unpackable returns.
    type Error: Debug;

    /// `unpack` attempts to return an Unpackable object stored in a prefix of `buf`.  The method
    /// returns the result and remaining unused buffer.
    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Self::Error>;
}

//////////////////////////////////////////// pack_helper ///////////////////////////////////////////

/// `pack_helper` takes a Packable object and an `&mut [u8]` and does the work to serialize the
/// packable into a prefix of the buffer.  The return value is the portion of the buffer that
/// remains unfilled after this operation.
pub fn pack_helper<T: Packable>(t: T, buf: &mut [u8]) -> &mut [u8] {
    let sz: usize = t.pack_sz();
    assert!(sz <= buf.len(), "packers should never be given short space");
    t.pack(&mut buf[..sz]);
    &mut buf[sz..]
}

//////////////////////////////////////////// StackPacker ///////////////////////////////////////////

const EMPTY: () = ();

/// `stack_pack` begins a tree of packable data on the stack.
pub fn stack_pack<'a, T: Packable + 'a>(t: T) -> StackPacker<'a, (), T> {
    StackPacker { prefix: &EMPTY, t }
}

/// [StackPacker] is the type returned by StackPack.  It's a pointer to something packable (usually
/// another StackPacker) and some type that we can directly pack.  Both are packable, but it's
/// usually the case that the former is another StackPacker while the latter is the type being
/// serialized in a call to `pack`.
pub struct StackPacker<'a, P, T>
where
    P: Packable + 'a,
    T: Packable + 'a,
{
    prefix: &'a P,
    t: T,
}

impl<'a, P, T> StackPacker<'a, P, T>
where
    P: Packable + 'a,
    T: Packable + 'a,
{
    /// `pack` returns a new StackPacker that will pack `self` at its prefix.  This does not
    /// actually do the packing, but defers it until calls to e.g. `into_slice`.  Consequently, the
    /// object `u` must not change between this call and subsequent calls.  Rust's type system
    /// generally enforces this by default, except where interior mutability is specifically added.
    pub fn pack<'b, U: Packable + 'b>(&'b self, u: U) -> StackPacker<'b, Self, U> {
        StackPacker { prefix: self, t: u }
    }

    /// `into_slice` packs the entire chain of `pack()` calls into the provided mutable buffer.
    /// The return value is a slice containing exactly those bytes written and no more.
    pub fn into_slice<'b>(&self, buf: &'b mut [u8]) -> &'b mut [u8] {
        let len = self.pack_sz();
        assert!(buf.len() >= len);
        let buf = &mut buf[0..len];
        Packable::pack(self, buf);
        buf
    }

    /// `to_vec` allocates a new vector and packs the entire chain of `pack()` calls into it.  The
    /// return value is a `Vec<u8>` sized to exactly the packed bytes.
    pub fn to_vec(&self) -> Vec<u8> {
        let len = self.pack_sz();
        let mut buf = vec![0u8; len];
        Packable::pack(self, &mut buf);
        buf
    }

    /// `append_to_vec` is a helper to extend a vector by the requisite size and then pack into the
    /// newly created space.
    pub fn append_to_vec(&self, v: &mut Vec<u8>) {
        let len = self.pack_sz();
        let v_sz = v.len();
        v.resize(v_sz + len, 0);
        Packable::pack(self, &mut v[v_sz..]);
    }

    /// Create a Packable object that will pack like `"<varint-length><bytes>"` where the length
    /// indicates how many bytes there are.  Nothing gets copied.  Usually this gets passed to
    /// another `stack_pack`, which will do the work.
    pub fn length_prefixed(&'a self) -> LengthPrefixer<'a, StackPacker<'a, P, T>> {
        LengthPrefixer {
            size: self.pack_sz(),
            body: self,
        }
    }
}

impl<'a, P, T> Packable for StackPacker<'a, P, T>
where
    P: Packable + 'a,
    T: Packable + 'a,
{
    fn pack_sz(&self) -> usize {
        self.prefix.pack_sz() + self.t.pack_sz()
    }

    fn pack(&self, out: &mut [u8]) {
        let (prefix, suffix): (&mut [u8], &mut [u8]) = out.split_at_mut(self.prefix.pack_sz());
        self.prefix.pack(prefix);
        self.t.pack(suffix);
    }
}

///////////////////////////////////////////// Unpacker /////////////////////////////////////////////

/// Unpacker parses a buffer start to finish.
#[derive(Clone, Default)]
pub struct Unpacker<'a> {
    buf: &'a [u8],
}

impl<'a> Unpacker<'a> {
    /// Create a new [Unpacker] that parses `buf`.
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf }
    }

    /// Unpack from buf into an object of type T.
    pub fn unpack<'b, E, T: Unpackable<'b, Error = E>>(&mut self) -> Result<T, E>
    where
        'a: 'b,
    {
        let (t, buf): (T, &'a [u8]) = Unpackable::unpack(self.buf)?;
        self.buf = buf;
        Ok(t)
    }

    /// Return true if and only if there's no buffer left to parse.
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Return the remaining buffer.
    pub fn remain(&self) -> &'a [u8] {
        self.buf
    }

    /// Advance the buffer by `by`.  Saturating.
    pub fn advance(&mut self, by: usize) {
        if by > self.buf.len() {
            self.buf = &[];
        } else {
            self.buf = &self.buf[by..];
        }
    }
}

////////////////////////////////////////// Packable for &P /////////////////////////////////////////

impl<P: Packable> Packable for &P {
    fn pack_sz(&self) -> usize {
        (*self).pack_sz()
    }

    fn pack(&self, out: &mut [u8]) {
        (*self).pack(out)
    }
}

////////////////////////////// Packable/Unpackable for sized integers //////////////////////////////

macro_rules! packable_with_to_le_bytes {
    ($what:ty) => {
        impl Packable for $what {
            fn pack_sz(&self) -> usize {
                self.to_le_bytes().len()
            }

            fn pack(&self, out: &mut [u8]) {
                let b = &self.to_le_bytes();
                assert_eq!(b.len(), out.len());
                out.copy_from_slice(b);
            }
        }

        impl<'a> Unpackable<'a> for $what {
            type Error = Error;

            fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
                const SZ: usize = std::mem::size_of::<$what>();
                if buf.len() >= SZ {
                    let mut fbuf: [u8; SZ] = [0; SZ];
                    fbuf.copy_from_slice(&buf[0..SZ]);
                    Ok((<$what>::from_le_bytes(fbuf), &buf[SZ..]))
                } else {
                    Err(Error::BufferTooShort {
                        required: SZ,
                        had: buf.len(),
                    })
                }
            }
        }
    };
}

packable_with_to_le_bytes!(i8);
packable_with_to_le_bytes!(u8);

packable_with_to_le_bytes!(i16);
packable_with_to_le_bytes!(u16);

packable_with_to_le_bytes!(i32);
packable_with_to_le_bytes!(u32);

packable_with_to_le_bytes!(i64);
packable_with_to_le_bytes!(u64);

/////////////////////////////// Packable/Unpackable for sized floats ///////////////////////////////

// NOTE(rescrv): I tried to dedupe this macro from the above.  Whether I made a typo or there's
// something deeper here, I exceeded half the time it takes to copy and dupe the tests, so that's
// the path I'm taking.  Suggestions to dedupe (that do not change the tests) are welcome!
macro_rules! packable_with_to_bits_to_le_bytes {
    ($what:ty, $indirect:ty) => {
        impl Packable for $what {
            fn pack_sz(&self) -> usize {
                self.to_bits().to_le_bytes().len()
            }

            fn pack(&self, out: &mut [u8]) {
                let b = &self.to_bits().to_le_bytes();
                assert_eq!(b.len(), out.len());
                out.copy_from_slice(b);
            }
        }
        impl<'a> Unpackable<'a> for $what {
            type Error = Error;

            fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
                const SZ: usize = std::mem::size_of::<$what>();
                if buf.len() >= SZ {
                    let mut fbuf: [u8; SZ] = [0; SZ];
                    fbuf.copy_from_slice(&buf[0..SZ]);
                    Ok((
                        <$what>::from_bits(<$indirect>::from_le_bytes(fbuf)),
                        &buf[SZ..],
                    ))
                } else {
                    Err(Error::BufferTooShort {
                        required: SZ,
                        had: buf.len(),
                    })
                }
            }
        }
    };
}

packable_with_to_bits_to_le_bytes!(f32, u32);
packable_with_to_bits_to_le_bytes!(f64, u64);

//////////////////////////////////////// Packable/Unpackable ///////////////////////////////////////

impl Packable for char {
    fn pack_sz(&self) -> usize {
        (*self as u32).pack_sz()
    }

    fn pack(&self, out: &mut [u8]) {
        (*self as u32).pack(out)
    }
}

impl<'a> Unpackable<'a> for char {
    type Error = Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(char, &'b [u8]), Error> {
        let (c, buf) = u32::unpack(buf)?;
        if let Some(c) = char::from_u32(c) {
            Ok((c, buf))
        } else {
            Err(Error::NotAChar { value: c })
        }
    }
}

//////////////////////////////////////////// length_free ///////////////////////////////////////////

/// Pack a byte slice without a length prefix.  The resulting format is equivalent to concatenating
/// the individual packings.
pub fn length_free<P: Packable>(slice: &[P]) -> LengthFree<P> {
    LengthFree { slice }
}

/// A type that packs a slice of objects by concatenating their packed representations.  Does not
/// prepend a length.
pub struct LengthFree<'a, P: Packable> {
    slice: &'a [P],
}

impl<P: Packable> Packable for LengthFree<'_, P> {
    fn pack_sz(&self) -> usize {
        self.slice.iter().map(|x| x.pack_sz()).sum()
    }

    fn pack(&self, out: &mut [u8]) {
        let mut out = out;
        for i in 0..self.slice.len() {
            out = pack_helper(&self.slice[i], out);
        }
    }
}

////////////////////////////////////////// LengthPrefixer //////////////////////////////////////////

/// A type that packs a slice of objects by concatenating their packed representations.  Prepends a
/// length.
pub struct LengthPrefixer<'a, P>
where
    P: Packable + 'a,
{
    // memoized body.pack_sz
    size: usize,
    body: &'a P,
}

impl<'a, P> Packable for LengthPrefixer<'a, P>
where
    P: Packable + 'a,
{
    fn pack_sz(&self) -> usize {
        let vsz: v64 = self.size.into();
        vsz.pack_sz() + self.size
    }

    fn pack(&self, out: &mut [u8]) {
        let vsz: v64 = self.size.into();
        let (prefix, suffix): (&mut [u8], &mut [u8]) = out.split_at_mut(vsz.pack_sz());
        vsz.pack(prefix);
        self.body.pack(suffix);
    }
}

/////////////////////////////////////////////// &[u8] //////////////////////////////////////////////

impl Packable for &[u8] {
    fn pack_sz(&self) -> usize {
        let vsz: v64 = self.len().into();
        vsz.pack_sz() + self.len()
    }

    fn pack(&self, out: &mut [u8]) {
        let vsz: v64 = self.len().into();
        let (prefix, suffix): (&mut [u8], &mut [u8]) = out.split_at_mut(vsz.pack_sz());
        vsz.pack(prefix);
        suffix.copy_from_slice(self);
    }
}

impl<'a> Unpackable<'a> for &'a [u8] {
    type Error = Error;

    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let (vsz, buf): (v64, &'b [u8]) = v64::unpack(buf)?;
        let x: usize = vsz.into();
        if x > buf.len() {
            Err(Error::BufferTooShort {
                required: x,
                had: buf.len(),
            })
        } else {
            Ok((&buf[0..x], &buf[x..]))
        }
    }
}

////////////////////////////// Packable/Unpackable for n-tuple, n > 1 //////////////////////////////

macro_rules! impl_pack_unpack_tuple {
    () => {
        impl Packable for () {
            fn pack_sz(&self) -> usize {
                0
            }

            fn pack(&self, _: &mut [u8]) {}
        }

        impl<'a> Unpackable<'a> for () {
            type Error = Error;

            fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
                Ok(((), buf))
            }
        }
    };
    ( $($name:ident)+ ) => {
        #[allow(non_snake_case)]
        impl<$($name: Packable),+> Packable for ($($name,)+) {
            fn pack_sz(&self) -> usize {
                let ($(ref $name,)+) = *self;
                $($name.pack_sz() + )+ /* expansion ended with "+" */ 0
            }

            fn pack(&self, buf: &mut[u8]) {
                let ($(ref $name,)+) = *self;
                let pa = stack_pack(());
                $(let pa = pa.pack($name);)+
                pa.into_slice(buf);
            }
        }

        #[allow(non_snake_case)]
        impl<'a, ER: Debug, $($name: Unpackable<'a, Error=ER> + 'a),+> Unpackable<'a> for ($($name,)+) {
            type Error = ER;

            fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Self::Error> {
                let mut up: Unpacker<'b> = Unpacker::new(buf);
                $(let $name = up.unpack()?;)+
                let rem: &'b [u8] = up.remain();
                Ok((($($name,)+), rem))
            }
        }
    };
}

impl_pack_unpack_tuple! {}

impl_pack_unpack_tuple! { A }
impl_pack_unpack_tuple! { A B }
impl_pack_unpack_tuple! { A B C }
impl_pack_unpack_tuple! { A B C D }
impl_pack_unpack_tuple! { A B C D E }
impl_pack_unpack_tuple! { A B C D E F }
impl_pack_unpack_tuple! { A B C D E F G }
impl_pack_unpack_tuple! { A B C D E F G H }
impl_pack_unpack_tuple! { A B C D E F G H I }
impl_pack_unpack_tuple! { A B C D E F G H I J }
impl_pack_unpack_tuple! { A B C D E F G H I J K }
impl_pack_unpack_tuple! { A B C D E F G H I J K L }
impl_pack_unpack_tuple! { A B C D E F G H I J K L M }
impl_pack_unpack_tuple! { A B C D E F G H I J K L M N }
impl_pack_unpack_tuple! { A B C D E F G H I J K L M N O }
impl_pack_unpack_tuple! { A B C D E F G H I J K L M N O P }
impl_pack_unpack_tuple! { A B C D E F G H I J K L M N O P Q }
impl_pack_unpack_tuple! { A B C D E F G H I J K L M N O P Q R }
impl_pack_unpack_tuple! { A B C D E F G H I J K L M N O P Q R S }
impl_pack_unpack_tuple! { A B C D E F G H I J K L M N O P Q R S T }
impl_pack_unpack_tuple! { A B C D E F G H I J K L M N O P Q R S T U }
impl_pack_unpack_tuple! { A B C D E F G H I J K L M N O P Q R S T U V }
impl_pack_unpack_tuple! { A B C D E F G H I J K L M N O P Q R S T U V W }
impl_pack_unpack_tuple! { A B C D E F G H I J K L M N O P Q R S T U V W X }
impl_pack_unpack_tuple! { A B C D E F G H I J K L M N O P Q R S T U V W X Y }
impl_pack_unpack_tuple! { A B C D E F G H I J K L M N O P Q R S T U V W X Y Z }

////////////////////////////////////////////// Result //////////////////////////////////////////////

impl<T: Packable, E: Packable> Packable for Result<T, E> {
    fn pack_sz(&self) -> usize {
        match self {
            Ok(x) => stack_pack(v64::from(10))
                .pack(v64::from(x.pack_sz()))
                .pack(x)
                .pack_sz(),
            Err(e) => stack_pack(v64::from(18))
                .pack(v64::from(e.pack_sz()))
                .pack(e)
                .pack_sz(),
        }
    }

    fn pack(&self, out: &mut [u8]) {
        match self {
            Ok(x) => {
                stack_pack(v64::from(10))
                    .pack(v64::from(x.pack_sz()))
                    .pack(x)
                    .into_slice(out);
            }
            Err(e) => {
                stack_pack(v64::from(18))
                    .pack(v64::from(e.pack_sz()))
                    .pack(e)
                    .into_slice(out);
            }
        }
    }
}

impl<'a, T, E> Unpackable<'a> for Result<T, E>
where
    T: Unpackable<'a>,
    E: Unpackable<'a>
        + Debug
        + From<Error>
        + From<<T as Unpackable<'a>>::Error>
        + From<<E as Unpackable<'a>>::Error>,
{
    type Error = E;

    fn unpack<'b>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Self::Error>
    where
        'b: 'a,
    {
        let mut up = Unpacker::new(buf);
        let tag: v64 = up.unpack()?;
        if <v64 as Into<u64>>::into(tag) > u32::MAX as u64 {
            return Err(Error::TagTooLarge { tag: tag.into() }.into());
        }
        let tag: u32 = tag.try_into().unwrap();
        match tag {
            10 => {
                let x: v64 = up.unpack()?;
                let buf: &[u8] = &up.remain()[..x.into()];
                up.advance(x.into());
                let (t, _): (T, _) = <T as Unpackable>::unpack(buf)?;
                Ok((Ok(t), up.remain()))
            }
            18 => {
                let x: v64 = up.unpack()?;
                let buf: &[u8] = &up.remain()[..x.into()];
                up.advance(x.into());
                let (e, _): (E, _) = <E as Unpackable>::unpack(buf)?;
                Ok((Err(e), up.remain()))
            }
            _ => Err(Error::UnknownDiscriminant { discriminant: tag }.into()),
        }
    }
}

///////////////////////////////////////////// mod tests ////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_void() {
        let buf: &mut [u8; 0] = &mut [];
        ().pack(buf);
        let mut up = Unpacker::new(buf);
        let x: Result<(), Error> = up.unpack();
        assert_eq!(Ok(()), x, "human got decode wrong?");
        assert_eq!(0, up.buf.len(), "human got remainder wrong?");
    }

    macro_rules! test_pack_with_to_le_bytes {
        ($what:ty, $num:expr, $x:expr, $human:expr) => {{
            const HUMAN: &[u8] = $human;
            const X: $what = $x as $what;
            const LEN: usize = $num;
            let exp = &X.to_le_bytes();
            assert_eq!(HUMAN, exp, "human got test vector wrong?");
            assert_eq!(LEN, exp.len(), "human got test vector wrong?");
            {
                let buf: &mut [u8; LEN] = &mut <[u8; LEN]>::default();
                X.pack(buf);
                assert_eq!(exp, buf, "human got implementation wrong?");
                assert_eq!(HUMAN, buf, "human got test macro wrong?");
            }
            {
                let mut up = Unpacker::new(HUMAN);
                let x = up.unpack();
                let expect: Result<$what, Error> = Ok(X);
                assert_eq!(expect, x, "human got implementation wrong?");
                assert_eq!(0, up.buf.len(), "human got remainder wrong?");
            }
        }};
    }

    #[test]
    fn pack_and_unpack_integers() {
        // don't want human error, so want to automate test
        // don't know how to test the testing automation
        // be a bit verbose and do both, checking all three
        test_pack_with_to_le_bytes!(u8, 1, 0xc0u8, &[0xc0]);
        test_pack_with_to_le_bytes!(i8, 1, 0xc0u8, &[0xc0]);
        test_pack_with_to_le_bytes!(u16, 2, 0xc0ffu16, &[0xff, 0xc0]);
        test_pack_with_to_le_bytes!(i16, 2, 0xc0ffu16, &[0xff, 0xc0]);
        test_pack_with_to_le_bytes!(i32, 4, 0xc0ffeedau32, &[0xda, 0xee, 0xff, 0xc0]);
        test_pack_with_to_le_bytes!(u32, 4, 0xc0ffeedau32, &[0xda, 0xee, 0xff, 0xc0]);
        test_pack_with_to_le_bytes!(
            i64,
            8,
            0xc0ffeeda7e11f00du64,
            &[0x0d, 0xf0, 0x11, 0x7e, 0xda, 0xee, 0xff, 0xc0]
        );
        test_pack_with_to_le_bytes!(
            u64,
            8,
            0xc0ffeeda7e11f00du64,
            &[0x0d, 0xf0, 0x11, 0x7e, 0xda, 0xee, 0xff, 0xc0]
        );
    }

    macro_rules! test_pack_with_to_bits_to_le_bytes {
        ($what:ty, $num:expr, $x:expr, $human:expr) => {{
            const HUMAN: &[u8] = $human;
            const X: $what = $x as $what;
            const LEN: usize = $num;
            let exp = &X.to_bits().to_le_bytes();
            assert_eq!(HUMAN, exp, "human got test vector wrong?");
            assert_eq!(LEN, exp.len(), "human got test vector wrong?");
            {
                let buf: &mut [u8; LEN] = &mut <[u8; LEN]>::default();
                X.pack(buf);
                assert_eq!(exp, buf, "human got implementation wrong?");
                assert_eq!(HUMAN, buf, "human got test macro wrong?");
            }
            {
                let mut up = Unpacker::new(HUMAN);
                let x = up.unpack();
                let expect: Result<$what, Error> = Ok(X);
                assert_eq!(expect, x, "human got implementation wrong?");
                assert_eq!(0, up.buf.len(), "human got remainder wrong?");
            }
        }};
    }

    #[test]
    fn pack_and_unpack_floats() {
        test_pack_with_to_bits_to_le_bytes!(f32, 4, 16711938.0, &[0x02, 0x01, 0x7f, 0x4b]);
        test_pack_with_to_bits_to_le_bytes!(
            f64,
            8,
            9006104071832581.0,
            &[0x05, 0x04, 0x03, 0x02, 0x01, 0xff, 0x3f, 0x43]
        );
    }

    #[test]
    fn split_tuple() {
        // Inspired by a bug where the unpacker was told to return &'static [] so that it would
        // pass the type checker.  Need a test that has tuple with remainder as it's not the
        // typical, or even expected case.
        let (a, b, c, d) = (42u8, 13u8, 73u8, 32u8);
        let mut buf = [0u8; 4];
        (a, b, c, d).pack(&mut buf);
        assert_eq!([a, b, c, d], buf, "human got serialization wrong?");
        let mut up = Unpacker::new(&buf);
        let (ap, bp): (u8, u8) = up.unpack().unwrap();
        let (cp, dp): (u8, u8) = up.unpack().unwrap();
        assert_eq!(
            [a, b, c, d],
            [ap, bp, cp, dp],
            "human got deserialization wrong?"
        );
    }

    #[test]
    fn length_free() {
        let buf = &mut [0u8; 64];
        let buf = stack_pack(super::length_free(&[0u8, 1u8, 2u8])).into_slice(buf);
        assert_eq!([0, 1, 2], buf, "human got length_free wrong?");
    }

    #[test]
    fn stack_pack_into_slice() {
        let buf = &mut [0u8; 64];
        let buf = stack_pack(42u64).into_slice(buf);
        assert_eq!(
            &[42, 0, 0, 0, 0, 0, 0, 0],
            buf,
            "human got into_slice wrong?"
        );
    }

    #[test]
    fn stack_pack_to_vec() {
        let buf: &[u8] = &stack_pack(42u64).to_vec();
        assert_eq!(&[42, 0, 0, 0, 0, 0, 0, 0], &buf, "human got to_vec wrong?");
    }

    #[test]
    fn stack_packer() {
        let pa = stack_pack(());
        let pa = pa.pack(1u8);
        let pa = pa.pack(770u16);
        let pa = pa.pack(117835012u32);
        let pa = pa.pack(1084818905618843912u64);
        let mut buf = [0u8; 16];
        let buf = pa.into_slice(&mut buf);
        assert_eq!([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15], buf);
    }

    #[test]
    fn unpacker() {
        let buf: &[u8] = &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
        let mut up = Unpacker::new(buf);
        let x = up.unpack::<Error, ()>();
        assert_eq!(Ok(()), x, "human got () unpacker wrong?");
        let x = up.unpack::<Error, u8>();
        assert_eq!(Ok(1u8), x, "human got u8 unpacker wrong?");
        let x = up.unpack::<Error, u16>();
        assert_eq!(Ok(770u16), x, "human got u16 unpacker wrong?");
        let x = up.unpack::<Error, u32>();
        assert_eq!(Ok(117835012u32), x, "human got u32 unpacker wrong?");
        let x = up.unpack::<Error, u64>();
        assert_eq!(
            Ok(1084818905618843912u64),
            x,
            "human got u64 unpacker wrong?"
        );
        assert_eq!(&[] as &[u8], up.buf, "human got remaining buffer wrong?");
    }

    #[test]
    fn pack_and_unpack_slice() {
        let buf: &[u8] = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
        let pa = stack_pack(buf);
        let exp: &[u8] = &[16, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
        let got: &[u8] = &pa.to_vec();
        assert_eq!(exp, got);
        let mut up = Unpacker::new(exp);
        let got: &[u8] = up.unpack().expect("unpack slice");
        assert_eq!(buf, got);
    }
}
