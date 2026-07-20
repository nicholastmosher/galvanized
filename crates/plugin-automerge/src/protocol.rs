//! From https://github.com/n0-computer/iroh-examples/blob/6af4d24151b53b93e1d97061c792f77b33917ec2/iroh-automerge-repo/src/lib.rs
//!
//! Combines [`iroh`] with automerge's [`samod`] library, a library to create "automerge repositories"
//! in rust that speak the automerge repo protocol.
use std::{
    collections::BTreeSet,
    error::Error,
    io::ErrorKind,
    sync::{Arc, Mutex, RwLock},
};

use crate::codec::Codec;
use anyhow::{Result, bail};
use futures::future::BoxFuture;
use iroh::{
    Endpoint, EndpointAddr,
    endpoint::Connection,
    protocol::{AcceptError, ProtocolHandler},
};
use plugin_p2p::KeyedProtocolHandler;
use samod::{BackoffConfig, Dialer, DialerHandle, Repo, Transport, Url};
use tokio_util::codec::{FramedRead, FramedWrite};
use tracing::{debug, info};

/// Combines an [`iroh::Endpoint`] with a [`Repo`] (automerge repository) and
/// implements [`iroh::protocol::ProtocolHandler`] to accept incoming connections
/// in an [`iroh::protocol::Router`].
#[derive(derive_more::Debug, Clone)]
pub struct AutomergeProtocol {
    endpoint: Endpoint,
    #[debug(skip)]
    repo: Arc<RwLock<Option<Repo>>>,
    _peers: Arc<Mutex<BTreeSet<iroh::EndpointId>>>,
}

impl KeyedProtocolHandler for AutomergeProtocol {
    const ALPN: &[u8] = b"iroh/automerge-repo/1";
}

impl AutomergeProtocol {
    /// Constructs a new [`AutomergeProtocol`].
    pub fn new(endpoint: Endpoint, repo_builder: BoxFuture<'static, Repo>) -> Self {
        let protocol = AutomergeProtocol {
            endpoint,
            repo: Default::default(),
            _peers: Default::default(),
        };
        let protocol_clone = protocol.clone();
        tokio::task::spawn(async move {
            let repo = repo_builder.await;
            let mut repo_lock = protocol_clone.repo.write().unwrap();
            *repo_lock = Some(repo);
        });
        protocol
    }

    /// Attempts to continuously sync with a peer at given address.
    ///
    /// To wait for the connection to be established use [`Repo::when_connected`]
    /// (accessible via [`Self::repo`]: `iroh_repo.repo().when_connected(..)`).
    /// with the other endpoint's string-encoded [`EndpointId`] as the [`PeerId`].
    ///
    /// [`EndpointId`]: iroh::EndpointId
    /// [`PeerId`]: samod::PeerId
    pub async fn dial_peer(&self, addr: impl Into<iroh::EndpointAddr>) -> Result<DialerHandle> {
        let Some(repo) = self.repo() else {
            bail!("Automerge repo not initialized");
        };
        let dialer = Arc::new(IrohDialer::new(self.endpoint.clone(), addr.into()));
        debug!("IrohSamod dial_peer starting");
        let dialer_handle = repo.dial(BackoffConfig::default(), dialer)?;
        let _peer_info = dialer_handle.established().await?;
        info!("IrohSamod finished dialing");
        Ok(dialer_handle)
    }

    /// Returns a reference to the stored [`Repo`] instance inside.
    pub fn repo(&self) -> Option<Repo> {
        self.repo.read().unwrap().clone()
    }

    /// Returns a reference to the Iroh [`Endpoint`] instance inside.
    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }
}

impl ProtocolHandler for AutomergeProtocol {
    async fn accept(&self, connection: Connection) -> Result<(), AcceptError> {
        let Some(repo) = self.repo() else {
            return Err(AcceptError::from_err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Automerge repo not initialized",
            )));
        };

        let url = format!("iroh://{}", connection.remote_id())
            .parse::<Url>()
            .expect("valid URL");

        let (send, recv) = connection.accept_bi().await?;
        let acceptor = repo
            .make_acceptor(url)
            .map_err(|error| AcceptError::from_err(error))?;

        let sink = FramedWrite::new(send, Codec::new(connection.remote_id()));
        let stream = FramedRead::new(recv, Codec::new(connection.remote_id()));
        let transport = Transport::new(stream, sink);
        let connection_handle = acceptor.accept(transport).map_err(AcceptError::from_err)?;
        info!("Accepted: got connection handle");

        let _peer_info = connection_handle
            .handshake_complete()
            .await
            .map_err(|_error| {
                AcceptError::from_err(std::io::Error::new(
                    ErrorKind::NotConnected,
                    "failed to accept connection",
                ))
            })?;
        info!("Acceptor handshake complete");

        Ok(())
    }

    async fn shutdown(&self) {
        if let Some(repo) = self.repo() {
            repo.stop().await
        }
    }
}

/// Object that can dial an Iroh endpoint and open a Transport to it
struct IrohDialer {
    endpoint: Endpoint,
    remote: EndpointAddr,
}
impl IrohDialer {
    fn new(endpoint: Endpoint, remote: EndpointAddr) -> Self {
        Self { endpoint, remote }
    }
}

impl Dialer for IrohDialer {
    fn url(&self) -> Url {
        format!("iroh://{}", self.remote.id)
            .parse::<Url>()
            .expect("valid URL")
    }

    fn connect(
        &self,
    ) -> BoxFuture<'static, Result<Transport, Box<dyn Error + Send + Sync + 'static>>> {
        let endpoint = self.endpoint.clone();
        let remote_addr = self.remote.clone();
        Box::pin(async move {
            let alpn = AutomergeProtocol::ALPN;
            let connection = endpoint.connect(remote_addr, alpn).await?;
            let (send, recv) = connection.open_bi().await?;

            let sink = FramedWrite::new(send, Codec::new(connection.remote_id()));
            let stream = FramedRead::new(recv, Codec::new(connection.remote_id()));
            let transport = Transport::new(stream, sink);
            Ok(transport)
        })
    }
}
