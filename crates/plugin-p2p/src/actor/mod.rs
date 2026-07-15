//! Protocol handling actor that runs on a background executor.
//!
//! This actor maintains the state of the protocol and handles incoming events.

use std::{
    any::{Any, TypeId},
    ops::ControlFlow,
    sync::{Arc, RwLock},
};

use anyhow::{Context, Error, Result};
use async_shutdown::ShutdownManager;
use culpa::throws;
use futures::{FutureExt, Stream, StreamExt as _};
use futures_concurrency::stream::Merge;
use iroh::{
    Endpoint, EndpointId,
    endpoint::{Incoming, presets::N0},
    protocol::{DynProtocolHandler, ProtocolHandler},
};
use tokio::{
    runtime::Handle,
    sync::oneshot,
    task::{JoinError, JoinSet},
};
use tracing::{Instrument as _, error, field::Empty, info_span, trace, warn};
use zed::unstable::{gpui_tokio::Tokio, ui::App, util::ResultExt};

use protocol::ProtocolMap;

pub mod protocol;

type Shutdown<T = ()> = ShutdownManager<T>;
pub trait AnyDynProtocolHandler: Any + DynProtocolHandler {}
impl<P: Any + DynProtocolHandler> AnyDynProtocolHandler for P {}

/// External handle interface to the actor.
#[derive(derive_more::Debug, Clone)]
pub struct Actor {
    tx: flume::Sender<ActorInput>,
}

impl Actor {
    /// Spawn a new Actor in the background, this becomes the handle API.
    pub fn spawn(shutdown: Shutdown, cx: &mut App) -> Self {
        let tokio_handle = Tokio::handle(cx);
        let (tx, rx) = flume::bounded(10);
        let future = {
            let tokio_handle = tokio_handle.clone();
            async move {
                let engine = ActorEngine::new(tokio_handle, shutdown, rx).await?;
                engine.run().await;
                anyhow::Ok(())
            }
        };
        tokio_handle.spawn(future);

        Self {
            //
            tx,
        }
    }

    pub async fn register_protocol<P: ProtocolHandler>(
        &self,
        alpn: Vec<u8>,
        handler: P,
    ) -> Result<()> {
        let handler = Arc::new(handler);
        self.tx
            .send_async(ActorInput::RegisterProtocol { alpn, handler })
            .await?;
        Ok(())
    }

    pub async fn unregister_protocol(&self, alpn: Vec<u8>) -> Result<()> {
        self.tx
            .send_async(ActorInput::UnregisterProtocol { alpn })
            .await?;
        Ok(())
    }

    pub async fn protocol<P: Any + ProtocolHandler>(&self, alpn: Vec<u8>) -> Option<Arc<P>> {
        let (client_tx, rx) = oneshot::channel();
        self.tx
            .send_async(ActorInput::GetProtocol { alpn, client_tx })
            .await
            .expect("channel error while requesting protocol");
        let protocol = rx
            .await
            .expect("channel error while fetching protocol object");

        let protocol = protocol.and_then(|it| Arc::downcast::<P>(it).ok());
        protocol
    }

    pub async fn endpoint_id(&self) -> Option<iroh::EndpointId> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send_async(ActorInput::RequestEndpointId { tx })
            .await
            .expect("channel error while requesting endpoint id");
        let endpoint_id = rx.await.expect("channel error while receiving endpoint id");
        Some(endpoint_id)
    }
}

/// Internal actor state.
pub struct ActorEngine {
    rx: flume::Receiver<ActorInput>,
    endpoint: Endpoint,
    protocol_map: Arc<RwLock<ProtocolMap>>,
    task_quits: JoinSet<Option<()>>,
    shutdown: Shutdown,
    tokio_handle: Handle,
}

/// Input messages that the actor engine can receive and process
pub enum ActorInput {
    /// Register a new protocol handler for the given ALPN.
    RegisterProtocol {
        alpn: Vec<u8>,
        handler: Arc<dyn AnyDynProtocolHandler>,
    },

    /// Unregister the protocol handler for the given ALPN.
    UnregisterProtocol { alpn: Vec<u8> },

    /// Get the protocol handler for the given ALPN.
    GetProtocol {
        alpn: Vec<u8>,
        client_tx: oneshot::Sender<Option<Arc<dyn AnyDynProtocolHandler>>>,
    },

    /// Request the endpoint ID of the actor.
    RequestEndpointId { tx: oneshot::Sender<EndpointId> },
}

impl ActorEngine {
    /// Construct a new `ActorEngine`, initializing a new `Endpoint`.
    async fn new(
        tokio_handle: Handle,
        shutdown: Shutdown,
        rx: flume::Receiver<ActorInput>,
    ) -> Result<ActorEngine> {
        let endpoint = Endpoint::builder(N0)
            .bind()
            .await
            .context("failed to init Actor/Endpoint")?;
        let engine = ActorEngine {
            //
            rx,
            endpoint,
            protocol_map: Default::default(),
            task_quits: Default::default(),
            shutdown,
            tokio_handle,
        };
        Ok(engine)
    }

    /// Build from an existing `Endpoint`, avoids async and result of `new`.
    fn from_endpoint(
        endpoint: Endpoint,
        tokio_handle: Handle,
        shutdown: Shutdown,
        rx: flume::Receiver<ActorInput>,
    ) -> ActorEngine {
        ActorEngine {
            //
            rx,
            endpoint,
            protocol_map: Default::default(),
            task_quits: Default::default(),
            shutdown,
            tokio_handle,
        }
    }

    pub fn create_input_stream(&mut self) -> impl Stream<Item = ActorInput> + use<> {
        let rx_stream = self.rx.clone().into_stream();

        (
            //
            rx_stream,
        )
            .merge()
    }

    pub async fn init(self) -> Result<ActorEngine> {
        let endpoint = Endpoint::builder(N0)
            .bind()
            .await
            .context("failed to init Actor/Endpoint")?;
        let engine = ActorEngine {
            rx: self.rx,
            endpoint,
            protocol_map: self.protocol_map,
            task_quits: self.task_quits,
            shutdown: self.shutdown,
            tokio_handle: self.tokio_handle,
        };
        Ok(engine)
    }

    pub async fn run(mut self) {
        let mut input = std::pin::pin!(self.create_input_stream());
        loop {
            self.try_run(
                //
                &mut input,
            )
            .await
            .log_err();
        }
    }

    #[throws]
    pub async fn try_run(
        //
        &mut self,
        input: &mut (impl Unpin + Stream<Item = ActorInput>),
    ) {
        // Token will prevent shutdown from completing until dropped
        let _delay_token = self
            .shutdown
            .delay_shutdown_token()
            .context("cannot start protocol actor, process is shutting down")?;

        loop {
            tokio::select! {
                biased;
                // When shutdown is triggered, quit the loop and clean up
                _ = self.shutdown.wait_shutdown_triggered() => {
                    break;
                },
                // handle task terminations and quit on panics.
                Some(task_result) = self.task_quits.join_next() => {
                    if let ControlFlow::Break(()) = self.handle_task_quit(task_result) {
                        break;
                    }
                },
                // handle incoming p2p connections.
                Some(endpoint_incoming) = self.endpoint.accept() => {
                    self.handle_accept_incoming(endpoint_incoming);
                },
                Some(input_event) = input.next() => {
                    if let ControlFlow::Break(()) = self.try_handle_event(input_event).await? {
                        break;
                    }
                }
                else => {
                    break;
                }
            }
        }

        self.endpoint.close().await;
        self.task_quits.abort_all();
        while let Some(res) = self.task_quits.join_next().await {
            match res {
                Err(err) if err.is_panic() => error!("Task panicked: {err:?}"),
                _ => {}
            }
        }
    }

    /// Handle the case when a task has quit, potentially stopping the actor.
    ///
    /// Returns `ControlFlow::Break(())` if the task panicked or was cancelled.
    fn handle_task_quit(&mut self, task_result: Result<Option<()>, JoinError>) -> ControlFlow<()> {
        match task_result {
            Err(outer) => {
                if outer.is_panic() {
                    error!("Task panicked: {outer:?}");
                    return ControlFlow::Break(());
                } else if outer.is_cancelled() {
                    trace!("Task cancelled: {outer:?}");
                } else {
                    error!("Task failed: {outer:?}");
                    return ControlFlow::Break(());
                }
            }
            Ok(Some(())) => {
                trace!("Task finished");
            }
            Ok(None) => {
                trace!("Task cancelled");
            }
        }

        ControlFlow::Continue(())
    }

    fn handle_accept_incoming(&mut self, incoming: Incoming) {
        let protocols = self.protocol_map.clone();
        let span = info_span!(
            "router.accept",
            me = %self.endpoint.id().fmt_short(),
            remote=Empty,
            alpn=Empty,
        );
        let future = handle_connection(incoming, protocols);
        let future = self.shutdown.wrap_cancel(future).map(|_| Some(()));
        let future = future.instrument(span);
        self.task_quits.spawn(future);
    }

    /// Handle an incoming event, potentially modifying the actor state or
    /// spawning new tasks.
    ///
    /// This call is sequentially inline with the actor's event loop, so it
    /// should be considered a critical section and only minimal work should be
    /// performed here.
    ///
    /// - When making quick, direct edits to the actor state, do it here
    /// - When initiating a long-running task, spawn it here and hold any
    ///   necessary handles or channels to communicate with the task.
    #[throws]
    async fn try_handle_event(&mut self, event: ActorInput) -> ControlFlow<()> {
        match event {
            ActorInput::RegisterProtocol { alpn, handler } => {
                self.handle_install_protocol(alpn, handler);
            }
            ActorInput::UnregisterProtocol { alpn } => {
                self.handle_uninstall_protocol(&alpn);
            }
            ActorInput::GetProtocol { alpn, client_tx } => {
                let protocol = self.handle_get_protocol(&alpn);
                client_tx
                    .send(protocol)
                    .expect("channel error while sending get protocol response");
            }
            ActorInput::RequestEndpointId { tx } => {
                let endpoint_id = self.endpoint.id();
                tx.send(endpoint_id)
                    .expect("channel error sending endpoint id response");
            }
        }

        ControlFlow::Continue(())
    }

    /// Installs a new protocol handler for accepting inbound connections with the given ALPN.
    fn handle_install_protocol(&mut self, alpn: Vec<u8>, handler: Arc<dyn AnyDynProtocolHandler>) {
        let mut protocols = self.protocol_map.write().unwrap();
        protocols.insert(alpn, handler);
    }

    /// Uninstalls the protocol handler for the given ALPN.
    fn handle_uninstall_protocol(&mut self, alpn: &[u8]) {
        let mut protocols = self.protocol_map.write().unwrap();
        protocols.remove(alpn);
    }

    /// Returns the protocol handler for the given ALPN, if one is registered.
    fn handle_get_protocol(&mut self, alpn: &[u8]) -> Option<Arc<dyn AnyDynProtocolHandler>> {
        let protocols = self.protocol_map.read().unwrap();
        protocols.get(alpn)
    }
}

async fn handle_connection(incoming: Incoming, protocols: Arc<RwLock<ProtocolMap>>) {
    let mut accepting = match incoming.accept() {
        Ok(conn) => conn,
        Err(err) => {
            warn!("Ignoring connection: accepting failed: {err:#}");
            return;
        }
    };
    let alpn = match accepting.alpn().await {
        Ok(alpn) => alpn,
        Err(err) => {
            warn!("Ignoring connection: invalid handshake: {err:#}");
            return;
        }
    };
    tracing::Span::current().record("alpn", String::from_utf8_lossy(&alpn).to_string());
    let handler = {
        let protocols_read_lock = protocols.read().expect("lock");
        let Some(handler) = protocols_read_lock.get(&alpn).clone() else {
            warn!("Ignoring connection: unsupported ALPN protocol");
            return;
        };
        handler
    };
    match handler.on_accepting(accepting).await {
        Ok(connection) => {
            tracing::Span::current().record(
                "remote",
                tracing::field::display(connection.remote_id().fmt_short()),
            );
            if let Err(err) = handler.accept(connection).await {
                warn!("Handling incoming connection ended with error: {err}");
            }
        }
        Err(err) => {
            warn!("Accepting incoming connection ended with error: {err}");
        }
    }
}
