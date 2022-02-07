use derive_more::Display;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, Display)]
#[display(fmt = "{}", "&self.to_hex()[0..8]")]
pub struct Id(
    #[serde(serialize_with = "hex::serde::serialize")]
    #[serde(deserialize_with = "hex::serde::deserialize")]
    [u8; 32],
);

/// IdError describes the errors that can be returned by processing IDs
#[derive(Error, Debug)]
pub enum IdError {
    #[error("Hex decoding error")]
    HexError(#[from] hex::FromHexError),

    #[error("invalid length for ID '{0}'")]
    LengthError(String),
}

impl Id {
    pub fn from_hex(s: &str) -> Result<Self, IdError> {
        let unhex = hex::decode(s)?;
        Ok(Self(
            unhex
                .try_into()
                .map_err(|_err| IdError::LengthError(s.to_string()))?,
        ))
    }

    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }
}
