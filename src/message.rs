use std::io::{self, Read};
use {InputStream, OutputStream};

pub trait Message {
    fn serialize<O: OutputStream>(&self, out: &mut O) -> io::Result<()>;
}

pub trait LoadableMessage : Sized {
    fn load_from_stream<'a, R:'a+Read>(reader: &mut InputStream<'a, R>) -> io::Result<Self>;

    fn load<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut stream = InputStream::new(reader);
        LoadableMessage::load_from_stream(&mut stream)
    }
}
