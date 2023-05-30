use chrono::{DateTime, Local, Utc};
use log::debug;
use std::{
    collections::HashMap,
    io::Read,
    path::{Path, PathBuf},
};

use crate::{
    backend::{local::LocalDestination, node::Node},
    blob::BlobLocation,
    crypto::hasher::hash,
    error::{FileErrorKind, RusticResult},
    id::Id,
    index::IndexedBackend,
};

type RestoreInfo = HashMap<Id, HashMap<BlobLocation, Vec<FileLocation>>>;
type Filenames = Vec<PathBuf>;

#[derive(Debug, Clone, Copy)]
pub enum AddFileResult {
    Existing,
    Verified,
    New(u64),
    Modify(u64),
}

#[derive(Default, Debug, Clone, Copy)]
pub struct FileStats {
    pub restore: u64,
    pub unchanged: u64,
    pub verified: u64,
    pub modify: u64,
    pub additional: u64,
}

#[derive(Default, Debug, Clone, Copy)]
pub struct RestoreStats {
    pub file: FileStats,
    pub dir: FileStats,
}

/// struct that contains information of file contents grouped by
/// 1) pack ID,
/// 2) blob within this pack
/// 3) the actual files and position of this blob within those
#[derive(Debug, Default)]
pub struct FileInfos {
    pub names: Filenames,
    pub r: RestoreInfo,
    pub restore_size: u64,
    pub matched_size: u64,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct FileLocation {
    pub file_idx: usize,
    pub file_start: u64,
    pub matches: bool, //indicates that the file exists and these contents are already correct
}

impl FileInfos {
    #[must_use]
    pub fn new() -> Self {
        Self {
            names: Vec::new(),
            r: HashMap::new(),
            restore_size: 0,
            matched_size: 0,
        }
    }

    /// Add the file to [`FileInfos`] using `index` to get blob information.
    /// Returns the computed length of the file
    pub fn add_file<P>(
        &mut self,
        dest: &LocalDestination,
        file: &Node,
        name: P,
        index: &impl IndexedBackend,
        ignore_mtime: bool,
    ) -> RusticResult<AddFileResult>
    where
        P: Into<PathBuf> + AsRef<Path> + std::fmt::Debug,
    {
        let mut open_file = dest.get_matching_file(&name, file.meta.size);

        if !ignore_mtime {
            if let Some(meta) = open_file
                .as_ref()
                .map(std::fs::File::metadata)
                .transpose()
                .map_err(FileErrorKind::TransposingOptionResultFailed)?
            {
                // TODO: This is the same logic as in backend/ignore.rs => consollidate!
                let mtime = meta
                    .modified()
                    .ok()
                    .map(|t| DateTime::<Utc>::from(t).with_timezone(&Local));
                if meta.len() == file.meta.size && mtime == file.meta.mtime {
                    // File exists with fitting mtime => we suspect this file is ok!
                    debug!("file {name:?} exists with suitable size and mtime, accepting it!");
                    self.matched_size += file.meta.size;
                    return Ok(AddFileResult::Existing);
                }
            }
        }

        let file_idx = self.names.len();
        self.names.push(name.into());
        let mut file_pos = 0;
        let mut has_unmatched = false;
        for id in file.content.iter().flatten() {
            let ie = index
                .get_data(id)
                .ok_or_else(|| FileErrorKind::CouldNotFindIdInIndex(*id))?;
            let bl = BlobLocation {
                offset: ie.offset,
                length: ie.length,
                uncompressed_length: ie.uncompressed_length,
            };
            let length = bl.data_length();

            let matches = match &mut open_file {
                Some(file) => {
                    // Existing file content; check if SHA256 matches
                    let try_length = usize::try_from(length)
                        .map_err(FileErrorKind::ConversionFromU64ToUsizeFailed)?;
                    let mut vec = vec![0; try_length];
                    file.read_exact(&mut vec).is_ok() && id == &hash(&vec)
                }
                None => false,
            };

            let pack = self.r.entry(ie.pack).or_insert_with(HashMap::new);
            let blob_location = pack.entry(bl).or_insert_with(Vec::new);
            blob_location.push(FileLocation {
                file_idx,
                file_start: file_pos,
                matches,
            });

            if matches {
                self.matched_size += length;
            } else {
                self.restore_size += length;
                has_unmatched = true;
            }

            file_pos += length;
        }

        match (has_unmatched, open_file.is_some()) {
            (true, true) => Ok(AddFileResult::Modify(file_pos)),
            (false, true) => Ok(AddFileResult::Verified),
            (_, false) => Ok(AddFileResult::New(file_pos)),
        }
    }

    #[must_use]
    pub fn to_packs(&self) -> Vec<Id> {
        self.r
            .iter()
            // filter out packs which we need
            .filter(|(_, blob)| blob.iter().any(|(_, fls)| fls.iter().all(|fl| !fl.matches)))
            .map(|(pack, _)| *pack)
            .collect()
    }
}
