//! prototk is a protocol buffer (protobuf) library with a low-level API.  Unlike protobuf libraries
//! that focus on ease of use, code generation, or performance, prototk aims to expose every level
//! of abstraction it has internally so that developers can use as much or as little as they wish.

pub mod field_types;
pub mod varint;
pub mod zigzag;

pub use varint::v64;
pub use zigzag::unzigzag;
pub use zigzag::zigzag;

/////////////////////////////////////////////// Error //////////////////////////////////////////////

/// Error captures the possible error conditions for packing and unpacking.
// TODO(rescrv):  Some notion of the error context so that these can be tracked down.
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    /// BufferTooShort indicates that there was a need to pack or unpack more bytes than were
    /// available in the underlying memory.
    BufferTooShort { required: usize, had: usize },
    /// A N-byte buffer is not N bytes.
    BufferWrongSize { required: usize, had: usize },
    /// InvalidFieldNumber indicates that the field is not a user-assignable field.
    InvalidFieldNumber {
        field_number: u32,
        what: &'static str,
    },
    /// UnhandledWireType inidcates that the wire type is not currently understood by prototk.
    UnhandledWireType { wire_type: u32 },
    /// TagTooLarge indicates the tag would overflow a 32-bit number.
    TagTooLarge { tag: u64 },
    /// VarintOverflow indicates that a varint field did not terminate with a number < 128.
    VarintOverflow { bytes: usize },
    /// UnsignedOverflow indicates that a value will not fit its intended (unsigned) target.
    UnsignedOverflow { value: u64 },
    /// SignedOverflow indicates that a value will not fit its intended (signed) target.
    SignedOverflow { value: i64 },

    // TODO(rescrv): custom error type so that apps can extend
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::BufferTooShort { required, had } => {
                write!(f, "buffer too short:  expected {}, had {}", required, had)
            }
            Error::BufferWrongSize { required, had } => {
                write!(f, "buffer wrong size:  expected {}, had {}", required, had)
            }
            Error::InvalidFieldNumber { field_number, what } => {
                write!(f, "invalid field_number={}: {}", field_number, what)
            }
            Error::UnhandledWireType { wire_type } => write!(
                f,
                "wire_type={} not handled by this implementation",
                wire_type
            ),
            Error::TagTooLarge { tag } => write!(f, "tag={} overflows 32-bits", tag),
            Error::VarintOverflow { bytes } => {
                write!(f, "varint did not fit in space={} bytes", bytes)
            },
            Error::UnsignedOverflow { value } => {
                write!(f, "unsigned integer cannot hold value={}", value)
            }
            Error::SignedOverflow { value } => {
                write!(f, "signed integer cannot hold value={}", value)
            }
        }
    }
}

///////////////////////////////////////////// Packable /////////////////////////////////////////////

/// Packable objects can be serialized into an `&mut [u8]`.
///
/// The actual serialized form of the object is left unspecified by the Packable trait.  *There is
/// no requirement that the bytes be a valid protobuf message.*  This is an intentional restriction
/// on the trait to make sure that abstractions for combining packer objects can be applied for all
/// packers, regardless of format.
///
/// Packable objects should avoid interior mutability to the extent necessary to ensure that anyone
/// holding a non-mutable reference can assume the packed output will not change for the duration
/// of the reference.
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
    {
        let pa = stack_pack(&self);
        let vec: Vec<u8> = pa.to_vec();
        match writer.write_all(&vec) {
            Ok(_) => {
                Ok(vec.len())
            },
            Err(x) => {
                Err(x)
            },
        }
    }
}

//////////////////////////////////////////// Unpackable ////////////////////////////////////////////

/// Unpackable objects can be deserialized from an `&[u8]`.
///
/// The format understood by `T:Unpackable` must correspond to the format serialized by
/// `T:Packable`.  *As explained on [Packable](trait.Packable.html), there is no requirement that
/// the bytes be a valid protobuf message.*
pub trait Unpackable<'a>: Sized {
    /// `unpack` attempts to return an Unpackable object stored in a prefix of `buf`.  The method
    /// returns the result and remaining unused buffer.  An error consumes an implementation-defined
    /// portion of the buffer, but should typically consume zero bytes.
    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error>;
}

//////////////////////////////////// Packable/Unpackable for &P ////////////////////////////////////

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
                for i in 0..out.len() {
                    out[i] = b[i];
                }
            }
        }
        impl<'a> Unpackable<'a> for $what {
            fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
                const SZ: usize = std::mem::size_of::<$what>();
                if buf.len() >= SZ {
                    let mut fbuf: [u8; SZ] = [0; SZ];
                    for i in 0..SZ {
                        fbuf[i] = buf[i];
                    }
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
                for i in 0..out.len() {
                    out[i] = b[i];
                }
            }
        }
        impl<'a> Unpackable<'a> for $what {
            fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
                const SZ: usize = std::mem::size_of::<$what>();
                if buf.len() >= SZ {
                    let mut fbuf: [u8; SZ] = [0; SZ];
                    for i in 0..SZ {
                        fbuf[i] = buf[i];
                    }
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

/////////////////////////////////// Packable/Unpackable for &[P] ///////////////////////////////////

impl<P: Packable> Packable for &[P] {
    fn pack_sz(&self) -> usize {
        let v: v64 = self.len().into();
        let h: usize = v.pack_sz();
        // TODO(rescrv):  not good for bytes or other small objs :-(
        let b: usize = self.iter().map(|x| x.pack_sz()).sum();
        h + b
    }

    fn pack(&self, out: &mut [u8]) {
        let v: v64 = self.len().into();
        let mut out = pack_helper(v, out);
        for i in 0..self.len() {
            out = pack_helper(&self[i], out);
        }
    }
}

impl<'a, U: Unpackable<'a> + 'a> Unpackable<'a> for Vec<U> {
    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let mut up = Unpacker::new(buf);
        let len: v64 = up.unpack()?;
        let mut elems: Vec<U> = Vec::new();
        for _ in 0u64..len.into() {
            let elem: U = up.unpack()?;
            elems.push(elem);
        }
        let sz = up.remain().len();
        Ok((elems, &buf[buf.len()+sz..]))
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
        impl<'a, $($name: Unpackable<'a> + 'a),+> Unpackable<'a> for ($($name,)+) {
            fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
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

////////////////////////////////////////////// helpers /////////////////////////////////////////////

/// `pack_helper` takes a Packable object and an `&mut [u8]` and does the work to serialize the
/// packable into a prefix of the buffer.  The return value is the portion of the buffer that
/// remains unfilled after this operation.
pub fn pack_helper<'a, T: Packable>(t: T, buf: &'a mut [u8]) -> &'a mut [u8] {
    let sz: usize = t.pack_sz();
    assert!(sz <= buf.len(), "packers should never be given short space");
    t.pack(&mut buf[..sz]);
    &mut buf[sz..]
}

/// `stack_pack` begins construction of a [packer-like](struct.Packer.html) object on the stack.
pub fn stack_pack<'a, T: Packable + 'a>(t: T) -> StackPacker<'a, (), T> {
    StackPacker {
        prefix: &EMPTY,
        t: t,
    }
}

pub fn length_free<'a, P:Packable>(slice: &'a [P]) -> LengthFree<'a, P> {
    LengthFree {
        slice,
    }
}

//////////////////////////////////////////// LengthFree ////////////////////////////////////////////

pub struct LengthFree<'a, P:Packable> {
    slice: &'a [P],
}

impl<'a, P:Packable> Packable for LengthFree<'a, P> {
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

//////////////////////////////////////////// StackPacker ///////////////////////////////////////////

const EMPTY: () = ();

/// StackPacker provides similar chained-pack API as [Packer](struct.Packer.html), but uses stack
/// allocation and monomorphism to do so without obvious sources of heap allocation.  Beyond this
/// hypothesized performance advantage, the StackPacker can inspect the sequence of Packables to be
/// serialized and dynamically allocate the requisite space in a single allocation.
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
        let mut buf = Vec::new();
        buf.resize(len, 0u8);
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
        let (prefix, suffix): (&mut [u8], &mut [u8]) =
            out.split_at_mut(self.prefix.pack_sz());
        self.prefix.pack(prefix);
        self.t.pack(suffix);
    }
}

////////////////////////////////////////// LengthPrefixer //////////////////////////////////////////

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
        let (prefix, suffix): (&mut [u8], &mut[u8]) =
            out.split_at_mut(vsz.pack_sz());
        vsz.pack(prefix);
        self.body.pack(suffix);
    }
}

///////////////////////////////////////////// Unpacker /////////////////////////////////////////////

#[derive(Clone, Default)]
pub struct Unpacker<'a> {
    buf: &'a [u8],
}

impl<'a> Unpacker<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf }
    }

    pub fn unpack<'b, T: Unpackable<'b>>(&mut self) -> Result<T, Error>
        where
        'a: 'b,
    {
        let (t, buf): (T, &'a [u8]) = Unpackable::unpack(self.buf)?;
        self.buf = buf;
        Ok(t)
    }

    pub fn empty(&self) -> bool {
        self.buf.len() == 0
    }

    pub fn remain(&self) -> &'a [u8] {
        self.buf
    }
}

///////////////////////////////////////////// WireType /////////////////////////////////////////////

#[derive(Debug, PartialEq, Eq)]
pub enum WireType {
    /// Varint is wire type 0.  The payload is a single v64.
    Varint,
    /// SixtyFour represents wire type 1.  The payload is a single u64.
    SixtyFour,
    /// LengthDelimited represents wire type 2.  The payload depends upon how the system interprets
    /// the field number.
    LengthDelimited,
    // wiretype 3 and 4 were deprecated and therefore not implemented
    /// ThirtyTwo represents wire type 5.  The payload is a single u32.
    ThirtyTwo,
}

impl WireType {
    pub fn new(tag_bits: u32) -> Result<WireType, Error> {
        match tag_bits {
            0 => Ok(WireType::Varint),
            1 => Ok(WireType::SixtyFour),
            2 => Ok(WireType::LengthDelimited),
            5 => Ok(WireType::ThirtyTwo),
            _ => Err(Error::UnhandledWireType {
                wire_type: tag_bits,
            }),
        }
    }

    /// `tag_bits` returns the WireType's contribution to the tag, suitable for bit-wise or'ing with
    /// the FieldNumber.
    pub fn tag_bits(&self) -> u32 {
        match self {
            WireType::Varint => 0,
            WireType::SixtyFour => 1,
            WireType::LengthDelimited => 2,
            WireType::ThirtyTwo => 5,
        }
    }
}

//////////////////////////////////////////// FieldNumber ///////////////////////////////////////////

const FIRST_FIELD_NUMBER: u32 = 1;
const LAST_FIELD_NUMBER: u32 = (1 << 29) - 1;

const FIRST_RESERVED_FIELD_NUMBER: u32 = 19000;
const LAST_RESERVED_FIELD_NUMBER: u32 = 19999;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldNumber {
    field_number: u32,
}

impl FieldNumber {
    pub fn must(field_number: u32) -> FieldNumber {
        FieldNumber::new(field_number).unwrap()
    }

    pub fn new(field_number: u32) -> Result<FieldNumber, Error> {
        if field_number < FIRST_FIELD_NUMBER {
            return Err(Error::InvalidFieldNumber {
                field_number,
                what: "field number must be positive integer",
            });
        }
        if field_number > LAST_FIELD_NUMBER {
            return Err(Error::InvalidFieldNumber {
                field_number,
                what: "field number too large",
            });
        }
        if field_number >= FIRST_RESERVED_FIELD_NUMBER && field_number <= LAST_RESERVED_FIELD_NUMBER
        {
            return Err(Error::InvalidFieldNumber {
                field_number,
                what: "field is reserved",
            });
        }
        Ok(FieldNumber { field_number })
    }
}

impl Into<u32> for FieldNumber {
    fn into(self) -> u32 {
        self.field_number
    }
}

impl std::cmp::PartialEq<u32> for FieldNumber {
    fn eq(&self, other: &u32) -> bool {
        self.field_number == *other
    }
}

//////////////////////////////////////////////// Tag ///////////////////////////////////////////////

#[derive(Debug)]
pub struct Tag {
    pub field_number: FieldNumber,
    pub wire_type: WireType,
}

#[macro_export]
macro_rules! tag {
    ($field_number:literal, $wire_type:ident) => {
        $crate::Tag {
            field_number: $crate::FieldNumber::must($field_number),
            wire_type: $crate::WireType::$wire_type,
        }
    };
}

impl Tag {
    fn v64(&self) -> v64 {
        let f: u32 = self.field_number.into();
        let w: u32 = self.wire_type.tag_bits();
        let t: u32 = (f << 3) | w;
        t.into()
    }
}

impl Packable for Tag {
    fn pack_sz(&self) -> usize {
        let v = self.v64();
        v.pack_sz()
    }

    fn pack(&self, buf: &mut [u8]) {
        let v = self.v64();
        v.pack(buf);
    }
}

impl<'a> Unpackable<'a> for Tag {
    fn unpack<'b: 'a>(buf: &'b [u8]) -> Result<(Self, &'b [u8]), Error> {
        let mut up = Unpacker::new(buf);
        let tag: v64 = up.unpack()?;
        let tag: u64 = tag.into();
        if tag > u32::max_value() as u64 {
            return Err(Error::TagTooLarge { tag });
        }
        let tag: u32 = tag as u32;
        let f: u32 = tag >> 3;
        let w: u32 = tag & 7;
        let field_number = FieldNumber::new(f)?;
        let wire_type = WireType::new(w)?;
        Ok((
            Tag {
                field_number,
                wire_type,
            },
            up.remain(),
        ))
    }
}

///////////////////////////////////////////// FieldType ////////////////////////////////////////////

pub trait FieldType<'a>: Packable + Unpackable<'a> {
    const WIRE_TYPE: WireType;
    const LENGTH_PREFIXED: bool;

    type NativeType;

    fn into_native(self) -> Self::NativeType;

    fn assign<A: FieldTypeAssigner<NativeType=Self::NativeType>>(lhs: &mut A, x: Self::NativeType) {
        lhs.assign_field_type(x);
    }
}
////////////////////////////////////////// FieldTypePacker /////////////////////////////////////////

pub struct FieldTypePacker<'a, A, B> {
    t: Tag,
    a: std::marker::PhantomData<A>,
    b: &'a B,
}

impl<'a, A, B> FieldTypePacker<'a, A, B> {
    pub fn new(t: Tag, a: std::marker::PhantomData<A>, b: &'a B) -> Self {
        Self {
            t,
            a,
            b,
        }
    }
}

pub trait FieldTypePackable {}

impl FieldTypePackable for i32 {}
impl FieldTypePackable for i64 {}
impl FieldTypePackable for u32 {}
impl FieldTypePackable for u64 {}
impl FieldTypePackable for f32 {}
impl FieldTypePackable for f64 {}
impl<'a> FieldTypePackable for &'a [u8] {}
impl<'a> FieldTypePackable for Buffer {}
impl<'a> FieldTypePackable for &'a str {}
impl<'a> FieldTypePackable for String {}
impl<'a, M: Message<'a>> FieldTypePackable for M {}

impl<'a, F, T> Packable for FieldTypePacker<'a, F, T>
where
    F: FieldType<'a>,
    T: FieldTypePackable + Clone,
    &'a T: std::convert::Into<F>,
{
    fn pack_sz(&self) -> usize {
        let pb: F = self.b.into();
        stack_pack(&self.t).pack(&pb).pack_sz()
    }

    fn pack(&self, buf: &mut [u8]) {
        let pb: F = self.b.into();
        stack_pack(&self.t).pack(&pb).into_slice(buf);
    }
}

pub trait FieldTypeVectorPackable {}

impl FieldTypeVectorPackable for i32 {}
impl FieldTypeVectorPackable for i64 {}
impl FieldTypeVectorPackable for u32 {}
impl FieldTypeVectorPackable for u64 {}
impl FieldTypeVectorPackable for f32 {}
impl FieldTypeVectorPackable for f64 {}
impl<'a> FieldTypeVectorPackable for &'a [u8] {}
impl<'a, M: Message<'a>> FieldTypeVectorPackable for M {}

impl<'a, F, T> Packable for FieldTypePacker<'a, F, Vec<T>>
where
    F: FieldType<'a>,
    T: FieldTypeVectorPackable + Clone,
    &'a T: std::convert::Into<F>,
{
    fn pack_sz(&self) -> usize {
        let mut sz = self.t.pack_sz() * self.b.len();
        for x in self.b.iter() {
            let px: F = x.into();
            let elem_sz = px.pack_sz();
            if F::LENGTH_PREFIXED {
                sz += v64::from(elem_sz).pack_sz();
            }
            sz += elem_sz;
        }
        sz
    }

    fn pack(&self, buffer: &mut [u8]) {
        let tag_sz = self.t.pack_sz();
        let mut total_sz = 0;
        for x in self.b.iter() {
            // TODO(rescrv): cleanup
            let px: F = x.into();
            let sz = px.pack_sz();
            if F::LENGTH_PREFIXED {
                let prefix: v64 = sz.into();
                let buf = &mut buffer[total_sz..total_sz+tag_sz+prefix.pack_sz()+sz];
                stack_pack(&self.t).pack(prefix).pack(px).into_slice(buf);
            } else {
                let buf = &mut buffer[total_sz..total_sz+tag_sz+sz];
                stack_pack(&self.t).pack(px).into_slice(buf);
            }
            total_sz += tag_sz + sz;
        }
    }
}

///////////////////////////////////////// FieldTypeAssigner ////////////////////////////////////////

pub trait FieldTypeAssigner {
    type NativeType;

    fn assign_field_type(&mut self, x: Self::NativeType);
}

trait TemplateFieldTypeAssigner {}

impl FieldTypeAssigner for i32 {
    type NativeType = i32;

    fn assign_field_type(&mut self, x: i32) {
        *self = x;
    }
}

impl FieldTypeAssigner for i64 {
    type NativeType = i64;

    fn assign_field_type(&mut self, x: i64) {
        *self = x;
    }
}

impl FieldTypeAssigner for u32 {
    type NativeType = u32;

    fn assign_field_type(&mut self, x: u32) {
        *self = x;
    }
}

impl FieldTypeAssigner for u64 {
    type NativeType = u64;

    fn assign_field_type(&mut self, x: u64) {
        *self = x;
    }
}

impl FieldTypeAssigner for f32 {
    type NativeType = f32;

    fn assign_field_type(&mut self, x: f32) {
        *self = x;
    }
}

impl FieldTypeAssigner for f64 {
    type NativeType = f64;

    fn assign_field_type(&mut self, x: f64) {
        *self = x;
    }
}

impl<'a> FieldTypeAssigner for &'a [u8] {
    type NativeType = &'a [u8];

    fn assign_field_type(&mut self, x: &'a [u8]) {
        *self = x;
    }
}

impl<'a> FieldTypeAssigner for Buffer {
    type NativeType = Buffer;

    fn assign_field_type(&mut self, x: Buffer) {
        *self = x;
    }
}

impl<'a> FieldTypeAssigner for &'a str {
    type NativeType = &'a str;

    fn assign_field_type(&mut self, x: &'a str) {
        *self = x;
    }
}

impl FieldTypeAssigner for String {
    type NativeType = String;

    fn assign_field_type(&mut self, x: String) {
        *self = x;
    }
}

impl<'a, M: Message<'a>> FieldTypeAssigner for M {
    type NativeType = M;

    fn assign_field_type(&mut self, x: M) {
        *self = x;
    }
}

impl FieldTypeAssigner for Vec<i32> {
    type NativeType = i32;

    fn assign_field_type(&mut self, x: i32) {
        self.push(x);
    }
}

impl FieldTypeAssigner for Vec<i64> {
    type NativeType = i64;

    fn assign_field_type(&mut self, x: i64) {
        self.push(x);
    }
}

impl FieldTypeAssigner for Vec<u32> {
    type NativeType = u32;

    fn assign_field_type(&mut self, x: u32) {
        self.push(x);
    }
}

impl FieldTypeAssigner for Vec<u64> {
    type NativeType = u64;

    fn assign_field_type(&mut self, x: u64) {
        self.push(x);
    }
}

impl FieldTypeAssigner for Vec<f32> {
    type NativeType = f32;

    fn assign_field_type(&mut self, x: f32) {
        self.push(x);
    }
}

impl FieldTypeAssigner for Vec<f64> {
    type NativeType = f64;

    fn assign_field_type(&mut self, x: f64) {
        self.push(x);
    }
}

impl<'a, M: Message<'a>> FieldTypeAssigner for Vec<M> {
    type NativeType = M;

    fn assign_field_type(&mut self, x: M) {
        self.push(x);
    }
}

////////////////////////////////////////////// Message /////////////////////////////////////////////

// TODO(rescrv):  There's an extra clone type here because I couldn't do From/Into of
// message<M:Message> to make it zero copy.  Get the macros up and revisit.
pub trait Message<'a>: Clone + Default + Packable + Unpackable<'a> {
}

impl<'a, M> Message<'a> for &'a M
where
    M: Message<'a>,
    &'a M: Default + Packable + Unpackable<'a>,
{
}

////////////////////////////////////////////// Buffer //////////////////////////////////////////////

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Buffer {
    pub buf: Vec<u8>,
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
    fn pack_and_unpack_slice() {
        const SLICE: &[u8] = &[0x42, 0x43];
        const EXPECT: &[u8] = &[2, 0x42, 0x43];
        let buf: &mut [u8] = &mut [0; 3];
        SLICE.pack(buf);
        assert_eq!(EXPECT, buf, "human got serialization wrong?");
        let mut up = Unpacker::new(EXPECT);
        let out = up.unpack();
        assert_eq!(Ok(SLICE.to_vec()), out, "human got deserialization wrong?");
    }

    #[test]
    fn split_tuple() {
        // Inspired by a bug where the unpacker was told to return &'static [] so that it would
        // pass the type checker.  Need a test that has tuple with remainder as it's not the
        // typical, or even expected case.
        let (a, b, c, d) = (42u8, 13u8, 73u8, 32u8);
        let mut buf = [0u8;4];
        (a, b, c, d).pack(&mut buf);
        assert_eq!([a, b, c, d], buf, "human got serialization wrong?");
        let mut up = Unpacker::new(&buf);
        let (ap, bp): (u8, u8) = up.unpack().unwrap();
        let (cp, dp): (u8, u8) = up.unpack().unwrap();
        assert_eq!([a, b, c, d], [ap, bp, cp, dp], "human got deserialization wrong?");
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
        let mut up = Unpacker::new(&buf);
        let x = up.unpack::<()>();
        assert_eq!(Ok(()), x, "human got () unpacker wrong?");
        let x = up.unpack::<u8>();
        assert_eq!(Ok(1u8), x, "human got u8 unpacker wrong?");
        let x = up.unpack::<u16>();
        assert_eq!(Ok(770u16), x, "human got u16 unpacker wrong?");
        let x = up.unpack::<u32>();
        assert_eq!(Ok(117835012u32), x, "human got u32 unpacker wrong?");
        let x = up.unpack::<u64>();
        assert_eq!(
            Ok(1084818905618843912u64),
            x,
            "human got u64 unpacker wrong?"
        );
        assert_eq!(&[] as &[u8], up.buf, "human got remaining buffer wrong?");
    }

    // TODO(rescrv): error case tests
    // TODO(rescrv): tuple tests
    // TODO(rescrv): short field tests
    // TODO(rescrv): bytes tests
    // TODO(rescrv): message and reference tests within variant and struct
}
