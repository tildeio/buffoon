use std::io::{Writer, IoResult, IoError, OtherIoError};
use output_stream::OutputStreamBackend;
use wire_type::WireType::*;
use {Message, OutputStream};

pub struct OutputWriter<'a, W:'a> {
    curr: uint,
    nested: &'a [uint],
    writer: &'a mut W
}

impl<'a, W: Writer> OutputWriter<'a, W> {
    pub fn new(nested: &'a [uint], writer: &'a mut W) -> OutputWriter<'a, W> {
        OutputWriter {
            curr: 0,
            nested: nested,
            writer: writer
        }
    }
}

impl<'a, W: Writer> OutputStreamBackend for OutputWriter<'a, W> {
    fn write_byte(&mut self, byte: u8) -> IoResult<()> {
        self.writer.write_u8(byte)
    }

    fn write_bytes(&mut self, bytes: &[u8]) -> IoResult<()> {
        self.writer.write(bytes)
    }
}

impl<'a, W: Writer> OutputStream for OutputWriter<'a, W> {
    fn write_message_field<M: Message>(&mut self, field: uint, msg: &M) -> IoResult<()> {
        if self.curr >= self.nested.len() {
            return invalid_serializer();
        }

        let size = self.nested[self.curr];
        self.curr += 1;

        if size > 0 {
            try!(self.write_head(field, LengthDelimited));
            try!(self.write_uint(size));

            try!(msg.serialize(self));
        };

        Ok(())
    }
}

fn invalid_serializer<T>() -> IoResult<T> {
    Err(IoError {
        kind: OtherIoError,
        desc: "invalid serializer for current message",
        detail: None
    })
}
