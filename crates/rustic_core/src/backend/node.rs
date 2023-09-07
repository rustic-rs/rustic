use std::{
    cmp::Ordering,
    ffi::{OsStr, OsString},
    fmt::Debug,
    path::Path,
    str::FromStr,
};

#[cfg(not(windows))]
use std::fmt::Write;
#[cfg(not(windows))]
use std::os::unix::ffi::OsStrExt;

#[cfg(not(windows))]
use crate::RusticResult;

use chrono::{DateTime, Local};
use derive_more::Constructor;
use serde::{Deserialize, Deserializer, Serialize};
use serde_aux::prelude::*;
use serde_with::{
    base64::{Base64, Standard},
    formats::Padded,
    serde_as, DeserializeAs, SerializeAs,
};

#[cfg(not(windows))]
use crate::error::NodeErrorKind;

use crate::id::Id;

#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Constructor)]
/// A node within the tree hierarchy
pub struct Node {
    /// Name of the node: filename or dirname.
    ///
    /// # Warning
    ///
    /// This contains an escaped variant of the name in order to handle non-unicode filenames.
    /// Don't access this field directly, use the [`Node::name()`] method instead!
    pub name: String,
    #[serde(flatten)]
    /// Information about node type
    pub node_type: NodeType,
    #[serde(flatten)]
    /// Node Metadata
    pub meta: Metadata,
    #[serde(default, deserialize_with = "deserialize_default_from_null")]
    /// Contents of the Node
    ///
    /// # Note
    ///
    /// This should be only set for regular files.
    pub content: Option<Vec<Id>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Subtree of the Node.
    ///
    /// # Note
    ///
    /// This should be only set for directories. (TODO: Check if this is correct)
    pub subtree: Option<Id>,
}

#[serde_as]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "lowercase")]
/// Types a [`Node`] can have with type-specific additional information
pub enum NodeType {
    /// Node is a regular file
    File,
    /// Node is a directory
    Dir,
    /// Node is a symlink
    Symlink {
        /// The target of the symlink
        ///
        /// # Warning
        ///
        /// This contains the target only if it is a valid unicode target.
        /// Dont't access this field directly, use the [`NodeType::to_link()`] method instead!
        linktarget: String,
        #[serde_as(as = "Option<Base64>")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        /// The raw link target saved as bytes.
        ///
        /// This is only filled (and mandatory) if the link target is non-unicode.
        linktarget_raw: Option<Vec<u8>>,
    },
    /// Node is a block device file
    Dev {
        #[serde(default)]
        /// Device id
        device: u64,
    },
    /// Node is a char device file
    Chardev {
        #[serde(default)]
        /// Device id
        device: u64,
    },
    /// Node is a fifo
    Fifo,
    /// Node is a socket
    Socket,
}

impl NodeType {
    #[cfg(not(windows))]
    /// Get a [`NodeType`] from a linktarget path
    pub fn from_link(target: &Path) -> Self {
        let (linktarget, linktarget_raw) = target.to_str().map_or_else(
            || {
                (
                    target.as_os_str().to_string_lossy().to_string(),
                    Some(target.as_os_str().as_bytes().to_vec()),
                )
            },
            |t| (t.to_string(), None),
        );
        Self::Symlink {
            linktarget,
            linktarget_raw,
        }
    }

    #[cfg(windows)]
    // Windows doen't support non-unicode link targets, so we assume unicode here.
    // TODO: Test and check this!
    /// Get a [`NodeType`] from a linktarget path
    pub fn from_link(target: &Path) -> Self {
        Self::Symlink {
            linktarget: target.as_os_str().to_string_lossy().to_string(),
            linktarget_raw: None,
        }
    }

    // Must be only called on NodeType::Symlink!
    /// Get the link path from a `NodeType::Symlink`.
    #[cfg(not(windows))]
    pub fn to_link(&self) -> &Path {
        match self {
            Self::Symlink {
                linktarget,
                linktarget_raw,
            } => linktarget_raw.as_ref().map_or_else(
                || Path::new(linktarget),
                |t| Path::new(OsStr::from_bytes(t)),
            ),
            _ => panic!("called method to_link on non-symlink!"),
        }
    }

    /// Convert a `NodeType::Symlink` to a `Path`.
    ///
    /// # Warning
    ///
    /// Must be only called on `NodeType::Symlink`!
    ///
    /// # Panics
    ///
    /// * If called on a non-symlink node
    /// * If the link target is not valid unicode
    // TODO: Implement non-unicode link targets correctly for windows
    #[cfg(windows)]
    pub fn to_link(&self) -> &Path {
        match self {
            Self::Symlink { linktarget, .. } => Path::new(linktarget),
            _ => panic!("called method to_link on non-symlink!"),
        }
    }
}

impl Default for NodeType {
    fn default() -> Self {
        Self::File
    }
}

/// Metadata of a [`Node`]
#[serde_with::apply(
    Option => #[serde(default, skip_serializing_if = "Option::is_none")],
    u64 => #[serde(default, skip_serializing_if = "is_default")],
)]
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct Metadata {
    /// Unix file mode
    pub mode: Option<u32>,
    /// Unix mtime (last modification time)
    pub mtime: Option<DateTime<Local>>,
    /// Unix atime (last access time)
    pub atime: Option<DateTime<Local>>,
    /// Unix ctime (last status change time)
    pub ctime: Option<DateTime<Local>>,
    /// Unix uid (user id)
    pub uid: Option<u32>,
    /// Unix gid (group id)
    pub gid: Option<u32>,
    /// Unix user name
    pub user: Option<String>,
    /// Unix group name
    pub group: Option<String>,
    /// Unix inode number
    pub inode: u64,
    /// Unix device id
    pub device_id: u64,
    /// Size of the node
    pub size: u64,
    /// Number of hardlinks to this node
    pub links: u64,
    /// Extended attributes of the node
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extended_attributes: Vec<ExtendedAttribute>,
}

// Deserialize a Base64-encoded value into Vec<u8>.
//
// # Arguments
//
// * `deserializer` - The deserializer to use.
//
// # Errors
//
// If the value is not a valid Base64-encoded value.
//
// # Returns
//
// The deserialized value.
//
// # Note
//
// Handles '"value" = null' by first deserializing into a Option.
fn deserialize_value<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let value: Option<Vec<u8>> = Base64::<Standard, Padded>::deserialize_as(deserializer)?;
    Ok(value.unwrap_or_default())
}

/// Extended attribute of a [`Node`]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtendedAttribute {
    /// Name of the extended attribute
    pub(crate) name: String,
    /// Value of the extended attribute
    #[serde(
        serialize_with = "Base64::<Standard,Padded>::serialize_as",
        deserialize_with = "deserialize_value"
    )]
    pub(crate) value: Vec<u8>,
}

pub(crate) fn is_default<T: Default + PartialEq>(t: &T) -> bool {
    t == &T::default()
}

impl Node {
    /// Create a new [`Node`] with the given name, type and metadata
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the node
    /// * `node_type` - Type of the node
    /// * `meta` - Metadata of the node
    ///
    /// # Returns
    ///
    /// The created [`Node`]
    #[must_use]
    pub(crate) fn new_node(name: &OsStr, node_type: NodeType, meta: Metadata) -> Self {
        Self {
            name: escape_filename(name),
            node_type,
            content: None,
            subtree: None,
            meta,
        }
    }
    #[must_use]
    /// Evaluates if this node is a directory
    pub const fn is_dir(&self) -> bool {
        matches!(self.node_type, NodeType::Dir)
    }

    #[must_use]
    /// Evaluates if this node is a symlink
    pub const fn is_symlink(&self) -> bool {
        matches!(self.node_type, NodeType::Symlink { .. })
    }

    #[must_use]
    /// Evaluates if this node is a regular file
    pub const fn is_file(&self) -> bool {
        matches!(self.node_type, NodeType::File)
    }

    #[must_use]
    /// Evaluates if this node is a special file
    pub const fn is_special(&self) -> bool {
        matches!(
            self.node_type,
            NodeType::Symlink { .. }
                | NodeType::Dev { .. }
                | NodeType::Chardev { .. }
                | NodeType::Fifo
                | NodeType::Socket
        )
    }

    #[must_use]
    /// Get the node name as `OsString`, handling name ecaping
    ///
    /// # Panics
    ///
    /// If the name is not valid unicode
    pub fn name(&self) -> OsString {
        unescape_filename(&self.name).unwrap_or_else(|_| OsString::from_str(&self.name).unwrap())
    }
}

/// An ordering function returning the latest node by mtime
///
/// # Arguments
///
/// * `n1` - First node
/// * `n2` - Second node
///
/// # Returns
///
/// The ordering of the two nodes
pub fn last_modified_node(n1: &Node, n2: &Node) -> Ordering {
    n1.meta.mtime.cmp(&n2.meta.mtime)
}

// TODO: Should be probably called `_lossy`
// TODO(Windows): This is not able to handle non-unicode filenames and
// doesn't treat filenames which need and escape (like `\`, `"`, ...) correctly
#[cfg(windows)]
fn escape_filename(name: &OsStr) -> String {
    name.to_string_lossy().to_string()
}

/// Unescape a filename
///
/// # Arguments
///
/// * `s` - The escaped filename
#[cfg(windows)]
fn unescape_filename(s: &str) -> Result<OsString, core::convert::Infallible> {
    OsString::from_str(s)
}

#[cfg(not(windows))]
/// Escape a filename
///
/// # Arguments
///
/// * `name` - The filename to escape
// This escapes the filename in a way that *should* be compatible to golangs
// stconv.Quote, see https://pkg.go.dev/strconv#Quote
// However, so far there was no specification what Quote really does, so this
// is some kind of try-and-error and maybe does not cover every case.
fn escape_filename(name: &OsStr) -> String {
    let mut input = name.as_bytes();
    let mut s = String::with_capacity(name.len());

    let push = |s: &mut String, p: &str| {
        for c in p.chars() {
            match c {
                '\\' => s.push_str("\\\\"),
                '\"' => s.push_str("\\\""),
                '\u{7}' => s.push_str("\\a"),
                '\u{8}' => s.push_str("\\b"),
                '\u{c}' => s.push_str("\\f"),
                '\n' => s.push_str("\\n"),
                '\r' => s.push_str("\\r"),
                '\t' => s.push_str("\\t"),
                '\u{b}' => s.push_str("\\v"),
                c => s.push(c),
            };
        }
    };

    loop {
        match std::str::from_utf8(input) {
            Ok(valid) => {
                push(&mut s, valid);
                break;
            }
            Err(error) => {
                let (valid, after_valid) = input.split_at(error.valid_up_to());
                push(&mut s, std::str::from_utf8(valid).unwrap());

                if let Some(invalid_sequence_length) = error.error_len() {
                    for b in &after_valid[..invalid_sequence_length] {
                        write!(s, "\\x{b:02x}").unwrap();
                    }
                    input = &after_valid[invalid_sequence_length..];
                } else {
                    for b in after_valid {
                        write!(s, "\\x{b:02x}").unwrap();
                    }
                    break;
                }
            }
        }
    }
    s
}

#[cfg(not(windows))]
/// Unescape a filename
///
/// # Arguments
///
/// * `s` - The escaped filename
// inspired by the enquote crate
fn unescape_filename(s: &str) -> RusticResult<OsString> {
    let mut chars = s.chars();
    let mut u = Vec::new();
    loop {
        match chars.next() {
            None => break,
            Some(c) => match c {
                '\\' => match chars.next() {
                    None => return Err(NodeErrorKind::UnexpectedEOF.into()),
                    Some(c) => match c {
                        '\\' => u.push(b'\\'),
                        '"' => u.push(b'"'),
                        '\'' => u.push(b'\''),
                        '`' => u.push(b'`'),
                        'a' => u.push(b'\x07'),
                        'b' => u.push(b'\x08'),
                        'f' => u.push(b'\x0c'),
                        'n' => u.push(b'\n'),
                        'r' => u.push(b'\r'),
                        't' => u.push(b'\t'),
                        'v' => u.push(b'\x0b'),
                        // hex
                        'x' => {
                            let hex = take(&mut chars, 2);
                            u.push(
                                u8::from_str_radix(&hex, 16)
                                    .map_err(NodeErrorKind::FromParseIntError)?,
                            );
                        }
                        // unicode
                        'u' => {
                            let n = u32::from_str_radix(&take(&mut chars, 4), 16)
                                .map_err(NodeErrorKind::FromParseIntError)?;
                            let c = std::char::from_u32(n).ok_or(NodeErrorKind::InvalidUnicode)?;
                            let mut bytes = vec![0u8; c.len_utf8()];
                            _ = c.encode_utf8(&mut bytes);
                            u.extend_from_slice(&bytes);
                        }
                        'U' => {
                            let n = u32::from_str_radix(&take(&mut chars, 8), 16)
                                .map_err(NodeErrorKind::FromParseIntError)?;
                            let c = std::char::from_u32(n).ok_or(NodeErrorKind::InvalidUnicode)?;
                            let mut bytes = vec![0u8; c.len_utf8()];
                            _ = c.encode_utf8(&mut bytes);
                            u.extend_from_slice(&bytes);
                        }
                        _ => return Err(NodeErrorKind::UnrecognizedEscape.into()),
                    },
                },
                // normal char
                _ => {
                    let mut bytes = vec![0u8; c.len_utf8()];
                    _ = c.encode_utf8(&mut bytes);
                    u.extend_from_slice(&bytes);
                }
            },
        }
    }

    Ok(OsStr::from_bytes(&u).to_os_string())
}

#[cfg(not(windows))]
#[inline]
// Iterator#take cannot be used because it consumes the iterator
fn take<I: Iterator<Item = char>>(iterator: &mut I, n: usize) -> String {
    let mut s = String::with_capacity(n);
    for _ in 0..n {
        s.push(iterator.next().unwrap_or_default());
    }
    s
}

#[cfg(not(windows))]
#[cfg(test)]
mod tests {
    use super::*;

    use quickcheck_macros::quickcheck;
    use rstest::rstest;

    #[quickcheck]
    fn escape_unescape_is_identity(bytes: Vec<u8>) -> bool {
        let name = OsStr::from_bytes(&bytes);
        name == match unescape_filename(&escape_filename(name)) {
            Ok(s) => s,
            Err(_) => return false,
        }
    }

    #[rstest]
    #[case(b"\\", r#"\\"#)]
    #[case(b"\"", r#"\""#)]
    #[case(b"'", r#"'"#)]
    #[case(b"`", r#"`"#)]
    #[case(b"\x07", r#"\a"#)]
    #[case(b"\x08", r#"\b"#)]
    #[case(b"\x0b", r#"\v"#)]
    #[case(b"\x0c", r#"\f"#)]
    #[case(b"\n", r#"\n"#)]
    #[case(b"\r", r#"\r"#)]
    #[case(b"\t", r#"\t"#)]
    #[case(b"\xab", r#"\xab"#)]
    #[case(b"\xc2", r#"\xc2"#)]
    #[case(b"\xff", r#"\xff"#)]
    #[case(b"\xc3\x9f", "\u{00df}")]
    #[case(b"\xe2\x9d\xa4", "\u{2764}")]
    #[case(b"\xf0\x9f\x92\xaf", "\u{01f4af}")]
    fn escape_cases(#[case] input: &[u8], #[case] expected: &str) {
        let name = OsStr::from_bytes(input);
        assert_eq!(expected, escape_filename(name));
    }

    #[rstest]
    #[case(r#"\\"#, b"\\")]
    #[case(r#"\""#, b"\"")]
    #[case(r#"\'"#, b"\'")]
    #[case(r#"\`"#, b"`")]
    #[case(r#"\a"#, b"\x07")]
    #[case(r#"\b"#, b"\x08")]
    #[case(r#"\v"#, b"\x0b")]
    #[case(r#"\f"#, b"\x0c")]
    #[case(r#"\n"#, b"\n")]
    #[case(r#"\r"#, b"\r")]
    #[case(r#"\t"#, b"\t")]
    #[case(r#"\xab"#, b"\xab")]
    #[case(r#"\xAB"#, b"\xab")]
    #[case(r#"\xFF"#, b"\xff")]
    #[case(r#"\u00df"#, b"\xc3\x9f")]
    #[case(r#"\u00DF"#, b"\xc3\x9f")]
    #[case(r#"\u2764"#, b"\xe2\x9d\xa4")]
    #[case(r#"\U0001f4af"#, b"\xf0\x9f\x92\xaf")]
    fn unescape_cases(#[case] input: &str, #[case] expected: &[u8]) {
        let expected = OsStr::from_bytes(expected);
        assert_eq!(expected, unescape_filename(input).unwrap());
    }

    #[quickcheck]
    fn from_link_to_link_is_identity(bytes: Vec<u8>) -> bool {
        let path = Path::new(OsStr::from_bytes(&bytes));
        path == NodeType::from_link(path).to_link()
    }
}
