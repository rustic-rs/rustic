use bytesize::ByteSize;

/// Convert a [`ByteSize`] to a human readable string
#[must_use]
pub fn bytes_size_to_string(b: u64) -> String {
    ByteSize(b).display().to_string()
}
