use std::num::NonZeroU32;

use binrw::{io::Cursor, BinRead, BinWrite};
use log::trace;

use crate::{
    backend::{decrypt::DecryptReadBackend, FileType},
    blob::BlobType,
    error::PackFileErrorKind,
    id::Id,
    repofile::indexfile::{IndexBlob, IndexPack},
    RusticResult,
};
pub(super) mod constants {
    // 32 equals the size of the crypto overhead
    // TODO: use from crypto mod
    /// The overhead of compression and encryption
    pub(super) const COMP_OVERHEAD: u32 = 32;
    /// The length of the length field within the pack header
    pub(super) const LENGTH_LEN: u32 = 4;
}

/// The length field within the pack header (which is the total length of the pack header)
#[derive(BinWrite, BinRead, Debug, Clone, Copy)]
#[brw(little)]
pub struct PackHeaderLength(pub u32);

impl PackHeaderLength {
    /// Create a new [`PackHeaderLength`] from a [`u32`]
    ///
    /// # Arguments
    ///
    /// * `len` - The length of the pack header
    #[must_use]
    pub(crate) const fn from_u32(len: u32) -> Self {
        Self(len)
    }

    /// Convert this pack header length into a [`u32`]
    #[must_use]
    pub(crate) const fn to_u32(self) -> u32 {
        self.0
    }

    /// Read pack header length from binary representation
    ///
    /// # Arguments
    ///
    /// * `data` - The binary representation of the pack header length
    ///
    /// # Errors
    ///
    /// * [`PackFileErrorKind::ReadingBinaryRepresentationFailed`] - If reading the binary representation failed
    ///
    /// [`PackFileErrorKind::ReadingBinaryRepresentationFailed`]: crate::error::PackFileErrorKind::ReadingBinaryRepresentationFailed
    pub(crate) fn from_binary(data: &[u8]) -> RusticResult<Self> {
        let mut reader = Cursor::new(data);
        Ok(
            Self::read(&mut reader)
                .map_err(PackFileErrorKind::ReadingBinaryRepresentationFailed)?,
        )
    }

    /// Generate the binary representation of the pack header length
    ///
    /// # Errors
    ///
    /// * [`PackFileErrorKind::WritingBinaryRepresentationFailed`] - If writing the binary representation failed
    ///
    /// [`PackFileErrorKind::WritingBinaryRepresentationFailed`]: crate::error::PackFileErrorKind::WritingBinaryRepresentationFailed
    pub(crate) fn to_binary(self) -> RusticResult<Vec<u8>> {
        let mut writer = Cursor::new(Vec::with_capacity(4));
        self.write(&mut writer)
            .map_err(PackFileErrorKind::WritingBinaryRepresentationFailed)?;
        Ok(writer.into_inner())
    }
}

/// An entry in the pack header
#[derive(BinRead, BinWrite, Debug, Clone, Copy)]
#[brw(little)]
pub enum HeaderEntry {
    /// Entry for an uncompressed data blob
    #[brw(magic(0u8))]
    Data {
        /// Lengths within a packfile
        len: u32,
        /// Id of data blob
        id: Id,
    },

    /// Entry for an uncompressed tree blob
    #[brw(magic(1u8))]
    Tree {
        /// Lengths within a packfile
        len: u32,
        /// Id of tree blob
        id: Id,
    },

    /// Entry for a compressed data blob
    #[brw(magic(2u8))]
    CompData {
        /// Lengths within a packfile
        len: u32,
        /// Raw blob length without compression/encryption
        len_data: u32,
        /// Id of compressed data blob
        id: Id,
    },

    /// Entry for a compressed tree blob
    #[brw(magic(3u8))]
    CompTree {
        /// Lengths within a packfile
        len: u32,
        /// Raw blob length withou compression/encryption
        len_data: u32,
        /// Id of compressed tree blob
        id: Id,
    },
}

impl HeaderEntry {
    /// The length of an uncompressed header entry
    const ENTRY_LEN: u32 = 37;

    /// The length of a compressed header entry
    pub(crate) const ENTRY_LEN_COMPRESSED: u32 = 41;

    /// Read a [`HeaderEntry`] from an [`IndexBlob`]
    ///
    /// # Arguments
    ///
    /// * `blob` - The [`IndexBlob`] to read from
    const fn from_blob(blob: &IndexBlob) -> Self {
        match (blob.uncompressed_length, blob.tpe) {
            (None, BlobType::Data) => Self::Data {
                len: blob.length,
                id: blob.id,
            },
            (None, BlobType::Tree) => Self::Tree {
                len: blob.length,
                id: blob.id,
            },
            (Some(len), BlobType::Data) => Self::CompData {
                len: blob.length,
                len_data: len.get(),
                id: blob.id,
            },
            (Some(len), BlobType::Tree) => Self::CompTree {
                len: blob.length,
                len_data: len.get(),
                id: blob.id,
            },
        }
    }

    /// The length of this header entry
    const fn length(&self) -> u32 {
        match &self {
            Self::Data { .. } | Self::Tree { .. } => Self::ENTRY_LEN,
            Self::CompData { .. } | Self::CompTree { .. } => Self::ENTRY_LEN_COMPRESSED,
        }
    }

    /// Convert this header entry into a [`IndexBlob`]
    ///
    /// # Arguments
    ///
    /// * `offset` - The offset to read from
    const fn into_blob(self, offset: u32) -> IndexBlob {
        match self {
            Self::Data { len, id } => IndexBlob {
                id,
                length: len,
                tpe: BlobType::Data,
                uncompressed_length: None,
                offset,
            },
            Self::Tree { len, id } => IndexBlob {
                id,
                length: len,
                tpe: BlobType::Tree,
                uncompressed_length: None,
                offset,
            },
            Self::CompData { len, id, len_data } => IndexBlob {
                id,
                length: len,
                tpe: BlobType::Data,
                uncompressed_length: NonZeroU32::new(len_data),
                offset,
            },
            Self::CompTree { len, id, len_data } => IndexBlob {
                id,
                length: len,
                tpe: BlobType::Tree,
                uncompressed_length: NonZeroU32::new(len_data),
                offset,
            },
        }
    }
}

/// Header of the pack file
#[derive(Debug, Clone)]
pub struct PackHeader(pub Vec<IndexBlob>);

impl PackHeader {
    /// Create a new [`PackHeader`] from a [`IndexPack`]
    ///
    /// # Arguments
    ///
    /// * `pack` - The binary representation of the pack header
    ///
    /// # Errors
    ///
    /// * [`PackFileErrorKind::ReadingBinaryRepresentationFailed`] - If reading the binary representation failed
    ///
    /// [`PackFileErrorKind::ReadingBinaryRepresentationFailed`]: crate::error::PackFileErrorKind::ReadingBinaryRepresentationFailed
    pub(crate) fn from_binary(pack: &[u8]) -> RusticResult<Self> {
        let mut reader = Cursor::new(pack);
        let mut offset = 0;
        let mut blobs = Vec::new();
        loop {
            let blob = match HeaderEntry::read(&mut reader) {
                Ok(entry) => entry.into_blob(offset),
                Err(err) if err.is_eof() => break,
                Err(err) => {
                    return Err(PackFileErrorKind::ReadingBinaryRepresentationFailed(err).into())
                }
            };
            offset += blob.length;
            blobs.push(blob);
        }
        Ok(Self(blobs))
    }

    /// Read the pack header directly from a packfile using the backend
    ///
    /// # Arguments
    ///
    /// * `be` - The backend to use
    /// * `id` - The id of the packfile
    /// * `size_hint` - The size hint for the pack header
    /// * `pack_size` - The size of the packfile
    ///
    /// # Errors
    ///
    /// * [`PackFileErrorKind::ReadingBinaryRepresentationFailed`] - If reading the binary representation failed
    /// * [`PackFileErrorKind::HeaderLengthTooLarge`] - If the header length is too large
    /// * [`PackFileErrorKind::HeaderLengthDoesNotMatchHeaderContents`] - If the header length does not match the header contents
    /// * [`PackFileErrorKind::HeaderPackSizeComputedDoesNotMatchRealPackFile`] - If the pack size computed from the header does not match the real pack file size
    ///
    /// [`PackFileErrorKind::ReadingBinaryRepresentationFailed`]: crate::error::PackFileErrorKind::ReadingBinaryRepresentationFailed
    /// [`PackFileErrorKind::HeaderLengthTooLarge`]: crate::error::PackFileErrorKind::HeaderLengthTooLarge
    /// [`PackFileErrorKind::HeaderLengthDoesNotMatchHeaderContents`]: crate::error::PackFileErrorKind::HeaderLengthDoesNotMatchHeaderContents
    /// [`PackFileErrorKind::HeaderPackSizeComputedDoesNotMatchRealPackFile`]: crate::error::PackFileErrorKind::HeaderPackSizeComputedDoesNotMatchRealPackFile
    pub(crate) fn from_file(
        be: &impl DecryptReadBackend,
        id: Id,
        size_hint: Option<u32>,
        pack_size: u32,
    ) -> RusticResult<Self> {
        // guess the header size from size_hint and pack_size
        // If the guess is too small, we have to re-read. If the guess is too large, we have to have read too much
        // but this should normally not matter too much. So we try to overguess here...
        let size_guess = size_hint.unwrap_or(0);

        // read (guessed) header + length field
        let read_size = size_guess + constants::LENGTH_LEN;
        let offset = pack_size - read_size;
        let mut data = be.read_partial(FileType::Pack, &id, false, offset, read_size)?;

        // get header length from the file
        let size_real =
            PackHeaderLength::from_binary(&data.split_off(size_guess as usize))?.to_u32();
        trace!("header size: {size_real}");

        if size_real + constants::LENGTH_LEN > pack_size {
            return Err(PackFileErrorKind::HeaderLengthTooLarge {
                size_real,
                pack_size,
            }
            .into());
        }

        // now read the header
        let data = if size_real <= size_guess {
            // header was already read
            data.split_off((size_guess - size_real) as usize)
        } else {
            // size_guess was too small; we have to read again
            let offset = pack_size - size_real - constants::LENGTH_LEN;
            be.read_partial(FileType::Pack, &id, false, offset, size_real)?
        };

        let header = Self::from_binary(&be.decrypt(&data)?)?;

        if header.size() != size_real {
            return Err(PackFileErrorKind::HeaderLengthDoesNotMatchHeaderContents {
                size_real,
                size_computed: header.size(),
            }
            .into());
        }

        if header.pack_size() != pack_size {
            return Err(
                PackFileErrorKind::HeaderPackSizeComputedDoesNotMatchRealPackFile {
                    size_real: pack_size,
                    size_computed: header.pack_size(),
                }
                .into(),
            );
        }

        Ok(header)
    }

    /// Convert this [`PackHeader`] into a [`Vec`] of [`IndexBlob`]s
    // Clippy lint: Destructor for [`PackHeader`] cannot be evaluated at compile time
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub(crate) fn into_blobs(self) -> Vec<IndexBlob> {
        self.0
    }

    /// Calculate the pack header size from the contained blobs
    fn size(&self) -> u32 {
        PackHeaderRef(&self.0).size()
    }

    /// Calculate the pack size from the contained blobs
    fn pack_size(&self) -> u32 {
        PackHeaderRef(&self.0).pack_size()
    }
}

/// As [`PackHeader`], but utilizing a reference instead
#[derive(Debug, Clone)]
pub struct PackHeaderRef<'a>(pub &'a [IndexBlob]);

impl<'a> PackHeaderRef<'a> {
    /// Create a new [`PackHeaderRef`] from a [`IndexPack`]
    ///
    /// # Arguments
    ///
    /// * `pack` - The [`IndexPack`] to create the [`PackHeaderRef`] from
    #[must_use]
    pub(crate) fn from_index_pack(pack: &'a IndexPack) -> Self {
        Self(&pack.blobs)
    }

    /// Calculate the pack header size from the contained blobs
    #[must_use]
    pub(crate) fn size(&self) -> u32 {
        self.0.iter().fold(constants::COMP_OVERHEAD, |acc, blob| {
            acc + HeaderEntry::from_blob(blob).length()
        })
    }

    /// Calculate the pack size from the contained blobs
    #[must_use]
    pub(crate) fn pack_size(&self) -> u32 {
        self.0.iter().fold(
            constants::COMP_OVERHEAD + constants::LENGTH_LEN,
            |acc, blob| acc + blob.length + HeaderEntry::from_blob(blob).length(),
        )
    }

    /// Generate the binary representation of the pack header
    ///
    /// # Errors
    ///
    /// * [`PackFileErrorKind::WritingBinaryRepresentationFailed`] - If writing the binary representation failed
    ///
    /// [`PackFileErrorKind::WritingBinaryRepresentationFailed`]: crate::error::PackFileErrorKind::WritingBinaryRepresentationFailed
    pub(crate) fn to_binary(&self) -> RusticResult<Vec<u8>> {
        let mut writer = Cursor::new(Vec::with_capacity(self.pack_size() as usize));
        // collect header entries
        for blob in self.0 {
            HeaderEntry::from_blob(blob)
                .write(&mut writer)
                .map_err(PackFileErrorKind::WritingBinaryRepresentationFailed)?;
        }
        Ok(writer.into_inner())
    }
}
