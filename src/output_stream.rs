use std::io;
use wire_type::WireType;
use wire_type::WireType::*;
use {Message};

pub trait OutputStream : OutputStreamBackend {
    /// Writes a nested message with the specified field number
    fn write_message_field<M: Message>(&mut self, field: uint, msg: &M) -> io::IoResult<()>;

    fn write_repeated_message_field<'a, M:'a + Message, I: Iterator<Item=&'a M>>(&mut self, field: uint, mut msgs: I) -> io::IoResult<()> {
        for msg in msgs {
            try!(self.write_message_field(field, msg));
        }

        Ok(())
    }

    fn write_varint_field<F: NumField>(&mut self, field: uint, val: F) -> io::IoResult<()> {
        val.write_varint_field(field, self)
    }

    fn write_byte_field(&mut self, field: uint, val: &[u8]) -> io::IoResult<()> {
        try!(self.write_head(field, LengthDelimited));
        try!(self.write_uint(val.len()));
        try!(self.write_bytes(val));
        Ok(())
    }

    fn write_repeated_byte_field<'a, I: Iterator<Item=&'a [u8]>>(&mut self, field: uint, mut vals: I) -> io::IoResult<()> {
        for val in vals {
            try!(self.write_byte_field(field, val));
        }

        Ok(())
    }

    fn write_str_field(&mut self, field: uint, val: &str) -> io::IoResult<()> {
        self.write_byte_field(field, val.as_bytes())
    }

    fn write_opt_str_field<S: Str>(&mut self, field: uint, val: Option<S>) -> io::IoResult<()> {
        match val {
            Some(s) => try!(self.write_str_field(field, s.as_slice())),
            None => {}
        }

        Ok(())
    }

    fn write_repeated_str_field<'a, I: Iterator<Item=&'a str>>(&mut self, field: uint, vals: I) -> io::IoResult<()> {
        self.write_repeated_byte_field(field, vals.map(|s| s.as_bytes()))
    }
}

pub trait OutputStreamBackend : Sized {
    fn write_bytes(&mut self, bytes: &[u8]) -> io::IoResult<()>;

    // Write a single byte
    fn write_byte(&mut self, byte: u8) -> io::IoResult<()>;

    fn write_uint(&mut self, val: uint) -> io::IoResult<()> {
        self.write_unsigned_varint(val as u64)
    }

    fn write_unsigned_varint(&mut self, mut val: u64) -> io::IoResult<()> {
        loop {
            // Grab up to 7 bits of the number
            let bits = (val & 0x7f) as u8;

            // Shift the remaining bits
            val >>= 7;

            if val == 0 {
                try!(self.write_byte(bits));
                return Ok(());
            }

            try!(self.write_byte(bits | 0x80));
        }
    }

    fn write_head(&mut self, field: uint, wire_type: WireType) -> io::IoResult<()> {
        // TODO: Handle overflow
        let bits = (field << 3) | (wire_type as uint);
        try!(self.write_uint(bits));
        Ok(())
    }
}

pub trait NumField {
    fn write_varint_field<O: OutputStream>(self, field: uint, out: &mut O) -> io::IoResult<()>;
}

impl NumField for uint {
    fn write_varint_field<O: OutputStream>(self, field: uint, out: &mut O) -> io::IoResult<()> {
        try!(out.write_head(field, Varint));
        try!(out.write_uint(self));
        Ok(())
    }
}

impl NumField for u64 {
    fn write_varint_field<O: OutputStream>(self, field: uint, out: &mut O) -> io::IoResult<()> {
        try!(out.write_head(field, Varint));
        try!(out.write_unsigned_varint(self));
        Ok(())
    }
}

impl<F: NumField> NumField for Option<F> {
    fn write_varint_field<O: OutputStream>(self, field: uint, out: &mut O) -> io::IoResult<()> {
        match self {
            Some(v) => v.write_varint_field(field, out),
            None => Ok(())
        }
    }
}
