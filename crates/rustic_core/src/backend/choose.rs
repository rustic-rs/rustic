use bytes::Bytes;

use crate::{
    backend::{
        local::LocalBackend, rclone::RcloneBackend, rest::RestBackend, FileType, ReadBackend,
        WriteBackend,
    },
    error::BackendErrorKind,
    id::Id,
    RusticResult,
};

#[derive(Clone, Debug)]
pub enum ChooseBackend {
    Local(LocalBackend),
    Rest(RestBackend),
    Rclone(RcloneBackend),
}

impl ChooseBackend {
    pub fn from_url(url: &str) -> RusticResult<Self> {
        Ok(match url.split_once(':') {
            #[cfg(windows)]
            Some((drive, _)) if drive.len() == 1 => Self::Local(LocalBackend::new(url)?),
            Some(("rclone", path)) => Self::Rclone(RcloneBackend::new(path)?),
            Some(("rest", path)) => Self::Rest(RestBackend::new(path)?),
            Some(("local", path)) => Self::Local(LocalBackend::new(path)?),
            Some((backend, _)) => {
                return Err(BackendErrorKind::BackendNotSupported(backend.to_owned()).into())
            }
            None => Self::Local(LocalBackend::new(url)?),
        })
    }
}

impl ReadBackend for ChooseBackend {
    fn location(&self) -> String {
        match self {
            Self::Local(local) => local.location(),
            Self::Rest(rest) => rest.location(),
            Self::Rclone(rclone) => rclone.location(),
        }
    }

    fn set_option(&mut self, option: &str, value: &str) -> RusticResult<()> {
        match self {
            Self::Local(local) => local.set_option(option, value),
            Self::Rest(rest) => rest.set_option(option, value),
            Self::Rclone(rclone) => rclone.set_option(option, value),
        }
    }

    fn list_with_size(&self, tpe: FileType) -> RusticResult<Vec<(Id, u32)>> {
        match self {
            Self::Local(local) => local.list_with_size(tpe),
            Self::Rest(rest) => rest.list_with_size(tpe),
            Self::Rclone(rclone) => rclone.list_with_size(tpe),
        }
    }

    fn read_full(&self, tpe: FileType, id: &Id) -> RusticResult<Bytes> {
        match self {
            Self::Local(local) => local.read_full(tpe, id),
            Self::Rest(rest) => rest.read_full(tpe, id),
            Self::Rclone(rclone) => rclone.read_full(tpe, id),
        }
    }

    fn read_partial(
        &self,
        tpe: FileType,
        id: &Id,
        cacheable: bool,
        offset: u32,
        length: u32,
    ) -> RusticResult<Bytes> {
        match self {
            Self::Local(local) => local.read_partial(tpe, id, cacheable, offset, length),
            Self::Rest(rest) => rest.read_partial(tpe, id, cacheable, offset, length),
            Self::Rclone(rclone) => rclone.read_partial(tpe, id, cacheable, offset, length),
        }
    }
}

impl WriteBackend for ChooseBackend {
    fn create(&self) -> RusticResult<()> {
        match self {
            Self::Local(local) => local.create(),
            Self::Rest(rest) => rest.create(),
            Self::Rclone(rclone) => rclone.create(),
        }
    }

    fn write_bytes(&self, tpe: FileType, id: &Id, cacheable: bool, buf: Bytes) -> RusticResult<()> {
        match self {
            Self::Local(local) => local.write_bytes(tpe, id, cacheable, buf),
            Self::Rest(rest) => rest.write_bytes(tpe, id, cacheable, buf),
            Self::Rclone(rclone) => rclone.write_bytes(tpe, id, cacheable, buf),
        }
    }

    fn remove(&self, tpe: FileType, id: &Id, cacheable: bool) -> RusticResult<()> {
        match self {
            Self::Local(local) => local.remove(tpe, id, cacheable),
            Self::Rest(rest) => rest.remove(tpe, id, cacheable),
            Self::Rclone(rclone) => rclone.remove(tpe, id, cacheable),
        }
    }
}
