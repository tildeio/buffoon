use std::fmt;
use std::old_io::{Reader, IoResult, IoError, EndOfFile, InvalidInput};
use std::num::from_u64;
use wire_type::WireType;
use wire_type::WireType::*;

pub struct InputStream<'a, R: 'a> {
    reader: &'a mut R
}

impl<'a, R: Reader> InputStream<'a, R> {
    pub fn new(reader: &'a mut R) -> InputStream<'a, R> {
        InputStream { reader: reader }
    }

    pub fn read_field<'b>(&'b mut self) -> IoResult<Option<Field<'b, 'a, R>>> {
        // Read the header byte. In this case, EOF errors are OK as they signify
        // that there is no field to read
        let head = match self.read_uint() {
            Ok(h) => h,
            Err(e) => {
                match e.kind {
                    EndOfFile => return Ok(None),
                    _ => return Err(e)
                }
            }
        };

        // Extract the type of the field
        let wire_type = match WireType::from_uint(head & 0x7) {
            Some(res) => res,
            None => return Err(unexpected_output("invalid wire type"))
        };

        Ok(Some(Field {
            input: self,
            tag: head >> 3,
            wire_type: wire_type
        }))
    }

    fn read_uint(&mut self) -> IoResult<uint> {
        Ok(match from_u64(try!(self.read_unsigned_varint())) {
            Some(val) => val,
            None => return Err(unexpected_output("requested value could not fit in uint"))
        })
    }

    // TODO: Handle overflow
    fn read_unsigned_varint(&mut self) -> IoResult<u64> {
        let mut ret: u64 = 0;
        let mut shift = 0;

        loop {
            let byte = try!(self.read_byte());
            let bits = (byte & 0x7f) as u64;

            ret |= bits << shift;
            shift += 7;

            if !has_msb(byte) {
                return Ok(ret);
            }
        }
    }

    fn read_length_delimited(&mut self) -> IoResult<Vec<u8>> {
        let len = try!(self.read_uint());
        self.read_exact(len)
    }

    fn skip(&mut self, mut n: uint) -> IoResult<()> {
        // Yes this is a terrible implementation, but something better depends on:
        // https://github.com/rust-lang/rust/issues/13989
        while n > 0 {
            try!(self.reader.read_byte());
            n -= 1;
        }

        Ok(())
    }

    fn read_exact(&mut self, len: uint) -> IoResult<Vec<u8>> {
        self.reader.read_exact(len)
    }

    #[inline]
    fn read_byte(&mut self) -> IoResult<u8> {
        self.reader.read_byte()
    }
}

pub struct Field<'b, 'a:'b, R:'a> {
    input: &'b mut InputStream<'a, R>,
    pub tag: uint,
    wire_type: WireType
}

impl<'a, 'b, R: Reader> Field<'a, 'b, R> {
    pub fn get_tag(&self) -> uint {
        self.tag
    }

    pub fn skip(&mut self) -> IoResult<()> {
        match self.wire_type {
            Varint => {
                try!(self.input.read_unsigned_varint());
            }
            SixtyFourBit => unimplemented!(),
            LengthDelimited => {
                let len = try!(self.input.read_uint());
                try!(self.input.skip(len));
            },
            StartGroup => unimplemented!(),
            EndGroup => unimplemented!(),
            ThirtyTwoBit => unimplemented!()
        }

        Ok(())
    }

    pub fn read_uint(&mut self) -> IoResult<uint> {
        match self.wire_type {
            Varint => self.input.read_uint(),
            _ => Err(unexpected_output("field type was not varint"))
        }
    }

    pub fn read_string(&mut self) -> IoResult<String> {
        match String::from_utf8(try!(self.read_bytes())) {
            Ok(s) => Ok(s),
            Err(_) => Err(unexpected_output("string not UTF-8 encoded"))
        }
    }

    pub fn read_bytes(&mut self) -> IoResult<Vec<u8>> {
        match self.wire_type {
            LengthDelimited => self.input.read_length_delimited(),
            _ => Err(unexpected_output("field type was not length delimited"))
        }
    }
}

impl<'a, 'b, R> fmt::Debug for Field<'a, 'b, R> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "Field(tag={:?}; wire-type={:?})", self.tag, self.wire_type)
    }
}

fn has_msb(byte: u8) -> bool {
    byte & 0x80 != 0
}

fn unexpected_output(desc: &'static str) -> IoError {
    IoError {
        kind: InvalidInput,
        desc: desc,
        detail: None
    }
}

#[cfg(test)]
mod test {
    use std::old_io::BufReader;
    use hamcrest::{assert_that,equal_to};
    use super::InputStream;

    #[test]
    pub fn test_reading_empty_stream() {
        with_input_stream(&[], |i| {
            assert!(i.read_field().unwrap().is_none());
        });
    }
    #[test]
    pub fn test_reading_string() {
        with_input_stream(b"\x0A\x04zomg", |i| {
            {
                let mut f = i.read_field().unwrap().unwrap();
                assert_that(f.get_tag(), equal_to(1u));
                assert_that(f.read_string().unwrap(), equal_to("zomg".to_string()));
            }

            assert!(i.read_field().unwrap().is_none());
        });
    }

    #[test]
    pub fn test_reading_single_byte_uint() {
        with_input_stream(b"\x00\x08", |i| {
            {
                let mut f = i.read_field().unwrap().unwrap();
                assert_that(f.get_tag(), equal_to(0u));
                assert_that(f.read_uint().unwrap(), equal_to(8u));
            }

            assert!(i.read_field().unwrap().is_none());
        });
    }

    #[test]
    pub fn test_reading_multi_byte_uint() {
        with_input_stream(b"\x00\x92\x0C", |i| {
            {
                let mut f = i.read_field().unwrap().unwrap();
                assert_that(f.get_tag(), equal_to(0u));
                assert_that(f.read_uint().unwrap(), equal_to(1554u));
            }

            assert!(i.read_field().unwrap().is_none());
        });
    }

    #[test]
    pub fn test_reading_sequential_fields() {
        with_input_stream(b"\x00\x08\x0A\x04zomg\x12\x03lol", |i| {
            {
                let mut f = i.read_field().unwrap().unwrap();
                assert_that(f.get_tag(), equal_to(0u));
                assert_that(f.read_uint().unwrap(), equal_to(8u));
            }

            {
                let mut f = i.read_field().unwrap().unwrap();
                assert_that(f.get_tag(), equal_to(1u));
                assert_that(f.read_string().unwrap(), equal_to("zomg".to_string()));
            }

            {
                let mut f = i.read_field().unwrap().unwrap();
                assert_that(f.get_tag(), equal_to(2u));
                assert_that(f.read_string().unwrap(), equal_to("lol".to_string()));
            }

            assert!(i.read_field().unwrap().is_none());
        });
    }

    #[test]
    pub fn test_skipping_string_field() {
        with_input_stream(b"\x00\x08\x0A\x04zomg\x12\x03lol", |i| {
            i.read_field().unwrap().unwrap().skip().unwrap();

            {
                let mut f = i.read_field().unwrap().unwrap();
                assert_that(f.get_tag(), equal_to(1u));
                assert_that(f.read_string().unwrap(), equal_to("zomg".to_string()));
            }

            i.read_field().unwrap().unwrap().skip().unwrap();

            assert!(i.read_field().unwrap().is_none());
        });
    }

    #[test]
    pub fn test_reading_multi_byte_tag_field() {
        with_input_stream(b"\x92\x01\x04zomg", |i| {
            {
                let mut f = i.read_field().unwrap().unwrap();
                assert_that(f.get_tag(), equal_to(18u));
                assert_that(f.read_string().unwrap(), equal_to("zomg".to_string()));
            }

            assert!(i.read_field().unwrap().is_none());
        });
    }

    #[test]
    pub fn test_reading_twice_from_field() {
        with_input_stream(b"\x92\x01\x04zomg\x92\x01\x04zomg", |i| {
            {
                let mut f = i.read_field().unwrap().unwrap();
                f.read_string().unwrap();

                assert!(f.read_string().is_err());
            }
        });
    }

    #[test]
    pub fn test_reading_incorrect_type_from_field() {
        with_input_stream(b"\x92\x01\x04zomg", |i| {
            {
                let mut f = i.read_field().unwrap().unwrap();
                assert!(f.read_uint().is_err());
            }
        });
    }

    fn with_input_stream<F: FnOnce(&mut InputStream<BufReader>)>(bytes: &[u8], action: F) {
        let mut reader = BufReader::new(bytes);
        let mut stream = InputStream::new(&mut reader);

        action(&mut stream)
    }
}
