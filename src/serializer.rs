use std::io;
use {Message, OutputStream};
use output_stream::OutputStreamBackend;
use output_writer::OutputWriter;
use wire_type::{LengthDelimited};

pub struct Serializer {
    size: uint,
    nested: Vec<uint>
}

impl Serializer {
    pub fn new() -> Serializer {
        Serializer {
            size: 0,
            nested: Vec::new()
        }
    }

    pub fn size(&self) -> uint {
        self.size
    }

    pub fn serialize<M: Message, W: Writer>(&self, msg: &M, writer: &mut W) -> io::IoResult<()> {
        let mut out = OutputWriter::new(self.nested.as_slice(), writer);

        try!(msg.serialize(&mut out));

        Ok(())
    }

    pub fn serialize_into<M: Message>(&self, msg: &M, dst: &mut [u8]) -> io::IoResult<()> {
        if self.size > dst.len() {
            return Err(io::IoError {
                kind: io::InvalidInput,
                desc: "destination buffer not large enough to contain serialized message",
                detail: None
            });
        }

        self.serialize(msg, &mut io::BufWriter::new(dst))
    }
}

impl OutputStreamBackend for Serializer {
    fn write_byte(&mut self, _: u8) -> io::IoResult<()> {
        // TODO: Handle overflow
        self.size += 1;
        Ok(())
    }

    fn write_bytes(&mut self, bytes: &[u8]) -> io::IoResult<()> {
        // TODO: Handle overflow
        self.size += bytes.len();
        Ok(())
    }
}

impl OutputStream for Serializer {
    fn write_message_field<M: Message>(&mut self, field: uint, msg: &M) -> io::IoResult<()> {
        let position = self.nested.len();
        let prev_count = self.size;

        // Add 0 as a placeholder for the current message
        self.nested.push(0);

        try!(msg.serialize(self));

        let nested_size = self.size - prev_count;

        if nested_size > 0 {
            *self.nested.get_mut(position) = nested_size;

            try!(self.write_head(field, LengthDelimited));
            try!(self.write_uint(nested_size));
        }

        Ok(())
    }
}
