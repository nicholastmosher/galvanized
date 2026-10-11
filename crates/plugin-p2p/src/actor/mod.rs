//! Protocol handling actor that runs on a background executor.
//!
//! This actor maintains the state of the protocol and handles incoming events.

use std::{any::Any, ops::ControlFlow, sync::Arc};

use anyhow::{Context, Error, Result};
use async_shutdown::ShutdownManager;
use culpa::throws;
use futures::{FutureExt, Stream, StreamExt as _};
use futures_concurrency::stream::Merge;
use gpui::App;
use gpui_util::ResultExt;
use iroh::{
    Endpoint,
    endpoint::{Incoming, presets::N0},
    protocol::{DynProtocolHandler, ProtocolHandler},
};
use tokio::{
    runtime::Handle,
    task::{JoinError, JoinSet},
};
use tracing::{Instrument as _, error, field::Empty, info, info_span, trace, warn};

use protocol::Protocols;

pub mod protocol;

type Shutdown<T = ()> = ShutdownManager<T>;
pub trait AnyDynProtocolHandler: Any + DynProtocolHandler {}
impl<P: Any + DynProtocolHandler> AnyDynProtocolHandler for P {}

/// External handle interface to the actor.
///
/// The Actor handle and ActorEngine share both the same endpoint and protocol map.
#[derive(derive_more::Debug, Clone)]
pub struct Actor {
    endpoint: Endpoint,
    protocols: Protocols,
    _tx: flume::Sender<ActorInput>,
}

impl Actor {
    /// Spawn a new Actor in the background, this becomes the handle API.
    pub fn spawn(
        shutdown: Shutdown,
        endpoint: Endpoint,
        protocols: Protocols,
        cx: &mut App,
    ) -> Self {
        let tokio_handle = tokio::runtime::Handle::current();
        let (tx, rx) = flume::bounded(10);
        let future = {
            let endpoint = endpoint.clone();
            let protocols = protocols.clone();
            let tokio_handle = tokio_handle.clone();
            async move {
                let engine =
                    ActorEngine::new(tokio_handle, shutdown, endpoint, protocols, rx).await?;
                engine.run().await;
                anyhow::Ok(())
            }
        };
        tokio_handle.spawn(future);

        Self {
            endpoint,
            protocols,
            _tx: tx,
        }
    }

    pub fn register_protocol<P: ProtocolHandler>(&self, alpn: Vec<u8>, handler: impl Into<Arc<P>>) {
        self.protocols.insert(alpn, handler.into());
        self.sync_alpns();
    }

    pub fn unregister_protocol(
        &self,
        alpn: &[u8],
    ) -> Option<(Vec<u8>, Arc<dyn AnyDynProtocolHandler>)> {
        let result = self.protocols.remove(alpn);
        self.sync_alpns();
        result
    }

    /// Synchronize the endpoint's ALPN list with the registered protocols.
    ///
    /// iroh's [`Endpoint`] maintains its own list of accepted ALPNs for QUIC handshake
    /// negotiation. When protocols are registered or unregistered dynamically, we must
    /// update the endpoint so it accepts incoming connections for the correct ALPNs.
    fn sync_alpns(&self) {
        let alpns: Vec<Vec<u8>> = self.protocols.alpns().collect();
        self.endpoint.set_alpns(alpns);
    }

    pub fn protocol<P: Any + ProtocolHandler>(&self, alpn: &[u8]) -> Option<Arc<P>> {
        let protocol = self.protocols.get(alpn);
        let protocol = protocol.and_then(|it| Arc::downcast::<P>(it).ok());
        protocol
    }

    pub fn endpoint_id(&self) -> iroh::EndpointId {
        self.endpoint.id()
    }

    pub fn endpoint(&self) -> Endpoint {
        self.endpoint.clone()
    }
}

/// Internal actor state.
pub struct ActorEngine {
    rx: flume::Receiver<ActorInput>,
    endpoint: Endpoint,
    protocols: Protocols,
    task_quits: JoinSet<Option<()>>,
    shutdown: Shutdown,
    tokio_handle: Handle,
}

/// Input messages that the actor engine can receive and process
pub enum ActorInput {}

impl ActorEngine {
    /// Construct a new `ActorEngine`, initializing a new `Endpoint`.
    async fn new(
        tokio_handle: Handle,
        shutdown: Shutdown,
        endpoint: Endpoint,
        protocols: Protocols,
        rx: flume::Receiver<ActorInput>,
    ) -> Result<ActorEngine> {
        let engine = Self::from_endpoint(tokio_handle, shutdown, protocols, endpoint, rx);
        Ok(engine)
    }

    /// Build from an existing `Endpoint`, avoids async and result of `new`.
    fn from_endpoint(
        tokio_handle: Handle,
        shutdown: Shutdown,
        protocols: Protocols,
        endpoint: Endpoint,
        rx: flume::Receiver<ActorInput>,
    ) -> ActorEngine {
        ActorEngine {
            rx,
            endpoint,
            protocols,
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
            protocols: self.protocols,
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
                // Currently redundant, this is where any control-event handling would go
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
        let protocols = self.protocols.clone();
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
    // NOTE: This was previously used for registering/unregistering protocols.
    // Now we share the Protocols map between the actor
    #[throws]
    async fn try_handle_event(&mut self, event: ActorInput) -> ControlFlow<()> {
        match event {
            //
        }

        // ControlFlow::Continue(())
    }
}

async fn handle_connection(incoming: Incoming, protocols: Protocols) {
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
    let incoming_alpn = String::from_utf8_lossy(&alpn).to_string();
    tracing::Span::current().record("alpn", incoming_alpn.clone());

    {
        let registered_alpns = protocols
            .alpns()
            .map(|it| String::from_utf8_lossy(&it).to_string())
            .collect::<Vec<_>>();
        info!(
            ?registered_alpns,
            incoming_alpn, "Handling incoming connection"
        );
    }
    let Some(handler) = protocols.get(&alpn).clone() else {
        warn!("Ignoring connection: unsupported ALPN protocol");
        return;
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
