use std::sync::Arc;

use dashmap::DashMap;
use iroh::{
    Endpoint, EndpointAddr, EndpointId,
    endpoint::Connection,
    protocol::{ProtocolHandler, Router},
};
use tracing::info;
use zed::unstable::{
    gpui::{AppContext, Global},
    ui::App,
};

pub fn init(cx: &mut App) {
    // Global Iroh state includes Endpoint, but it's not available synchronously at startup
    // So we init it with endpoint: None, and kick off async init to install later
    let iroh = Iroh::new();
    cx.set_global(GlobalIroh(iroh));
    cx.spawn(async move |cx| {
        let endpoint = Endpoint::builder().bind().await?;
        cx.update_global::<GlobalIroh, _>(|it, _cx| {
            it.0.init(endpoint);
        });
        anyhow::Ok(())
    })
    .detach_and_log_err(cx);

    //
    let iroh = cx.iroh();
}

struct GlobalIroh(Iroh);
impl Global for GlobalIroh {}
pub trait IrohExt {
    fn iroh(&self) -> Iroh;
}
impl<'a, C: AppContext> IrohExt for &'a mut C {
    fn iroh(&self) -> Iroh {
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
    protocol: Protocol,
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

    /// Install the Endpoint after its async initialization completes
    fn init(&mut self, endpoint: Endpoint) {
        let protocol = Protocol::new();
        let router = Router::builder(endpoint.clone())
            .accept(Protocol::ALPN, protocol.clone())
            .spawn();
        let state = IrohState {
            endpoint,
            protocol,
            router,
        };
        self.state = Some(state);
    }

    pub fn endpoint_id(&self) -> Option<EndpointId> {
        self.state.as_ref().map(|it| it.endpoint.id())
    }

    /// Returns a list of Endpoint IDs of the remote peers connected via our Protocol
    pub fn remote_peers(&self) -> Option<Vec<EndpointId>> {
        let state = self.state.as_ref()?;

        let remote_peers = state
            .protocol
            .peer_state
            .iter()
            .map(|it| it.key().clone())
            .collect::<Vec<_>>();

        Some(remote_peers)
    }

    pub fn connect(&self, cx: &mut App, addr: impl Into<EndpointAddr>) {
        let Some(state) = &self.state else {
            return;
        };

        let addr = addr.into();
        cx.spawn({
            let state = state.clone();
            async move |_cx| {
                let id = addr.id.clone();
                let connection = state.endpoint.connect(addr, Protocol::ALPN).await?;

                state.protocol.peer_state.insert(id, connection);

                // TODO handle outbound connection
                info!("Connection established! TODO");

                anyhow::Ok(())
            }
        })
        .detach_and_log_err(cx);
    }
}

#[derive(Debug, Clone)]
struct Protocol {
    // Connection state by remote peer EndpointId
    peer_state: Arc<DashMap<EndpointId, Connection>>,
}

impl Protocol {
    const ALPN: &'static [u8] = b"/tagged/1";

    fn new() -> Self {
        Self {
            peer_state: Arc::new(DashMap::new()),
        }
    }
}

impl ProtocolHandler for Protocol {
    fn accept(
        &self,
        connection: iroh::endpoint::Connection,
    ) -> impl Future<Output = Result<(), iroh::protocol::AcceptError>> + Send {
        async move {
            let remote_id = connection.remote_id();
            self.peer_state.entry(remote_id).or_insert(connection);
            //
            Ok(())
        }
    }
}
