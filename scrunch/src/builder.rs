use buffertk::{stack_pack, v64, Packable};

use prototk::{FieldNumber, FieldType, Tag, WireType};

////////////////////////////////////////////// Helper //////////////////////////////////////////////

pub trait Helper {
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn vec(&self) -> &Vec<u8>;
    fn vec_mut(&mut self) -> &mut Vec<u8>;
}

impl Helper for Vec<u8> {
    fn len(&self) -> usize {
        Vec::len(self)
    }

    fn vec(&self) -> &Vec<u8> {
        self
    }

    fn vec_mut(&mut self) -> &mut Vec<u8> {
        self
    }
}

////////////////////////////////////////////// Builder /////////////////////////////////////////////

pub struct Builder<'a, H: Helper>
where
    H: Helper + 'a,
{
    field_number: Option<FieldNumber>,
    starting_size: usize,
    helper: &'a mut H,
}

impl<'a> Builder<'a, Vec<u8>> {
    pub fn new<'b: 'a>(helper: &'a mut Vec<u8>) -> Self {
        let field_number = None;
        let starting_size = helper.len();
        Self {
            field_number,
            starting_size,
            helper,
        }
    }
}

impl<'a, H: Helper> Drop for Builder<'a, H> {
    fn drop(&mut self) {
        if let Some(field_number) = self.field_number.take() {
            let tag = Tag {
                field_number,
                wire_type: WireType::LengthDelimited,
            };
            let buf = self.helper.vec_mut();
            let current_size = buf.len();
            let len = current_size - self.starting_size;
            let len_v64 = v64::from(len);
            let pa = stack_pack(tag);
            let pa = pa.pack(len_v64);
            let pa_size = pa.pack_sz();
            buf.resize(current_size + pa_size, 0);
            for i in 0..len {
                buf[current_size + pa_size - i - 1] = buf[current_size - i - 1];
            }
            Packable::pack(
                &pa,
                &mut buf[self.starting_size..self.starting_size + pa_size],
            );
        }
    }
}

// TODO(rescrv):  Cleanup this interface.
impl<'a, H: Helper> Builder<'a, H> {
    pub fn sub(&mut self, field_number: FieldNumber) -> Builder<'_, Self> {
        let field_number = Some(field_number);
        let starting_size = self.len();
        let helper = self;
        Builder {
            field_number,
            starting_size,
            helper,
        }
    }

    pub fn append_bytes(&mut self, field_number: FieldNumber, bytes: &[u8]) {
        let fp = prototk::field_types::bytes::field_packer(field_number, &bytes);
        stack_pack(fp).append_to_vec(self.helper.vec_mut());
    }

    pub fn append_u32(&mut self, field_number: FieldNumber, value: u32) {
        let fp = prototk::field_types::uint32::field_packer(field_number, &value);
        stack_pack(fp).append_to_vec(self.helper.vec_mut());
    }

    pub fn append_u64(&mut self, field_number: FieldNumber, value: u64) {
        let fp = prototk::field_types::uint64::field_packer(field_number, &value);
        stack_pack(fp).append_to_vec(self.helper.vec_mut());
    }

    pub fn append_packable<P: Packable>(&mut self, field_number: FieldNumber, packable: &P) {
        let tag = Tag {
            field_number,
            wire_type: WireType::LengthDelimited,
        };
        let len = packable.pack_sz();
        let len = v64::from(len);
        stack_pack(tag)
            .pack(len)
            .pack(packable)
            .append_to_vec(self.helper.vec_mut());
    }

    pub fn append_vec_u32(&mut self, field_number: FieldNumber, nums: &[u32]) {
        let tag = Tag {
            field_number,
            wire_type: WireType::Varint,
        };
        for num in nums.iter() {
            stack_pack(tag)
                .pack(v64::from(*num))
                .append_to_vec(self.helper.vec_mut());
        }
    }

    pub fn append_vec_usize(&mut self, field_number: FieldNumber, nums: &[usize]) {
        let tag = Tag {
            field_number,
            wire_type: WireType::Varint,
        };
        for num in nums.iter() {
            stack_pack(tag)
                .pack(v64::from(*num))
                .append_to_vec(self.helper.vec_mut());
        }
    }

    pub fn append_raw(&mut self, bytes: &[u8]) {
        self.helper.vec_mut().extend_from_slice(bytes);
    }

    pub fn append_raw_packable<P: Packable>(&mut self, packable: &P) {
        stack_pack(packable).append_to_vec(self.helper.vec_mut());
    }

    pub fn relative_len(&self) -> usize {
        self.helper.vec().len() - self.starting_size
    }

    pub fn relative_bytes<'b, 'c>(&'b self, start: usize) -> &'c [u8]
    where
        'b: 'c,
    {
        &self.helper.vec()[self.starting_size + start..]
    }
}

impl<'a, H: Helper> Helper for Builder<'a, H> {
    fn len(&self) -> usize {
        self.helper.len()
    }

    fn vec(&self) -> &Vec<u8> {
        self.helper.vec()
    }

    fn vec_mut(&mut self) -> &mut Vec<u8> {
        self.helper.vec_mut()
    }
}

////////////////////////////////////////// parse_one_field /////////////////////////////////////////

/// Parse one protobuf field that's length-delimited.
/// Returns None if the tag, length, or bytes do not unpack from bytes.
/// Returns Some((tag, value, remain)).  This is the value unpacked, not remainder.
pub fn parse_one_field_bytes(bytes: &[u8]) -> Option<(Tag, &[u8], &[u8])> {
    let (tag, bytes) = <Tag as buffertk::Unpackable>::unpack(bytes).ok()?;
    let (len, bytes) = <v64 as buffertk::Unpackable>::unpack(bytes).ok()?;
    let len: usize = len.into();
    if bytes.len() >= len {
        Some((tag, &bytes[..len], &bytes[len..]))
    } else {
        None
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use prototk::FieldNumber;

    use super::Builder;

    #[test]
    fn empty() {
        let mut buf = Vec::new();
        let _builder = Builder::new(&mut buf);
    }

    #[test]
    fn append_bytes() {
        let mut buf = Vec::new();
        let mut builder = Builder::new(&mut buf);
        builder.append_bytes(FieldNumber::must(1), &[0, 1, 2, 3]);
        drop(builder);
        assert_eq!(&[10u8, 4, 0, 1, 2, 3], buf.as_slice());
    }

    #[test]
    fn sub_append_bytes() {
        let mut buf = Vec::new();
        let mut builder = Builder::new(&mut buf);
        let mut sub = builder.sub(FieldNumber::must(1));
        sub.append_bytes(FieldNumber::must(2), &[0, 1, 2, 3]);
        drop(sub);
        drop(builder);
        assert_eq!(&[10u8, 6, 18, 4, 0, 1, 2, 3], buf.as_slice());
    }

    #[test]
    fn sub_append_raw() {
        let mut buf = Vec::new();
        let mut builder = Builder::new(&mut buf);
        let mut sub = builder.sub(FieldNumber::must(1));
        sub.append_raw(&[0]);
        sub.append_raw(&[1]);
        sub.append_raw(&[2]);
        sub.append_raw(&[3]);
        drop(sub);
        drop(builder);
        assert_eq!(&[10u8, 4, 0, 1, 2, 3], buf.as_slice());
    }
}
