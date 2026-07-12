use iroh::protocol::ProtocolHandler;
use zed::unstable::{
    gpui::Task,
    ui::{App, Context},
};

use crate::actor::Actor;

pub mod actor;
pub mod codec;
pub mod iroh_repo;
pub mod v1;

pub fn init(cx: &mut App) {
    //
}

pub struct P2p {
    //
    actor: Actor,
}

impl P2p {
    pub fn new(cx: &mut App) -> Self {
        let shutdown = async_shutdown::ShutdownManager::new();
        let actor = Actor::spawn(shutdown, cx);

        Self { actor }
    }

    pub fn from_actor(actor: Actor) -> Self {
        Self { actor }
    }

    pub fn endpoint_id(&self, cx: &mut Context<Self>) -> Task<Option<iroh::EndpointId>> {
        let actor = self.actor.clone();
        let task = cx.spawn(async move |_this, _cx| actor.endpoint_id().await);
        task
    }

    pub fn register_protocol(
        &self,
        alpn: Vec<u8>,
        handler: impl ProtocolHandler,
        cx: &mut Context<Self>,
    ) {
        let actor = self.actor.clone();
        cx.spawn(async move |_this, _cx| {
            actor.register_protocol(alpn, handler).await?;
            anyhow::Ok(())
        })
        .detach_and_log_err(cx);
    }

    pub fn unregister_protocol(&self, alpn: Vec<u8>, cx: &mut Context<Self>) {
        let actor = self.actor.clone();
        cx.spawn(async move |_this, _cx| {
            actor.unregister_protocol(alpn).await?;
            anyhow::Ok(())
        })
        .detach_and_log_err(cx);
    }
}
