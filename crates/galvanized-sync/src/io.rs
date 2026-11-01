//! IO layer for the sans-IO engine.
//!
//! The engine emits [`IoTask`] *effects* and the runtime (ticket 007, over
//! Iroh) executes them and reports back via [`IoResult`]. The engine itself
//! does no IO. [`Storage`] and [`Transport`] are the runtime-side contracts
//! the engine's effects refer to; they are defined here so consumers and
//! 007 implement against a single contract, but the engine does not call
//! them directly.

use std::fmt;
use std::sync::Arc;

use thiserror::Error;

/// Identifies a connection within a [`crate::Hub`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConnectionId(pub u64);

/// Identifies an in-flight IO task reported back via [`IoResult`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IoTaskId(pub u64);

/// A storage key (a sequence of byte components).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StorageKey(pub Vec<Vec<u8>>);

impl StorageKey {
    /// Constructs a key from byte components.
    pub fn new(components: Vec<Vec<u8>>) -> Self {
        Self(components)
    }

    /// Returns the key components.
    pub fn components(&self) -> &[Vec<u8>] {
        &self.0
    }
}

/// A storage operation the runtime executes on behalf of the engine.
#[derive(Debug, Clone)]
pub enum StorageTask {
    /// Fetch the value for a key.
    Get { key: StorageKey },
    /// Insert or replace a value.
    Put { key: StorageKey, value: Vec<u8> },
    /// Remove a key.
    Remove { key: StorageKey },
    /// Fetch all entries whose keys are in `[start, end)`.
    Range { start: StorageKey, end: StorageKey },
}

/// An effect the runtime must execute. Emitted by [`crate::Hub::handle_event`]
/// in [`crate::HubResults`].
#[derive(Debug, Clone)]
pub enum IoTask {
    /// Send bytes to a connected peer.
    SendMsg {
        connection: ConnectionId,
        bytes: Vec<u8>,
    },
    /// Close a connection.
    CloseConnection { connection: ConnectionId },
    /// Perform a storage operation.
    Storage { task: StorageTask },
    /// Load a document's bytes from storage.
    LoadDocument { document_id: crate::DocumentId },
}

/// The result of a completed [`IoTask`], reported back to the engine via
/// `HubEvent::IoComplete`.
#[derive(Debug, Clone)]
pub enum IoResult {
    /// A send completed.
    Sent { task: IoTaskId },
    /// A storage operation completed.
    StorageDone {
        task: IoTaskId,
        value: Option<Vec<u8>>,
        range: Vec<(StorageKey, Vec<u8>)>,
    },
    /// A document load completed.
    DocumentLoaded {
        document_id: crate::DocumentId,
        bytes: Option<Vec<u8>>,
    },
    /// An IO task failed.
    Failed { task: IoTaskId, error: IoError },
}

#[derive(Debug, Clone, Error)]
pub enum IoError {
    #[error("storage error: {0}")]
    Storage(String),
    #[error("send error: {0}")]
    Send(String),
    #[error("not found")]
    NotFound,
}

/// The runtime-side KV store contract (range queries). The engine emits
/// [`IoTask::Storage`]; the runtime implements this and reports
/// [`IoResult::StorageDone`].
pub trait Storage: Send + Sync {
    fn get(&self, key: &StorageKey) -> Result<Option<Vec<u8>>, IoError>;
    fn put(&self, key: &StorageKey, value: &[u8]) -> Result<(), IoError>;
    fn remove(&self, key: &StorageKey) -> Result<(), IoError>;
    fn range(
        &self,
        start: &StorageKey,
        end: &StorageKey,
    ) -> Result<Vec<(StorageKey, Vec<u8>)>, IoError>;
}

/// The runtime-side stream/sink contract for a connection. Ticket 007
/// implements this over Iroh QUIC streams. The engine emits
/// [`IoTask::SendMsg`]; the runtime routes them through this.
pub trait Transport: Send + Sync {
    fn send(&self, connection: ConnectionId, bytes: &[u8]) -> Result<(), IoError>;
    fn close(&self, connection: ConnectionId) -> Result<(), IoError>;
}

/// An in-memory [`Storage`] for tests. Not the real on-disk store (that is a
/// separate open question, ticket 007); this exists so the engine's storage
/// effects can be exercised without a runtime.
#[cfg(test)]
pub(crate) struct InMemoryStorage {
    inner: std::sync::Mutex<std::collections::BTreeMap<StorageKey, Vec<u8>>>,
}

#[cfg(test)]
impl InMemoryStorage {
    pub(crate) fn new() -> Self {
        Self {
            inner: std::sync::Mutex::new(std::collections::BTreeMap::new()),
        }
    }
}

#[cfg(test)]
impl Storage for InMemoryStorage {
    fn get(&self, key: &StorageKey) -> Result<Option<Vec<u8>>, IoError> {
        Ok(self.inner.lock().unwrap().get(key).cloned())
    }
    fn put(&self, key: &StorageKey, value: &[u8]) -> Result<(), IoError> {
        self.inner
            .lock()
            .unwrap()
            .insert(key.clone(), value.to_vec());
        Ok(())
    }
    fn remove(&self, key: &StorageKey) -> Result<(), IoError> {
        self.inner.lock().unwrap().remove(key);
        Ok(())
    }
    fn range(
        &self,
        start: &StorageKey,
        end: &StorageKey,
    ) -> Result<Vec<(StorageKey, Vec<u8>)>, IoError> {
        let inner = self.inner.lock().unwrap();
        Ok(inner
            .range(start.clone()..end.clone())
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect())
    }
}

/// A type-erased [`Storage`] handle the engine can hold.
pub type DynStorage = Arc<dyn Storage>;

impl fmt::Display for ConnectionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "conn-{}", self.0)
    }
}

impl fmt::Display for IoTaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "task-{}", self.0)
    }
}

impl fmt::Display for StorageKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[")?;
        for (i, comp) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, "/")?;
            }
            write!(f, "{}", hex(comp))?;
        }
        write!(f, "]")
    }
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}
