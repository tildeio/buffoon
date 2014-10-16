
#[cfg(test)]
extern crate hamcrest;

use std::io::{Reader, IoResult};
pub use message::{Message, LoadableMessage};
pub use input_stream::{InputStream, Field};
pub use output_stream::OutputStream;
pub use serializer::Serializer;

mod message;
mod input_stream;
mod output_stream;
mod output_writer;
mod serializer;
mod wire_type;

pub fn load<'a, M: LoadableMessage, R: Reader>(reader: &mut R) -> IoResult<M> {
    LoadableMessage::load(reader)
}

pub fn serializer_for<M: Message>(msg: &M) -> IoResult<Serializer> {
    let mut serializer = Serializer::new();

    // populate the message size info
    try!(msg.serialize(&mut serializer));

    Ok(serializer)
}

pub fn serialize<M: Message>(msg: &M) -> IoResult<Vec<u8>> {
    let serializer = try!(serializer_for(msg));
    let mut bytes = Vec::from_elem(serializer.size(), 0);

    try!(serializer.serialize_into(msg, bytes.as_mut_slice()));
    Ok(bytes)
}

#[cfg(test)]
mod test {
    use std::io::IoResult;
    use super::{Message, OutputStream, serialize};

    struct Empty;

    impl Message for Empty {
        fn serialize<O: OutputStream>(&self, _: &mut O) -> IoResult<()> {
            Ok(())
        }
    }

    #[test]
    pub fn test_writing_unit_struct() {
        let bytes = serialize(&Empty).unwrap();
        assert!(bytes.is_empty());
    }

    struct Simple;

    impl Message for Simple {
        fn serialize<O: OutputStream>(&self, out: &mut O) -> IoResult<()> {
            try!(out.write_str_field(1, "hello"));
            // try!(output.write_varint_field(2, self.config()));
            // try!(output.write_repeated_str_field(3, self.cmd().iter().map(|s| s.as_slice())));

            Ok(())
        }
    }

    #[test]
    pub fn test_writing_simple_message() {
        let bytes = serialize(&Simple).unwrap();
        let expect = b"\x0A\x05hello";
        assert!(bytes.as_slice() == expect, "expect={}; actual={}", expect, bytes);
    }
}