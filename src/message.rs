use std::old_io::IoResult;
use {InputStream, OutputStream};

pub trait Message {
    fn serialize<O: OutputStream>(&self, out: &mut O) -> IoResult<()>;
}

pub trait LoadableMessage : Sized {
    fn load_from_stream<'a, R:'a+Reader>(reader: &mut InputStream<'a, R>) -> IoResult<Self>;

    fn load<R: Reader>(reader: &mut R) -> IoResult<Self> {
        let mut stream = InputStream::new(reader);
        LoadableMessage::load_from_stream(&mut stream)
    }
}
