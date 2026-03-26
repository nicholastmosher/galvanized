use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};
use tracing::warn;
use willow25::{entry::Entry, path, prelude::WriteCapability, storage::Store as _};
use zed::unstable::{
    gpui::{AppContext, Entity, Subscription},
    ui::App,
};

use crate::willow::WillowExt;

// Need a name for "types which "
pub trait Willowize: 'static + JsonSchema + Serialize + for<'de> Deserialize<'de> {
    //
}

/// Behavior for `Entity<T: Willowize>`
pub trait EntryHandle {
    fn load(&self, cx: &mut App);
    fn save(&self, cx: &mut App);
    fn sync(&self, cx: &mut App) -> Subscription;
}

impl<T: Willowize> EntryHandle for Entity<T> {
    fn load(&self, cx: &mut App) {
        // TODO: Use explicit parameters rather than "active" context?
        let profile_entity = cx.willow().active_profile().unwrap();
        let (sub_id, sub_key) = cx.read_entity(&profile_entity, |it, cx| it.parts());
        let space_entity = cx.willow().active_space().unwrap();
        let (ns_id, ns_key) = cx.read_entity(&space_entity, |it, cx| it.parts());

        // let maybe_entry = cx
        //     .willow()
        //     .state
        //     .borrow_mut()
        //     .entity_entries
        //     .get(&self.clone().into_any());
        // let Some(entry) = maybe_entry else {
        //     warn!("Missing AuthorizedEntry for Entity");
        //     return;
        // };

        // let it = cx.willow().state.borrow_mut().store.get_entry(
        //     &ns_id,
        //     entry,
        //     expected_digest,
        //     payload_slice,
        // );
    }

    fn save(&self, cx: &mut App) {
        let value = self.read(cx);
        let serialized = serde_json::to_string(value).unwrap();

        // TODO: Use explicit parameters rather than "active" context?
        let profile_entity = cx.willow().active_profile().unwrap();
        let (sub_id, sub_key) = cx.read_entity(&profile_entity, |it, cx| it.parts());
        let space_entity = cx.willow().active_space().unwrap();
        let (ns_id, ns_key) = cx.read_entity(&space_entity, |it, cx| it.parts());

        let entry = Entry::builder()
            // What is the context of this call? How do we know chich namespace or subspace IDs to use?
            .namespace_id(ns_id)
            .subspace_id(sub_id.clone())
            .path(path!("/todo/path"))
            .now()
            .unwrap()
            .payload(&serialized)
            .build()
            .unwrap();
        let write_capability = WriteCapability::new_owned(&ns_key, sub_id);

        // Entry with content serialized from the given Entity
        let authorized_entry = entry
            .into_authorised_entry(&write_capability, &sub_key)
            .unwrap();

        let willow = cx.willow();

        // Foreground: no Sync requirement, but shouldn't do heavy lifting
        cx.spawn({
            let authorized_entry = authorized_entry.clone();
            async move |cx| {
                {
                    let state = cx.willow().state.clone();
                    // let mut state = state.lock().unwrap();
                    let mut state = state.borrow_mut();
                    let write_visible = state.store.insert_entry(authorized_entry).await?;
                }

                anyhow::Ok(())
            }
        })
        .detach_and_log_err(cx);
    }

    fn sync(&self, cx: &mut App) -> Subscription {
        cx.observe(self, |this, cx| {
            //
            this.save(cx);
        })
    }
}

// FromContext
// Like App -> Context<T>
// But App -> WillowContext<T>
// impl<'a> From<&'a mut App> for &'a mut WillowContext<T> { }
// meh no

fn doit<T: Willowize>(it: T) {
    let it = serde_json::to_vec(&it).unwrap();
    let it = serde_json::from_slice::<T>(&*it).unwrap();
    let schema = schema_for!(T);
}
