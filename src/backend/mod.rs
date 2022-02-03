pub mod decrypt;
pub mod local;

pub use decrypt::DecryptBackend;
pub use local::LocalBackend;

use crate::id::*;

pub enum FileType {
    Config,
    Index,
    Key,
    Snapshot,
    Pack,
}

impl FileType {
    pub fn name(&self) -> &str {
        match &self {
            FileType::Config => "config",
            FileType::Snapshot => "snapshot",
            FileType::Index => "index",
            FileType::Key => "keys",
            FileType::Pack => "data",
        }
    }
}

pub trait ReadBackend {
    type Error: Send + Sync + std::error::Error + 'static;
    fn location(&self) -> &str;
    fn list(&self, tpe: FileType) -> Result<Vec<Id>, Self::Error>;
    fn read_full(&self, tpe: FileType, id: Id) -> Result<Vec<u8>, Self::Error>;
    fn read_partial(
        &self,
        tpe: FileType,
        id: Id,
        offset: u32,
        length: u32,
    ) -> Result<Vec<u8>, Self::Error>;
}
