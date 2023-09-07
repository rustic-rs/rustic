use bytes::Bytes;

use crate::{backend::FileType, backend::ReadBackend, backend::WriteBackend, id::Id, RusticResult};

/// A hot/cold backend implementation.
///
/// # Type Parameters
///
/// * `BE` - The backend to use.
#[derive(Clone, Debug)]
pub struct HotColdBackend<BE: WriteBackend> {
    /// The backend to use.
    be: BE,
    /// The backend to use for hot files.
    hot_be: Option<BE>,
}

impl<BE: WriteBackend> HotColdBackend<BE> {
    /// Creates a new `HotColdBackend`.
    ///
    /// # Type Parameters
    ///
    /// * `BE` - The backend to use.
    ///
    /// # Arguments
    ///
    /// * `be` - The backend to use.
    /// * `hot_be` - The backend to use for hot files.
    pub fn new(be: BE, hot_be: Option<BE>) -> Self {
        Self { be, hot_be }
    }
}

impl<BE: WriteBackend> ReadBackend for HotColdBackend<BE> {
    fn location(&self) -> String {
        self.be.location()
    }

    fn set_option(&mut self, option: &str, value: &str) -> RusticResult<()> {
        self.be.set_option(option, value)
    }

    fn list_with_size(&self, tpe: FileType) -> RusticResult<Vec<(Id, u32)>> {
        self.be.list_with_size(tpe)
    }

    fn read_full(&self, tpe: FileType, id: &Id) -> RusticResult<Bytes> {
        self.hot_be
            .as_ref()
            .map_or_else(|| self.be.read_full(tpe, id), |be| be.read_full(tpe, id))
    }

    fn read_partial(
        &self,
        tpe: FileType,
        id: &Id,
        cacheable: bool,
        offset: u32,
        length: u32,
    ) -> RusticResult<Bytes> {
        match (&self.hot_be, cacheable || tpe != FileType::Pack) {
            (None, _) | (Some(_), false) => {
                self.be.read_partial(tpe, id, cacheable, offset, length)
            }
            (Some(be), true) => be.read_partial(tpe, id, cacheable, offset, length),
        }
    }
}

impl<BE: WriteBackend> WriteBackend for HotColdBackend<BE> {
    fn create(&self) -> RusticResult<()> {
        self.be.create()?;
        if let Some(be) = &self.hot_be {
            be.create()?;
        }
        Ok(())
    }

    fn write_bytes(&self, tpe: FileType, id: &Id, cacheable: bool, buf: Bytes) -> RusticResult<()> {
        if let Some(be) = &self.hot_be {
            if tpe != FileType::Config && (cacheable || tpe != FileType::Pack) {
                be.write_bytes(tpe, id, cacheable, buf.clone())?;
            }
        }
        self.be.write_bytes(tpe, id, cacheable, buf)
    }

    fn remove(&self, tpe: FileType, id: &Id, cacheable: bool) -> RusticResult<()> {
        // First remove cold file
        self.be.remove(tpe, id, cacheable)?;
        if let Some(be) = &self.hot_be {
            if cacheable || tpe != FileType::Pack {
                be.remove(tpe, id, cacheable)?;
            }
        }
        Ok(())
    }
}
