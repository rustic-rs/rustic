use integer_sqrt::IntegerSquareRoot;

use std::{
    num::NonZeroU32,
    sync::{Arc, RwLock},
    time::SystemTime,
};

use bytes::{Bytes, BytesMut};
use chrono::Local;
use crossbeam_channel::{bounded, Receiver, Sender};
use pariter::{scope, IteratorExt};
use zstd::encode_all;

use crate::{
    backend::{decrypt::DecryptFullBackend, decrypt::DecryptWriteBackend, FileType},
    blob::BlobType,
    crypto::{hasher::hash, CryptoKey},
    error::PackerErrorKind,
    id::Id,
    index::indexer::SharedIndexer,
    repofile::{
        configfile::ConfigFile, indexfile::IndexBlob, indexfile::IndexPack,
        packfile::PackHeaderLength, packfile::PackHeaderRef, snapshotfile::SnapshotSummary,
    },
    RusticResult,
};
pub(super) mod constants {
    use std::time::Duration;

    pub(super) const KB: u32 = 1024;
    pub(super) const MB: u32 = 1024 * KB;
    // the absolute maximum size of a pack: including headers it should not exceed 4 GB
    pub(super) const MAX_SIZE: u32 = 4076 * MB;
    pub(super) const MAX_COUNT: u32 = 10_000;
    pub(super) const MAX_AGE: Duration = Duration::from_secs(300);
}

#[derive(Debug, Clone, Copy)]
pub struct PackSizer {
    default_size: u32,
    grow_factor: u32,
    size_limit: u32,
    current_size: u64,
    min_packsize_tolerate_percent: u32,
    max_packsize_tolerate_percent: u32,
}

impl PackSizer {
    #[must_use]
    pub fn from_config(config: &ConfigFile, blob_type: BlobType, current_size: u64) -> Self {
        let (default_size, grow_factor, size_limit) = config.packsize(blob_type);
        let (min_packsize_tolerate_percent, max_packsize_tolerate_percent) =
            config.packsize_ok_percents();
        Self {
            default_size,
            grow_factor,
            size_limit,
            current_size,
            min_packsize_tolerate_percent,
            max_packsize_tolerate_percent,
        }
    }

    #[must_use]
    pub fn pack_size(&self) -> u32 {
        (self.current_size.integer_sqrt() as u32 * self.grow_factor + self.default_size)
            .min(self.size_limit)
            .min(constants::MAX_SIZE)
    }

    // returns whether the given size is not too small or too large
    #[must_use]
    pub fn size_ok(&self, size: u32) -> bool {
        let target_size = self.pack_size();
        // Note: we cast to u64 so that no overflow can occur in the multiplications
        u64::from(size) * 100
            >= u64::from(target_size) * u64::from(self.min_packsize_tolerate_percent)
            && u64::from(size) * 100
                <= u64::from(target_size) * u64::from(self.max_packsize_tolerate_percent)
    }

    fn add_size(&mut self, added: u32) {
        self.current_size += u64::from(added);
    }
}

#[allow(missing_debug_implementations)]
#[derive(Clone)]
pub struct Packer<BE: DecryptWriteBackend> {
    // This is a hack: raw_packer and indexer are only used in the add_raw() method.
    // TODO: Refactor as actor, like the other add() methods
    raw_packer: Arc<RwLock<RawPacker<BE>>>,
    indexer: SharedIndexer<BE>,
    sender: Sender<(Bytes, Id, Option<u32>)>,
    finish: Receiver<RusticResult<PackerStats>>,
}

impl<BE: DecryptWriteBackend> Packer<BE> {
    pub fn new(
        be: BE,
        blob_type: BlobType,
        indexer: SharedIndexer<BE>,
        config: &ConfigFile,
        total_size: u64,
    ) -> RusticResult<Self> {
        let key = be.key().clone();
        let raw_packer = Arc::new(RwLock::new(RawPacker::new(
            be,
            blob_type,
            indexer.clone(),
            config,
            total_size,
        )));
        let zstd = config.zstd()?;

        let (tx, rx) = bounded(0);
        let (finish_tx, finish_rx) = bounded::<RusticResult<PackerStats>>(0);
        let packer = Self {
            raw_packer: raw_packer.clone(),
            indexer: indexer.clone(),
            sender: tx,
            finish: finish_rx,
        };

        let _join_handle = std::thread::spawn(move || {
            scope(|scope| {
                let status = rx
                    .into_iter()
                    .readahead_scoped(scope)
                    .filter(|(_, id, _)| !indexer.read().unwrap().has(id))
                    .filter(|(_, id, _)| !raw_packer.read().unwrap().has(id))
                    .readahead_scoped(scope)
                    .parallel_map_scoped(
                        scope,
                        |(data, id, size_limit): (Bytes, Id, Option<u32>)| {
                            let data_len: u32 = data
                                .len()
                                .try_into()
                                .map_err(PackerErrorKind::IntConversionFailed)?;
                            let (data, uncompressed_length) = match zstd {
                                None => (key.encrypt_data(&data)?, None),
                                // compress if requested
                                Some(level) => (
                                    key.encrypt_data(&encode_all(&*data, level)?)?,
                                    NonZeroU32::new(data_len),
                                ),
                            };
                            Ok((
                                data,
                                id,
                                u64::from(data_len),
                                uncompressed_length,
                                size_limit,
                            ))
                        },
                    )
                    .readahead_scoped(scope)
                    .try_for_each(|item: RusticResult<_>| {
                        let (data, id, data_len, ul, size_limit) = item?;
                        raw_packer
                            .write()
                            .unwrap()
                            .add_raw(&data, &id, data_len, ul, size_limit)
                    })
                    .and_then(|_| raw_packer.write().unwrap().finalize());
                _ = finish_tx.send(status);
            })
            .unwrap();
        });

        Ok(packer)
    }

    /// adds the blob to the packfile
    pub fn add(&self, data: Bytes, id: Id) -> RusticResult<()> {
        // compute size limit based on total size and size bounds
        self.add_with_sizelimit(data, id, None)
    }

    /// adds the blob to the packfile, allows specifying a size limit for the pack file
    fn add_with_sizelimit(&self, data: Bytes, id: Id, size_limit: Option<u32>) -> RusticResult<()> {
        self.sender
            .send((data, id, size_limit))
            .map_err(PackerErrorKind::SendingCrossbeamMessageFailed)?;
        Ok(())
    }

    /// adds the already encrypted (and maybe compressed) blob to the packfile
    fn add_raw(
        &self,
        data: &[u8],
        id: &Id,
        data_len: u64,
        uncompressed_length: Option<NonZeroU32>,
        size_limit: Option<u32>,
    ) -> RusticResult<()> {
        // only add if this blob is not present
        if self.indexer.read().unwrap().has(id) {
            Ok(())
        } else {
            self.raw_packer.write().unwrap().add_raw(
                data,
                id,
                data_len,
                uncompressed_length,
                size_limit,
            )
        }
    }

    pub fn finalize(self) -> RusticResult<PackerStats> {
        // cancel channel
        drop(self.sender);
        // wait for items in channel to be processed
        self.finish.recv().unwrap()
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct PackerStats {
    blobs: u64,
    data: u64,
    data_packed: u64,
}

impl PackerStats {
    pub fn apply(self, summary: &mut SnapshotSummary, tpe: BlobType) {
        summary.data_added += self.data;
        summary.data_added_packed += self.data_packed;
        match tpe {
            BlobType::Tree => {
                summary.tree_blobs += self.blobs;
                summary.data_added_trees += self.data;
                summary.data_added_trees_packed += self.data_packed;
            }
            BlobType::Data => {
                summary.data_blobs += self.blobs;
                summary.data_added_files += self.data;
                summary.data_added_files_packed += self.data_packed;
            }
        }
    }
}

#[allow(missing_debug_implementations, clippy::module_name_repetitions)]
pub(crate) struct RawPacker<BE: DecryptWriteBackend> {
    be: BE,
    blob_type: BlobType,
    file: BytesMut,
    size: u32,
    count: u32,
    created: SystemTime,
    index: IndexPack,
    file_writer: Option<Actor>,
    pack_sizer: PackSizer,
    stats: PackerStats,
}

impl<BE: DecryptWriteBackend> RawPacker<BE> {
    fn new(
        be: BE,
        blob_type: BlobType,
        indexer: SharedIndexer<BE>,
        config: &ConfigFile,
        total_size: u64,
    ) -> Self {
        let file_writer = Some(Actor::new(
            FileWriterHandle {
                be: be.clone(),
                indexer,
                cacheable: blob_type.is_cacheable(),
            },
            1,
            1,
        ));

        let pack_sizer = PackSizer::from_config(config, blob_type, total_size);

        Self {
            be,
            blob_type,
            file: BytesMut::new(),
            size: 0,
            count: 0,
            created: SystemTime::now(),
            index: IndexPack::default(),
            file_writer,
            pack_sizer,
            stats: PackerStats::default(),
        }
    }

    fn finalize(&mut self) -> RusticResult<PackerStats> {
        self.save()?;
        self.file_writer.take().unwrap().finalize()?;
        Ok(std::mem::take(&mut self.stats))
    }

    fn write_data(&mut self, data: &[u8]) -> RusticResult<u32> {
        let len = data
            .len()
            .try_into()
            .map_err(PackerErrorKind::IntConversionFailed)?;
        self.file.extend_from_slice(data);
        self.size += len;
        Ok(len)
    }

    // adds the already compressed/encrypted blob to the packfile without any check
    fn add_raw(
        &mut self,
        data: &[u8],
        id: &Id,
        data_len: u64,
        uncompressed_length: Option<NonZeroU32>,
        size_limit: Option<u32>,
    ) -> RusticResult<()> {
        self.stats.blobs += 1;
        self.stats.data += data_len;
        let data_len_packed: u64 = data
            .len()
            .try_into()
            .map_err(PackerErrorKind::IntConversionFailed)?;
        self.stats.data_packed += data_len_packed;

        let size_limit = size_limit.unwrap_or_else(|| self.pack_sizer.pack_size());
        let offset = self.size;
        let len = self.write_data(data)?;
        self.index
            .add(*id, self.blob_type, offset, len, uncompressed_length);
        self.count += 1;

        // check if PackFile needs to be saved
        if self.count >= constants::MAX_COUNT
            || self.size >= size_limit
            || self
                .created
                .elapsed()
                .map_err(PackerErrorKind::CouldNotGetElapsedTimeFromSystemTime)?
                >= constants::MAX_AGE
        {
            self.pack_sizer.add_size(self.index.pack_size());
            self.save()?;
            self.size = 0;
            self.count = 0;
            self.created = SystemTime::now();
        }
        Ok(())
    }

    /// writes header and length of header to packfile
    fn write_header(&mut self) -> RusticResult<()> {
        // comput the pack header
        let data = PackHeaderRef::from_index_pack(&self.index).to_binary()?;

        // encrypt and write to pack file
        let data = self.be.key().encrypt_data(&data)?;

        let headerlen = data
            .len()
            .try_into()
            .map_err(PackerErrorKind::IntConversionFailed)?;
        _ = self.write_data(&data)?;

        // finally write length of header unencrypted to pack file
        _ = self.write_data(&PackHeaderLength::from_u32(headerlen).to_binary()?)?;

        Ok(())
    }

    fn save(&mut self) -> RusticResult<()> {
        if self.size == 0 {
            return Ok(());
        }

        self.write_header()?;

        // write file to backend
        let index = std::mem::take(&mut self.index);
        let file = std::mem::replace(&mut self.file, BytesMut::new());
        self.file_writer
            .as_ref()
            .unwrap()
            .send((file.into(), index))?;

        Ok(())
    }

    fn has(&self, id: &Id) -> bool {
        self.index.blobs.iter().any(|b| &b.id == id)
    }
}

#[derive(Clone)]
pub(crate) struct FileWriterHandle<BE: DecryptWriteBackend> {
    be: BE,
    indexer: SharedIndexer<BE>,
    cacheable: bool,
}

impl<BE: DecryptWriteBackend> FileWriterHandle<BE> {
    fn process(&self, load: (Bytes, Id, IndexPack)) -> RusticResult<IndexPack> {
        let (file, id, mut index) = load;
        index.id = id;
        self.be
            .write_bytes(FileType::Pack, &id, self.cacheable, file)?;
        index.time = Some(Local::now());
        Ok(index)
    }

    fn index(&self, index: IndexPack) -> RusticResult<()> {
        self.indexer.write().unwrap().add(index)?;
        Ok(())
    }
}

pub(crate) struct Actor {
    sender: Sender<(Bytes, IndexPack)>,
    finish: Receiver<RusticResult<()>>,
}

impl Actor {
    fn new<BE: DecryptWriteBackend>(
        fwh: FileWriterHandle<BE>,
        queue_len: usize,
        _par: usize,
    ) -> Self {
        let (tx, rx) = bounded(queue_len);
        let (finish_tx, finish_rx) = bounded::<RusticResult<()>>(0);

        let _join_handle = std::thread::spawn(move || {
            scope(|scope| {
                let status = rx
                    .into_iter()
                    .readahead_scoped(scope)
                    .map(|(file, index): (Bytes, IndexPack)| {
                        let id = hash(&file);
                        (file, id, index)
                    })
                    .readahead_scoped(scope)
                    .map(|load| fwh.process(load))
                    .readahead_scoped(scope)
                    .try_for_each(|index| fwh.index(index?));
                _ = finish_tx.send(status);
            })
            .unwrap();
        });

        Self {
            sender: tx,
            finish: finish_rx,
        }
    }

    fn send(&self, load: (Bytes, IndexPack)) -> RusticResult<()> {
        self.sender
            .send(load)
            .map_err(PackerErrorKind::SendingCrossbeamMessageFailedForIndexPack)?;
        Ok(())
    }

    fn finalize(self) -> RusticResult<()> {
        // cancel channel
        drop(self.sender);
        // wait for items in channel to be processed
        self.finish.recv().unwrap()
    }
}

#[allow(missing_debug_implementations)]
pub struct Repacker<BE>
where
    BE: DecryptFullBackend,
{
    be: BE,
    packer: Packer<BE>,
    size_limit: u32,
}

impl<BE: DecryptFullBackend> Repacker<BE> {
    pub fn new(
        be: BE,
        blob_type: BlobType,
        indexer: SharedIndexer<BE>,
        config: &ConfigFile,
        total_size: u64,
    ) -> RusticResult<Self> {
        let packer = Packer::new(be.clone(), blob_type, indexer, config, total_size)?;
        let size_limit = PackSizer::from_config(config, blob_type, total_size).pack_size();
        Ok(Self {
            be,
            packer,
            size_limit,
        })
    }

    pub fn add_fast(&self, pack_id: &Id, blob: &IndexBlob) -> RusticResult<()> {
        let data = self.be.read_partial(
            FileType::Pack,
            pack_id,
            blob.tpe.is_cacheable(),
            blob.offset,
            blob.length,
        )?;
        self.packer.add_raw(
            &data,
            &blob.id,
            0,
            blob.uncompressed_length,
            Some(self.size_limit),
        )?;
        Ok(())
    }

    pub fn add(&self, pack_id: &Id, blob: &IndexBlob) -> RusticResult<()> {
        let data = self.be.read_encrypted_partial(
            FileType::Pack,
            pack_id,
            blob.tpe.is_cacheable(),
            blob.offset,
            blob.length,
            blob.uncompressed_length,
        )?;
        self.packer
            .add_with_sizelimit(data, blob.id, Some(self.size_limit))?;
        Ok(())
    }

    pub fn finalize(self) -> RusticResult<PackerStats> {
        self.packer.finalize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repofile::configfile::DEFAULT_GROW_FACTOR;

    #[test]
    fn test_pack_sizer() {
        let config = ConfigFile::default();
        let pack_sizer = PackSizer::from_config(&config, BlobType::Data, 0);
        assert_eq!(pack_sizer.pack_size(), DEFAULT_GROW_FACTOR * 1024 * 1024);

        let pack_sizer = PackSizer::from_config(&config, BlobType::Data, 1024 * 1024);
        // TODO!
        // probably related to:
        // 32 * sqrt(reposize in bytes) = 1 MB * sqrt(reposize in GB)
        //          left: `33587200`,      right: `33554432`
        assert_eq!(pack_sizer.pack_size(), DEFAULT_GROW_FACTOR * 1024 * 1024);

        let pack_sizer = PackSizer::from_config(&config, BlobType::Data, 1024 * 1024 * 1024 * 2);
        assert_eq!(
            pack_sizer.pack_size(),
            DEFAULT_GROW_FACTOR * 1024 * 1024 * 2
        );

        let pack_sizer = PackSizer::from_config(&config, BlobType::Data, 1024 * 1024 * 1024 * 3);
        assert_eq!(
            pack_sizer.pack_size(),
            DEFAULT_GROW_FACTOR * 1024 * 1024 * 2
        );

        let pack_sizer = PackSizer::from_config(&config, BlobType::Data, 1024 * 1024 * 1024 * 4);
        assert_eq!(
            pack_sizer.pack_size(),
            DEFAULT_GROW_FACTOR * 1024 * 1024 * 2
        );
    }

    // TODO: Are these reasonable tests?
    #[test]
    fn test_pack_sizer_size_ok() {
        let config = ConfigFile::default();
        let pack_sizer = PackSizer::from_config(&config, BlobType::Data, 0);
        assert!(pack_sizer.size_ok(0));
        assert!(pack_sizer.size_ok(1024 * 1024));
        assert!(pack_sizer.size_ok(1024 * 1024 * 1024 * 2));
        assert!(pack_sizer.size_ok(1024 * 1024 * 1024 * 3));

        let pack_sizer = PackSizer::from_config(&config, BlobType::Data, 1024 * 1024);
        assert!(pack_sizer.size_ok(0));
        assert!(pack_sizer.size_ok(1024 * 1024));
        assert!(pack_sizer.size_ok(1024 * 1024 * 1024 * 2));
        assert!(pack_sizer.size_ok(1024 * 1024 * 1024 * 3));

        let pack_sizer = PackSizer::from_config(&config, BlobType::Data, 1024 * 1024 * 1024 * 2);
        assert!(pack_sizer.size_ok(0));
        assert!(pack_sizer.size_ok(1024 * 1024));
        assert!(pack_sizer.size_ok(1024 * 1024 * 1024 * 2));
        assert!(pack_sizer.size_ok(1024 * 1024 * 1024 * 3));

        let pack_sizer = PackSizer::from_config(&config, BlobType::Data, 1024 * 1024 * 1024 * 3);
        assert!(pack_sizer.size_ok(0));
        assert!(pack_sizer.size_ok(1024 * 1024));
        assert!(pack_sizer.size_ok(1024 * 1024 * 1024 * 2));
        assert!(pack_sizer.size_ok(1024 * 1024 * 1024 * 3));
    }
}
