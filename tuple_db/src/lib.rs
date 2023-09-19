use buffertk::{stack_pack, v64, Packable};

use prototk::{FieldNumber, Tag, WireType};

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
    frame: MessageFrame,
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
            frame: begin,
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
            frame: begin,
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
        stack_pack(tag).append_to_vec(&mut self.msg);
        self.msg.extend_from_slice(value);
    }

    fn shift_frame(&mut self, offset_of_u64: usize) -> Result<usize, &'static str> {
        todo!();
    }

    fn seal(mut self) -> Result<Vec<u8>, &'static str> {
        let mut in_progress = Vec::new();
        let mut bytes_dropped = 0;
        while !self.frames.is_empty() {
            let frame_idx = self.frames.len() - 1;
            let back = self.frames[frame_idx];
            match back {
                MessageFrame::Begin { offset: begin_offset, tag_sz } => {
                    if in_progress.is_empty() {
                        return Err("logic error: in_progress was empty");
                    }
                    let (in_progress_offset, in_progress_idx) = in_progress.pop().unwrap();
                    if in_progress_idx != frame_idx {
                        return Err("logic error: index miscalculation");
                    }
                    let post_tag_offset = begin_offset + tag_sz + 8;
                    if post_tag_offset >= in_progress_offset {
                        return Err("logic error: offset too small");
                    }
                    let to_move = in_progress_offset - post_tag_offset;
                    let to_move_v64 = v64::from(to_move);
                    if to_move_v64.pack_sz() > 8 {
                        return Err("logic error: data too large");
                    }
                    let to_move_v64_pack_sz = to_move_v64.pack_sz();
                    let newly_dropped_bytes = 8 - to_move_v64_pack_sz;
                    for src in post_tag_offset..in_progress_offset {
                        let dst = src + bytes_dropped;
                        self.msg[dst] = self.msg[src];
                    }
                    for tag_byte in begin_offset..begin_offset + tag_sz {
                        self.msg[tag_byte + newly_dropped_bytes] = self.msg[tag_byte];
                    }
                    let length_start = begin_offset + tag_sz + newly_dropped_bytes;
                    let length_slice = &mut self.msg[length_start..length_start + to_move_v64_pack_sz];
                    stack_pack(to_move_v64).into_slice(length_slice);
                    bytes_dropped += newly_dropped_bytes;
                    self.frames.pop();
                },
                MessageFrame::BeginMapWithMessage { offset, tag_sz, key_offset } => {
                    if in_progress.is_empty() {
                        return Err("logic error: in_progress was empty");
                    }
                    let (in_progress_offset, in_progress_idx) = in_progress.pop().unwrap();
                    /*
                    if in_progress_idx != frame_idx {
                        return Err("logic error: index miscalculation");
                    }
                    let post_tag_offset = begin_offset + tag_sz + 8;
                    if post_tag_offset >= in_progress_offset {
                        return Err("logic error: offset too small");
                    }
                    let to_move = in_progress_offset - post_tag_offset;
                    let to_move_v64 = v64::from(to_move);
                    if to_move_v64.pack_sz() > 8 {
                        return Err("logic error: data too large");
                    }
                    let to_move_v64_pack_sz = to_move_v64.pack_sz();
                    let newly_dropped_bytes = 8 - to_move_v64_pack_sz;
                    for src in post_tag_offset..in_progress_offset {
                        let dst = src + bytes_dropped;
                        self.msg[dst] = self.msg[src];
                    }
                    for tag_byte in begin_offset..begin_offset + tag_sz {
                        self.msg[tag_byte + newly_dropped_bytes] = self.msg[tag_byte];
                    }
                    let length_start = begin_offset + tag_sz + newly_dropped_bytes;
                    let length_slice = &mut self.msg[length_start..length_start + to_move_v64_pack_sz];
                    stack_pack(to_move_v64).into_slice(length_slice);
                    bytes_dropped += newly_dropped_bytes;
                    self.frames.pop();
                    */
                },
                MessageFrame::End { offset, begin } => {
                    in_progress.push((offset, begin));
                    self.frames.pop();
                },
            }
        }
        for i in 0..self.msg.len() - bytes_dropped {
            self.msg[i] = self.msg[i + bytes_dropped];
        }
        self.msg.truncate(self.msg.len() - bytes_dropped);
        Ok(self.msg)
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
        let tag = Tag { field_number: FieldNumber::must(1), wire_type: WireType::Varint };
        pb.emit_breakout(tag, &[42]);
        // The message
        let msg: &[u8] = &pb.seal().unwrap();
        assert_eq!(&[8, 42], &msg);
    }

    #[test]
    fn single_message_terminal() {
        let mut pb = ProtoBuilder::default();
        // A protocol buffers message of { 1:uint64 => 42 }.
        let tag = Tag { field_number: FieldNumber::must(1), wire_type: WireType::LengthDelimited };
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
        let tag = Tag { field_number: FieldNumber::must(1), wire_type: WireType::LengthDelimited };
        let begin = pb.begin_message(tag, &[]);
        // A protocol buffers message of { 1:uint64 => 42 }.
        let tag = Tag { field_number: FieldNumber::must(2), wire_type: WireType::Varint };
        pb.emit_breakout(tag, &[42]);
        // Finish the message
        pb.end_message(begin);
        let msg: &[u8] = &pb.seal().unwrap();
        assert_eq!(&[10, 2, 16, 42], &msg);
    }

    #[test]
    fn message_within_message() {
        let mut pb = ProtoBuilder::default();
        // Let's create a protocol buffers message with a breakout field.
        let tag = Tag { field_number: FieldNumber::must(1), wire_type: WireType::LengthDelimited };
        let begin = pb.begin_message(tag, &[]);
        // A protocol buffers message of { 1:uint64 => 42 }.
        pb.emit_inline(&[8, 42]);
        // Finish the message
        pb.end_message(begin);
        let msg: &[u8] = &pb.seal().unwrap();
        assert_eq!(&[10, 2, 8, 42], &msg);
    }
}
