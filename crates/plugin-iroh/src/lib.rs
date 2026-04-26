use std::{collections::BTreeSet, sync::Arc};

use anyhow::{Context, Result, bail};
use automerge::Automerge;
use bytes::BytesMut;
use dashmap::DashMap;
use futures::{SinkExt as _, Stream};
use iroh::{
    Endpoint, EndpointAddr, EndpointId,
    endpoint::{Connection, RecvStream, SendStream},
    protocol::{AcceptError, ProtocolHandler, Router},
};
use iroh_repo::IrohSamod;
use samod::{
    DocHandle, DocumentId, PeerId,
    storage::{InMemoryStorage, TokioFilesystemStorage},
};
use serde::{Deserialize, Serialize};
use tokio::runtime::Handle;
use tokio_util::codec::{Decoder, Framed, LengthDelimitedCodec};
use tracing::info;
use uuid::Uuid;
use zed::unstable::{
    db::smol::stream::StreamExt,
    gpui::{AppContext, Entity, Global},
    gpui_tokio::Tokio,
    ui::App,
    util::{ResultExt, TryFutureExt},
};

pub mod codec;
pub mod iroh_repo;

// --- Iroh Plugin API

pub fn init(cx: &mut App) {
    let iroh = Iroh::new(cx);
    let iroh_entity = iroh.entity.clone();
    cx.set_global(GlobalIroh(iroh_entity.clone()));

    let tokio_handle = Tokio::handle(cx);
    cx.spawn(async move |cx| {
        let uuid = Uuid::new_v4();
        let base_path = format!("/tmp/iroh-automerge-{uuid}");
        let endpoint = Endpoint::builder().bind().await?;

        let automerge_repo = samod::Repo::build_tokio()
            .with_peer_id(PeerId::from_string(endpoint.id().to_string()))
            .with_storage(TokioFilesystemStorage::new(format!(
                "{}/{}",
                base_path,
                endpoint.id(),
            )))
            // .with_storage(InMemoryStorage::new())
            .load()
            .await;

        let protocol_automerge = IrohSamod::new(endpoint.clone(), automerge_repo);
        let protocol_galvanized =
            GalvanizedProtocol::new(endpoint.clone(), protocol_automerge.clone(), tokio_handle);
        let router = Router::builder(endpoint.clone())
            .accept(IrohSamod::SYNC_ALPN, protocol_automerge.clone())
            .accept(GalvanizedProtocol::ALPN, protocol_galvanized.clone())
            .spawn();

        let state = IrohState {
            endpoint,
            protocol_automerge,
            protocol_galvanized,
            router,
        };

        iroh_entity.update(cx, |entity, _cx| {
            entity.state = Some(state);
        });

        anyhow::Ok(())
    })
    .detach_and_log_err(cx);
}

struct GlobalIroh(Entity<IrohEntity>);
impl Global for GlobalIroh {}
pub trait IrohExt {
    type Context: AppContext;
    fn iroh(&mut self) -> Iroh<'_, Self::Context>;
}
impl<C: AppContext> IrohExt for C {
    type Context = C;
    fn iroh(&mut self) -> Iroh<'_, C> {
        let entity = self.read_global::<GlobalIroh, _>(|it, _cx| it.0.clone());
        Iroh { cx: self, entity }
    }
}

/// Entrypoint to the GPUI-style API for Iroh/p2p operations
pub struct Iroh<'a, C: AppContext> {
    cx: &'a mut C,
    entity: Entity<IrohEntity>,
}

#[derive(Clone)]
pub struct IrohEntity {
    state: Option<IrohState>,
}

impl<'a, C: AppContext> Iroh<'a, C> {
    /// Instantiate a new Iroh instance with an uninitialized endpoint
    fn new(cx: &'a mut C) -> Iroh<'a, C> {
        let inner = IrohEntity { state: None };
        let entity = cx.new(|_| inner);
        Iroh { cx, entity }
    }

    pub fn endpoint_id(&self) -> Result<EndpointId> {
        self.cx.read_entity(&self.entity, |it, _cx| {
            let state = it
                .state
                .as_ref()
                .context("Iroh is not yet initialized")?
                .endpoint
                .id();
            anyhow::Ok(state)
        })
    }

    pub fn galvanized(&self) -> Result<GalvanizedProtocol> {
        self.cx.read_entity(&self.entity, |entity, _cx| {
            let galvanized = entity
                .state
                .as_ref()
                .context("Iroh is not yet initialized")?
                .protocol_galvanized
                .clone();
            anyhow::Ok(galvanized)
        })
    }
}

#[derive(Clone)]
pub struct IrohState {
    endpoint: Endpoint,
    protocol_automerge: IrohSamod,
    protocol_galvanized: GalvanizedProtocol,
    router: Router,
}

// --- Galvanized Protocol

#[derive(Debug, Clone)]
pub struct GalvanizedProtocol {
    endpoint: Endpoint,
    protocol_automerge: IrohSamod,
    // Connection state by remote peer EndpointId
    peer_state: Arc<DashMap<EndpointId, PeerState>>,
    tokio_handle: Handle,
}

impl GalvanizedProtocol {
    const ALPN: &'static [u8] = b"/galvanized/1";

    fn new(endpoint: Endpoint, protocol_automerge: IrohSamod, tokio_handle: Handle) -> Self {
        Self {
            endpoint,
            protocol_automerge,
            peer_state: Arc::new(DashMap::new()),
            tokio_handle,
        }
    }

    pub async fn connect(&self, addr: impl Into<EndpointAddr>) -> Result<()> {
        let addr = addr.into();
        let endpoint_id = addr.id.clone();
        let connection = self.endpoint.connect(addr, Self::ALPN).await?;
        let (mut send_stream, recv_stream) = connection.open_bi().await?;

        {
            let message = Envelope::from(GalvanizedMessage::CreatedStream);
            let message_bytes = serde_json::to_vec(&message).unwrap();
            let mut framed = Framed::new(&mut send_stream, LengthDelimitedCodec::new());
            framed.send(message_bytes.into()).await?;
        }

        let peer_state = PeerState {
            connection,
            send_stream,
            recv_stream: Some(recv_stream),
            doc_handle: None,
        };
        self.peer_state.insert(endpoint_id, peer_state);

        Ok(())
    }

    pub fn remote_peers(&self) -> BTreeSet<EndpointId> {
        self
            //
            .peer_state
            .iter()
            .map(|it| it.key().clone())
            .collect()
    }

    /// Creates a shared document with the given peer, or opens an existing one if it exists.
    pub async fn create_or_open_doc(&self, peer: &EndpointId) -> Result<DocHandle> {
        // Connect task needs continuous polling on a task
        let _join_handle = self.tokio_handle.spawn({
            let addr = EndpointAddr::from(*peer);
            let automerge = self.protocol_automerge.clone();
            async move {
                info!("Starting outbound Automerge sync_with");
                if let Some(finished_reason) = automerge.sync_with(addr).await.log_err() {
                    info!(?finished_reason, "Sync finished");
                }
            }
        });

        self.protocol_automerge
            .repo()
            .when_connected(PeerId::from_string(peer.to_string()))
            .await?;
        info!("Dial: Connected to Automerge peer");

        // Open path: Dashmap entry read lock
        {
            let peer_read = self
                .peer_state
                .get(peer)
                .with_context(|| format!("read: No Peer State for {peer}"))?;

            if let Some(doc_handle) = peer_read.doc_handle.clone() {
                return Ok(doc_handle);
            }
        }

        // Create path: Dashmap entry write lock
        let doc_handle = {
            let doc_handle = self
                .protocol_automerge
                .repo()
                .create(Automerge::new())
                .await
                .context("failed to create doc")?;

            let mut peer_write = self
                .peer_state
                .get_mut(peer)
                .with_context(|| format!("write: No Peer State for {peer}"))?;
            peer_write.doc_handle = Some(doc_handle.clone());

            let message = Envelope::from(GalvanizedMessage::CreatedDoc(
                doc_handle.document_id().clone(),
            ));
            let message_bytes = serde_json::to_vec(&message).unwrap();
            let mut framed = Framed::new(&mut peer_write.send_stream, LengthDelimitedCodec::new());
            // WARN: Holding peer_state write lock during sending
            framed.send(message_bytes.into()).await?;

            doc_handle
        };

        Ok(doc_handle)
    }
}

impl ProtocolHandler for GalvanizedProtocol {
    fn accept(
        &self,
        connection: Connection,
    ) -> impl Future<Output = Result<(), AcceptError>> + Send {
        async move {
            let endpoint_id = connection.remote_id();
            // NOTE: Requires dialer to send a message on open_bi stream to pass this await
            let (send_stream, recv_stream) = connection.accept_bi().await?;

            let peer_state = PeerState {
                connection,
                send_stream,
                recv_stream: None,
                doc_handle: None,
            };
            self.peer_state.entry(endpoint_id).or_insert(peer_state);

            self.run_loop(endpoint_id, recv_stream).await;
            Ok(())
        }
    }
}

type FrameItem = Result<BytesMut, <LengthDelimitedCodec as Decoder>::Error>;

// Receiving-end handlers
impl GalvanizedProtocol {
    //
    async fn run_loop(&self, peer: EndpointId, mut recv_stream: RecvStream) {
        let mut stream = Framed::new(&mut recv_stream, LengthDelimitedCodec::new());
        loop {
            self.try_run_loop(&peer, &mut stream).await.log_err();
        }
    }

    async fn try_run_loop(
        &self,
        peer: &EndpointId,
        stream: &mut (impl Unpin + Stream<Item = FrameItem>),
    ) -> anyhow::Result<()> {
        while let Some(frame) = stream.try_next().await? {
            self.try_handle_frame(peer, frame).await?;
        }

        Ok(())
    }

    async fn try_handle_frame(&self, peer: &EndpointId, frame: BytesMut) -> anyhow::Result<()> {
        let envelope = serde_json::from_slice::<Envelope>(&frame)?;
        match &envelope.message {
            GalvanizedMessage::CreatedStream => {
                // Only used to satisfy `.accept_bi`
                info!(?peer, "Received CreatedStream event");
            }
            GalvanizedMessage::CreatedDoc(doc_id) => {
                self.try_handle_created_doc(*peer, doc_id.clone()).await?;
            }
        }

        Ok(())
    }

    async fn try_handle_created_doc(
        &self,
        peer: EndpointId,
        doc_id: DocumentId,
    ) -> anyhow::Result<()> {
        // Handler implementation on task to avoid blocking handling other incoming connections
        let _join_handle = self.tokio_handle.spawn({
            let automerge = self.protocol_automerge.clone();
            let doc_id = doc_id.clone();
            let peer = peer.clone();
            let peer_state = self.peer_state.clone();
            async move {
                info!("Accept: CreateDoc started");

                automerge
                    .repo()
                    .when_connected(PeerId::from_string(peer.to_string()))
                    .await?;
                info!("Accept: Connected to Automerge peer");

                let maybe_doc = automerge
                    .repo()
                    .find(doc_id.clone())
                    .await
                    .with_context(|| {
                        format!("unexpected error while trying to find Automerge document peer={peer} doc_id={doc_id:?}")
                    })?;

                let Some(doc_handle) = maybe_doc else {
                    bail!("failed to find Automerge document peer={peer} doc_id={doc_id:?}");
                };
                info!("Accept: Found Automerge document!");

                let Some(mut peer_state) = peer_state.get_mut(&peer) else {
                    bail!("Missing PeerState for peer {peer}");
                };

                peer_state.doc_handle = Some(doc_handle);
                anyhow::Ok(())
            }.log_err()
        });

        anyhow::Ok(())
    }
}

#[derive(derive_more::Debug)]
pub struct PeerState {
    connection: Connection,
    send_stream: SendStream,
    recv_stream: Option<RecvStream>,
    #[debug("DocHandle")]
    doc_handle: Option<DocHandle>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Envelope {
    //
    message: GalvanizedMessage,
}

impl From<GalvanizedMessage> for Envelope {
    fn from(message: GalvanizedMessage) -> Self {
        Self { message }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum GalvanizedMessage {
    CreatedStream,
    CreatedDoc(DocumentId),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_message() {
        let message = GalvanizedMessage::CreatedStream;
        let envelope = Envelope { message };
        let serialized = serde_json::to_string(&envelope).unwrap();
        println!("Serialized: {}", serialized);
    }
}

// ---

// impl Iroh {
//     // /// Instantiate a new Iroh instance with an uninitialized endpoint
//     // fn new() -> Self {
//     //     Self {
//     //         //
//     //         state: None,
//     //     }
//     // }

//     pub fn endpoint_id(&self) -> Option<EndpointId> {
//         self.state.as_ref().map(|it| it.endpoint.id())
//     }

//     /// Returns a list of Endpoint IDs of the remote peers connected via our Protocol
//     pub fn remote_peers(&self) -> Option<Vec<EndpointId>> {
//         let state = self.state.as_ref()?;

//         let remote_peers = state
//             .protocol_galvanized
//             .peer_state
//             .iter()
//             .map(|it| it.key().clone())
//             .collect::<Vec<_>>();

//         Some(remote_peers)
//     }

//     pub fn sync(&self, cx: &mut App, addr: impl Into<EndpointAddr>) {
//         let Some(state) = &self.state else {
//             return;
//         };

//         // Outbound: Automerge sync (poll task)
//         let addr = addr.into();
//         Tokio::spawn(cx, {
//             let addr = addr.clone();
//             let state = state.clone();
//             async move {
//                 let peer_id = addr.id.clone();
//                 let conn_finished_reason = state.protocol_automerge.sync_with(addr).await?;
//                 info!(?peer_id, ?conn_finished_reason, "Connection finished");
//                 anyhow::Ok(())
//             }
//         })
//         .detach_and_log_err(cx);

//         // Outbound: GalvanizedProtocol connection
//         Tokio::spawn(cx, {
//             let addr = addr.clone();
//             let state = state.clone();
//             async move {
//                 let peer_id = addr.id.clone();
//                 // let conn_finished_reason = state.protocol_automerge.sync_with(addr).await?;
//                 let connection = state
//                     .endpoint
//                     .connect(addr.clone(), GalvanizedProtocol::ALPN)
//                     .await?;
//                 state
//                     .protocol_galvanized
//                     .peer_state
//                     .entry(addr.id)
//                     .or_insert(Arc::new(RwLock::new(PeerState::new(connection))));
//                 info!(?peer_id, "Connected TaggedProtocol");
//                 anyhow::Ok(())
//             }
//         })
//         .detach_and_log_err(cx);

//         // Inbound: Automerge sync (wait for connection)
//         cx.spawn({
//             let state = state.clone();
//             async move |_cx| {
//                 state
//                     .protocol_automerge
//                     .repo()
//                     .when_connected(PeerId::from_string(addr.id.to_string()))
//                     .await?;

//                 // state
//                 //     .protocol_tagged
//                 //     .peer_state
//                 //     .entry(addr.id)
//                 //     .or_insert(None);

//                 info!(peer_id = ?addr.id, "Connected to automerge-repo peer");
//                 anyhow::Ok(())
//             }
//         })
//         .detach_and_log_err(cx);
//     }

//     pub fn galvanized(&self) -> GalvanizedProtocol {
//         self.state.as_ref().unwrap().protocol_galvanized.clone()
//     }

//     pub fn create_doc<C: AppContext>(&self, cx: &mut C) -> Task<anyhow::Result<DocHandle>> {
//         let Some(state) = self.state.clone() else {
//             return Task::ready(Err(anyhow!("Missing IrohState")));
//         };

//         let task = cx.background_spawn(async move {
//             // TODO embed initial doc for global document
//             let initial_content = Automerge::new();
//             let handle = state
//                 .protocol_automerge
//                 .repo()
//                 .create(initial_content.clone())
//                 .await?;

//             anyhow::Ok(handle)
//         });

//         task
//     }

//     pub fn find_doc<C: AppContext>(
//         &self,
//         doc_id: DocumentId,
//         cx: &mut C,
//     ) -> Task<anyhow::Result<Option<DocHandle>>> {
//         let Some(state) = self.state.clone() else {
//             return Task::ready(Err(anyhow!("Missing IrohState")));
//             // return;
//         };

//         cx.background_spawn(async move {
//             let doc_handle = state.protocol_automerge.repo().find(doc_id).await?;
//             anyhow::Ok(doc_handle)
//         })
//     }
// }
