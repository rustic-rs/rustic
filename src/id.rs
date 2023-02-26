use std::fmt;
use std::ops::Deref;
use std::path::Path;

use binrw::{BinRead, BinWrite};
use derive_more::{Constructor, Display};
use rand::{thread_rng, RngCore};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const LEN: usize = 32;
const HEX_LEN: usize = LEN * 2;

#[derive(
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Constructor,
    Serialize,
    Deserialize,
    BinWrite,
    BinRead,
    Display,
)]
#[display(fmt = "{}", "&self.to_hex()[0..8]")]
pub struct Id(
    #[serde(serialize_with = "hex::serde::serialize")]
    #[serde(deserialize_with = "hex::serde::deserialize")]
    [u8; LEN],
);

/// [`IdError`] describes the errors that can be returned by processing IDs
#[derive(Error, Debug)]
pub enum IdError {
    #[error("Hex decoding error")]
    HexError(#[from] hex::FromHexError),
}

impl Id {
    pub fn from_hex(s: &str) -> Result<Self, IdError> {
        let mut id = Self::default();

        hex::decode_to_slice(s, &mut id.0).map_err(IdError::HexError)?;

        Ok(id)
    }

    pub fn random() -> Self {
        let mut id = Self::default();
        thread_rng().fill_bytes(&mut id.0);
        id
    }

    pub fn to_hex(self) -> HexId {
        let mut hex_id = HexId::EMPTY;
        // HexId's len is LEN * 2
        hex::encode_to_slice(self.0, &mut hex_id.0).unwrap();
        hex_id
    }

    pub fn is_null(&self) -> bool {
        self == &Id::default()
    }
}

impl fmt::Debug for Id {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", &*self.to_hex())
    }
}

#[derive(Copy, Clone)]
pub struct HexId([u8; HEX_LEN]);

impl HexId {
    const EMPTY: Self = Self([b'0'; HEX_LEN]);

    pub fn as_str(&self) -> &str {
        // This is only ever filled with hex chars, which are ascii
        std::str::from_utf8(&self.0).unwrap()
    }
}

impl Deref for HexId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsRef<Path> for HexId {
    fn as_ref(&self) -> &Path {
        self.as_str().as_ref()
    }
}
