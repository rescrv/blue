use std::fmt::Debug;

use buffertk::{stack_pack, v64, Packable};
use keyvalint::KeyValueRef;
use prototk::field_types::*;
use prototk::{FieldNumber, Tag, WireType};
use tuple_key::{Element, KeyDataType, TupleKey, TupleKeyIterator};
use zerror::Z;
use zerror_core::ErrorCore;

use super::{DataType, Error, IoToZ, Schema, SchemaEntry};

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
            .with_info("post_u64_offset", post_u64_offset)
            .with_info("in_progress_offset", in_progress_offset);
        }
        let msg_sz_v64 = v64::from(msg_sz);
        let msg_sz_v64_pack_sz = msg_sz_v64.pack_sz();
        if msg_sz_v64_pack_sz > 8 {
            return Err(Error::LogicError {
                core: ErrorCore::default(),
                what: "offset_too_small".to_owned(),
            })
            .as_z()
            .with_info("msg_sz_v64_pack_sz", msg_sz_v64_pack_sz);
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
                        .with_info("in_progress_idx", in_progress_idx)
                        .with_info("frame_idx", frame_idx);
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
                        .with_info("in_progress_idx", in_progress_idx)
                        .with_info("frame_idx", frame_idx);
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

///////////////////////////////////////////// utilities ////////////////////////////////////////////

pub fn parse_as_prototk(val: &[u8], ty: KeyDataType) -> Result<Vec<u8>, &'static str> {
    match ty {
        KeyDataType::unit => Ok(Vec::new()),
        KeyDataType::fixed32 => {
            Ok(stack_pack(fixed32(<u32 as Element>::parse_from(val)?)).to_vec())
        }
        KeyDataType::fixed64 => {
            Ok(stack_pack(fixed64(<u64 as Element>::parse_from(val)?)).to_vec())
        }
        KeyDataType::sfixed32 => {
            Ok(stack_pack(sfixed32(<i32 as Element>::parse_from(val)?)).to_vec())
        }
        KeyDataType::sfixed64 => {
            Ok(stack_pack(sfixed64(<i64 as Element>::parse_from(val)?)).to_vec())
        }
        KeyDataType::string => {
            Ok(stack_pack(string(&<String as Element>::parse_from(val)?)).to_vec())
        }
    }
}

/////////////////////////////////////////// ObjectBuilder //////////////////////////////////////////

pub struct ObjectBuilder {
    schema: Schema,
    builder: ProtoBuilder,
    current_type: SchemaEntry,
    frame_stack: Vec<Option<MessageFrameWrapper>>,
    prev_tuple_key: TupleKey,
}

impl ObjectBuilder {
    pub fn new(schema: Schema) -> Self {
        let builder = ProtoBuilder::default();
        let current_type = SchemaEntry::default();
        let frame_stack = vec![];
        let prev_tuple_key = TupleKey::default();
        Self {
            schema,
            builder,
            current_type,
            frame_stack,
            prev_tuple_key,
        }
    }

    pub fn next(&mut self, kvr: KeyValueRef) -> Result<(), Error> {
        if kvr.value.is_none() {
            // NOTE(rescrv): deletes have no place in protoql; drop them silently.
            return Ok(());
        }
        let schema_entry = match self.schema.lookup_schema_for_key(kvr.key)? {
            Some(schema_entry) => schema_entry,
            None => {
                // NOTE(rescrv): keys that don't match schema have no place in protoql; drop them silently.
                return Ok(());
            }
        };
        let common = TupleKeyIterator::number_of_elements_in_common_prefix(
            self.prev_tuple_key.iter(),
            TupleKeyIterator::from(kvr.key),
        );
        self.prev_tuple_key = TupleKey::from(kvr.key);
        while !self.current_type.is_extendable_by(schema_entry)
            || self.current_type.key().elements().len() > common
        {
            self.current_type.pop_field();
            // SAFETY(rescrv): We know we pushed onto the stack.
            if let Some(frame) = self.frame_stack.pop().unwrap() {
                self.builder.end_message(frame);
            }
            // We know that we only push to the stacks in unison.
            // The zero key will always extend so we won't pop zero.
            assert_eq!(
                self.current_type.key().elements().len(),
                self.frame_stack.len()
            );
        }
        // We'll go until we add everything necessary to extend.
        let mut tki = TupleKeyIterator::from(kvr.key);
        for _ in 0..2 * self.current_type.key().elements().len() {
            tki.next().ok_or(Error::Corruption {
                core: ErrorCore::default(),
                what: "tuple key exhausted".to_owned(),
            })?;
        }
        while !schema_entry.is_extendable_by(&self.current_type) {
            // We know the current type should be extensible by construction.
            assert!(self.current_type.is_extendable_by(schema_entry));
            // We know we have the longer key from the popping above.
            let ct_sz = self.current_type.key().elements().len();
            let st_sz = schema_entry.key().elements().len();
            assert!(ct_sz < st_sz);
            let next_field = &schema_entry.key().elements()[ct_sz];
            let (value_ty, terminal) = if ct_sz + 1 >= st_sz {
                (schema_entry.value(), true)
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
            let tk_key =
                parse_as_prototk(tk_key, next_field.ty()).map_err(|err| Error::Corruption {
                    core: ErrorCore::default(),
                    what: err.to_string(),
                })?;
            self.current_type.push_field(next_field.clone(), value_ty);
            match (next_field.ty(), value_ty) {
                (KeyDataType::unit, DataType::message) => {
                    let msg_tag = Tag {
                        field_number: next_field.number(),
                        wire_type: value_ty.wire_type(),
                    };
                    self.frame_stack
                        .push(Some(self.builder.begin_message(msg_tag, &[])));
                    if terminal {
                        self.builder.emit_inline(kvr.value.unwrap());
                    }
                }
                (KeyDataType::unit, _) => {
                    let unit_tag = Tag {
                        field_number: next_field.number(),
                        wire_type: value_ty.wire_type(),
                    };
                    self.frame_stack.push(None);
                    if terminal {
                        self.builder.emit_breakout(unit_tag, kvr.value.unwrap());
                    }
                }
                (_, _) => {
                    let map_tag = Tag {
                        field_number: next_field.number(),
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
                    self.frame_stack.push(Some(
                        self.builder
                            .begin_map_with_message(map_tag, &[key_tag, &tk_key, value_tag]),
                    ));
                    if terminal {
                        self.builder.emit_inline(kvr.value.unwrap());
                    }
                }
            }
        }
        Ok(())
    }

    pub fn seal(mut self) -> Result<Vec<u8>, Error> {
        while !self.frame_stack.is_empty() {
            // SAFETY(rescrv): We're popping from a non-empty stack.
            if let Some(frame) = self.frame_stack.pop().unwrap() {
                self.builder.end_message(frame);
            }
        }
        self.builder.seal()
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
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

    use tuple_key::Direction;

    use crate::{SchemaKey, SchemaKeyElement};

    use super::*;

    fn test_schema() -> Schema {
        let mut schema = Schema::default();
        // Create field 1 such that it is a uint64 scalar.
        schema
            .add_to_schema(SchemaEntry::new(
                SchemaKey::new(vec![SchemaKeyElement::new(
                    FieldNumber::must(1),
                    KeyDataType::unit,
                    Direction::Forward,
                )]),
                DataType::uint64,
            ))
            .unwrap();
        // Create field 2 such that it is a message.
        schema
            .add_to_schema(SchemaEntry::new(
                SchemaKey::new(vec![SchemaKeyElement::new(
                    FieldNumber::must(2),
                    KeyDataType::unit,
                    Direction::Forward,
                )]),
                DataType::message,
            ))
            .unwrap();
        // Extend field 2 with a breakout.
        schema
            .add_to_schema(SchemaEntry::new(
                SchemaKey::new(vec![
                    SchemaKeyElement::new(
                        FieldNumber::must(2),
                        KeyDataType::unit,
                        Direction::Forward,
                    ),
                    SchemaKeyElement::new(
                        FieldNumber::must(3),
                        KeyDataType::unit,
                        Direction::Forward,
                    ),
                ]),
                DataType::uint64,
            ))
            .unwrap();
        // Create field 4 such that it is a map of fixed64 to string.
        schema
            .add_to_schema(SchemaEntry::new(
                SchemaKey::new(vec![SchemaKeyElement::new(
                    FieldNumber::must(4),
                    KeyDataType::fixed64,
                    Direction::Forward,
                )]),
                DataType::string,
            ))
            .unwrap();
        // Create field 5 such that it is a map of string to message.
        schema
            .add_to_schema(SchemaEntry::new(
                SchemaKey::new(vec![SchemaKeyElement::new(
                    FieldNumber::must(5),
                    KeyDataType::string,
                    Direction::Forward,
                )]),
                DataType::message,
            ))
            .unwrap();
        // Extend field 5 with a breakout.
        schema
            .add_to_schema(SchemaEntry::new(
                SchemaKey::new(vec![
                    SchemaKeyElement::new(
                        FieldNumber::must(5),
                        KeyDataType::string,
                        Direction::Forward,
                    ),
                    SchemaKeyElement::new(
                        FieldNumber::must(6),
                        KeyDataType::unit,
                        Direction::Forward,
                    ),
                ]),
                DataType::uint64,
            ))
            .unwrap();
        schema
    }

    #[test]
    fn obj_builder_default() {
        let schema = test_schema();
        let obj_builder = ObjectBuilder::new(schema);
        let buf = obj_builder.seal().unwrap();
        assert!(buf.is_empty());
    }

    #[test]
    fn unit_to_uint64() {
        let mut key = TupleKey::default();
        key.extend(FieldNumber::must(1));
        let schema = test_schema();
        let mut obj_builder = ObjectBuilder::new(schema);
        obj_builder
            .next(KeyValueRef {
                key: key.as_bytes(),
                timestamp: 0,
                value: Some(&[42]),
            })
            .unwrap();
        let buf = obj_builder.seal().unwrap();
        assert_eq!(&[8, 42], &buf.as_slice());
    }

    #[test]
    fn unit_to_message() {
        let mut key = TupleKey::default();
        key.extend(FieldNumber::must(2));
        let schema = test_schema();
        let mut obj_builder = ObjectBuilder::new(schema);
        obj_builder
            .next(KeyValueRef {
                key: key.as_bytes(),
                timestamp: 0,
                value: Some(&[8, 42]),
            })
            .unwrap();
        let buf = obj_builder.seal().unwrap();
        assert_eq!(&[18, 2, 8, 42], &buf.as_slice());
    }

    #[test]
    fn scalar_within_message1() {
        let mut key = TupleKey::default();
        key.extend(FieldNumber::must(2));
        key.extend(FieldNumber::must(3));
        let schema = test_schema();
        let mut obj_builder = ObjectBuilder::new(schema);
        obj_builder
            .next(KeyValueRef {
                key: key.as_bytes(),
                timestamp: 0,
                value: Some(&[42]),
            })
            .unwrap();
        let buf = obj_builder.seal().unwrap();
        assert_eq!(&[18, 2, 24, 42], &buf.as_slice());
    }

    #[test]
    fn scalar_within_message2() {
        let mut key1 = TupleKey::default();
        key1.extend(FieldNumber::must(2));
        let mut key2 = key1.clone();
        key2.extend(FieldNumber::must(3));
        let schema = test_schema();
        let mut obj_builder = ObjectBuilder::new(schema);
        obj_builder
            .next(KeyValueRef {
                key: key1.as_bytes(),
                timestamp: 0,
                value: Some(&[8, 33]),
            })
            .unwrap();
        obj_builder
            .next(KeyValueRef {
                key: key2.as_bytes(),
                timestamp: 0,
                value: Some(&[42]),
            })
            .unwrap();
        let buf = obj_builder.seal().unwrap();
        assert_eq!(&[18, 4, 8, 33, 24, 42], &buf.as_slice());
    }

    #[test]
    fn map_uint64_to_string1() {
        let mut key = TupleKey::default();
        key.extend_with_key(FieldNumber::must(4), 42u64, Direction::Forward);
        let schema = test_schema();
        let mut obj_builder = ObjectBuilder::new(schema);
        obj_builder
            .next(KeyValueRef {
                key: key.as_bytes(),
                timestamp: 0,
                value: Some(&[104, 101, 108, 108, 111]),
            })
            .unwrap();
        let buf = obj_builder.seal().unwrap();
        assert_eq!(
            &[34, 16, 9, 42, 0, 0, 0, 0, 0, 0, 0, 18, 5, 104, 101, 108, 108, 111],
            &buf.as_slice()
        );
    }

    #[test]
    fn map_uint64_to_string2() {
        let mut key1 = TupleKey::default();
        key1.extend_with_key(FieldNumber::must(4), 42u64, Direction::Forward);
        let mut key2 = TupleKey::default();
        key2.extend_with_key(FieldNumber::must(4), 69u64, Direction::Forward);
        let schema = test_schema();
        let mut obj_builder = ObjectBuilder::new(schema);
        obj_builder
            .next(KeyValueRef {
                key: key1.as_bytes(),
                timestamp: 0,
                value: Some(&[104, 101, 108, 108, 111]),
            })
            .unwrap();
        obj_builder
            .next(KeyValueRef {
                key: key2.as_bytes(),
                timestamp: 0,
                value: Some(&[119, 111, 114, 108, 100]),
            })
            .unwrap();
        let buf = obj_builder.seal().unwrap();
        assert_eq!(
            &[
                34, 16, 9, 42, 0, 0, 0, 0, 0, 0, 0, 18, 5, 104, 101, 108, 108, 111, 34, 16, 9, 69,
                0, 0, 0, 0, 0, 0, 0, 18, 5, 119, 111, 114, 108, 100
            ],
            &buf.as_slice(),
        );
    }

    #[test]
    fn map_string_to_message1() {
        let mut key = TupleKey::default();
        key.extend_with_key(FieldNumber::must(5), "hello".to_owned(), Direction::Forward);
        let schema = test_schema();
        let mut obj_builder = ObjectBuilder::new(schema);
        obj_builder
            .next(KeyValueRef {
                key: key.as_bytes(),
                timestamp: 0,
                value: Some(&[]),
            })
            .unwrap();
        let buf = obj_builder.seal().unwrap();
        assert_eq!(
            &[42, 9, 10, 5, 104, 101, 108, 108, 111, 18, 0],
            &buf.as_slice()
        );
    }

    #[test]
    fn map_string_to_message2() {
        let mut key1 = TupleKey::default();
        key1.extend_with_key(FieldNumber::must(5), "hello".to_owned(), Direction::Forward);
        let mut key2 = key1.clone();
        key2.extend(FieldNumber::must(6));
        let schema = test_schema();
        let mut obj_builder = ObjectBuilder::new(schema);
        obj_builder
            .next(KeyValueRef {
                key: key1.as_bytes(),
                timestamp: 0,
                value: Some(&[]),
            })
            .unwrap();
        obj_builder
            .next(KeyValueRef {
                key: key2.as_bytes(),
                timestamp: 0,
                value: Some(&[33]),
            })
            .unwrap();
        let buf = obj_builder.seal().unwrap();
        assert_eq!(
            &[42, 11, 10, 5, 104, 101, 108, 108, 111, 18, 2, 48, 33],
            &buf.as_slice(),
        );
    }
}
