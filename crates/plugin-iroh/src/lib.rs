use std::sync::{Arc, RwLock};

use anyhow::{Result, anyhow};
use automerge::Automerge;
use dashmap::DashMap;
use iroh::{
    Endpoint, EndpointAddr, EndpointId,
    endpoint::{Connection, RecvStream, SendStream},
    protocol::{AcceptError, ProtocolHandler, Router},
};
use iroh_repo::IrohSamod;
use samod::{DocHandle, DocumentId, PeerId, storage::TokioFilesystemStorage};
use tracing::info;
use zed::unstable::{
    gpui::{AppContext, Entity, Global, Task},
    gpui_tokio::Tokio,
    ui::App,
};

pub mod codec;
pub mod iroh_repo;

// --- Iroh Plugin API

pub fn init(cx: &mut App) {
    let iroh = Iroh::new(cx);
    let iroh_entity = iroh.entity.clone();
    cx.set_global(GlobalIroh(iroh_entity.clone()));

    cx.spawn(async move |cx| {
        let base_path = "/tmp/iroh-automerge";
        let endpoint = Endpoint::builder().bind().await?;

        let automerge_repo = samod::Repo::build_tokio()
            .with_peer_id(PeerId::from_string(endpoint.id().to_string()))
            .with_storage(TokioFilesystemStorage::new(format!(
                "{}/{}",
                base_path,
                endpoint.id(),
            )))
            .load()
            .await;

        let protocol_automerge = IrohSamod::new(endpoint.clone(), automerge_repo);
        let protocol_galvanized =
            GalvanizedProtocol::new(endpoint.clone(), protocol_automerge.clone());
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

        iroh_entity.update(cx, |it, _cx| {
            it.state = Some(state);
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
            it.state
                .as_ref()
                .map(|it| it.endpoint.id())
                .ok_or_else(|| anyhow!("Iroh should be initialized"))
        })
    }

    pub fn galvanized(&self) -> Result<GalvanizedProtocol> {
        self.cx.read_entity(&self.entity, |it, _cx| {
            it.state
                .as_ref()
                .map(|it| it.protocol_galvanized.clone())
                .ok_or_else(|| anyhow!("Iroh should be initialized"))
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
    peer_state: Arc<DashMap<EndpointId, Arc<RwLock<PeerState>>>>,
}

impl GalvanizedProtocol {
    const ALPN: &'static [u8] = b"/galvanized/1";

    fn new(endpoint: Endpoint, protocol_automerge: IrohSamod) -> Self {
        Self {
            endpoint,
            protocol_automerge,
            peer_state: Arc::new(DashMap::new()),
        }
    }

    pub async fn connect(&self, addr: impl Into<EndpointAddr>) -> Result<()> {
        let addr = addr.into();
        let endpoint_id = addr.id.clone();
        let connection = self.endpoint.connect(addr, Self::ALPN).await?;
        let (send_stream, recv_stream) = connection.open_bi().await?;

        Ok(())
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
                recv_stream: Some(recv_stream),
                doc_handle: None,
            };

            self.peer_state
                .entry(endpoint_id)
                .or_insert(Arc::new(RwLock::new(peer_state)));
            Ok(())
        }
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
