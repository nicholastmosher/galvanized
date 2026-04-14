use std::sync::Arc;

use anyhow::anyhow;
use automerge::Automerge;
use dashmap::DashMap;
use iroh::{
    Endpoint, EndpointAddr, EndpointId,
    endpoint::Connection,
    protocol::{ProtocolHandler, Router},
};
use iroh_repo::IrohSamod;
use samod::{DocHandle, DocumentId, PeerId, storage::TokioFilesystemStorage};
use tracing::info;
use zed::unstable::{
    gpui::{AppContext, Global, Task},
    gpui_tokio::Tokio,
    ui::App,
};

pub mod codec;
pub mod iroh_repo;

pub fn init(cx: &mut App) {
    // Global Iroh state includes Endpoint, but it's not available synchronously at startup
    // So we init it with endpoint: None, and kick off async init to install later
    let iroh = Iroh::new();
    cx.set_global(GlobalIroh(iroh));
    cx.spawn(async move |cx| {
        let base_path = "/tmp/iroh-automerge";
        let endpoint = Endpoint::builder().bind().await?;
        let repo = samod::Repo::build_tokio()
            .with_peer_id(PeerId::from_string(endpoint.id().to_string()))
            .with_storage(TokioFilesystemStorage::new(format!(
                "{}/{}",
                base_path,
                endpoint.id(),
            )))
            .load()
            .await;
        let protocol_automerge = IrohSamod::new(endpoint.clone(), repo);
        let protocol_tagged = TaggedProtocol::new();
        let router = Router::builder(endpoint.clone())
            .accept(IrohSamod::SYNC_ALPN, protocol_automerge.clone())
            .accept(TaggedProtocol::ALPN, protocol_tagged.clone())
            // .accept(IrohSamod::SYNC_ALPN, protocol_tagged.clone())
            .spawn();

        let state = IrohState {
            endpoint,
            protocol_automerge,
            protocol_tagged,
            router,
        };

        cx.update_global::<GlobalIroh, _>(|it, _cx| {
            it.0.state = Some(state);
        });
        anyhow::Ok(())
    })
    .detach_and_log_err(cx);
}

struct GlobalIroh(Iroh);
impl Global for GlobalIroh {}
pub trait IrohExt {
    fn iroh(&mut self) -> Iroh;
}
impl<C: AppContext> IrohExt for C {
    fn iroh(&mut self) -> Iroh {
        self.read_global::<GlobalIroh, _>(|it, _cx| it.0.clone())
    }
}

/// Entrypoint to the GPUI-style API for Iroh/p2p operations
#[derive(Clone)]
pub struct Iroh {
    state: Option<IrohState>,
}

#[derive(Clone)]
pub struct IrohState {
    endpoint: Endpoint,
    protocol_automerge: IrohSamod,
    protocol_tagged: TaggedProtocol,
    router: Router,
}

impl Iroh {
    /// Instantiate a new Iroh instance with an uninitialized endpoint
    fn new() -> Self {
        Self {
            //
            state: None,
        }
    }

    pub fn endpoint_id(&self) -> Option<EndpointId> {
        self.state.as_ref().map(|it| it.endpoint.id())
    }

    /// Returns a list of Endpoint IDs of the remote peers connected via our Protocol
    pub fn remote_peers(&self) -> Option<Vec<EndpointId>> {
        let state = self.state.as_ref()?;

        let remote_peers = state
            .protocol_tagged
            .peer_state
            .iter()
            .map(|it| it.key().clone())
            .collect::<Vec<_>>();

        Some(remote_peers)
    }

    pub fn sync(&self, cx: &mut App, addr: impl Into<EndpointAddr>) {
        let Some(state) = &self.state else {
            return;
        };

        let addr = addr.into();
        Tokio::spawn(cx, {
            let addr = addr.clone();
            let state = state.clone();
            async move {
                let peer_id = addr.id.clone();
                let conn_finished_reason = state.protocol_automerge.sync_with(addr).await?;
                info!(?peer_id, ?conn_finished_reason, "Connection finished");
                anyhow::Ok(())
            }
        })
        .detach_and_log_err(cx);

        Tokio::spawn(cx, {
            let addr = addr.clone();
            let state = state.clone();
            async move {
                let peer_id = addr.id.clone();
                // let conn_finished_reason = state.protocol_automerge.sync_with(addr).await?;
                let connection = state
                    .endpoint
                    .connect(addr.clone(), TaggedProtocol::ALPN)
                    .await?;
                state
                    .protocol_tagged
                    .peer_state
                    .entry(addr.id)
                    .or_insert(Some(connection));
                info!(?peer_id, "Connected TaggedProtocol");
                anyhow::Ok(())
            }
        })
        .detach_and_log_err(cx);

        cx.spawn({
            let state = state.clone();
            async move |_cx| {
                state
                    .protocol_automerge
                    .repo()
                    .when_connected(PeerId::from_string(addr.id.to_string()))
                    .await?;

                state
                    .protocol_tagged
                    .peer_state
                    .entry(addr.id)
                    .or_insert(None);

                info!(peer_id = ?addr.id, "Connected to automerge-repo peer");
                anyhow::Ok(())
            }
        })
        .detach_and_log_err(cx);
    }

    pub fn create_doc<C: AppContext>(&self, cx: &mut C) -> Task<anyhow::Result<DocHandle>> {
        let Some(state) = self.state.clone() else {
            return Task::ready(Err(anyhow!("Missing IrohState")));
        };

        let task = cx.background_spawn(async move {
            // TODO embed initial doc for global document
            let initial_content = Automerge::new();
            let handle = state
                .protocol_automerge
                .repo()
                .create(initial_content.clone())
                .await?;

            anyhow::Ok(handle)
        });

        task
    }

    pub fn find_doc<C: AppContext>(
        &self,
        doc_id: DocumentId,
        cx: &mut C,
    ) -> Task<anyhow::Result<Option<DocHandle>>> {
        let Some(state) = self.state.clone() else {
            return Task::ready(Err(anyhow!("Missing IrohState")));
            // return;
        };

        cx.background_spawn(async move {
            let doc_handle = state.protocol_automerge.repo().find(doc_id).await?;
            anyhow::Ok(doc_handle)
        })
    }
}

#[derive(Debug, Clone)]
struct TaggedProtocol {
    // Connection state by remote peer EndpointId
    peer_state: Arc<DashMap<EndpointId, Option<Connection>>>,
}

impl TaggedProtocol {
    const ALPN: &'static [u8] = b"/tagged/1";

    fn new() -> Self {
        Self {
            peer_state: Arc::new(DashMap::new()),
        }
    }
}

impl ProtocolHandler for TaggedProtocol {
    fn accept(
        &self,
        connection: iroh::endpoint::Connection,
    ) -> impl Future<Output = Result<(), iroh::protocol::AcceptError>> + Send {
        async move {
            let remote_id = connection.remote_id();
            self.peer_state.entry(remote_id).or_insert(Some(connection));
            //
            Ok(())
        }
    }
}
