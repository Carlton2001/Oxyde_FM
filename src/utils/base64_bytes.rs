//! Byte fields of JSON messages as base64 text. As an array of numbers, a byte
//! costs about four characters of JSON; as base64, one and a third. Arrays
//! written by earlier versions still read.
use base64::Engine;
use serde::de::{self, SeqAccess, Visitor};
use std::fmt;

pub(crate) fn serialize<S: serde::Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&base64::engine::general_purpose::STANDARD.encode(bytes))
}

pub(crate) fn deserialize<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
    struct Bytes;
    impl<'de> Visitor<'de> for Bytes {
        type Value = Vec<u8>;
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("base64 text or an array of bytes")
        }
        fn visit_str<E: de::Error>(self, text: &str) -> Result<Vec<u8>, E> {
            base64::engine::general_purpose::STANDARD
                .decode(text)
                .map_err(E::custom)
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut items: A) -> Result<Vec<u8>, A::Error> {
            let mut bytes = Vec::with_capacity(items.size_hint().unwrap_or(0).min(1 << 20));
            while let Some(byte) = items.next_element()? {
                bytes.push(byte);
            }
            Ok(bytes)
        }
    }
    deserializer.deserialize_any(Bytes)
}
