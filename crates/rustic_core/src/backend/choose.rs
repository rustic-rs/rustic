use bytes::Bytes;

use crate::{
    backend::{
        local::LocalBackend, rclone::RcloneBackend, rest::RestBackend, FileType, ReadBackend,
        WriteBackend,
    },
    error::BackendErrorKind,
    error::RusticResult,
    id::Id,
};

/// Backend helper that chooses the correct backend based on the url.
#[derive(Clone, Debug)]
pub enum ChooseBackend {
    /// Local backend.
    Local(LocalBackend),
    /// REST backend.
    Rest(RestBackend),
    /// Rclone backend.
    Rclone(RcloneBackend),
}

impl ChooseBackend {
    /// Create a new [`ChooseBackend`] from a given url.
    ///
    /// # Arguments
    ///
    /// * `url` - The url to create the [`ChooseBackend`] from.
    ///
    /// # Errors
    ///
    /// * [`BackendErrorKind::BackendNotSupported`] - If the backend is not supported.
    /// * [`LocalErrorKind::DirectoryCreationFailed`] - If the directory could not be created.
    /// * [`RestErrorKind::UrlParsingFailed`] - If the url could not be parsed.
    /// * [`RestErrorKind::BuildingClientFailed`] - If the client could not be built.
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
    /// Returns the location of the backend.
    fn location(&self) -> String {
        match self {
            Self::Local(local) => local.location(),
            Self::Rest(rest) => rest.location(),
            Self::Rclone(rclone) => rclone.location(),
        }
    }

    /// Sets an option of the backend.
    ///
    /// # Arguments
    ///
    /// * `option` - The option to set.
    /// * `value` - The value to set the option to.
    fn set_option(&mut self, option: &str, value: &str) -> RusticResult<()> {
        match self {
            Self::Local(local) => local.set_option(option, value),
            Self::Rest(rest) => rest.set_option(option, value),
            Self::Rclone(rclone) => rclone.set_option(option, value),
        }
    }

    /// Lists all files with their size of the given type.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the files to list.
    ///
    /// # Errors
    ///
    /// If the backend does not support listing files.
    ///
    /// # Returns
    ///
    /// A vector of tuples containing the id and size of the files.
    fn list_with_size(&self, tpe: FileType) -> RusticResult<Vec<(Id, u32)>> {
        match self {
            Self::Local(local) => local.list_with_size(tpe),
            Self::Rest(rest) => rest.list_with_size(tpe),
            Self::Rclone(rclone) => rclone.list_with_size(tpe),
        }
    }

    /// Reads full data of the given file.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the file.
    /// * `id` - The id of the file.
    ///
    /// # Errors
    ///
    /// * [`LocalErrorKind::ReadingContentsOfFileFailed`] - If the file could not be read.
    /// * [`reqwest::Error`] - If the request failed.
    /// * [`RestErrorKind::BackoffError`] - If the backoff failed.
    ///
    /// # Returns
    ///
    /// The data read.
    fn read_full(&self, tpe: FileType, id: &Id) -> RusticResult<Bytes> {
        match self {
            Self::Local(local) => local.read_full(tpe, id),
            Self::Rest(rest) => rest.read_full(tpe, id),
            Self::Rclone(rclone) => rclone.read_full(tpe, id),
        }
    }

    /// Reads partial data of the given file.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the file.
    /// * `id` - The id of the file.
    /// * `cacheable` - Whether the file is cacheable.
    /// * `offset` - The offset to read from.
    /// * `length` - The length to read.
    ///
    /// # Returns
    ///
    /// The data read.
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
    /// Creates the backend.
    fn create(&self) -> RusticResult<()> {
        match self {
            Self::Local(local) => local.create(),
            Self::Rest(rest) => rest.create(),
            Self::Rclone(rclone) => rclone.create(),
        }
    }

    /// Writes the given data to the given file.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the file.
    /// * `id` - The id of the file.
    /// * `cacheable` - Whether the file is cacheable.
    /// * `buf` - The data to write.
    fn write_bytes(&self, tpe: FileType, id: &Id, cacheable: bool, buf: Bytes) -> RusticResult<()> {
        match self {
            Self::Local(local) => local.write_bytes(tpe, id, cacheable, buf),
            Self::Rest(rest) => rest.write_bytes(tpe, id, cacheable, buf),
            Self::Rclone(rclone) => rclone.write_bytes(tpe, id, cacheable, buf),
        }
    }

    /// Removes the given file.
    ///
    /// # Arguments
    ///
    /// * `tpe` - The type of the file.
    /// * `id` - The id of the file.
    /// * `cacheable` - Whether the file is cacheable.
    fn remove(&self, tpe: FileType, id: &Id, cacheable: bool) -> RusticResult<()> {
        match self {
            Self::Local(local) => local.remove(tpe, id, cacheable),
            Self::Rest(rest) => rest.remove(tpe, id, cacheable),
            Self::Rclone(rclone) => rclone.remove(tpe, id, cacheable),
        }
    }
}
