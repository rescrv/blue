use std::cmp::Ordering;
use std::fmt::{Debug, Display, Formatter};

use buffertk::{stack_pack, v64, Packable};

use prototk::field_types::*;
use prototk::{FieldNumber, Tag, WireType};
use prototk_derive::Message;

use tuple_key::{DataType, Element, SchemaEntry, TupleKey, TupleKeyIterator, TupleSchema};

use zerror::{iotoz, Z};
use zerror_core::ErrorCore;

/////////////////////////////////////////////// Error //////////////////////////////////////////////

#[derive(Clone, Debug, Message)]
pub enum Error {
    #[prototk(442368, message)]
    Success {
        #[prototk(1, message)]
        core: ErrorCore,
    },
    #[prototk(442369, message)]
    TupleKeyError {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, message)]
        error: tuple_key::Error,
    },
    #[prototk(442370, message)]
    LogicError {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        what: String,
    },
    #[prototk(442371, message)]
    Corruption {
        #[prototk(1, message)]
        core: ErrorCore,
        #[prototk(2, string)]
        what: String,
    },
}

impl Error {
    fn core(&self) -> &ErrorCore {
        match self {
            Error::Success { core, .. } => core,
            Error::TupleKeyError { core, .. } => core,
            Error::LogicError { core, .. } => core,
            Error::Corruption { core, .. } => core,
        }
    }

    fn core_mut(&mut self) -> &mut ErrorCore {
        match self {
            Error::Success { core, .. } => core,
            Error::TupleKeyError { core, .. } => core,
            Error::LogicError { core, .. } => core,
            Error::Corruption { core, .. } => core,
        }
    }
}

impl Default for Error {
    fn default() -> Self {
        Error::Success {
            core: ErrorCore::default(),
        }
    }
}

impl Display for Error {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Error::Success { core: _ } => fmt.debug_struct("Success").finish(),
            Error::TupleKeyError { core: _, error } => fmt
                .debug_struct("TupleKeyError")
                .field("error", error)
                .finish(),
            Error::LogicError { core: _, what } => {
                fmt.debug_struct("LogicError").field("what", what).finish()
            }
            Error::Corruption { core: _, what } => {
                fmt.debug_struct("LogicError").field("what", what).finish()
            }
        }
    }
}

impl From<tuple_key::Error> for Error {
    fn from(error: tuple_key::Error) -> Self {
        Self::TupleKeyError {
            core: ErrorCore::default(),
            error,
        }
    }
}

impl Z for Error {
    type Error = Self;

    fn long_form(&self) -> String {
        format!("{}", self) + "\n" + &self.core().long_form()
    }

    fn with_token(mut self, identifier: &str, value: &str) -> Self::Error {
        self.core_mut().set_token(identifier, value);
        self
    }

    fn with_url(mut self, identifier: &str, url: &str) -> Self::Error {
        self.core_mut().set_url(identifier, url);
        self
    }

    fn with_variable<X: Debug>(mut self, variable: &str, x: X) -> Self::Error
    where
        X: Debug,
    {
        self.core_mut().set_variable(variable, x);
        self
    }
}

iotoz! {Error}

/////////////////////////////////////////// MessageFrame ///////////////////////////////////////////

/// MessageFrame captures the boundaries of a message and the internal holes left for sizes.
#[derive(Copy, Clone, Debug)]
enum MessageFrame {
    Begin {
        offset: usize,
        tag_sz: usize,
    },
    BeginMapWithMessage {
        offset: usize,
        tag_sz: usize,
        key_offset: usize,
    },
    End {
        offset: usize,
        begin: usize,
    },
}

//////////////////////////////////////// MessageFrameWrapper ///////////////////////////////////////

struct MessageFrameWrapper {
    index: usize,
}

/////////////////////////////////////////// ProtoBuilder ///////////////////////////////////////////

#[derive(Debug, Default)]
struct ProtoBuilder {
    msg: Vec<u8>,
    frames: Vec<MessageFrame>,
}

impl ProtoBuilder {
    fn begin_message(&mut self, tag: Tag, buffers: &[&[u8]]) -> MessageFrameWrapper {
        let begin = MessageFrame::Begin {
            offset: self.msg.len(),
            tag_sz: tag.pack_sz(),
        };
        let wrap = MessageFrameWrapper {
            index: self.frames.len(),
        };
        self.frames.push(begin);
        stack_pack(tag).append_to_vec(&mut self.msg);
        for _ in 0..8 {
            self.msg.push(0);
        }
        for buffer in buffers {
            self.msg.extend_from_slice(buffer)
        }
        wrap
    }

    fn begin_map_with_message(&mut self, tag: Tag, buffers: &[&[u8]]) -> MessageFrameWrapper {
        let msg_len = self.msg.len();
        stack_pack(&tag).append_to_vec(&mut self.msg);
        for _ in 0..8 {
            self.msg.push(0);
        }
        for buffer in buffers {
            self.msg.extend_from_slice(buffer)
        }
        let begin = MessageFrame::BeginMapWithMessage {
            offset: msg_len,
            tag_sz: tag.pack_sz(),
            key_offset: self.msg.len(),
        };
        let wrap = MessageFrameWrapper {
            index: self.frames.len(),
        };
        self.frames.push(begin);
        for _ in 0..8 {
            self.msg.push(0);
        }
        wrap
    }

    fn end_message(&mut self, wrapper: MessageFrameWrapper) {
        let end = MessageFrame::End {
            offset: self.msg.len(),
            begin: wrapper.index,
        };
        self.frames.push(end);
    }

    fn emit_inline(&mut self, value: &[u8]) {
        self.msg.extend_from_slice(value);
    }

    fn emit_breakout(&mut self, tag: Tag, value: &[u8]) {
        stack_pack(&tag).append_to_vec(&mut self.msg);
        self.msg.extend_from_slice(value);
    }

    fn shift_frame(
        &mut self,
        offset_of_u64: usize,
        in_progress_offset: usize,
        msg_sz: usize,
        bytes_dropped: usize,
    ) -> Result<usize, Error> {
        let post_u64_offset = offset_of_u64 + 8;
        if post_u64_offset > in_progress_offset {
            return Err(Error::LogicError {
                core: ErrorCore::default(),
                what: "offset_too_small".to_owned(),
            })
            .as_z()
            .with_variable("post_u64_offset", post_u64_offset)
            .with_variable("in_progress_offset", in_progress_offset);
        }
        let msg_sz_v64 = v64::from(msg_sz);
        let msg_sz_v64_pack_sz = msg_sz_v64.pack_sz();
        if msg_sz_v64_pack_sz > 8 {
            return Err(Error::LogicError {
                core: ErrorCore::default(),
                what: "offset_too_small".to_owned(),
            })
            .as_z()
            .with_variable("msg_sz_v64_pack_sz", msg_sz_v64_pack_sz);
        }
        let newly_dropped_bytes = 8 - msg_sz_v64_pack_sz;
        for src in (post_u64_offset..in_progress_offset).rev() {
            let dst = src + bytes_dropped;
            self.msg[dst] = self.msg[src];
        }
        let length_start = offset_of_u64 + bytes_dropped + newly_dropped_bytes;
        let length_slice = &mut self.msg[length_start..length_start + msg_sz_v64_pack_sz];
        stack_pack(msg_sz_v64).into_slice(length_slice);
        Ok(newly_dropped_bytes)
    }

    fn seal(mut self) -> Result<Vec<u8>, Error> {
        let mut in_progress = Vec::new();
        let mut bytes_dropped = 0;
        while !self.frames.is_empty() {
            let frame_idx = self.frames.len() - 1;
            let back = self.frames[frame_idx];
            match back {
                MessageFrame::Begin {
                    offset: begin_offset,
                    tag_sz,
                } => {
                    if in_progress.is_empty() {
                        return Err(Error::LogicError {
                            core: ErrorCore::default(),
                            what: "in_progress was empty".to_owned(),
                        });
                    }
                    let (in_progress_offset, in_progress_idx) = in_progress.pop().unwrap();
                    if in_progress_idx != frame_idx {
                        return Err(Error::LogicError {
                            core: ErrorCore::default(),
                            what: "index miscalculation".to_owned(),
                        })
                        .as_z()
                        .with_variable("in_progress_idx", in_progress_idx)
                        .with_variable("frame_idx", frame_idx);
                    }
                    let msg_sz = in_progress_offset - begin_offset - tag_sz - 8;
                    let newly_dropped_bytes = self.shift_frame(
                        begin_offset + tag_sz,
                        in_progress_offset,
                        msg_sz,
                        bytes_dropped,
                    )?;
                    for tag_byte in (begin_offset..begin_offset + tag_sz).rev() {
                        self.msg[tag_byte + newly_dropped_bytes] = self.msg[tag_byte];
                    }
                    bytes_dropped += newly_dropped_bytes;
                    self.frames.pop();
                }
                MessageFrame::BeginMapWithMessage {
                    offset: begin_offset,
                    tag_sz,
                    key_offset,
                } => {
                    if in_progress.is_empty() {
                        return Err(Error::LogicError {
                            core: ErrorCore::default(),
                            what: "in_progress was empty".to_owned(),
                        });
                    }
                    let (in_progress_offset, in_progress_idx) = in_progress.pop().unwrap();
                    if in_progress_idx != frame_idx {
                        return Err(Error::LogicError {
                            core: ErrorCore::default(),
                            what: "index miscalculation".to_owned(),
                        })
                        .as_z()
                        .with_variable("in_progress_idx", in_progress_idx)
                        .with_variable("frame_idx", frame_idx);
                    }
                    let msg_sz = in_progress_offset - key_offset - 8;
                    let first_dropped_bytes =
                        self.shift_frame(key_offset, in_progress_offset, msg_sz, bytes_dropped)?;
                    bytes_dropped += first_dropped_bytes;
                    let msg_sz =
                        in_progress_offset - begin_offset - tag_sz - 16 + (8 - first_dropped_bytes);
                    let second_dropped_bytes =
                        self.shift_frame(begin_offset + tag_sz, key_offset, msg_sz, bytes_dropped)?;
                    bytes_dropped += second_dropped_bytes;
                    for tag_byte in (begin_offset..begin_offset + tag_sz).rev() {
                        self.msg[tag_byte + bytes_dropped] = self.msg[tag_byte];
                    }
                    self.frames.pop();
                }
                MessageFrame::End { offset, begin } => {
                    in_progress.push((offset, begin));
                    self.frames.pop();
                }
            }
        }
        for i in 0..self.msg.len() - bytes_dropped {
            self.msg[i] = self.msg[i + bytes_dropped];
        }
        self.msg.truncate(self.msg.len() - bytes_dropped);
        Ok(self.msg)
    }
}

////////////////////////////////////////////// KeyRef //////////////////////////////////////////////

// TODO(rescrv): dedupe KeyRef and KeyValueRef with sst.

#[derive(Clone, Debug)]
pub struct KeyRef<'a> {
    pub key: &'a [u8],
    pub timestamp: u64,
}

impl<'a> Eq for KeyRef<'a> {}

impl<'a> PartialEq for KeyRef<'a> {
    fn eq(&self, rhs: &KeyRef) -> bool {
        self.cmp(rhs) == std::cmp::Ordering::Equal
    }
}

impl<'a> Ord for KeyRef<'a> {
    fn cmp(&self, rhs: &KeyRef) -> std::cmp::Ordering {
        compare_key(self.key, self.timestamp, rhs.key, rhs.timestamp)
    }
}

impl<'a> PartialOrd for KeyRef<'a> {
    fn partial_cmp(&self, rhs: &KeyRef) -> Option<std::cmp::Ordering> {
        Some(self.cmp(rhs))
    }
}

impl<'a, 'b: 'a> From<&'a KeyValueRef<'b>> for KeyRef<'a> {
    fn from(kvr: &'a KeyValueRef<'b>) -> KeyRef<'a> {
        Self {
            key: kvr.key,
            timestamp: kvr.timestamp,
        }
    }
}

//////////////////////////////////////////// KeyValueRef ///////////////////////////////////////////

#[derive(Clone, Debug)]
pub struct KeyValueRef<'a> {
    pub key: &'a [u8],
    pub timestamp: u64,
    pub value: Option<&'a [u8]>,
}

impl<'a> Display for KeyValueRef<'a> {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        let key = String::from_utf8(
            self.key
                .iter()
                .flat_map(|b| std::ascii::escape_default(*b))
                .collect::<Vec<u8>>(),
        )
        .unwrap();
        if let Some(value) = self.value {
            let value = String::from_utf8(
                value
                    .iter()
                    .flat_map(|b| std::ascii::escape_default(*b))
                    .collect::<Vec<u8>>(),
            )
            .unwrap();
            write!(fmt, "\"{}\" @ {} -> \"{}\"", key, self.timestamp, value)
        } else {
            write!(fmt, "\"{}\" @ {} -> <TOMBSTONE>", key, self.timestamp)
        }
    }
}

impl<'a> Eq for KeyValueRef<'a> {}

impl<'a> PartialEq for KeyValueRef<'a> {
    fn eq(&self, rhs: &KeyValueRef) -> bool {
        let lhs: KeyRef = self.into();
        let rhs: KeyRef = rhs.into();
        lhs.eq(&rhs)
    }
}

impl<'a> Ord for KeyValueRef<'a> {
    fn cmp(&self, rhs: &KeyValueRef) -> std::cmp::Ordering {
        let lhs: KeyRef = self.into();
        let rhs: KeyRef = rhs.into();
        lhs.cmp(&rhs)
    }
}

impl<'a> PartialOrd for KeyValueRef<'a> {
    fn partial_cmp(&self, rhs: &KeyValueRef) -> Option<std::cmp::Ordering> {
        Some(self.cmp(rhs))
    }
}

/////////////////////////////////////////// compare_bytes //////////////////////////////////////////

// Content under CC By-Sa.  I just use as is, as can you.
// https://codereview.stackexchange.com/questions/233872/writing-slice-compare-in-a-more-compact-way
pub fn compare_bytes(a: &[u8], b: &[u8]) -> Ordering {
    for (ai, bi) in a.iter().zip(b.iter()) {
        match ai.cmp(bi) {
            Ordering::Equal => continue,
            ord => return ord,
        }
    }

    /* if every single element was equal, compare length */
    a.len().cmp(&b.len())
}
// End borrowed code

//////////////////////////////////////////// compare_key ///////////////////////////////////////////

pub fn compare_key(
    key_lhs: &[u8],
    timestamp_lhs: u64,
    key_rhs: &[u8],
    timestamp_rhs: u64,
) -> Ordering {
    compare_bytes(key_lhs, key_rhs).then(timestamp_lhs.cmp(&timestamp_rhs).reverse())
}

////////////////////////////////////////////// Cursor //////////////////////////////////////////////

pub trait Cursor {
    type Error: From<tuple_key::Error>;
    fn next(&mut self) -> Result<Option<KeyValueRef<'_>>, Self::Error>;
}

///////////////////////////////////////////// utilities ////////////////////////////////////////////

fn parse_as_prototk(val: &[u8], ty: DataType) -> Result<Vec<u8>, &'static str> {
    match ty {
        DataType::unit => Ok(Vec::new()),
        DataType::int32 => Ok(stack_pack(int32(<i32 as Element>::parse_from(val)?)).to_vec()),
        DataType::int64 => Ok(stack_pack(int64(<i64 as Element>::parse_from(val)?)).to_vec()),
        DataType::uint32 => Ok(stack_pack(uint32(<u32 as Element>::parse_from(val)?)).to_vec()),
        DataType::uint64 => Ok(stack_pack(uint64(<u64 as Element>::parse_from(val)?)).to_vec()),
        DataType::sint32 => Ok(stack_pack(sint32(<i32 as Element>::parse_from(val)?)).to_vec()),
        DataType::sint64 => Ok(stack_pack(sint64(<i64 as Element>::parse_from(val)?)).to_vec()),
        DataType::fixed32 => Ok(stack_pack(fixed32(<u32 as Element>::parse_from(val)?)).to_vec()),
        DataType::fixed64 => Ok(stack_pack(fixed64(<u64 as Element>::parse_from(val)?)).to_vec()),
        DataType::sfixed32 => Ok(stack_pack(sfixed32(<i32 as Element>::parse_from(val)?)).to_vec()),
        DataType::sfixed64 => Ok(stack_pack(sfixed64(<i64 as Element>::parse_from(val)?)).to_vec()),
        DataType::float => Err("float is not supported"),
        DataType::double => Err("double is not supported"),
        DataType::Bool => Err("Bool is not supported"),
        DataType::bytes => Ok(stack_pack(bytes(&<Vec<u8> as Element>::parse_from(val)?)).to_vec()),
        DataType::bytes16 => {
            Ok(stack_pack(bytes16(<[u8; 16] as Element>::parse_from(val)?)).to_vec())
        }
        DataType::bytes32 => {
            Ok(stack_pack(bytes32(<[u8; 32] as Element>::parse_from(val)?)).to_vec())
        }
        DataType::bytes64 => Err("bytes64 is not supported"),
        DataType::string => Ok(stack_pack(string(&<String as Element>::parse_from(val)?)).to_vec()),
        DataType::message => Err("message is not supported"),
    }
}

/////////////////////////////////////////////// Merge //////////////////////////////////////////////

pub trait Merge {
    type Error: From<tuple_key::Error>;
    fn merge(&self, cursor: impl Cursor<Error = Self::Error>) -> Result<Vec<u8>, Self::Error>;
}

impl Merge for TupleSchema {
    type Error = Error;

    fn merge(&self, mut cursor: impl Cursor<Error = Self::Error>) -> Result<Vec<u8>, Self::Error> {
        let mut builder = ProtoBuilder::default();
        let mut current_type = SchemaEntry::default();
        let mut frame_stack = Vec::new();
        let mut prev_tuple_key = TupleKey::default();
        'cursoring: while let Some(kvr) = cursor.next()? {
            if kvr.value.is_none() {
                // NOTE(rescrv): deletes have no place in tuple db; drop them silently.
                continue 'cursoring;
            }
            let schema_type = match self.lookup_schema_for_key(kvr.key)? {
                Some(schema_entry) => schema_entry,
                None => {
                    continue 'cursoring;
                }
            };
            let common = TupleKeyIterator::number_of_elements_in_common_prefix(
                prev_tuple_key.iter(),
                TupleKeyIterator::from(kvr.key),
            );
            prev_tuple_key = TupleKey::from(kvr.key);
            while !current_type.is_extendable_by(schema_type)
                || current_type.key().fields().len() > common
            {
                current_type.pop_field();
                if let Some(frame) = frame_stack.pop().unwrap() {
                    builder.end_message(frame);
                }
                // We know that we only push to the stacks in unison.
                // The zero key will always extend so we won't pop zero.
                assert_eq!(current_type.key().fields().len(), frame_stack.len());
            }

            // We'll go until we add everything necessary to extend.
            let mut tki = TupleKeyIterator::from(kvr.key);
            for _ in 0..2 * current_type.key().fields().len() {
                tki.next().ok_or(Error::Corruption {
                    core: ErrorCore::default(),
                    what: "tuple key exhausted".to_owned(),
                })?;
            }
            while !schema_type.is_extendable_by(&current_type) {
                // We know the current type should be extensible by construction.
                assert!(current_type.is_extendable_by(schema_type));
                // We know we have the longer key from the popping above.
                let ct_sz = current_type.key().fields().len();
                let st_sz = schema_type.key().fields().len();
                assert!(ct_sz < st_sz);
                let next_field = &schema_type.key().fields()[ct_sz];
                let (value_ty, terminal) = if ct_sz + 1 >= st_sz {
                    (schema_type.value(), true)
                } else {
                    (DataType::message, false)
                };
                let _ = tki.next().ok_or(Error::Corruption {
                    core: ErrorCore::default(),
                    what: "tuple key exhausted".to_owned(),
                })?;
                let tk_key = tki.next().ok_or(Error::Corruption {
                    core: ErrorCore::default(),
                    what: "tuple key exhausted".to_owned(),
                })?;
                let tk_key = parse_as_prototk(tk_key, next_field.ty()).map_err(|err| Error::Corruption {
                    core: ErrorCore::default(),
                    what: err.to_string(),
                })?;
                current_type.push_field(next_field.clone(), value_ty);
                match (next_field.ty(), value_ty) {
                    (DataType::unit, DataType::message) => {
                        let msg_tag = Tag {
                            field_number: FieldNumber::must(next_field.number()),
                            wire_type: value_ty.wire_type(),
                        };
                        frame_stack.push(Some(builder.begin_message(msg_tag, &[])));
                        if terminal {
                            builder.emit_inline(kvr.value.unwrap());
                        }
                    }
                    (DataType::unit, _) => {
                        let unit_tag = Tag {
                            field_number: FieldNumber::must(next_field.number()),
                            wire_type: value_ty.wire_type(),
                        };
                        frame_stack.push(None);
                        if terminal {
                            builder.emit_breakout(unit_tag, kvr.value.unwrap());
                        }
                    }
                    (_, _) => {
                        let map_tag = Tag {
                            field_number: FieldNumber::must(next_field.number()),
                            wire_type: WireType::LengthDelimited,
                        };
                        let key_tag: &[u8] = &stack_pack(Tag {
                            field_number: FieldNumber::must(1),
                            wire_type: next_field.ty().wire_type(),
                        })
                        .to_vec();
                        let value_tag: &[u8] = &stack_pack(Tag {
                            field_number: FieldNumber::must(2),
                            wire_type: value_ty.wire_type(),
                        })
                        .to_vec();
                        frame_stack.push(Some(
                            builder.begin_map_with_message(map_tag, &[key_tag, &tk_key, value_tag]),
                        ));
                        if terminal {
                            builder.emit_inline(kvr.value.unwrap());
                        }
                    }
                }
            }
        }
        while !frame_stack.is_empty() {
            if let Some(frame) = frame_stack.pop().unwrap() {
                builder.end_message(frame);
            }
        }
        builder.seal()
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod proto_builder {
    use super::*;

    #[test]
    fn default() {
        ProtoBuilder::default();
    }

    #[test]
    fn single_scalar_terminal() {
        let mut pb = ProtoBuilder::default();
        // A protocol buffers message of { 1:uint64 => 42 }.
        let tag = Tag {
            field_number: FieldNumber::must(1),
            wire_type: WireType::Varint,
        };
        pb.emit_breakout(tag, &[42]);
        // The message
        let msg: &[u8] = &pb.seal().unwrap();
        assert_eq!(&[8, 42], &msg);
    }

    #[test]
    fn single_message_terminal() {
        let mut pb = ProtoBuilder::default();
        // A protocol buffers message of { 1:uint64 => 42 }.
        let tag = Tag {
            field_number: FieldNumber::must(1),
            wire_type: WireType::LengthDelimited,
        };
        let begin = pb.begin_message(tag, &[&[8u8, 42]]);
        pb.end_message(begin);
        // The message
        let msg: &[u8] = &pb.seal().unwrap();
        assert_eq!(&[10, 2, 8, 42], &msg);
    }

    #[test]
    fn scalar_within_message() {
        let mut pb = ProtoBuilder::default();
        // Let's create a protocol buffers message with a breakout field.
        let tag = Tag {
            field_number: FieldNumber::must(1),
            wire_type: WireType::LengthDelimited,
        };
        let begin = pb.begin_message(tag, &[]);
        // A protocol buffers message of { 1:uint64 => 42 }.
        let tag = Tag {
            field_number: FieldNumber::must(2),
            wire_type: WireType::Varint,
        };
        pb.emit_breakout(tag, &[42]);
        // Finish the message
        pb.end_message(begin);
        let msg: &[u8] = &pb.seal().unwrap();
        assert_eq!(&[10, 2, 16, 42], &msg);
    }

    #[test]
    fn message_within_message1() {
        let mut pb = ProtoBuilder::default();
        // Let's create a protocol buffers message with a breakout field.
        let tag = Tag {
            field_number: FieldNumber::must(1),
            wire_type: WireType::LengthDelimited,
        };
        let begin = pb.begin_message(tag, &[]);
        // A protocol buffers message of { 1:uint64 => 42 }.
        pb.emit_inline(&[8, 42]);
        // Finish the message
        pb.end_message(begin);
        let msg: &[u8] = &pb.seal().unwrap();
        assert_eq!(&[10, 2, 8, 42], &msg);
    }

    #[test]
    fn message_within_message2() {
        let mut pb = ProtoBuilder::default();
        // Let's create a protocol buffers message with a breakout field.
        let tag = Tag {
            field_number: FieldNumber::must(1),
            wire_type: WireType::LengthDelimited,
        };
        let begin = pb.begin_message(tag, &[&[8u8, 42]]);
        // Finish the message
        pb.end_message(begin);
        let msg: &[u8] = &pb.seal().unwrap();
        assert_eq!(&[10, 2, 8, 42], &msg);
    }

    #[test]
    fn map_with_scalar_key_scalar_value_two_entries() {
        let mut pb = ProtoBuilder::default();
        // The first key for the map.  The value will be a string.
        let key_tag1: &[u8] = &stack_pack(Tag {
            field_number: FieldNumber::must(1),
            wire_type: WireType::Varint,
        })
        .to_vec();
        let key_buf1: &[u8] = &[42];
        let value_tag1: &[u8] = &stack_pack(Tag {
            field_number: FieldNumber::must(2),
            wire_type: WireType::LengthDelimited,
        })
        .to_vec();
        let value_buf1: &[u8] = &[103, 117, 105, 116, 97, 114];
        // The second key for the map.  The value will be a string.
        let key_tag2: &[u8] = &stack_pack(Tag {
            field_number: FieldNumber::must(1),
            wire_type: WireType::Varint,
        })
        .to_vec();
        let key_buf2: &[u8] = &[42];
        let value_tag2: &[u8] = &stack_pack(Tag {
            field_number: FieldNumber::must(2),
            wire_type: WireType::LengthDelimited,
        })
        .to_vec();
        let value_buf2: &[u8] = &[100, 114, 117, 109, 115];
        // Let's create a protocol buffers message with a map field.
        let tag = Tag {
            field_number: FieldNumber::must(7),
            wire_type: WireType::LengthDelimited,
        };
        let begin = pb.begin_map_with_message(tag.clone(), &[&key_tag1, &key_buf1, &value_tag1]);
        // Emit the value inline because we captured the value tag.
        pb.emit_inline(value_buf1);
        // Finish the message
        pb.end_message(begin);
        // Second verse, practically the first.
        let begin = pb.begin_map_with_message(tag, &[&key_tag2, &key_buf2, &value_tag2]);
        // Emit the value inline because we captured the value tag.
        pb.emit_inline(value_buf2);
        // Finish the message
        pb.end_message(begin);
        let msg: &[u8] = &pb.seal().unwrap();
        assert_eq!(
            &[
                58, 10, 8, 42, 18, 6, 103, 117, 105, 116, 97, 114, 58, 9, 8, 42, 18, 5, 100, 114,
                117, 109, 115
            ],
            &msg
        );
    }

    #[test]
    fn map_with_scalar_key_message_value() {
        let mut pb = ProtoBuilder::default();
        // The key for the map.  The value will be a message.
        let key_tag: &[u8] = &stack_pack(Tag {
            field_number: FieldNumber::must(1),
            wire_type: WireType::Varint,
        })
        .to_vec();
        let key_buf: &[u8] = &[42];
        let value_tag: &[u8] = &stack_pack(Tag {
            field_number: FieldNumber::must(2),
            wire_type: WireType::LengthDelimited,
        })
        .to_vec();
        let value_buf: &[u8] = &[0, 1, 2, 3, 4, 5, 6, 7];
        // Let's create a protocol buffers message with a map field.
        let tag = Tag {
            field_number: FieldNumber::must(7),
            wire_type: WireType::LengthDelimited,
        };
        let begin = pb.begin_map_with_message(tag, &[&key_tag, &key_buf, &value_tag]);
        // Emit the value inline because we captured the value tag.
        pb.emit_inline(value_buf);
        // Finish the message
        pb.end_message(begin);
        let msg: &[u8] = &pb.seal().unwrap();
        assert_eq!(&[58, 12, 8, 42, 18, 8, 0, 1, 2, 3, 4, 5, 6, 7], &msg);
    }

    #[test]
    fn map_with_message_key_message_value() {
        let mut pb = ProtoBuilder::default();
        // The key for the map.  The value will be a message.
        let key_tag: &[u8] = &stack_pack(Tag {
            field_number: FieldNumber::must(1),
            wire_type: WireType::LengthDelimited,
        })
        .to_vec();
        let key_buf: &[u8] = &[4, 42, 43, 44, 45];
        let value_tag: &[u8] = &stack_pack(Tag {
            field_number: FieldNumber::must(2),
            wire_type: WireType::LengthDelimited,
        })
        .to_vec();
        let value_buf: &[u8] = &[0, 1, 2, 3, 4, 5, 6, 7];
        // Let's create a protocol buffers message with a map field.
        let tag = Tag {
            field_number: FieldNumber::must(7),
            wire_type: WireType::LengthDelimited,
        };
        let begin = pb.begin_map_with_message(tag, &[&key_tag, &key_buf, &value_tag]);
        // Emit the value inline because we captured the value tag.
        pb.emit_inline(value_buf);
        // Finish the message
        pb.end_message(begin);
        let msg: &[u8] = &pb.seal().unwrap();
        assert_eq!(
            &[58, 16, 10, 4, 42, 43, 44, 45, 18, 8, 0, 1, 2, 3, 4, 5, 6, 7],
            &msg
        );
    }

    #[test]
    fn map_with_string_key_message_value() {
        let mut pb = ProtoBuilder::default();
        // The key for the map.  The value will be a message.
        let key_tag: &[u8] = &stack_pack(Tag {
            field_number: FieldNumber::must(1),
            wire_type: WireType::LengthDelimited,
        })
        .to_vec();
        let key_buf: &[u8] = &[5, 104, 101, 108, 108, 111];
        let value_tag: &[u8] = &stack_pack(Tag {
            field_number: FieldNumber::must(2),
            wire_type: WireType::LengthDelimited,
        })
        .to_vec();
        let value_buf: &[u8] = &[];
        // Let's create a protocol buffers message with a map field.
        let tag = Tag {
            field_number: FieldNumber::must(5),
            wire_type: WireType::LengthDelimited,
        };
        let begin = pb.begin_map_with_message(tag, &[&key_tag, &key_buf, &value_tag]);
        // Emit the value inline because we captured the value tag.
        pb.emit_inline(value_buf);
        // Finish the message
        pb.end_message(begin);
        let msg: &[u8] = &pb.seal().unwrap();
        assert_eq!(&[42, 9, 10, 5, 104, 101, 108, 108, 111, 18, 0], &msg);
    }
}

#[cfg(test)]
mod merge {
    use tuple_key::{SchemaField, SchemaKey};

    use super::*;

    #[derive(Default)]
    struct TestCursor {
        entries: Vec<(Vec<u8>, Vec<u8>)>,
        offset: usize,
    }

    impl TestCursor {
        fn add_entry(&mut self, key: &[u8], value: &[u8]) {
            self.entries.push((key.to_vec(), value.to_vec()))
        }
    }

    impl Cursor for TestCursor {
        type Error = Error;

        fn next(&mut self) -> Result<Option<KeyValueRef<'_>>, Self::Error> {
            if self.offset >= self.entries.len() {
                return Ok(None);
            }
            let offset = self.offset;
            self.offset += 1;
            Ok(Some(KeyValueRef {
                key: &self.entries[offset].0,
                timestamp: 0,
                value: Some(&self.entries[offset].1),
            }))
        }
    }

    fn test_schema() -> TupleSchema {
        let mut schema = TupleSchema::default();
        // Create field 1 such that it is a uint64 scalar.
        schema
            .add_to_schema(SchemaEntry::new(
                SchemaKey::new(vec![SchemaField::new(1, DataType::unit)]),
                DataType::uint64,
            ))
            .unwrap();
        // Create field 2 such that it is a message.
        schema
            .add_to_schema(SchemaEntry::new(
                SchemaKey::new(vec![SchemaField::new(2, DataType::unit)]),
                DataType::message,
            ))
            .unwrap();
        // Extend field 2 with a breakout.
        schema
            .add_to_schema(SchemaEntry::new(
                SchemaKey::new(vec![
                    SchemaField::new(2, DataType::unit),
                    SchemaField::new(3, DataType::unit),
                ]),
                DataType::uint64,
            ))
            .unwrap();
        // Create field 4 such that it is a map of varint to string.
        schema
            .add_to_schema(SchemaEntry::new(
                SchemaKey::new(vec![SchemaField::new(4, DataType::uint64)]),
                DataType::string,
            ))
            .unwrap();
        // Create field 5 such that it is a map of string to message.
        schema
            .add_to_schema(SchemaEntry::new(
                SchemaKey::new(vec![SchemaField::new(5, DataType::string)]),
                DataType::message,
            ))
            .unwrap();
        // Extend field 5 with a breakout.
        schema
            .add_to_schema(SchemaEntry::new(
                SchemaKey::new(vec![
                    SchemaField::new(5, DataType::string),
                    SchemaField::new(6, DataType::unit),
                ]),
                DataType::uint64,
            ))
            .unwrap();
        schema
    }

    #[test]
    fn default() {
        let cursor = TestCursor::default();
        let schema = test_schema();
        let buf = schema.merge(cursor).unwrap();
        assert!(buf.is_empty());
    }

    #[test]
    fn unit_to_uint64() {
        let mut cursor = TestCursor::default();
        let mut key = TupleKey::default();
        key.extend(FieldNumber::must(1));
        cursor.add_entry(key.as_bytes(), &[42]);
        let schema = test_schema();
        let buf: &[u8] = &schema.merge(cursor).unwrap();
        assert_eq!(&[8, 42], &buf);
    }

    #[test]
    fn unit_to_message() {
        let mut cursor = TestCursor::default();
        let mut key = TupleKey::default();
        key.extend(FieldNumber::must(2));
        cursor.add_entry(key.as_bytes(), &[8, 42]);
        let schema = test_schema();
        let buf: &[u8] = &schema.merge(cursor).unwrap();
        assert_eq!(&[18, 2, 8, 42], &buf);
    }

    #[test]
    fn scalar_within_message1() {
        let mut cursor = TestCursor::default();
        let mut key = TupleKey::default();
        key.extend(FieldNumber::must(2));
        key.extend(FieldNumber::must(3));
        cursor.add_entry(key.as_bytes(), &[42]);
        let schema = test_schema();
        let buf: &[u8] = &schema.merge(cursor).unwrap();
        assert_eq!(&[18, 2, 24, 42], &buf);
    }

    #[test]
    fn scalar_within_message2() {
        let mut cursor = TestCursor::default();
        let mut key = TupleKey::default();
        key.extend(FieldNumber::must(2));
        cursor.add_entry(key.as_bytes(), &[8, 33]);
        key.extend(FieldNumber::must(3));
        cursor.add_entry(key.as_bytes(), &[42]);
        let schema = test_schema();
        let buf: &[u8] = &schema.merge(cursor).unwrap();
        assert_eq!(&[18, 4, 8, 33, 24, 42], &buf);
    }

    #[test]
    fn map_uint64_to_string1() {
        let mut cursor = TestCursor::default();
        let mut key = TupleKey::default();
        key.extend_with_key(FieldNumber::must(4), 42u64);
        cursor.add_entry(key.as_bytes(), &[104, 101, 108, 108, 111]);
        let schema = test_schema();
        let buf: &[u8] = &schema.merge(cursor).unwrap();
        assert_eq!(&[34, 9, 8, 42, 18, 5, 104, 101, 108, 108, 111], &buf);
    }

    #[test]
    fn map_uint64_to_string2() {
        let mut cursor = TestCursor::default();
        let mut key = TupleKey::default();
        key.extend_with_key(FieldNumber::must(4), 42u64);
        cursor.add_entry(key.as_bytes(), &[104, 101, 108, 108, 111]);
        let mut key = TupleKey::default();
        key.extend_with_key(FieldNumber::must(4), 69u64);
        cursor.add_entry(key.as_bytes(), &[119, 111, 114, 108, 100]);
        let schema = test_schema();
        let buf: &[u8] = &schema.merge(cursor).unwrap();
        assert_eq!(
            &[
                34, 9, 8, 42, 18, 5, 104, 101, 108, 108, 111, 34, 9, 8, 69, 18, 5, 119, 111, 114,
                108, 100
            ],
            buf
        );
    }

    #[test]
    fn map_string_to_message1() {
        let mut cursor = TestCursor::default();
        let mut key = TupleKey::default();
        key.extend_with_key(FieldNumber::must(5), "hello".to_owned());
        cursor.add_entry(key.as_bytes(), &[]);
        let schema = test_schema();
        let buf: &[u8] = &schema.merge(cursor).unwrap();
        assert_eq!(&[42, 9, 10, 5, 104, 101, 108, 108, 111, 18, 0], &buf);
    }

    #[test]
    fn map_string_to_message2() {
        let mut cursor = TestCursor::default();
        let mut key = TupleKey::default();
        key.extend_with_key(FieldNumber::must(5), "hello".to_owned());
        cursor.add_entry(key.as_bytes(), &[]);
        key.extend(FieldNumber::must(6));
        cursor.add_entry(key.as_bytes(), &[33]);
        let schema = test_schema();
        let buf: &[u8] = &schema.merge(cursor).unwrap();
        assert_eq!(
            &[42, 11, 10, 5, 104, 101, 108, 108, 111, 18, 2, 48, 33],
            &buf
        );
    }
}
