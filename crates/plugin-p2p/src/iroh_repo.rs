//! From https://github.com/n0-computer/iroh-examples/blob/6af4d24151b53b93e1d97061c792f77b33917ec2/iroh-automerge-repo/src/lib.rs
//!
//! Combines [`iroh`] with automerge's [`samod`] library, a library to create "automerge repositories"
//! in rust that speak the automerge repo protocol.
use std::{
    collections::BTreeSet,
    error::Error,
    sync::{Arc, Mutex},
};

use crate::codec::Codec;
use anyhow::{Result, anyhow};
use futures::{Stream, StreamExt as _, future::BoxFuture, stream::BoxStream};
use iroh::{
    Endpoint, EndpointAddr, EndpointId,
    endpoint::Connection,
    protocol::{AcceptError, ProtocolHandler},
};
use samod::{
    AcceptorEvent, AcceptorHandle, BackoffConfig, ConnDirection, ConnFinishedReason,
    ConnectionHandle, Dialer, DialerHandle, PeerId, Repo, Transport, Url,
    storage::TokioFilesystemStorage, transport::BoxSink,
};
use tokio_util::codec::{Framed, FramedRead, FramedWrite, LengthDelimitedCodec};
use tracing::{debug, field::debug, info};
use zed::unstable::gpui::{App, AsyncApp, Global};

// pub fn init(cx: &mut App) {
//     cx.set_global(GlobalIrohRepo(None));
//     cx.spawn(async move |cx: &mut AsyncApp| {
//         let secret_key = iroh::SecretKey::generate(&mut rand::rng());
//         let endpoint = iroh::Endpoint::builder()
//             .secret_key(secret_key)
//             .bind()
//             .await?;
//         let base_path = "/tmp/iroh-automerge";
//         let repo = samod::Repo::build_tokio()
//             .with_peer_id(PeerId::from_string(endpoint.id().to_string()))
//             .with_storage(TokioFilesystemStorage::new(format!(
//                 "{}/{}",
//                 base_path,
//                 endpoint.id(),
//             )))
//             .load()
//             .await;
//         let proto = IrohSamod::new(endpoint.clone(), repo);
//         let _router = iroh::protocol::Router::builder(endpoint)
//             .accept(IrohSamod::SYNC_ALPN, proto.clone())
//             .spawn();
//         let iroh_repository = IrohRepository { proto };

//         cx.update_global(|&mut GlobalIrohRepo(ref mut repo), _| {
//             *repo = Some(Arc::new(iroh_repository))
//         });
//         anyhow::Ok(())
//     })
//     .detach();
// }

pub struct GlobalIrohRepo(pub Option<Arc<IrohRepository>>);
impl Global for GlobalIrohRepo {}

pub struct IrohRepository {
    pub proto: IrohSamod,
    // pub router: iroh::protocol::Router,
}

/// Combines an [`iroh::Endpoint`] with a [`Repo`] (automerge repository) and
/// implements [`iroh::protocol::ProtocolHandler`] to accept incoming connections
/// in an [`iroh::protocol::Router`].
#[derive(derive_more::Debug, Clone)]
pub struct IrohSamod {
    endpoint: iroh::Endpoint,
    #[debug(skip)]
    repo: Repo,
    peers: Arc<Mutex<BTreeSet<iroh::EndpointId>>>,
}

impl IrohSamod {
    pub const SYNC_ALPN: &[u8] = b"iroh/automerge-repo/1";

    /// Constructs a new [`IrohRepo`].
    pub fn new(endpoint: iroh::Endpoint, repo: Repo) -> Self {
        IrohSamod {
            endpoint,
            repo,
            peers: Default::default(),
        }
    }

    /// Attempts to continuously sync with a peer at given address.
    ///
    /// To wait for the connection to be established use [`Repo::when_connected`]
    /// (accessible via [`Self::repo`]: `iroh_repo.repo().when_connected(..)`).
    /// with the other endpoint's string-encoded [`EndpointId`] as the [`PeerId`].
    ///
    /// [`EndpointId`]: iroh::EndpointId
    /// [`PeerId`]: samod::PeerId
    pub fn dial_peer(&self, addr: impl Into<iroh::EndpointAddr>) -> Result<DialerHandle> {
        let dialer = Arc::new(IrohDialer::new(self.endpoint.clone(), addr.into()));
        debug!("IrohSamod dial_peer starting");
        let dialer_handle = self.repo().dial(BackoffConfig::default(), dialer)?;
        info!("IrohSamod finished dialing");
        Ok(dialer_handle)
    }

    // pub async fn dial_peer(&self, addr: impl Into<iroh::EndpointAddr>) -> Result<ConnectionHandle> {
    //     let connection = self.endpoint.connect(addr, Self::SYNC_ALPN).await?;
    //     let url = format!("iroh://{}", connection.remote_id())
    //         .parse::<Url>()
    //         .expect("valid URL");
    //     // let acceptor = AcceptorHandle::accept_tokio_io(&self, io);
    //     let (send, recv) = connection.open_bi().await?;
    //     let io = tokio::io::join(recv, send);
    //     let acceptor = self
    //         .repo()
    //         .make_acceptor(url)
    //         .map_err(|error| AcceptError::from_err(error))?;
    //     let connection_handle = acceptor
    //         .accept_tokio_io(io)
    //         .map_err(|error| AcceptError::from_err(error))?;

    //     Ok(connection_handle)
    // }

    /// Returns a reference to the stored [`Repo`] instance inside.
    pub fn repo(&self) -> &Repo {
        &self.repo
    }

    pub fn endpoint(&self) -> &iroh::Endpoint {
        &self.endpoint
    }
}

impl ProtocolHandler for IrohSamod {
    async fn accept(&self, connection: Connection) -> Result<(), AcceptError> {
        let url = format!("iroh://{}", connection.remote_id())
            .parse::<Url>()
            .expect("valid URL");
        // let acceptor = AcceptorHandle::accept_tokio_io(&self, io);
        let (send, recv) = connection.accept_bi().await?;
        let io = tokio::io::join(recv, send);
        let acceptor = self
            .repo()
            .make_acceptor(url)
            .map_err(|error| AcceptError::from_err(error))?;
        let connection_handle = acceptor
            .accept_tokio_io(io)
            .map_err(|error| AcceptError::from_err(error))?;
        info!("Accepted: got connection handle");

        // let endpoint_id = connection.remote_id();
        // let (send, recv) = connection.accept_bi().await?;
        // {
        //     // Connection established, update peers list
        //     let mut lock = self.peers.lock().unwrap();
        //     lock.insert(endpoint_id.clone());
        // }

        // let dialer = Arc::new(IrohDialer::new(, remote))
        // self.repo().dial(BackoffConfig::default(), dialer)

        // self.repo.make_acceptor(url)

        // let dialer = TransportDialer;
        // let dialer_handle = self
        //     //
        //     .repo
        //     .dial(BackoffConfig::default(), Arc::new(dialer))
        //     .map_err(AcceptError::from_err)?;

        // tracing::info!("Samod starting inbound sync");
        // let conn_finished = self
        //     .repo
        //     .connect(
        //         FramedRead::new(recv, Codec::new(endpoint_id)),
        //         FramedWrite::new(send, Codec::new(endpoint_id)),
        //         ConnDirection::Incoming,
        //     )
        //     .await;

        // {
        //     // Connection closed, remove from peers list
        //     let mut lock = self.peers.lock().unwrap();
        //     lock.remove(&endpoint_id);
        // }

        // tracing::debug!(%endpoint_id, ?conn_finished, "Connection we accepted shut down");
        Ok(())
    }

    async fn shutdown(&self) {
        self.repo.stop().await
    }
}

pub struct IrohDialer {
    endpoint: Endpoint,
    remote: EndpointAddr,
}
impl IrohDialer {
    pub fn new(endpoint: Endpoint, remote: EndpointAddr) -> Self {
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
            let alpn = IrohSamod::SYNC_ALPN;
            let connection = endpoint.connect(remote_addr, alpn).await?;
            let (sender, receiver) = connection.open_bi().await?;
            let read_write = tokio::io::join(receiver, sender);
            let transport = Transport::from_tokio_io(read_write);
            Ok(transport)
        })
    }
}
