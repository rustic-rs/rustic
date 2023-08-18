// Implement MockBackend

use crate::{
    backend::{FileType, Id, ReadBackend, WriteBackend},
    RusticResult,
};

pub(crate) struct MockBackend;

impl MockBackend {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl ReadBackend for MockBackend {
    fn location(&self) -> String {
        "mock".to_string()
    }

    fn set_option(&mut self, option: &str, value: &str) -> RusticResult<()> {
        todo!()
    }

    fn list_with_size(&self, tpe: FileType) -> RusticResult<Vec<(Id, u32)>> {
        todo!()
    }

    fn read_full(&self, tpe: FileType, id: &Id) -> RusticResult<bytes::Bytes> {
        todo!()
    }

    fn read_partial(
        &self,
        tpe: FileType,
        id: &Id,
        cacheable: bool,
        offset: u32,
        length: u32,
    ) -> RusticResult<bytes::Bytes> {
        todo!()
    }
}

impl WriteBackend for MockBackend {
    fn create(&self) -> RusticResult<()> {
        todo!()
    }

    fn write_bytes(
        &self,
        tpe: FileType,
        id: &Id,
        cacheable: bool,
        buf: bytes::Bytes,
    ) -> RusticResult<()> {
        todo!()
    }

    fn remove(&self, tpe: FileType, id: &Id, cacheable: bool) -> RusticResult<()> {
        todo!()
    }
}

impl Clone for MockBackend {
    fn clone(&self) -> Self {
        Self
    }
}

impl Default for MockBackend {
    fn default() -> Self {
        Self
    }
}
