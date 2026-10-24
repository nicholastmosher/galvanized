use anyhow::Result;
use autosurgeon::{Hydrate, Reconcile, hydrate, reconcile};
use gpui::{App, AppContext, Entity, Task, TaskExt};
use plugin_galvanized::domain::profile::{Profile, ProfileHandle};
use plugin_p2p::P2pHandle;
use samod::DocHandle;
use tracing::info;

use crate::protocol::AutomergeProtocol;

pub mod codec;
pub mod protocol;
pub mod ticket;

pub fn init(cx: &mut App) {
    // For each new P2p instance, register the Automerge protocol handler.
    cx.observe_new::<Profile>(|_profile, _window, cx| {
        let profile = cx.entity();
        cx.spawn(async move |_this, cx| {
            let endpoint = profile.p2p(cx).endpoint(cx);
            let protocol_automerge = AutomergeProtocol::new(endpoint.clone()).await;
            profile.update(cx, |profile, cx| {
                profile.p2p().register_protocol(protocol_automerge, cx);
            });
            info!(
                endpoint_id = %endpoint.id(),
                "Registered AutomergeProtocol with P2p instance"
            );
            anyhow::Ok(())
        })
        .detach_and_log_err(cx);
    })
    .detach();
}

/// Trait to be implemented by types used as Entities which want to sync with peers.
pub trait Automergable: 'static {
    /// A data type representing this Entity which syncs between peers.
    type Crdt: Hydrate + Reconcile + Send;

    /// Returns an instance of the document's [`DocHandle`].
    fn doc_handle(&self) -> DocHandle;
}

/// Convenience trait to provide functionality to `Entity<T: AutomergeEntity>`.
pub trait AutomergeHandle {
    /// The data type that's synced between peers.
    type Crdt: Send;

    /// Returns an instance of the document's [`DocHandle`].
    fn doc_handle<C: AppContext>(&self, cx: &mut C) -> DocHandle;

    /// Returns a freshly-hydrated object from the current document.
    fn read<C: AppContext>(&self, cx: &mut C) -> Task<Result<Self::Crdt>>;

    /// Sets the state of a document to a given value.
    fn set<C: AppContext>(&self, cx: &mut C, crdt: Self::Crdt) -> Task<Result<()>>;

    /// Provides a mutable reference to a hydrated object, then reconciles any edits to the document.
    fn update<C: AppContext>(
        &self,
        cx: &mut C,
        f: impl 'static + Send + FnOnce(&mut Self::Crdt),
    ) -> Task<Result<()>>;
}

impl<T: Automergable> AutomergeHandle for Entity<T> {
    type Crdt = T::Crdt;

    fn doc_handle<C: AppContext>(&self, cx: &mut C) -> DocHandle {
        cx.read_entity(self, |it, _cx| it.doc_handle())
    }

    fn read<C: AppContext>(&self, cx: &mut C) -> Task<Result<T::Crdt>> {
        use autosurgeon::hydrate;
        let doc_handle = self.doc_handle(cx);
        let task = cx.background_spawn(async move {
            let data = doc_handle.with_document(|am| {
                let chat_data = hydrate::<_, Self::Crdt>(am)?;
                anyhow::Ok(chat_data)
            })?;
            Ok(data)
        });
        task
    }

    fn set<C: AppContext>(&self, cx: &mut C, data: Self::Crdt) -> Task<Result<()>> {
        let doc_handle = self.doc_handle(cx);
        let task = cx.background_spawn(async move {
            doc_handle.with_document(|am| {
                let mut tx = am.transaction();
                reconcile(&mut tx, &data)?;
                let (_change_hash, _patch_log) = tx.commit();
                anyhow::Ok(())
            })?;
            Ok(())
        });
        task
    }

    fn update<C: AppContext>(
        &self,
        cx: &mut C,
        f: impl 'static + Send + FnOnce(&mut Self::Crdt),
    ) -> Task<Result<()>> {
        let doc_handle = self.doc_handle(cx);
        cx.background_spawn(async move {
            doc_handle.with_document(|am| {
                let mut tx = am.transaction();
                // Read the current document into a data object
                let mut crdt = hydrate::<_, Self::Crdt>(&tx)?;
                // // Pass the data object by &mut to allow client to update "current" value
                f(&mut crdt);
                // Reconcile the updated data back into the document
                reconcile(&mut tx, &crdt)?;
                let (_change_hash, _patch_log) = tx.commit();
                info!("Wrote new message to Automerge");
                anyhow::Ok(())
            })?;
            anyhow::Ok(())
        })
    }
}

#[cfg(test)]
#[allow(warnings)]
mod tests {
    use super::*;
    use gpui::{self, App, Context, Entity};
    use samod::DocHandle;

    // Compile-only test, can't make #[gpui::test] work yet
    fn test_api(mut _cx: &App) {
        pub struct Calendar {
            doc_handle: DocHandle,
        }
        impl Calendar {
            fn new(doc_handle: DocHandle, _cx: &mut Context<Self>) -> Self {
                Self { doc_handle }
            }
        }
        impl Automergable for Calendar {
            type Crdt = CalendarData;
            fn doc_handle(&self) -> DocHandle {
                self.doc_handle.clone()
            }
        }
        #[derive(Debug, Clone, PartialEq, Hydrate, Reconcile)]
        pub struct CalendarData {
            //
            events: Vec<String>,
        }
        impl CalendarData {
            fn new() -> Self {
                Self {
                    events: Default::default(),
                }
            }
        }

        fn example(doc_handle: DocHandle, cx: &mut App) -> Entity<Calendar> {
            let calendar = cx.new(|cx| Calendar::new(doc_handle, cx));
            let _doc_handle = calendar.doc_handle(cx);
            calendar
        }
    }
}
