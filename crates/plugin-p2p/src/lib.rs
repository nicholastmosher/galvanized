use std::{any::Any, sync::Arc};

use iroh::{Endpoint, endpoint::presets::N0, protocol::ProtocolHandler};
use zed::unstable::{
    gpui::{AppContext, Entity, Task},
    ui::{App, Context},
};

use crate::actor::{Actor, protocol::Protocols};

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

    /// An object shared with the actor that holds the installed protocol handlers.
    ///
    /// This allows us to call, without async, to access the API object of any given protocol.
    protocols: Protocols,
}

impl P2p {
    /// Create a new `P2p` instance, with an empty protocol stack and spawning a new actor.
    pub fn new(endpoint: Endpoint, cx: &mut App) -> Self {
        let shutdown = async_shutdown::ShutdownManager::new();
        let protocols = Protocols::default();
        let actor = Actor::spawn(shutdown, endpoint, protocols.clone(), cx);

        Self { actor, protocols }
    }

    /// Creates a new `P2p` instance from an existing actor, with an empty protocol stack.
    pub fn from_actor(actor: Actor) -> Self {
        Self {
            actor,
            protocols: Default::default(),
        }
    }

    pub fn endpoint_id(&self, cx: &mut Context<Self>) -> Task<Option<iroh::EndpointId>> {
        let actor = self.actor.clone();
        let task = cx.spawn(async move |_this, _cx| actor.endpoint_id().await);
        task
    }

    pub fn endpoint(&self, cx: &mut Context<Self>) -> Task<iroh::Endpoint> {
        let actor = self.actor.clone();
        cx.spawn(async move |_this, _cx| actor.endpoint().await)
    }

    pub fn register_protocol<P: ProtocolHandler>(
        &mut self,
        alpn: Vec<u8>,
        handler: P,
        cx: &mut Context<Self>,
    ) {
        let handler = Arc::new(handler);
        self.protocols.insert(alpn.clone(), handler.clone());

        let actor = self.actor.clone();
        cx.spawn(async move |_this, _cx| {
            actor.register_protocol::<P>(alpn, handler).await?;
            anyhow::Ok(())
        })
        .detach_and_log_err(cx);
    }

    pub fn unregister_protocol(&mut self, alpn: Vec<u8>, cx: &mut Context<Self>) {
        self.protocols.remove(&alpn);

        let actor = self.actor.clone();
        cx.spawn(async move |_this, _cx| {
            actor.unregister_protocol(alpn).await?;
            anyhow::Ok(())
        })
        .detach_and_log_err(cx);
    }

    pub fn protocol<P: Any + ProtocolHandler>(
        &self,
        alpn: Vec<u8>,
        _cx: &mut Context<Self>,
    ) -> Option<Arc<P>> {
        let protocol = self.protocols.get(&alpn);
        let protocol = protocol.and_then(|it| Arc::downcast::<P>(it).ok());
        protocol
    }
}

pub trait P2pHandle {
    //
    fn register_protocol<C: AppContext>(
        &self,
        alpn: Vec<u8>,
        handler: impl ProtocolHandler,
        cx: &mut C,
    );
    fn unregister_protocol<C: AppContext>(&self, alpn: Vec<u8>, cx: &mut C);
    fn protocol<P: Any + ProtocolHandler, C: AppContext>(
        &self,
        alpn: Vec<u8>,
        cx: &mut C,
    ) -> Option<Arc<P>>;
    fn endpoint_id<C: AppContext>(&self, cx: &mut C) -> Task<Option<iroh::EndpointId>>;
    fn endpoint<C: AppContext>(&self, cx: &mut C) -> Task<iroh::Endpoint>;
}

impl P2pHandle for Entity<P2p> {
    fn register_protocol<C: AppContext>(
        &self,
        alpn: Vec<u8>,
        handler: impl ProtocolHandler,
        cx: &mut C,
    ) {
        cx.update_entity(self, |this, cx| {
            this.register_protocol(alpn, handler, cx);
        });
    }

    fn unregister_protocol<C: AppContext>(&self, alpn: Vec<u8>, cx: &mut C) {
        cx.update_entity(self, |this, cx| {
            this.unregister_protocol(alpn, cx);
        });
    }

    fn protocol<P: Any + ProtocolHandler, C: AppContext>(
        &self,
        alpn: Vec<u8>,
        cx: &mut C,
    ) -> Option<Arc<P>> {
        cx.update_entity(self, |this, cx| this.protocol::<P>(alpn, cx))
    }

    fn endpoint_id<C: AppContext>(&self, cx: &mut C) -> Task<Option<iroh::EndpointId>> {
        cx.update_entity(self, |this, cx| this.endpoint_id(cx))
    }

    fn endpoint<C: AppContext>(&self, cx: &mut C) -> Task<iroh::Endpoint> {
        cx.update_entity(self, |this, cx| this.endpoint(cx))
    }
}
