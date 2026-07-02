// Copying from Iroh's protocol.rs, hoping to make it more dynamic/pluggable

use anyhow::Error;
use anyhow::anyhow;
use futures::future::join_all;
use iroh::endpoint::ConnectingError;
use iroh::endpoint::ConnectionError;
use iroh::endpoint::RemoteEndpointIdError;
use iroh::endpoint::quic;
use iroh::protocol::DynProtocolHandler;
use std::collections::BTreeMap;
use thiserror::Error;

use std::sync::Arc;

#[allow(missing_docs)]
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum AcceptError {
    #[error(transparent)]
    Connecting {
        #[from]
        source: ConnectingError,
    },
    #[error(transparent)]
    Connection {
        #[from]
        source: ConnectionError,
    },
    #[error(transparent)]
    MissingRemoteEndpointId { source: RemoteEndpointIdError },
    #[error("Not allowed.")]
    NotAllowed {},
    #[error(transparent)]
    User { source: Error },
}

impl AcceptError {
    /// Creates a new user error from an arbitrary error type.
    // TODO(Frando): Rename to `from_std`
    #[track_caller]
    pub fn from_err<T: std::error::Error + Send + Sync + 'static>(value: T) -> Self {
        AcceptError::User {
            source: anyhow!(value),
        }
    }

    /// Creates a new user error from an arbitrary boxed error.
    #[track_caller]
    pub fn from_boxed(value: Box<dyn std::error::Error + Send + Sync>) -> Self {
        AcceptError::User {
            source: anyhow!(value),
        }
    }
}

impl From<std::io::Error> for AcceptError {
    fn from(err: std::io::Error) -> Self {
        Self::from_err(err)
    }
}

impl From<quic::ClosedStream> for AcceptError {
    fn from(err: quic::ClosedStream) -> Self {
        Self::from_err(err)
    }
}

/// A typed map of protocol handlers, mapping them from ALPNs.
#[derive(Debug, Default)]
pub(crate) struct ProtocolMap(BTreeMap<Vec<u8>, Arc<dyn DynProtocolHandler>>);

impl ProtocolMap {
    /// Returns the registered protocol handler for an ALPN as a [`Arc<dyn ProtocolHandler>`].
    pub(crate) fn get(&self, alpn: &[u8]) -> Option<Arc<dyn DynProtocolHandler>> {
        self.0.get(alpn).map(|p| p.clone())
    }

    /// Inserts a protocol handler.
    pub(crate) fn insert(&mut self, alpn: Vec<u8>, handler: Arc<dyn DynProtocolHandler>) {
        self.0.insert(alpn, handler);
    }

    /// Removes a protocol handler by ALPN, returning the removed handler if one existed.
    pub(crate) fn remove(&mut self, alpn: &[u8]) -> Option<Arc<dyn DynProtocolHandler>> {
        self.0.remove(alpn)
    }

    /// Returns an iterator of all registered ALPN protocol identifiers.
    pub(crate) fn alpns(&self) -> impl Iterator<Item = &Vec<u8>> {
        self.0.keys()
    }

    /// Shuts down all protocol handlers.
    ///
    /// Calls and awaits [`ProtocolHandler::shutdown`] for all registered handlers concurrently.
    pub(crate) async fn shutdown(&self) {
        let handlers = self.0.values().map(|p| p.shutdown());
        join_all(handlers).await;
    }
}
