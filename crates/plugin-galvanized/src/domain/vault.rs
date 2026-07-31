use std::collections::{BTreeMap, HashMap};

use anyhow::{Context as _, Result, bail};
use galvanized_cap::{
    CapCell, StrictSendCap, WillowKeyStore, WillowRead, WillowScope, WillowWrite,
};
use gpui::{AppContext, Context, Entity, IntoElement, SharedString, Task};
use iroh::{Endpoint, endpoint::presets::N0};
use plugin_vault::{
    VaultsExt as _, vault_actor::VaultToken, vault_actor::VaultUnlock, vault_db::VaultId,
};
use plugin_willow::WillowExt;
use plugin_willow::willow_serde::SubspaceIdSerde;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use tracing::{info, warn};
use willow25::entry::{NamespaceId, SubspaceId};

use crate::domain::{
    profile::{Profile, ProfileData},
    space::{Space, SpaceData},
};

/// A Vault stores a user's Profiles and Spaces encrypted with a password.
///
/// One Vault may contain multiple Spaces and Profiles, which correspond
/// to Willow Namespaces and Subspaces, respectively.
///
/// A Vault also provides protected access to the Willow data it owns.
/// Willow data access is gated behind the vault being unlocked, and the
/// unlocked content is held behind a capability ([`CapCell`]) that blanks on
/// lock or timeout.
pub struct Vault {
    /// Public metadata about the user, visible while the vault is locked.
    metadata: VaultMetadata,

    /// The ID of the vault underlying this User.
    vault_id: VaultId,

    /// If unlocked, a token granting access to the underlying Vault.
    ///
    /// The token carries the capability to read and write to the vault, and
    /// is obtained by unlocking the vault. A token's capabilities may expire
    /// over time or be revoked.
    vault_token: Option<VaultToken>,

    // ── T1 capability session state ───────────────────────────────────────
    /// The Willow *write* session capability, minted on unlock and revoked on
    /// lock. App plugins present this to the [`WillowKeyStore`] to gate Willow
    /// writes and "sign as Profile".
    willow_write: Option<StrictSendCap<WillowWrite, WillowScope>>,
    /// The Willow *read* session capability, held transiently between unlock
    /// and `load_content`, then moved into the [`CapCell`] gating the content.
    willow_read: Option<StrictSendCap<WillowRead, WillowScope>>,
    /// Revokers for the read/write session capabilities; the lock action
    /// revokes both, blanking every [`CapCell`] (and making the key store
    /// inert) in one step.
    read_revoker: Option<galvanized_cap::StrictRevoker>,
    write_revoker: Option<galvanized_cap::StrictRevoker>,
    /// The cap-gated secret custodian. `None` when locked.
    key_store: Option<WillowKeyStore>,

    /// The vault's unlocked content, gated behind the read session capability.
    /// Yields `None` once the capability is revoked (lock) or expired
    /// (timeout) — the `vault.rs:41` TODO, now built.
    unlocked_content:
        Option<CapCell<VaultContent, StrictSendCap<WillowRead, WillowScope>, WillowRead>>,

    // Entity caches for rendering. Cleared on lock (defense-in-depth: their
    // private secrets are already unreachable, but clearing them on lock is
    // the doctrine — capability expiry *and* in-memory unavailability).
    unlocked_spaces: BTreeMap<NamespaceId, Entity<Space>>,
    unlocked_active_profile: Option<Entity<Profile>>,
    unlocked_profiles: BTreeMap<SubspaceId, Entity<Profile>>,
    unlock_task: Option<Task<Result<()>>>,
}

impl Vault {
    /// Create a new User from the given [`VaultId`] and user name.
    pub fn new(vault_id: VaultId, user_name: String, cx: &mut Context<Self>) -> Self {
        let metadata = VaultMetadata::new(user_name);
        Self::from_metadata(vault_id, metadata, cx)
    }

    /// Create a new User using the given vault and user metadata.
    ///
    /// User metadata is public, and visible when the user's vault is locked.
    pub fn from_metadata(
        vault_id: VaultId,
        metadata: VaultMetadata,
        _cx: &mut Context<Self>,
    ) -> Self {
        Self {
            metadata,
            vault_id,
            vault_token: None,
            willow_write: None,
            willow_read: None,
            read_revoker: None,
            write_revoker: None,
            key_store: None,
            unlocked_content: None,
            unlocked_spaces: Default::default(),
            unlocked_active_profile: Default::default(),
            unlocked_profiles: Default::default(),
            unlock_task: None,
        }
    }

    /// The name of this [`User`].
    ///
    /// The name is public, and visible even when the user's vault is locked.
    pub fn name(&self) -> SharedString {
        self.metadata.user_name.clone()
    }

    /// Returns `true` when the vault is locked (no active session).
    pub fn is_locked(&self) -> bool {
        self.vault_token.is_none()
    }

    /// Attempts to unlock this User by unlocking its underlying Vault
    pub fn unlock(&mut self, password: String, cx: &mut Context<Self>) -> Task<Result<()>> {
        let vault_id = self.vault_id.clone();
        cx.spawn(async move |this, cx| {
            let session = cx.vaults().unlock(&vault_id, password).await?;

            this.update(cx, |this, cx| {
                this.apply_unlock_session(session);
                this.load_content(cx)
            })?
            .await?;

            info!("Unlocked vault");
            Ok(())
        })
    }

    /// Stores the capabilities minted on unlock (vault-access token plus the
    /// Willow session caps and their revokers). The read cap is stashed for
    /// [`load_content`] to wrap the content in a [`CapCell`].
    pub fn apply_unlock_session(&mut self, session: VaultUnlock) {
        let VaultUnlock {
            token,
            willow_read,
            willow_write,
            read_revoker,
            write_revoker,
        } = session;
        self.vault_token = Some(token);
        self.willow_read = Some(willow_read);
        self.willow_write = Some(willow_write);
        self.read_revoker = Some(read_revoker);
        self.write_revoker = Some(write_revoker);
    }

    /// Compatibility shim retained for callers that hand back a bare token.
    pub fn set_unlock_token(&mut self, token: VaultToken) {
        self.vault_token = Some(token);
    }

    /// The Willow write session capability, if the vault is unlocked. App
    /// plugins present this to the key store to gate Willow writes.
    pub fn willow_write_cap(&self) -> Option<StrictSendCap<WillowWrite, WillowScope>> {
        self.willow_write.clone()
    }

    /// The cap-gated secret custodian, if the vault is unlocked.
    pub fn key_store(&self) -> Option<WillowKeyStore> {
        self.key_store.clone()
    }

    /// Locks the vault: revokes every session capability (read + write),
    /// dropping the gated content and making the key store inert, and clears
    /// the in-memory entity caches. After this, gated content is inaccessible
    /// through the public API.
    pub fn lock(&mut self, cx: &mut Context<Self>) {
        if let Some(revoker) = self.read_revoker.take() {
            revoker.revoke();
        }
        if let Some(revoker) = self.write_revoker.take() {
            revoker.revoke();
        }
        self.willow_read = None;
        self.willow_write = None;
        self.key_store = None;
        self.unlocked_content = None;
        self.unlocked_spaces.clear();
        self.unlocked_profiles.clear();
        self.unlocked_active_profile = None;
        if let Some(token) = self.vault_token.take() {
            token.lock();
        }
        self.unlock_task = None;
        cx.notify();
    }

    /// If the given [`Vault`] is unlocked, fetch its content and store it in the entity
    ///
    /// This function is async because it drives IO to fetch and decrypt the vault's
    /// content from the underlying database. Be sure to cache any retrieved data
    /// to store in UI entities.
    pub fn load_content(&mut self, cx: &mut Context<Self>) -> Task<Result<()>> {
        let vault_handle = self.vault_token.clone().context("vault is not unlocked");
        let willow_read = self.willow_read.take();

        cx.spawn(async move |this, cx| {
            let vault_handle = vault_handle?;

            let vault_content = cx
                .vaults()
                .read(vault_handle, |vault| {
                    serde_json::from_slice::<VaultContent>(vault.secret())
                        .context("failed to deserialize VaultContent")
                })
                .await??;

            use futures_concurrency::prelude::*;
            let endpoints_futs = vault_content
                .profiles
                .iter()
                .map(|profile| async move {
                    let endpoint = Endpoint::builder(N0).bind().await?;
                    anyhow::Ok((profile.id(), endpoint))
                })
                .collect::<Vec<_>>();

            // Concurrently initialize one Endpoint per profile
            let profile_endpoints = endpoints_futs
                .join()
                .await
                .into_iter()
                .collect::<Result<HashMap<_, _>, _>>()?;

            this.update(cx, |this, cx| {
                // Build the cap-gated key store from the content's namespace/subspace
                // secrets only on first load (e.g. right after unlock). On reload
                // we keep the existing store so secrets added via `extend_key_store`
                // (newly created Spaces/Profiles) survive. The secrets move into
                // the store and are thereafter only accessible via a valid
                // WillowWrite cap.
                if this.key_store.is_none() {
                    let key_store = plugin_willow::extract_key_store(
                        vault_content.spaces.iter().map(|s| s.namespace()),
                        vault_content.profiles.iter().map(|p| p.subspace()),
                    );
                    this.key_store = Some(key_store.clone());
                }
                let key_store = this.key_store.clone();

                let write_cap = this.willow_write.clone();

                for space_data in &vault_content.spaces {
                    if !this.unlocked_spaces.contains_key(&space_data.id()) {
                        let space_entity = cx.new(|cx| Space::from_data(space_data.clone(), cx));
                        this.unlocked_spaces.insert(space_data.id(), space_entity);
                    }
                }

                for profile in &vault_content.profiles {
                    if !this.unlocked_profiles.contains_key(&profile.id()) {
                        let home_id = profile.home_space_id();
                        let Some(home_space) = this.unlocked_spaces.get(&home_id) else {
                            warn!("Found Profile with missing Home Space, skipping");
                            continue;
                        };
                        let Some(endpoint) = profile_endpoints.get(&profile.id()).cloned() else {
                            warn!(profile_id = ?profile.id(), "Failed to init endpoint for profile, skipping Profile");
                            continue;
                        };
                        let profile_id = profile.id();
                        let profile = cx.new(|cx| {
                            Profile::from_parts(
                                endpoint,
                                profile.clone(),
                                home_space.clone(),
                                write_cap.clone(),
                                key_store.clone(),
                                cx,
                            )
                        });

                        this.unlocked_profiles.insert(profile_id, profile);
                    }
                }

                if let Some(active_profile_id) = &vault_content.active_profile {
                    let active_profile = this
                        .unlocked_profiles
                        .iter()
                        .find(|&(id, _)| id == active_profile_id)
                        .map(|(_, profile)| profile.clone());
                    this.unlocked_active_profile = active_profile;
                }

                // Gate the content behind the read cap (CapCell). On first load after
                // unlock, `willow_read` builds the cell; on reload, the existing cell's
                // inner is refreshed.
                match (willow_read, this.unlocked_content.take()) {
                    (Some(cap), _) => {
                        this.unlocked_content =
                            Some(CapCell::new(cap, galvanized_cap::wildcard_target(), vault_content));
                    }
                    (None, Some(mut cell)) => {
                        cell.replace_inner(vault_content);
                        this.unlocked_content = Some(cell);
                    }
                    (None, None) => {
                        // No read capability available and no existing cell: the
                        // content must not be held ungated. This happens only if
                        // load_content is called on a vault whose session caps are
                        // gone (e.g. locked with a stale token); the read on the
                        // vault handle would already have failed above. Refuse to
                        // hold ungated content and surface it.
                        warn!("load_content: no read capability; refusing to hold ungated content");
                    }
                }
            })?;

            anyhow::Ok(())
        })
    }

    /// Write the vault's protected content back to the database, if it's unlocked
    fn update_content(
        &mut self,
        cx: &mut Context<Self>,
        update_fn: impl 'static + FnOnce(&mut VaultContent),
    ) -> Task<Result<()>> {
        let load_content_task = self.load_content(cx);
        let vault_handle = self.vault_token.clone().context("vault is not unlocked");

        cx.spawn(async move |this, cx| {
            // Reload the vault's content from the database before updating
            load_content_task.await?;
            let vault_handle = vault_handle?;

            let vault_content_bytes = this
                .update(cx, |this, _cx| {
                    let Some(cell) = this.unlocked_content.as_mut() else {
                        return None;
                    };
                    let Some(vault) = cell.get_mut() else {
                        return None;
                    };
                    update_fn(vault);
                    let vault_bytes =
                        serde_json::to_vec(vault).context("failed to serialize VaultContent");
                    Some(vault_bytes)
                })?
                .transpose()?
                .context("cannot update VaultContent, not loaded or locked")?;

            cx.vaults()
                .update(vault_handle, |mut vault| {
                    *vault.secret() = vault_content_bytes;
                })
                .await?;

            anyhow::Ok(())
        })
    }

    /// Call the give function with the user's secure content, if loaded
    ///
    /// If the content is not loaded, spawns a task to load it, after which subsequent
    /// calls will provide the content.
    pub fn with_content(
        &mut self,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut VaultAccess),
    ) -> Result<()> {
        if let Some(cell) = self.unlocked_content.as_mut() {
            if let Some(vault) = cell.get_mut() {
                let mut content = VaultAccess::new(vault);
                f(&mut content);
                return Ok(());
            }
        }

        let task = cx.spawn(async move |this, cx| {
            let Some(user) = this.upgrade() else {
                bail!("User weak handle has been dropped");
            };

            user.load_content(cx).await?;
            anyhow::Ok(())
        });
        self.unlock_task = Some(task);

        Ok(())
    }

    /// Create a new Communal Space, a Willow Communal Namespace with a display name
    ///
    /// This is side-effectful, after this the space will exist in the user's vault
    pub fn create_communal_space(
        &mut self,
        name: SharedString,
        cx: &mut Context<Self>,
    ) -> Task<Result<Entity<Space>>> {
        self.create_space(name, false, cx)
    }

    /// Create a new Owned Space, a Willow Owned Namespace with a display name
    ///
    /// This is side-effectful, after this the space will exist in the user's vault
    pub fn create_owned_space(
        &mut self,
        name: SharedString,
        cx: &mut Context<Self>,
    ) -> Task<Result<Entity<Space>>> {
        self.create_space(name, true, cx)
    }

    /// Create an owned or communal Space, a Willow namespace with a display name
    fn create_space(
        &mut self,
        name: SharedString,
        owned: bool,
        cx: &mut Context<Self>,
    ) -> Task<Result<Entity<Space>>> {
        cx.spawn(async move |this, cx| {
            let namespace = if owned {
                cx.willow().create_owned_namespace().await?
            } else {
                cx.willow().create_communal_namespace().await?
            };
            let space = cx.new(|cx| Space::new(name, namespace.clone(), cx));

            this.update(cx, {
                let space = space.clone();
                move |this, cx| {
                    // Gate the new namespace's secret behind the same capability surface.
                    if let Some(key_store) = this.key_store.as_mut() {
                        plugin_willow::extend_key_store(key_store, std::iter::once(&namespace), []);
                    }
                    let key = space.read(cx).id();
                    this.unlocked_spaces.insert(key, space.clone());
                    let space_data = space.read(cx).data().clone();
                    let task = this.update_content(cx, move |vault| {
                        vault.spaces.push(space_data);
                    });
                    task
                }
            })?
            .await?;

            // let space = cx.new(move |_cx| space);
            anyhow::Ok(space)
        })
    }

    pub fn spaces(&self) -> Vec<Entity<Space>> {
        self.unlocked_spaces.values().cloned().collect()
    }

    pub fn active_profile(&self) -> Option<Entity<Profile>> {
        self.unlocked_active_profile.clone()
    }

    // TODO capabilities
    pub fn set_active_profile(&mut self, profile: Entity<Profile>, cx: &mut Context<Self>) {
        self.unlocked_active_profile = Some(profile);
        cx.notify();
    }

    pub fn clear_active_profile(&mut self, cx: &mut Context<Self>) {
        self.unlocked_active_profile = None;
        cx.notify();
    }

    pub fn profiles(&self) -> Vec<Entity<Profile>> {
        self.unlocked_profiles.values().cloned().collect()
    }

    /// Create a Profile, a Willow subspace with a display name
    ///
    /// This is side-effectful, after this the Profile will exist in the user's vault
    pub fn create_profile(
        &mut self,
        name: SharedString,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        let home_space_task = self.create_space("Home".into(), true, cx);

        cx.spawn(async move |this, cx| {
            let home_space = home_space_task.await?;
            let subspace = cx.willow().create_subspace().await?;
            let endpoint = Endpoint::builder(N0).bind().await?;

            this.update(cx, |this, cx| {
                // Gate the new subspace's secret behind the same capability surface.
                if let Some(key_store) = this.key_store.as_mut() {
                    plugin_willow::extend_key_store(key_store, [], std::iter::once(&subspace));
                }
                let write_cap = this.willow_write.clone();
                let key_store = this.key_store.clone();
                let profile = cx.new(|cx| {
                    Profile::new(
                        endpoint,
                        subspace.clone(),
                        name,
                        home_space,
                        write_cap,
                        key_store,
                        cx,
                    )
                });

                let key = profile.read(cx).id();
                this.unlocked_profiles.insert(key.clone(), profile.clone());
                this.unlocked_active_profile = Some(profile.clone());
                let profile_data = profile.read(cx).data().clone();
                this.update_content(cx, move |vault| {
                    vault.active_profile = Some(key);
                    vault.profiles.push(profile_data);
                })
            })?
            .await?;

            anyhow::Ok(())
        })
    }
}

pub trait VaultHandle {
    /// Attempts to unlock this [`User`] by unlocking the underlying Vault
    /// with the given password.
    fn unlock<C: AppContext>(&self, cx: &mut C, password: String) -> Task<Result<()>>;

    fn load_content<C: AppContext>(&self, cx: &mut C) -> Task<Result<()>>;

    /// Locks the vault, revoking its session capabilities (T1).
    fn lock<C: AppContext>(&self, cx: &mut C);

    /// Utility function to provide access to the protected [`UserContent`] for rendering
    /// only when the [`User`] and its underlying Vault are unlocked.
    fn when_unlocked<C: AppContext, T>(
        &self,
        it: T,
        cx: &mut C,
        f: impl for<'a> FnOnce(T, &mut Vault, &mut VaultAccess, &'a mut Context<Vault>) -> T,
    ) -> T;
}

impl VaultHandle for Entity<Vault> {
    fn unlock<C: AppContext>(&self, cx: &mut C, password: String) -> Task<Result<()>> {
        cx.update_entity(self, |this, cx| this.unlock(password, cx))
    }

    fn load_content<C: AppContext>(&self, cx: &mut C) -> Task<Result<()>> {
        cx.update_entity(self, |this, cx| this.load_content(cx))
    }

    fn lock<C: AppContext>(&self, cx: &mut C) {
        cx.update_entity(self, |this, cx| this.lock(cx));
    }

    fn when_unlocked<C: AppContext, T>(
        &self,
        item: T,
        cx: &mut C,
        f: impl for<'a> FnOnce(T, &mut Vault, &mut VaultAccess, &'a mut Context<Vault>) -> T,
    ) -> T {
        cx.update_entity(self, |user, cx| {
            // If the user is unlocked and the content is cached, pass it to the caller's function.
            // We take the CapCell out so `f` can borrow `user` without aliasing the
            // cell's borrow, then put it back (mirroring the prior `take`/put-back
            // pattern for `unlocked_vault`). `Result<T, T>` carries `item` through
            // the cap-invalid path so it is restored for the fall-through.
            if let Some(mut cell) = user.unlocked_content.take() {
                let result: Result<T, T> = match cell.get_mut() {
                    Some(vault) => {
                        let mut user_content = VaultAccess::new(vault);
                        Ok(f(item, user, &mut user_content, cx))
                    }
                    None => Err(item),
                };
                user.unlocked_content = Some(cell);
                match result {
                    Ok(new_item) => return new_item,
                    Err(original) => {
                        // Capability invalid (locked/expired): spawn a reload,
                        // which will fail if the vault is locked, surfacing the
                        // re-lock state, and return the original item.
                        let task = cx.spawn(async move |this, cx| {
                            let Some(user) = this.upgrade() else {
                                bail!("User weak handle has been dropped");
                            };
                            user.load_content(cx).await?;
                            anyhow::Ok(())
                        });
                        user.unlock_task = Some(task);
                        return original;
                    }
                }
            }

            let task = cx.spawn(async move |this, cx| {
                let Some(user) = this.upgrade() else {
                    bail!("User weak handle has been dropped");
                };

                user.load_content(cx).await?;
                anyhow::Ok(())
            });
            user.unlock_task = Some(task);

            item
        })
    }
}

/// API object providing Vault behavior that is gated behind vault unlock.
pub struct VaultAccess<'a> {
    vault: &'a mut VaultContent,
}

impl<'a> VaultAccess<'a> {
    fn new(vault: &'a mut VaultContent) -> Self {
        Self { vault }
    }

    pub fn namespaces(&self) -> impl IntoIterator<Item = NamespaceId> {
        self.vault.spaces.iter().map(|ns| ns.id())
    }

    pub fn subspaces(&self) -> impl IntoIterator<Item = SubspaceId> {
        self.vault.profiles.iter().map(|s| s.id())
    }
}

/// Metadata about a user that is visible even when the underlying vault
/// holding the subspace is locked.
#[derive(Debug, Serialize, Deserialize)]
pub struct VaultMetadata {
    /// The name of the user, visible on the user login picker
    user_name: SharedString,
}

impl VaultMetadata {
    /// Create a new [`UserMetadata`] from the given public user name
    pub fn new(user_name: impl Into<SharedString>) -> Self {
        Self {
            user_name: user_name.into(),
        }
    }
}

/// Data structure to serialize the secret content of a [`Vault`]
#[serde_as]
#[derive(derive_more::Debug, Serialize, Deserialize)]
#[debug("VaultContent")]
pub struct VaultContent {
    #[serde(default)]
    spaces: Vec<SpaceData>,

    #[serde(default)]
    profiles: Vec<ProfileData>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde_as(as = "Option<SubspaceIdSerde>")]
    active_profile: Option<SubspaceId>,
}

impl VaultContent {
    pub fn new() -> Self {
        Self {
            spaces: Default::default(),
            profiles: Default::default(),
            active_profile: Default::default(),
        }
    }
}

impl<T> UnlockedVaultView for T where T: IntoElement {}
pub trait UnlockedVaultView {
    fn when_unlocked<C: AppContext>(
        self,
        user: &Entity<Vault>,
        cx: &mut C,
        f: impl FnOnce(Self, &mut Vault, &mut VaultAccess, &mut Context<Vault>) -> Self,
    ) -> Self
    where
        Self: Sized,
    {
        user.when_unlocked(self, cx, |this, user, user_content, cx| {
            f(this, user, user_content, cx)
        })
    }
}
