use std::{convert::TryFrom, fmt::Display};

use anyhow::{bail, ensure, Context, Result};

/// Number of bytes
const METADATA_LENGTH: usize = 1;
/// 64 bit number => 8 bytes
const UINT64_LENGTH: usize = 8;
/// 256 bits => 32 bytes
const HANDLE_LENGTH: usize = 32;
const LITERAL_CONTENT_LENGTH: usize = HANDLE_LENGTH - METADATA_LENGTH;
const CANONICAL_HASH_LENGTH: usize = HANDLE_LENGTH - UINT64_LENGTH - METADATA_LENGTH;

#[derive(Debug, PartialEq, serde::Deserialize, serde::Serialize, Clone)]
pub(crate) struct Task {
    pub(crate) handle: Handle,
    pub(crate) operation: Operation,
}

#[derive(Debug, PartialEq, serde::Deserialize, serde::Serialize, Clone, Copy)]
pub(crate) enum Operation {
    Apply,
    Eval,
    Fill,
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub(crate) struct Handle {
    pub(crate) size: u64,
    pub(crate) accessibility: Accessibility,
    pub(crate) content: Content,
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub(crate) enum Content {
    Other {
        object_type: Object,
        data: Nonliteral,
    },
    Literal([u8; LITERAL_CONTENT_LENGTH]),
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub(crate) enum Nonliteral {
    Canonical([u8; CANONICAL_HASH_LENGTH]),
    Local(u64),
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub(crate) enum Accessibility {
    Strict,
    Shallow,
    Lazy,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub(crate) enum Object {
    Blob,
    Tree,
    Thunk,
    Tag,
}

impl Handle {
    /// Parses a handle in format [unsigned 64 bit number as hex]-[u64 as hex]-[u64 as hex]-[u64 as hex].
    /// For example, d9-0-4-0 or 10-0-0-2400000000000000.
    pub(crate) fn from_hex(input: &str) -> Result<Self> {
        let handle_content = input
            .split('-')
            .map(|i| u64::from_str_radix(i, 16))
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to parse handle from hex")?;
        ensure!(
            handle_content.len() == 4,
            "Expected handle with exactly 4 parts when parsing from hex"
        );
        let handle_content: [u64; 4] = handle_content.try_into().unwrap();
        let handle_content: [u8; HANDLE_LENGTH] = handle_content
            .into_iter()
            .flat_map(|i| i.to_le_bytes().into_iter())
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        // metadata is
        // if handle is literal:
        //     | strict/shallow/lazy (2 bits) | 1 (1 bit) | size of blob (5 bits)
        // if not a literal:
        //     | strict/shallow/lazy (2 bits) | 0 (1 bit) | 00 | canonical/local (1 bit) | Blob/Tree/Thunk/Tag (2 bits)
        let metadata: u8 = handle_content[HANDLE_LENGTH - 1];
        let is_literal = metadata & 0b10_0000 != 0;
        let accessibility =
            Accessibility::try_from(metadata >> 6).context("getting accessibility bits")?;
        if is_literal {
            // Handle structure
            // data (8 bytes) | data (8 bytes) | data (8 bytes) | data (7 bytes) | metadata (1 byte)
            let literal_size = metadata & 0b1_1111;
            let mut content = [0u8; LITERAL_CONTENT_LENGTH];
            content.copy_from_slice(&handle_content[..LITERAL_CONTENT_LENGTH]);
            return Ok(Self {
                size: literal_size.into(),
                accessibility,
                content: Content::Literal(content),
            });
        }
        let is_canonical = metadata & 0b100 != 0;
        let object_type = Object::try_from(metadata & 0b11).context("getting object bits")?;
        let size = u64::from_le_bytes(
            handle_content[UINT64_LENGTH * 2..UINT64_LENGTH * 3]
                .try_into()
                .unwrap(),
        );
        if is_canonical {
            // Handle structure
            // hash (8 bytes) | hash (8 bytes) | size (8 bytes) | hash (7 bytes) | metadata (1 byte)
            let mut hash = [0u8; CANONICAL_HASH_LENGTH];
            println!("{:?}", handle_content);
            hash[..UINT64_LENGTH * 2].copy_from_slice(&handle_content[..UINT64_LENGTH * 2]);
            hash[UINT64_LENGTH * 2..]
                .copy_from_slice(&handle_content[UINT64_LENGTH * 3..HANDLE_LENGTH - 1]);
            return Ok(Self {
                size,
                accessibility,
                content: Content::Other {
                    object_type,
                    data: Nonliteral::Canonical(hash),
                },
            });
        }

        let local_id = u64::from_le_bytes(handle_content[..UINT64_LENGTH].try_into().unwrap());
        Ok(Self {
            size,
            accessibility,
            content: Content::Other {
                object_type,
                data: Nonliteral::Local(local_id),
            },
        })
    }

    /// Reconstructs the hex string version of a Handle
    pub(crate) fn to_hex(&self) -> String {
        self.to_buffer()
            .chunks_exact(UINT64_LENGTH)
            .map(|s: &[u8]| format!("{:x}", u64::from_le_bytes(s.try_into().unwrap())))
            .collect::<Vec<_>>()
            .join("-")
    }

    fn to_buffer(&self) -> [u8; HANDLE_LENGTH] {
        let mut out_content = [0_u8; HANDLE_LENGTH];

        out_content[HANDLE_LENGTH - 1] |= Into::<u8>::into(self.accessibility) << 6;

        let data = match &self.content {
            Content::Literal(data) => {
                out_content[..LITERAL_CONTENT_LENGTH].copy_from_slice(data);
                out_content[HANDLE_LENGTH - 1] |= 0b10_0000;
                out_content[HANDLE_LENGTH - 1] |= (self.size & 0xFF) as u8;
                return out_content;
            }
            Content::Other { object_type, data } => {
                out_content[HANDLE_LENGTH - 1] |= Into::<u8>::into(*object_type);
                data
            }
        };

        out_content[UINT64_LENGTH * 2..UINT64_LENGTH * 3].copy_from_slice(&self.size.to_le_bytes());

        match data {
            Nonliteral::Canonical(hash) => {
                out_content[HANDLE_LENGTH - 1] |= 0b100;
                out_content[..UINT64_LENGTH * 2].copy_from_slice(&hash[..UINT64_LENGTH * 2]);
                out_content[UINT64_LENGTH * 3..HANDLE_LENGTH - 1]
                    .copy_from_slice(&hash[UINT64_LENGTH * 2..]);
            }
            Nonliteral::Local(id) => {
                out_content[..UINT64_LENGTH].copy_from_slice(&id.to_le_bytes());
            }
        }

        out_content
    }
}

impl Display for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}: {}", self.handle.to_hex(), self.operation))
    }
}

impl TryFrom<u8> for Operation {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => Self::Apply,
            1 => Self::Eval,
            2 => Self::Fill,
            _ => bail!("Invalid number for Operation"),
        })
    }
}

impl From<Operation> for u8 {
    fn from(val: Operation) -> Self {
        match val {
            Operation::Apply => 0,
            Operation::Eval => 1,
            Operation::Fill => 2,
        }
    }
}

impl Display for Operation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Operation::Apply => "Apply",
            Operation::Eval => "Eval",
            Operation::Fill => "Fill",
        })
    }
}

impl Handle {
    pub(crate) fn get_literal_content(&self) -> Option<&[u8]> {
        if let Content::Literal(ref content) = self.content {
            Some(&content[..self.size as usize])
        } else {
            None
        }
    }
}

impl TryFrom<u8> for Accessibility {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => Self::Strict,
            1 => Self::Shallow,
            2 => Self::Lazy,
            _ => bail!("Invalid number for Accessibility"),
        })
    }
}

impl Into<u8> for Accessibility {
    fn into(self) -> u8 {
        match self {
            Accessibility::Strict => 0,
            Accessibility::Shallow => 1,
            Accessibility::Lazy => 2,
        }
    }
}

impl TryFrom<u8> for Object {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => Self::Tree,
            1 => Self::Thunk,
            2 => Self::Blob,
            3 => Self::Tag,
            _ => bail!("Invalid number for Object"),
        })
    }
}

impl From<Object> for u8 {
    fn from(val: Object) -> Self {
        match val {
            Object::Tree => 0,
            Object::Thunk => 1,
            Object::Blob => 2,
            Object::Tag => 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blob() {
        // &strict Blob(10|0|0|2400000000000000)
        let mut content = [0; LITERAL_CONTENT_LENGTH];
        content[0] = 16;
        let handle_string = "10-0-0-2400000000000000";
        let handle = Handle {
            // created as a u32
            size: 4,
            accessibility: Accessibility::Strict,
            content: Content::Literal(content),
        };
        assert_eq!(Handle::from_hex(handle_string).unwrap(), handle);
        assert_eq!(handle_string, handle.to_hex());
    }

    #[test]
    fn thunk() {
        // &strict Thunk(d9|0|4|100000000000000)
        let handle_string = "d9-0-4-100000000000000";
        let handle = Handle {
            // 4 entries
            size: 4,
            accessibility: Accessibility::Strict,
            content: Content::Other {
                object_type: Object::Thunk,
                data: Nonliteral::Local(217),
            },
        };
        assert_eq!(Handle::from_hex(handle_string).unwrap(), handle);
        assert_eq!(handle_string, handle.to_hex());
    }

    #[test]
    fn tag() {
        // &strict Tag(862fcba5ecaade2c|4b24159ac7c28a29|3|715eb1e41f37d42)
        let handle_string = "862fcba5ecaade2c-4b24159ac7c28a29-3-715eb1e41f37d42";
        let mut content = [0; CANONICAL_HASH_LENGTH];
        content[..UINT64_LENGTH].copy_from_slice(
            &u64::from_str_radix("862fcba5ecaade2c", 16)
                .unwrap()
                .to_le_bytes(),
        );
        content[UINT64_LENGTH..UINT64_LENGTH * 2].copy_from_slice(
            &u64::from_str_radix("4b24159ac7c28a29", 16)
                .unwrap()
                .to_le_bytes(),
        );
        // zero out the last byte because that is metadata
        // and is not part of the content hash
        content[UINT64_LENGTH * 2..].copy_from_slice(
            &u64::from_str_radix("0015eb1e41f37d42", 16)
                .unwrap()
                .to_le_bytes()[0..UINT64_LENGTH - 1],
        );
        let handle = Handle {
            // all tags have 3 entries
            size: 3,
            accessibility: Accessibility::Strict,
            content: Content::Other {
                object_type: Object::Tag,
                data: Nonliteral::Canonical(content),
            },
        };
        assert_eq!(Handle::from_hex(handle_string).unwrap(), handle);
        assert_eq!(handle_string, handle.to_hex());
    }
}
