use std::{any::Any, sync::Arc};

use iroh::protocol::ProtocolHandler;
use zed::unstable::{
    gpui::{AppContext, Entity, Task},
    ui::{App, Context},
};

use crate::actor::{Actor, protocol::ProtocolMap};

pub mod actor;
pub mod codec;
pub mod iroh_repo;
pub mod v1;

pub fn init(_cx: &mut App) {
    //
}

pub struct P2p {
    //
    actor: Actor,
    protocols: ProtocolMap,
}

impl P2p {
    pub fn new(cx: &mut App) -> Self {
        let shutdown = async_shutdown::ShutdownManager::new();
        let actor = Actor::spawn(shutdown, cx);
        let protocols = Default::default();

        Self { actor, protocols }
    }

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
}
