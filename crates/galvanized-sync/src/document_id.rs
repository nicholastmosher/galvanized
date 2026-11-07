//! The identity of an automerge document.
//!
//! Wire form: deterministic CBOR bytes (landed with the full 002 work).
//! Human-facing string form: base58 (`Display`/`FromStr`), also full 002.
//! For now the type is a newtype over a fixed 16-byte array with
//! `Debug`/`Clone`/`PartialEq`/`Eq`/`Hash` and byte accessors — enough for
//! the engine to carry it; the string forms land with 002's full deliverable.

use std::fmt;
use std::hash::Hash;
use std::str::FromStr;

use bs58;
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

impl AsRef<[u8]> for DocumentId {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for DocumentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // base58 with check, consistent with `Display`.
        bs58::encode(&self.0).with_check().into_string().fmt(f)
    }
}

impl fmt::Display for DocumentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // base58 with check, so a human-facing string form survives
        // single-bit transcription errors (ruling 002 Q5).
        bs58::encode(&self.0).with_check().into_string().fmt(f)
    }
}

impl FromStr for DocumentId {
    type Err = BadDocumentId;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = bs58::decode(s)
            .with_check(None)
            .into_vec()
            .map_err(|e| BadDocumentId(e.to_string()))?;
        let arr: [u8; 16] = bytes
            .as_slice()
            .try_into()
            .map_err(|_| BadDocumentId("expected 16 bytes".to_string()))?;
        Ok(Self(arr))
    }
}

#[derive(Debug, Error)]
#[error("invalid document id: {0}")]
pub struct BadDocumentId(pub String);
