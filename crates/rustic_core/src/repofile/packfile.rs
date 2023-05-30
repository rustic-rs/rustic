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
    pub(super) const COMP_OVERHEAD: u32 = 32;
    pub(super) const LENGTH_LEN: u32 = 4;
}

#[derive(BinWrite, BinRead, Debug, Clone, Copy)]
#[brw(little)]
pub struct PackHeaderLength(u32);

impl PackHeaderLength {
    #[must_use]
    pub const fn from_u32(len: u32) -> Self {
        Self(len)
    }

    #[must_use]
    pub const fn to_u32(&self) -> u32 {
        self.0
    }

    /// Read pack header length from binary representation
    pub fn from_binary(data: &[u8]) -> RusticResult<Self> {
        let mut reader = Cursor::new(data);
        Ok(
            Self::read(&mut reader)
                .map_err(PackFileErrorKind::ReadingBinaryRepresentationFailed)?,
        )
    }

    /// generate the binary representation of the pack header length
    pub fn to_binary(&self) -> RusticResult<Vec<u8>> {
        let mut writer = Cursor::new(Vec::with_capacity(4));
        self.write(&mut writer)
            .map_err(PackFileErrorKind::WritingBinaryRepresentationFailed)?;
        Ok(writer.into_inner())
    }
}

#[derive(BinRead, BinWrite, Debug, Clone, Copy)]
#[brw(little)]
pub enum HeaderEntry {
    #[brw(magic(0u8))]
    Data { len: u32, id: Id },

    #[brw(magic(1u8))]
    Tree { len: u32, id: Id },

    #[brw(magic(2u8))]
    CompData { len: u32, len_data: u32, id: Id },

    #[brw(magic(3u8))]
    CompTree { len: u32, len_data: u32, id: Id },
}

impl HeaderEntry {
    const ENTRY_LEN: u32 = 37;
    pub const ENTRY_LEN_COMPRESSED: u32 = 41;

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

    // the length of this header entry
    const fn length(&self) -> u32 {
        match &self {
            Self::Data { .. } | Self::Tree { .. } => Self::ENTRY_LEN,
            Self::CompData { .. } | Self::CompTree { .. } => Self::ENTRY_LEN_COMPRESSED,
        }
    }

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

#[derive(Debug, Clone)]
pub struct PackHeader(Vec<IndexBlob>);

impl PackHeader {
    /// Read the binary representation of the pack header
    pub fn from_binary(pack: &[u8]) -> RusticResult<Self> {
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
    pub fn from_file(
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

    // destructor for [`PackHeader`] cannot be evaluated at compile time
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn into_blobs(self) -> Vec<IndexBlob> {
        self.0
    }

    // calculate the pack header size from the contained blobs
    fn size(&self) -> u32 {
        PackHeaderRef(&self.0).size()
    }

    // calculate the pack size from the contained blobs
    fn pack_size(&self) -> u32 {
        PackHeaderRef(&self.0).pack_size()
    }
}

#[derive(Debug, Clone)]
pub struct PackHeaderRef<'a>(&'a [IndexBlob]);

impl<'a> PackHeaderRef<'a> {
    #[must_use]
    pub fn from_index_pack(pack: &'a IndexPack) -> Self {
        Self(&pack.blobs)
    }

    // calculate the pack header size from the contained blobs
    #[must_use]
    pub fn size(&self) -> u32 {
        self.0.iter().fold(constants::COMP_OVERHEAD, |acc, blob| {
            acc + HeaderEntry::from_blob(blob).length()
        })
    }

    // calculate the pack size from the contained blobs
    #[must_use]
    pub fn pack_size(&self) -> u32 {
        self.0.iter().fold(
            constants::COMP_OVERHEAD + constants::LENGTH_LEN,
            |acc, blob| acc + blob.length + HeaderEntry::from_blob(blob).length(),
        )
    }

    /// generate the binary representation of the pack header
    pub fn to_binary(&self) -> RusticResult<Vec<u8>> {
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
