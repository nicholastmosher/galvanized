//! The identity of an automerge document.
//!
//! Wire form: deterministic CBOR bytes (landed with the full 002 work).
//! Human-facing string form: base58 (`Display`/`FromStr`), also full 002.
//! For now the type is a newtype over a fixed 16-byte array with
//! `Debug`/`Clone`/`PartialEq`/`Eq`/`Hash` and byte accessors — enough for
//! the engine to carry it; the string forms land with 002's full deliverable.

use std::fmt;
use std::hash::Hash;

use thiserror::Error;

/// A 16-byte document identifier.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DocumentId([u8; 16]);

impl DocumentId {
    /// Generates a fresh random document id using the provided RNG.
    pub fn new<R: rand_core::RngCore>(rng: &mut R) -> Self {
        let mut bytes = [0u8; 16];
        rng.fill_bytes(&mut bytes);
        Self(bytes)
    }

    /// Constructs a document id from raw bytes.
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Returns the raw bytes of the document id.
    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl fmt::Debug for DocumentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DocumentId({})", hex(&self.0))
    }
}

/// Hex-encode a byte slice lowercase (used for `Debug` until base58
/// `Display` lands with the full 002 work).
fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

#[derive(Debug, Error)]
#[error("invalid document id: {0}")]
pub struct BadDocumentId(pub String);
