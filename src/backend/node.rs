use std::ffi::{OsStr, OsString};
use std::fmt::Debug;
use std::os::unix::ffi::OsStrExt;
use std::str::FromStr;

use anyhow::{anyhow, bail, Result};
use chrono::{DateTime, Local};
use derive_getters::Getters;
use derive_more::{Constructor, IsVariant};
use serde::{Deserialize, Serialize};
use serde_aux::prelude::*;

use crate::id::Id;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Constructor)]
pub struct Node {
    pub name: String,
    #[serde(flatten)]
    pub node_type: NodeType,
    #[serde(flatten)]
    pub meta: Metadata,
    #[serde(default, deserialize_with = "deserialize_default_from_null")]
    pub content: Option<Vec<Id>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtree: Option<Id>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, IsVariant)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum NodeType {
    File,
    Dir,
    Symlink {
        linktarget: String,
    },
    Dev {
        #[serde(default)]
        device: u64,
    },
    Chardev {
        #[serde(default)]
        device: u64,
    },
    Fifo,
    Socket,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, Getters)]
pub struct Metadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mtime: Option<DateTime<Local>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atime: Option<DateTime<Local>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ctime: Option<DateTime<Local>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub inode: u64,
    #[serde(default, skip_serializing_if = "is_default")]
    pub device_id: u64,
    #[serde(default, skip_serializing_if = "is_default")]
    pub size: u64,
    #[serde(default, skip_serializing_if = "is_default")]
    pub links: u64,
}

fn is_default<T: Default + PartialEq>(t: &T) -> bool {
    t == &T::default()
}

impl Node {
    pub fn new_node(name: &OsStr, node_type: NodeType, meta: Metadata) -> Self {
        Node {
            name: escape_filename(name),
            node_type,
            content: None,
            subtree: None,
            meta,
        }
    }
    pub fn is_dir(&self) -> bool {
        self.node_type == NodeType::Dir
    }

    pub fn set_subtree(&mut self, id: Id) {
        self.subtree = Some(id);
    }

    pub fn set_content(&mut self, content: Vec<Id>) {
        self.content = Some(content);
    }

    pub fn name(&self) -> OsString {
        unescape_filename(&self.name).unwrap_or_else(|_| OsString::from_str(&self.name).unwrap())
    }

    pub fn node_type(&self) -> &NodeType {
        &self.node_type
    }

    pub fn meta(&self) -> &Metadata {
        &self.meta
    }

    pub fn content(&self) -> &Vec<Id> {
        self.content.as_ref().unwrap()
    }

    pub fn subtree(&self) -> &Option<Id> {
        &self.subtree
    }
}

pub fn escape_filename(name: &OsStr) -> String {
    name.as_bytes().escape_ascii().to_string()
}

// inspired by the enquote crate
pub fn unescape_filename(s: &str) -> Result<OsString> {
    let mut chars = s.chars();
    let mut u = Vec::new();
    loop {
        match chars.next() {
            None => break,
            Some(c) => match c {
                '\\' => match chars.next() {
                    None => bail!("UnexpectedEOF"),
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
                            u.push(u8::from_str_radix(&hex, 16)?)
                        }
                        // unicode
                        'u' => {
                            let n = u32::from_str_radix(&take(&mut chars, 4), 16)?;
                            let c =
                                std::char::from_u32(n).ok_or_else(|| anyhow!("invalid unicode"))?;
                            let mut bytes = vec![0u8; c.len_utf8()];
                            c.encode_utf8(&mut bytes);
                            u.extend_from_slice(&bytes);
                        }
                        'U' => {
                            let n = u32::from_str_radix(&take(&mut chars, 8), 16)?;
                            let c =
                                std::char::from_u32(n).ok_or_else(|| anyhow!("invalid unicode"))?;
                            let mut bytes = vec![0u8; c.len_utf8()];
                            c.encode_utf8(&mut bytes);
                            u.extend_from_slice(&bytes);
                        }
                        _ => bail!("UnrecognizedEscape"),
                    },
                },
                // normal char
                _ => {
                    let mut bytes = vec![0u8; c.len_utf8()];
                    c.encode_utf8(&mut bytes);
                    u.extend_from_slice(&bytes);
                }
            },
        }
    }

    Ok(OsStr::from_bytes(&u).to_os_string())
}

#[inline]
// Iterator#take cannot be used because it consumes the iterator
fn take<I: Iterator<Item = char>>(iterator: &mut I, n: usize) -> String {
    let mut s = String::with_capacity(n);
    for _ in 0..n {
        s.push(iterator.next().unwrap_or_default());
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    use quickcheck_macros::quickcheck;
    use rstest::rstest;

    #[quickcheck]
    fn escape_unescape_is_identity(bytes: Vec<u8>) -> bool {
        let name = OsStr::from_bytes(&bytes);
        name == &match unescape_filename(&escape_filename(name)) {
            Ok(s) => s,
            Err(_) => return false,
        }
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
        assert_eq!(expected, unescape_filename(input).unwrap())
    }
}
