use crate::{
    backend::{FileType, Id, ReadBackend, WriteBackend},
    RusticResult,
};

#[derive(Clone, Debug, Default)]
pub(crate) struct MockBackend;

impl MockBackend {
    #[allow(unused)]
    pub(crate) fn new() -> Self {
        Self
    }
}

impl ReadBackend for MockBackend {
    fn location(&self) -> String {
        "mock".to_string()
    }

    fn set_option(&mut self, _option: &str, _value: &str) -> RusticResult<()> {
        Ok(())
    }

    fn list_with_size(&self, _tpe: FileType) -> RusticResult<Vec<(Id, u32)>> {
        let id = Id::random();
        Ok(vec![(id, 0)])
    }

    fn read_full(&self, _tpe: FileType, _id: &Id) -> RusticResult<bytes::Bytes> {
        Ok("mock".as_bytes().into())
    }

    fn read_partial(
        &self,
        _tpe: FileType,
        _id: &Id,
        _cacheable: bool,
        _offset: u32,
        _length: u32,
    ) -> RusticResult<bytes::Bytes> {
        Ok("mock".as_bytes().into())
    }
}

impl WriteBackend for MockBackend {
    fn create(&self) -> RusticResult<()> {
        Ok(())
    }

    fn write_bytes(
        &self,
        _tpe: FileType,
        _id: &Id,
        _cacheable: bool,
        _buf: bytes::Bytes,
    ) -> RusticResult<()> {
        Ok(())
    }

    fn remove(&self, _tpe: FileType, _id: &Id, _cacheable: bool) -> RusticResult<()> {
        Ok(())
    }
}
