use std::io::{self, Write};
use {Message, OutputStream};
use output_stream::OutputStreamBackend;
use output_writer::OutputWriter;
use wire_type::WireType::*;

pub struct Serializer {
    size: usize,
    nested: Vec<usize>
}

impl Serializer {
    pub fn new() -> Serializer {
        Serializer {
            size: 0,
            nested: Vec::new()
        }
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn serialize<M: Message, W: Write>(&self, msg: &M, writer: &mut W) -> io::Result<()> {
        let mut out = OutputWriter::new(self.nested.as_slice(), writer);

        try!(msg.serialize(&mut out));

        Ok(())
    }

    pub fn serialize_into<M: Message>(&self, msg: &M, dst: &mut [u8]) -> io::Result<()> {
        if self.size > dst.len() {
            return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "destination buffer not large enough to contain serialized message",
                    None));
        }

        self.serialize(msg, &mut io::BufWriter::new(dst))
    }
}

impl OutputStreamBackend for Serializer {
    fn write_byte(&mut self, _: u8) -> io::Result<()> {
        // TODO: Handle overflow
        self.size += 1;
        Ok(())
    }

    fn write_bytes(&mut self, bytes: &[u8]) -> io::Result<()> {
        // TODO: Handle overflow
        self.size += bytes.len();
        Ok(())
    }
}

impl OutputStream for Serializer {
    fn write_message_field<M: Message>(&mut self, field: usize, msg: &M) -> io::Result<()> {
        let position = self.nested.len();
        let prev_count = self.size;

        // Add 0 as a placeholder for the current message
        self.nested.push(0);

        try!(msg.serialize(self));

        let nested_size = self.size - prev_count;

        if nested_size > 0 {
            self.nested[position] = nested_size;

            try!(self.write_head(field, LengthDelimited));
            try!(self.write_usize(nested_size));
        }

        Ok(())
    }
}
