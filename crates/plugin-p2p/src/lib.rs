use std::{
    any::{Any, type_name},
    sync::Arc,
};

use anyhow::Result;
use iroh::{Endpoint, EndpointId, protocol::ProtocolHandler};
use tracing::info;
use zed::unstable::{
    gpui::{AppContext, Entity, WeakEntity},
    ui::{App, Context},
};

use crate::actor::{Actor, AnyDynProtocolHandler, protocol::Protocols};

pub mod actor;
pub mod codec;
pub mod iroh_repo;
pub mod v1;

pub fn init(_cx: &mut App) {
    //
}

/// Entity representing an Iroh p2p instance, with dynamic protocol support.
///
/// Constructing an instance of `P2p` will spawn an actor that allows installing
/// [`ProtocolHandler`]s dynamically, to allow plugins to register custom protocols.
pub struct P2p {
    /// The handle to the actor that manages the p2p connection.
    actor: Actor,
}

impl P2p {
    /// Create a new `P2p` instance, with an empty protocol stack and spawning a new actor.
    pub fn new(endpoint: Endpoint, cx: &mut App) -> Self {
        let shutdown = async_shutdown::ShutdownManager::new();
        let protocols = Protocols::default();
        let actor = Actor::spawn(shutdown, endpoint, protocols, cx);
        Self::from_actor(actor)
    }

    /// Creates a new `P2p` instance from an existing actor, with an empty protocol stack.
    pub fn from_actor(actor: Actor) -> Self {
        Self { actor }
    }

    /// Returns the endpoint id of this `P2p` instance.
    pub fn endpoint_id(&self, _cx: &mut Context<Self>) -> EndpointId {
        self.actor.endpoint_id()
    }

    /// Returns the endpoint of this `P2p` instance.
    pub fn endpoint(&self) -> Endpoint {
        self.actor.endpoint()
    }

    /// Registers a protocol handler for the given ALPN protocol.
    ///
    /// There may only be one protocol handler registered for each ALPN.
    pub fn register_protocol<P: Any + ProtocolHandler>(&mut self, alpn: Vec<u8>, handler: P) {
        self.actor.register_protocol(alpn.clone(), handler);
        info!(?alpn, type=type_name::<P>(), "Registered protocol handler");
    }

    /// Removes the protocol handler for the given ALPN protocol.
    pub fn unregister_protocol(
        &mut self,
        alpn: &[u8],
    ) -> Option<(Vec<u8>, Arc<dyn AnyDynProtocolHandler>)> {
        self.actor.unregister_protocol(alpn)
    }

    /// Returns the protocol handler for the given ALPN protocol, if one is registered.
    pub fn protocol<P: Any + ProtocolHandler>(
        &self,
        alpn: Vec<u8>,
        _cx: &mut Context<Self>,
    ) -> Option<Arc<P>> {
        let protocol = self.actor.protocol::<P>(&alpn);
        info!(
            protocol = ?protocol,
            "Found protocol for ALPN: {}",
            String::from_utf8_lossy(&alpn)
        );
        let protocol = protocol.and_then(|it| Arc::downcast::<P>(it).ok());
        info!(
            "Successfully downcasted protocol for ALPN: {}",
            String::from_utf8_lossy(&alpn)
        );
        protocol
    }
}

pub trait P2pHandle {
    type Result<T>;
    fn register_protocol<P, C>(&self, handler: P, cx: &mut C) -> Self::Result<()>
    where
        P: KeyedProtocolHandler,
        C: AppContext;
    fn unregister_protocol<C: AppContext>(
        &self,
        alpn: &[u8],
        cx: &mut C,
    ) -> Self::Result<Option<(Vec<u8>, Arc<dyn AnyDynProtocolHandler>)>>;
    fn protocol<P: Any + KeyedProtocolHandler, C: AppContext>(
        &self,
        cx: &mut C,
    ) -> Self::Result<Option<Arc<P>>>;
    fn endpoint_id<C: AppContext>(&self, cx: &mut C) -> Self::Result<EndpointId>;
    fn endpoint<C: AppContext>(&self, cx: &mut C) -> Self::Result<Endpoint>;
}

pub trait KeyedProtocolHandler: ProtocolHandler {
    const ALPN: &'static [u8];
}

impl P2pHandle for Entity<P2p> {
    type Result<T> = T;

    fn register_protocol<P, C>(&self, handler: P, cx: &mut C) -> Self::Result<()>
    where
        P: KeyedProtocolHandler,
        C: AppContext,
    {
        cx.update_entity(self, |p2p, _cx| {
            p2p.register_protocol(P::ALPN.to_vec(), handler);
        });
    }

    fn unregister_protocol<C: AppContext>(
        &self,
        alpn: &[u8],
        cx: &mut C,
    ) -> Self::Result<Option<(Vec<u8>, Arc<dyn AnyDynProtocolHandler>)>> {
        cx.update_entity(self, |p2p, _cx| p2p.unregister_protocol(alpn))
    }

    fn protocol<P: Any + KeyedProtocolHandler, C: AppContext>(
        &self,
        cx: &mut C,
    ) -> Self::Result<Option<Arc<P>>> {
        cx.update_entity(self, |this, cx| this.protocol::<P>(P::ALPN.to_vec(), cx))
    }

    fn endpoint_id<C: AppContext>(&self, cx: &mut C) -> Self::Result<EndpointId> {
        cx.update_entity(self, |this, cx| this.endpoint_id(cx))
    }

    fn endpoint<C: AppContext>(&self, cx: &mut C) -> Self::Result<Endpoint> {
        cx.read_entity(self, |it, _cx| it.endpoint())
    }
}

impl P2pHandle for WeakEntity<P2p> {
    type Result<T> = Result<T>;

    fn register_protocol<P, C>(&self, handler: P, cx: &mut C) -> Self::Result<()>
    where
        P: KeyedProtocolHandler,
        C: AppContext,
    {
        self.update(cx, |p2p, _cx| {
            p2p.register_protocol(P::ALPN.to_vec(), handler)
        })
    }

    fn unregister_protocol<C: AppContext>(
        &self,
        alpn: &[u8],
        cx: &mut C,
    ) -> Self::Result<Option<(Vec<u8>, Arc<dyn AnyDynProtocolHandler>)>> {
        self.update(cx, |p2p, _cx| p2p.unregister_protocol(alpn))
    }

    fn protocol<P: Any + KeyedProtocolHandler, C: AppContext>(
        &self,
        cx: &mut C,
    ) -> Self::Result<Option<Arc<P>>> {
        self.update(cx, |it, cx| it.protocol::<P>(P::ALPN.to_vec(), cx))
    }

    fn endpoint_id<C: AppContext>(&self, cx: &mut C) -> Self::Result<EndpointId> {
        self.update(cx, |it, cx| it.endpoint_id(cx))
    }

    fn endpoint<C: AppContext>(&self, cx: &mut C) -> Self::Result<Endpoint> {
        self.update(cx, |it, _cx| it.endpoint())
    }
}
