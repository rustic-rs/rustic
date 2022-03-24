use std::fs::{self, File};
use std::io::{copy, Read, Seek, SeekFrom};
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use vlog::*;
use walkdir::WalkDir;

use super::{FileType, Id, ReadBackend, WriteBackend};

#[derive(Clone)]
pub struct LocalBackend {
    path: PathBuf,
}

impl LocalBackend {
    pub fn new(path: &str) -> Self {
        Self { path: path.into() }
    }

    fn path(&self, tpe: FileType, id: &Id) -> PathBuf {
        let hex_id = id.to_hex();
        match tpe {
            FileType::Config => self.path.join("config"),
            FileType::Pack => self.path.join("data").join(&hex_id[0..2]).join(&hex_id),
            _ => self.path.join(tpe.name()).join(&hex_id),
        }
    }
}

#[async_trait]
impl ReadBackend for LocalBackend {
    type Error = std::io::Error;

    fn location(&self) -> &str {
        self.path.to_str().unwrap()
    }

    fn list(&self, tpe: FileType) -> Result<Vec<Id>, Self::Error> {
        let walker = WalkDir::new(self.path.join(tpe.name()))
            .into_iter()
            .filter_map(walkdir::Result::ok)
            .filter(|e| e.file_type().is_file())
            .map(|e| Id::from_hex(&e.file_name().to_string_lossy()))
            .filter_map(Result::ok);
        Ok(walker.collect())
    }

    fn list_with_size(&self, tpe: FileType) -> Result<Vec<(Id, u32)>, Self::Error> {
        let walker = WalkDir::new(self.path.join(tpe.name()))
            .into_iter()
            .filter_map(walkdir::Result::ok)
            .filter(|e| e.file_type().is_file())
            .map(|e| {
                (
                    Id::from_hex(&e.file_name().to_string_lossy()).unwrap(),
                    e.metadata().unwrap().len().try_into().unwrap(),
                )
            });

        Ok(walker.collect())
    }

    async fn read_full(&self, tpe: FileType, id: &Id) -> Result<Vec<u8>, Self::Error> {
        fs::read(self.path(tpe, id))
    }

    async fn read_partial(
        &self,
        tpe: FileType,
        id: &Id,
        offset: u32,
        length: u32,
    ) -> Result<Vec<u8>, Self::Error> {
        let mut file = File::open(self.path(tpe, id))?;
        file.seek(SeekFrom::Start(offset.try_into().unwrap()))?;
        let mut vec = vec![0; length.try_into().unwrap()];
        file.read_exact(&mut vec)?;
        Ok(vec)
    }
}

#[async_trait]
impl WriteBackend for LocalBackend {
    type Error = std::io::Error;

    async fn write_full(
        &self,
        tpe: FileType,
        id: &Id,
        r: &mut (impl Read + Send + Sync),
    ) -> Result<(), Self::Error> {
        v3!("writing tpe: {:?}, id: {}", &tpe, &id);
        let filename = self.path(tpe, id);
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&filename)?;
        copy(r, &mut file)?;
        file.sync_all()
    }
}

impl LocalBackend {
    pub fn walker(&self) -> impl Iterator<Item = PathBuf> {
        let path = self.path.clone();
        WalkDir::new(path.clone())
            .min_depth(1)
            .into_iter()
            .filter_map(walkdir::Result::ok)
            .map(move |e| e.path().strip_prefix(path.clone()).unwrap().into())
    }

    pub fn remove_dir(&self, item: impl AsRef<Path>) {
        let dirname = self.path.join(item);
        fs::remove_dir(&dirname).unwrap();
    }

    pub fn remove_file(&self, item: impl AsRef<Path>) {
        let filename = self.path.join(item);
        fs::remove_file(&filename).unwrap();
    }

    pub fn create_dir(&self, item: impl AsRef<Path>) {
        let dirname = self.path.join(item);
        fs::create_dir(&dirname).unwrap();
    }

    pub fn create_symlink(&self, item: impl AsRef<Path>, dest: impl AsRef<Path>) {
        let filename = self.path.join(item);
        std::os::unix::fs::symlink(dest, filename).unwrap();
    }

    pub fn create_file(&self, item: impl AsRef<Path>, size: u64) {
        let filename = self.path.join(item);
        let f = fs::File::create(filename).unwrap();
        f.set_len(size).unwrap();
    }

    pub fn write_at(&self, item: impl AsRef<Path>, offset: u64, data: &[u8]) {
        let filename = self.path.join(item);
        let file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&filename)
            .unwrap();
        file.write_all_at(data, offset).unwrap();
    }
}
