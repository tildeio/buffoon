use std::fmt;
use std::io::{self, Read};
use std::num::FromPrimitive;
use wire_type::WireType;
use wire_type::WireType::*;

pub struct InputStream<'a, R: 'a> {
    reader: &'a mut R
}

impl<'a, R: Read> InputStream<'a, R> {
    pub fn new(reader: &'a mut R) -> InputStream<'a, R> {
        InputStream { reader: reader }
    }

    pub fn read_field<'b>(&'b mut self) -> io::Result<Option<Field<'b, 'a, R>>> {
        // Read the header byte. In this case, EOF errors are OK as they signify
        // that there is no field to read
        let head = match self.read_usize() {
            Ok(Some(h)) => h,
            Ok(None) => return Ok(None),
            Err(e) => return Err(e),
        };

        // Extract the type of the field
        let wire_type = match WireType::from_usize(head & 0x7) {
            Some(res) => res,
            None => return Err(unexpected_output("invalid wire type"))
        };

        Ok(Some(Field {
            input: self,
            tag: head >> 3,
            wire_type: wire_type
        }))
    }

    fn read_usize(&mut self) -> io::Result<Option<usize>> {
        if let Some(num) = try!(self.read_unsigned_varint()) {
            match FromPrimitive::from_u64(num) {
                Some(num) => return Ok(Some(num)),
                None => return Err(unexpected_output("requested value could not fit in usize")),
            }
        }

        Ok(None)
    }

    fn read_u64(&mut self) -> io::Result<Option<u64>> {
        self.read_unsigned_varint()
    }

    // TODO: Handle overflow
    fn read_unsigned_varint(&mut self) -> io::Result<Option<u64>> {
        let mut ret: u64 = 0;
        let mut shift = 0;

        while let Some(byte) = try!(self.read_byte()) {
            let bits = (byte & 0x7f) as u64;

            ret |= bits << shift;
            shift += 7;

            if !has_msb(byte) {
                return Ok(Some(ret));
            }
        }

        match shift {
            0 => Ok(None),
            _ => Err(eof()),
        }
    }

    fn read_length_delimited(&mut self) -> io::Result<Option<Vec<u8>>> {
        if let Some(len) = try!(self.read_usize()) {
            return self.read_exact(len).map(|ret| Some(ret));
        }

        Ok(None)
    }

    fn skip(&mut self, n: usize) -> io::Result<usize> {
        let mut i = 0;
        // Yes this is a terrible implementation, but something better depends on:
        // https://github.com/rust-lang/rust/issues/13989
        while i < n {
            if let None = try!(self.read_byte()) {
                return Ok(i);
            }

            i += 1;
        }

        Ok(i)
    }

    fn read_exact(&mut self, len: usize) -> io::Result<Vec<u8>> {
        use std::slice;

        let mut ret = Vec::with_capacity(len);

        unsafe {
            let mut buf = slice::from_raw_parts_mut(ret.as_mut_slice().as_mut_ptr(), len);
            let mut off = 0;

            while off < len {
                let cnt = try!(self.reader.read(&mut buf[off..]));

                if cnt == 0 {
                    return Err(eof());
                }

                off += cnt;
            }

            ret.set_len(len);
        }

        Ok(ret)
    }

    #[inline]
    fn read_byte(&mut self) -> io::Result<Option<u8>> {
        let mut buf = [0; 1];

        if 1 == try!(self.reader.read(&mut buf)) {
            return Ok(Some(buf[0]));
        }

        Ok(None)
    }
}

pub struct Field<'b, 'a:'b, R:'a> {
    input: &'b mut InputStream<'a, R>,
    pub tag: usize,
    wire_type: WireType
}

impl<'a, 'b, R: Read> Field<'a, 'b, R> {
    pub fn get_tag(&self) -> usize {
        self.tag
    }

    pub fn skip(&mut self) -> io::Result<()> {
        match self.wire_type {
            Varint => {
                if let Some(_) = try!(self.input.read_unsigned_varint()) {
                    return Ok(());
                }

                Err(eof())
            }
            SixtyFourBit => unimplemented!(),
            LengthDelimited => {
                if let Some(len) = try!(self.input.read_usize()) {
                    if len == try!(self.input.skip(len)) {
                        return Ok(());
                    }
                }

                Err(eof())
            }
            StartGroup => unimplemented!(),
            EndGroup => unimplemented!(),
            ThirtyTwoBit => unimplemented!()
        }
    }

    pub fn read_u64(&mut self) -> io::Result<u64> {
        match self.wire_type {
            Varint => {
                if let Some(val) = try!(self.input.read_u64()) {
                    return Ok(val);
                }

                Err(eof())
            }
            _ => Err(unexpected_output("field type was not varint"))
        }
    }

    pub fn read_string(&mut self) -> io::Result<String> {
        match String::from_utf8(try!(self.read_bytes())) {
            Ok(s) => Ok(s),
            Err(_) => Err(unexpected_output("string not UTF-8 encoded"))
        }
    }

    pub fn read_bytes(&mut self) -> io::Result<Vec<u8>> {
        match self.wire_type {
            LengthDelimited => {
                if let Some(val) = try!(self.input.read_length_delimited()) {
                    return Ok(val);
                }

                Err(eof())
            }
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

fn unexpected_output(desc: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, desc, None)
}

fn eof() -> io::Error {
    return unexpected_output("unexpected EOF");
}

#[cfg(test)]
mod test {
    use std::io::Cursor;
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
                assert_that(f.get_tag(), equal_to(1));
                assert_that(f.read_string().unwrap(), equal_to("zomg".to_string()));
            }

            assert!(i.read_field().unwrap().is_none());
        });
    }

    #[test]
    pub fn test_reading_single_byte_usize() {
        with_input_stream(b"\x00\x08", |i| {
            {
                let mut f = i.read_field().unwrap().unwrap();
                assert_that(f.get_tag(), equal_to(0));
                assert_that(f.read_u64().unwrap(), equal_to(8));
            }

            assert!(i.read_field().unwrap().is_none());
        });
    }

    #[test]
    pub fn test_reading_multi_byte_usize() {
        with_input_stream(b"\x00\x92\x0C", |i| {
            {
                let mut f = i.read_field().unwrap().unwrap();
                assert_that(f.get_tag(), equal_to(0));
                assert_that(f.read_u64().unwrap(), equal_to(1554));
            }

            assert!(i.read_field().unwrap().is_none());
        });
    }

    #[test]
    pub fn test_reading_sequential_fields() {
        with_input_stream(b"\x00\x08\x0A\x04zomg\x12\x03lol", |i| {
            {
                let mut f = i.read_field().unwrap().unwrap();
                assert_that(f.get_tag(), equal_to(0));
                assert_that(f.read_u64().unwrap(), equal_to(8));
            }

            {
                let mut f = i.read_field().unwrap().unwrap();
                assert_that(f.get_tag(), equal_to(1));
                assert_that(f.read_string().unwrap(), equal_to("zomg".to_string()));
            }

            {
                let mut f = i.read_field().unwrap().unwrap();
                assert_that(f.get_tag(), equal_to(2));
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
                assert_that(f.get_tag(), equal_to(1));
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
                assert_that(f.get_tag(), equal_to(18));
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
                assert!(f.read_u64().is_err());
            }
        });
    }

    fn with_input_stream<F: FnOnce(&mut InputStream<Cursor<&[u8]>>)>(bytes: &[u8], action: F) {
        let mut reader = Cursor::new(bytes);
        let mut stream = InputStream::new(&mut reader);

        action(&mut stream)
    }
}
