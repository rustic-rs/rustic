use std::fs::{self, File};
use std::io::{copy, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::FileExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use vlog::*;
use walkdir::WalkDir;

use super::{node::Metadata, FileType, Id, ReadBackend, WriteBackend, ALL_FILE_TYPES};

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

    async fn list(&self, tpe: FileType) -> Result<Vec<Id>, Self::Error> {
        if tpe == FileType::Config {
            return Ok(match self.path.join("config").exists() {
                true => vec![Id::default()],
                false => Vec::new(),
            });
        }

        let walker = WalkDir::new(self.path.join(tpe.name()))
            .into_iter()
            .filter_map(walkdir::Result::ok)
            .filter(|e| e.file_type().is_file())
            .map(|e| Id::from_hex(&e.file_name().to_string_lossy()))
            .filter_map(Result::ok);
        Ok(walker.collect())
    }

    async fn list_with_size(&self, tpe: FileType) -> Result<Vec<(Id, u32)>, Self::Error> {
        let path = self.path.join(tpe.name());

        if tpe == FileType::Config {
            return Ok(match path.exists() {
                true => vec![(
                    Id::default(),
                    path.metadata().unwrap().len().try_into().unwrap(),
                )],
                false => Vec::new(),
            });
        }

        let walker = WalkDir::new(path)
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
    async fn create(&self) -> Result<(), Self::Error> {
        for tpe in ALL_FILE_TYPES {
            fs::create_dir_all(self.path.join(tpe.name()))?;
        }
        for i in 0u8..=255 {
            fs::create_dir_all(self.path.join("data").join(hex::encode([i])))?;
        }
        Ok(())
    }

    async fn write_file(&self, tpe: FileType, id: &Id, mut f: File) -> Result<(), Self::Error> {
        v3!("writing tpe: {:?}, id: {}", &tpe, &id);
        let filename = self.path(tpe, id);
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&filename)?;
        copy(&mut f, &mut file)?;
        file.sync_all()
    }

    async fn write_bytes(&self, tpe: FileType, id: &Id, buf: Vec<u8>) -> Result<(), Self::Error> {
        v3!("writing tpe: {:?}, id: {}", &tpe, &id);
        let filename = self.path(tpe, id);
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&filename)?;
        file.write_all(&buf)?;
        file.sync_all()
    }

    async fn remove(&self, tpe: FileType, id: &Id) -> Result<(), Self::Error> {
        v3!("writing tpe: {:?}, id: {}", &tpe, &id);
        let filename = self.path(tpe, id);
        fs::remove_file(filename)
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

    // TODO: uid/gid and times
    pub fn set_metadata(&self, item: impl AsRef<Path>, meta: &Metadata) {
        let mode = *meta.mode();
        if mode == 0 {
            return;
        }
        let filename = self.path.join(item);
        std::fs::set_permissions(&filename, fs::Permissions::from_mode(mode))
            .unwrap_or_else(|_| panic!("error chmod {:?}", filename));
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
