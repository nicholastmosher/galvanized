use std::{collections::HashMap, path::PathBuf};

use anyhow::{Context as _, Result};
use plugin_vault::{VaultsExt as _, vault_actor::VaultHandle, vault_db::VaultId};
use serde::{Deserialize, Serialize};
use tracing::info;
use willow25::{
    entry::{
        Entry, SubspaceId, SubspaceSecret, randomly_generate_communal_namespace,
        randomly_generate_owned_namespace, randomly_generate_subspace,
    },
    path,
    prelude::{AuthorisedEntry, WriteCapability},
    storage::MemoryStore,
};
use zed::unstable::{
    gpui::{AnyEntity, AppContext, Entity, Global},
    gpui_tokio::Tokio,
    ui::{App, SharedString},
};

use crate::{model::Willowize, space::Space};

pub mod model;
// pub mod profile;
pub mod space;
// pub mod tasks;
pub mod ui;
pub mod willow_serde;

pub fn init(cx: &mut App) {
    let store_path = zed::unstable::paths::data_dir();
    let state = cx.new(|_cx| WillowState::new(store_path.to_path_buf()));
    cx.set_global(GlobalWillow(state));

    ui::init(cx);
}

impl Global for GlobalWillow {}
struct GlobalWillow(Entity<WillowState>);

/// Extension trait to add a convenient `cx.willow()` API for Willow
// Make WillowExt<T> to allow impls with third-party marker types?
pub trait WillowExt {
    type Context: AppContext;
    fn willow(&mut self) -> WillowCx<'_, Self::Context>;
}

impl<C: AppContext> WillowExt for C {
    type Context = C;
    fn willow(&mut self) -> WillowCx<'_, Self::Context> {
        let state = self.read_global::<GlobalWillow, _>(|it, _cx| it.0.clone());
        WillowCx {
            cx: self,
            entity: state,
        }
    }
}

/// Willow API entrypoint
///
/// Willow "store" level operations
// #[derive(Clone)]
pub struct WillowCx<'a, C: AppContext> {
    cx: &'a mut C,
    /// Local state per Willow instance
    // state: Arc<Mutex<WillowState>>,
    // state: Rc<RefCell<WillowState>>,
    entity: Entity<WillowState>,
}

/// State of a Willow instance. Probably 1:1 with a "store" on disk at a given path
struct WillowState {
    spaces: Vec<Entity<Space>>,
    active_space: Option<Entity<Space>>,

    _store_path: PathBuf,

    store: MemoryStore,
}

#[derive(Debug, Serialize, Deserialize)]
struct SubspaceVault {
    subspace_secret: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SubspaceMetadata {
    #[serde(with = "willow_serde::subspace_id")]
    subspace_id: SubspaceId,

    #[serde(skip_serializing_if = "Option::is_none")]
    extra: Option<serde_json::Value>,
}

impl SubspaceMetadata {
    pub fn new(subspace_id: SubspaceId) -> Self {
        Self {
            subspace_id: subspace_id.to_bytes().into(),
            extra: None,
        }
    }

    pub fn extra(&self) -> Option<&serde_json::Value> {
        self.extra.as_ref()
    }

    pub fn extra_mut(&mut self) -> Option<&mut serde_json::Value> {
        self.extra.as_mut()
    }
}

/// Domain object for a Willow subspace
///
/// This stores a [`VaultId`] which can be used together with a
/// password to unlock a vault where the [`SubspaceSecret`] is kept.
#[derive(Debug)]
pub struct Subspace {
    metadata: SubspaceMetadata,
    vault_id: VaultId,
    /// Held only after unlocking the vault
    ///
    /// Holding a vault's handle does not guarantee that it's unlocked,
    /// only that is has been unlocked at some point in the past. Handle
    /// access is subject to expiring capabilities, which will need to
    /// be refreshed by re-unlocking over time.
    vault_handle: Option<VaultHandle>,
}

impl Subspace {
    pub fn new(vault_id: VaultId, metadata: SubspaceMetadata) -> Self {
        Self {
            metadata,
            vault_id,
            vault_handle: None,
        }
    }

    pub fn id(&self) -> SubspaceId {
        self.metadata.subspace_id.clone()
    }

    pub fn metadata(&self) -> &SubspaceMetadata {
        &self.metadata
    }
}

pub trait SubspaceHandle {
    fn id<C: AppContext>(&self, cx: &C) -> SubspaceId;

    fn read_metadata<'a, C: AppContext, R>(
        &self,
        cx: &'a C,
        f: impl FnOnce(&SubspaceMetadata, &App) -> R,
    ) -> R;

    fn in_unlocked<C: AppContext, F>(&self, cx: &C, f: F)
    where
        F: FnOnce(&UnlockedSubspace);
}

impl SubspaceHandle for Entity<Subspace> {
    fn id<C: AppContext>(&self, cx: &C) -> SubspaceId {
        cx.read_entity(self, |this, _cx| this.id())
    }

    fn read_metadata<'a, C: AppContext, R>(
        &self,
        cx: &'a C,
        f: impl FnOnce(&SubspaceMetadata, &App) -> R,
    ) -> R {
        cx.read_entity(self, |it, cx| f(&it.metadata, cx))
    }

    /// Provides access to secure aspects of the [`Subspace`], i.e. locked behind vault access.
    fn in_unlocked<C: AppContext, F>(&self, cx: &C, f: F)
    where
        F: FnOnce(&UnlockedSubspace),
    {
        // let vault_handle = cx.vaults().unlock(vault_handle, read_fn);
    }
}

/// A [`Subspace`]'s privileged API, which may only be accessed while unlocked
pub struct UnlockedSubspace {
    //
}

impl<'a, C: AppContext> WillowCx<'a, C> {
    /// Creates a new Willow subspace, storing the private key in a new vault
    /// encrypted by the given password.
    pub async fn create_subspace<S: Serialize>(
        &mut self,
        password: String,
        metadata_extra: Option<&S>,
    ) -> Result<Entity<Subspace>> {
        let vault_id = self.cx.vaults().create(password.to_string()).await?;
        let vault_handle = self.cx.vaults().unlock(&vault_id, password).await?;

        let (subspace_id, subspace_secret) = Tokio::spawn(self.cx, async move {
            randomly_generate_subspace(&mut rand_core_0_6_4::OsRng)
        })
        .await?;

        let extra = metadata_extra
            .map(|extra| serde_json::to_value(extra))
            .transpose()?;
        let subspace_metadata = SubspaceMetadata {
            subspace_id: subspace_id.to_bytes().into(),
            extra,
        };
        info!(?subspace_metadata, "Creating subspace with metadata");

        let subspace_metadata_bytes = serde_json::to_vec(&subspace_metadata)
            .context("failed to serialize subspace metadata")?;

        let subspace_vault = SubspaceVault {
            subspace_secret: subspace_secret.into_bytes().into(),
        };
        let subspace_vault_bytes =
            serde_json::to_vec(&subspace_vault).context("failed to serialize subspace vault")?;

        // Initial vault should be empty, so we just write to the vault buffers
        self.cx
            .vaults()
            .update(vault_handle, |mut vault| {
                *vault.metadata() = subspace_metadata_bytes;
                *vault.secret() = subspace_vault_bytes;
            })
            .await
            .context("failed to write new subspace to vault")?;
        info!(id = ?subspace_metadata.subspace_id, "Wrote subspace to vault");

        let subspace = Subspace::new(vault_id, subspace_metadata);
        let subspace = self.cx.new(|_cx| subspace);
        Ok(subspace)
    }

    pub async fn list_subspaces(&mut self) -> Result<Vec<Entity<Subspace>>> {
        let vaults = self.cx.vaults().list().await?;
        info!(?vaults, "vaults");

        let tasks = vaults
            .iter()
            .cloned()
            .map(|vault_id| {
                self.cx
                    .vaults()
                    .read_metadata(&vault_id.clone(), move |vault| {
                        info!(vault_metadata_bytes = ?vault.metadata(), "Reading metadata from vault");

                        // We'll filter out anything that doesn't deeserialize as a SubspaceMetadata
                        let meta =
                            serde_json::from_slice::<SubspaceMetadata>(vault.metadata());

                        info!(?meta, "Deserialized vault metadata");
                        meta.map(|it| (vault_id.clone(), it)).ok()
                    })
            })
            .collect::<Vec<_>>();

        // Concurrently query/read the metadata from each vault
        use futures_concurrency::prelude::*;
        let maybe_submetas = tasks
            .join()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;

        let submetas = maybe_submetas
            .into_iter()
            .filter_map(|it| it)
            .collect::<Vec<_>>();

        let subspaces = submetas
            .into_iter()
            .map(|(vault_id, metadata)| Subspace::new(vault_id, metadata))
            .map(|subspace| self.cx.new(|_cx| subspace))
            .collect::<Vec<_>>();

        info!(?subspaces, "willow.list_subspaces()");
        Ok(subspaces)
    }

    pub async fn unlock_subspace(
        &mut self,
        subspace: &Entity<Subspace>,
        password: String,
    ) -> Result<()> {
        let vault_id = self
            .cx
            .read_entity(&subspace, |subspace, _cx| subspace.vault_id.clone());
        let vault_handle = self.cx.vaults().unlock(&vault_id, password).await?;

        self.cx.update_entity(&subspace, |subspace, _cx| {
            subspace.vault_handle = Some(vault_handle);
        });

        Ok(())
    }

    pub fn create_entry(&mut self, subspace: &Entity<Subspace>) {
        //
    }

    // // TODO: Better profile creation API
    // pub fn create_profile(
    //     //
    //     &mut self,
    //     name: impl Into<SharedString>,
    // ) -> Entity<Profile> {
    //     let (_subspace_id, sub_secret) = randomly_generate_subspace(&mut rand_core_0_6_4::OsRng);
    //     let profile = self.cx.new(move |cx| {
    //         //
    //         Profile::new(name, sub_secret, cx)
    //     });

    //     self.cx.update_entity(&self.entity, |state, _cx| {
    //         state.profiles.push(profile.clone());
    //         if state.active_profile.is_none() {
    //             state.active_profile = Some(profile.clone());
    //         }
    //     });

    //     profile
    // }

    pub fn create_owned_space(&mut self, name: impl Into<SharedString>) -> Entity<Space> {
        let (_namespace_id, ns_secret) =
            randomly_generate_owned_namespace(&mut rand_core_0_6_4::OsRng);
        let space = self.cx.new(move |cx| Space::new(name, ns_secret, cx));

        self.cx.update_entity(&self.entity, |state, _cx| {
            state.spaces.push(space.clone());
        });

        space
    }

    pub fn create_communal_space(&mut self, name: impl Into<SharedString>) -> Entity<Space> {
        let (_namespace_id, ns_secret) =
            randomly_generate_communal_namespace(&mut rand_core_0_6_4::OsRng);
        let space = self.cx.new(move |cx| Space::new(name, ns_secret, cx));

        self.cx.update_entity(&self.entity, |state, _cx| {
            state.spaces.push(space.clone());
        });

        space
    }

    // pub fn active_profile(&self) -> Option<Entity<Profile>> {
    //     self.cx
    //         .read_entity(&self.entity, |state, _cx| state.active_profile.clone())
    // }

    // pub fn profiles(&self) -> Vec<Entity<Profile>> {
    //     self.cx
    //         .read_entity(&self.entity, |state, _cx| state.profiles.clone())
    // }

    pub fn active_space(&self) -> Option<Entity<Space>> {
        self.cx
            .read_entity(&self.entity, |state, _cx| state.active_space.clone())
    }

    pub fn set_active_space(&mut self, space: Entity<Space>) {
        self.cx.update_entity(&self.entity, |state, _cx| {
            state.active_space = Some(space);
        });
    }

    pub fn spaces(&self) -> Vec<Entity<Space>> {
        self.cx
            .read_entity(&self.entity, |state, _cx| state.spaces.clone())
    }

    // Todo
    // - this needs to be a friendly easy api
    // - input is the user's entity of the object?
    //   - Need to offer to convert from Entity to value?
    //   - Or take callbacks that say how to manipulate the object

    fn sync<T: Willowize>(&self, it: &Entity<T>, cx: &mut App) {
        // Sync from in-memory to disk
        let sub = cx.observe(it, |it, cx| {
            // TODO: on entity change, check to sync with Willow
            // - Compare hash to avoid sync-looping?
            let value = it.read(cx);
        });

        // TODO: Sync from disk to in-memory
        // cx.willow().observe(it, |it, cx| {
        //     //
        // });
        //
    }

    // // trait Willowize: 'static + JsonSchema + Serialize + for<'de> Deserialize<'de> {}
    // fn todo_write_to_willow<T: Willowize>(&self, input: &Entity<T>, cx: &mut App) {
    //     let value = input.read(cx);
    //     let serialized = serde_json::to_string(value).unwrap();

    //     // TODO: Use explicit parameters rather than "active" context?
    //     let profile_entity = cx.willow().active_profile().unwrap();
    //     let (sub_id, sub_key) = cx.read_entity(&profile_entity, |it, cx| it.parts());
    //     let space_entity = cx.willow().active_space().unwrap();
    //     let (ns_id, ns_key) = cx.read_entity(&space_entity, |it, cx| it.parts());

    //     let entry = Entry::builder()
    //         // What is the context of this call? How do we know chich namespace or subspace IDs to use?
    //         .namespace_id(ns_id)
    //         .subspace_id(sub_id.clone())
    //         .path(path!("/todo/path"))
    //         .now()
    //         .unwrap()
    //         .payload(&serialized)
    //         .build();
    //     let write_capability = WriteCapability::new_owned(&ns_key, sub_id);

    //     // Entry with content serialized from the given Entity
    //     let authorized_entry = entry
    //         .into_authorised_entry(&write_capability, &sub_key)
    //         .unwrap();

    //     // Foreground: no Sync requirement, but shouldn't do heavy lifting
    //     cx.spawn({
    //         let authorized_entry = authorized_entry.clone();
    //         async move |cx| {
    //             //
    //             anyhow::Ok(())
    //         }
    //     })
    //     .detach_and_log_err(cx);

    //     // // Background: Requires Sync
    //     // let _task = cx.background_spawn({
    //     //     let authorized_entry = authorized_entry.clone();
    //     //     async move {
    //     //         let willow = willow;
    //     //         let state = willow.state.clone();
    //     //         let mut state = state.borrow_mut();
    //     //         let write_visible = state.store.insert_entry(authorized_entry).await?;
    //     //         //
    //     //         anyhow::Ok(())
    //     //     }
    //     // });
    // }

    // // Memory -> Willow: Entity<T>
    // // Willow -> Memory: WillowEntity<T> ? To encode space/subspace/path?
    // fn todo_read_from_willow<T: Willowize>(&self, cx: &mut App) -> Result<T> {
    //     todo!()
    // }
}

impl WillowState {
    fn new(store_path: PathBuf) -> Self {
        let spaces = vec![];

        let store = MemoryStore::new();

        Self {
            spaces,
            active_space: None,
            _store_path: store_path,
            store,
        }
    }
}

// pub struct WillowObject<T> {
//     _phantom: PhantomData<T>,
// }

// pub struct WillowFeed<T> {
//     _phantom: PhantomData<T>,
// }

// /// A Willow Entity is a handle representing an object with a well-known type
// ///
// /// To be a somewhat complete and well-addressed handle, a WillowEntity includes
// /// information about the namespace and subspace of the underlying Entry.
// ///
// /// So an Entity is like an address/handle for an Area, so it's defined by its
// /// namespace, subspace, and path prefix (directory). The definition of a Willow
// /// Area also includes a time range, I want to think about how to represent time
// /// in a dedicated brainstorm.
// ///
// /// - Area in the spec has `subspace_id: SubspaceId | any`, which implies an
// ///   arbitrary restriction in the expressiveness of the API. I think it should
// ///   easily be possible to specify a list of subspaces we're interested in.
// struct WillowEntity<T: WillowModel> {
//     _phantom: PhantomData<T>,
// }

// struct WillowContext<T> {
//     _phantom: PhantomData<T>,
// }

// impl<T: WillowModel> WillowEntity<T> {
//     fn read(&self, _cx: &mut WillowContext<T>) -> Option<&T> {
//         None
//     }
// }

// // WillowComponent?
// // WillowSpec
// // WillowArea
// // WillowModel <-- expresses paths to multiple files, typed extractors
// // - Model would refer to a multi-"file" data construction which is located
// //   at a path and described by the set of files the model refers to, as well
// //   as the types of those files.
// pub trait WillowModel: JsonSchema + Serialize + for<'de> Deserialize<'de> {}
