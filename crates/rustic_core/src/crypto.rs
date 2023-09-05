use crate::RusticResult;

pub(crate) mod aespoly1305;
pub(crate) mod hasher;

/// A trait for encrypting and decrypting data.
pub trait CryptoKey: Clone + Sized + Send + Sync + 'static {
    /// Decrypt the given data.
    ///
    /// # Arguments
    ///
    /// * `data` - The data to decrypt.
    ///
    /// # Returns
    ///
    /// A vector containing the decrypted data.
    fn decrypt_data(&self, data: &[u8]) -> RusticResult<Vec<u8>>;

    /// Encrypt the given data.
    ///
    /// # Arguments
    ///
    /// * `data` - The data to encrypt.
    ///
    /// # Returns
    ///
    /// A vector containing the encrypted data.
    fn encrypt_data(&self, data: &[u8]) -> RusticResult<Vec<u8>>;
}
