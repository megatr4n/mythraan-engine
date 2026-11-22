//! Stable identifiers for assets.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

/// A stable asset identifier backed by a 16-byte UUID.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct AssetId(Uuid);

impl AssetId {
    /// Creates an identifier from UUID bytes in network byte order.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(Uuid::from_bytes(bytes))
    }

    /// Returns the UUID bytes in network byte order.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }
}

impl FromStr for AssetId {
    type Err = uuid::Error;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(text).map(Self)
    }
}

impl fmt::Display for AssetId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl Serialize for AssetId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for AssetId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Uuid::deserialize(deserializer).map(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_test::{Configure, Token, assert_tokens};

    const TEXT: &str = "550e8400-e29b-41d4-a716-446655440000";
    const BYTES: [u8; 16] = [
        0x55, 0x0e, 0x84, 0x00, 0xe2, 0x9b, 0x41, 0xd4, 0xa7, 0x16, 0x44, 0x66, 0x55, 0x44, 0x00,
        0x00,
    ];

    #[test]
    fn round_trips_text_and_bytes() {
        let id: AssetId = TEXT.parse().unwrap();
        assert_eq!(id.to_string(), TEXT);
        assert_eq!(id.as_bytes(), &BYTES);
        assert_eq!(AssetId::from_bytes(BYTES), id);
        assert_eq!(std::mem::size_of::<AssetId>(), BYTES.len());
        assert!(
            "550e8400-e29b-41d4-a716-44665544000g"
                .parse::<AssetId>()
                .is_err()
        );
    }

    #[test]
    fn round_trips_text_and_binary_serde_representations() {
        let id = AssetId::from_bytes(BYTES);
        assert_tokens(&id.readable(), &[Token::Str(TEXT)]);
        assert_tokens(&id.compact(), &[Token::Bytes(&BYTES)]);
    }
}
