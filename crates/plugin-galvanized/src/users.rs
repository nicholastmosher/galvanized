use std::collections::BTreeMap;

use anyhow::{Context as _, Result, bail};
use plugin_vault::{VaultsExt as _, vault_actor::VaultHandle, vault_db::VaultId};
use plugin_willow::{Namespace, Subspace, WillowExt};
use serde::{Deserialize, Serialize};
use tracing::info;
use willow25::entry::{NamespaceId, SubspaceId};
use zed::unstable::{
    gpui::{AppContext, Entity, Task},
    ui::{Context, IntoElement, SharedString},
};

/// A User is conceptually the representation of a user that owns a Vault.
///
/// A User represents a user and their access to Willow data.
///
/// A User is backed by a vault which stores Willow namespace and subspace
/// keys. One User may control any number of namespace and/or subspace keys.
///
/// A User also provides protected access to the Willow data it owns.
/// Willow data access is gated behind the User's vault being unlocked.
#[derive(derive_more::Debug)]
pub struct User {
    /// Public metadata about the user, visible while the vault is locked.
    metadata: UserMetadata,

    /// The ID of the vault underlying this User.
    vault_id: VaultId,

    /// If unlocked, the handle to the Vault that stores this User's data.
    ///
    /// The handle carries the capability to read and write to the vault, and
    /// is obtained by unlocking the vault. A handle's capabilities may expire
    /// over time or be revoked.
    vault_handle: Option<VaultHandle>,

    // TODO: Create a capability-powered caching wrapper, with timeout and revoke {
    unlocked_vault: Option<UserVault>,
    unlocked_spaces: BTreeMap<NamespaceId, Entity<Space>>,
    unlocked_profiles: BTreeMap<SubspaceId, Entity<Profile>>,
    // } END TODO: capability-powered caching
    unlock_task: Option<Task<Result<()>>>,
}

impl User {
    /// Create a new User from the given [`VaultId`] and user name.
    pub fn new(vault_id: VaultId, user_name: String, cx: &mut Context<Self>) -> Self {
        let metadata = UserMetadata::new(user_name);
        Self::from_metadata(vault_id, metadata, cx)
    }

    /// Create a new User using the given vault and user metadata.
    ///
    /// User metadata is public, and visible when the user's vault is locked.
    pub fn from_metadata(
        vault_id: VaultId,
        metadata: UserMetadata,
        _cx: &mut Context<Self>,
    ) -> Self {
        Self {
            metadata,
            vault_id,
            vault_handle: None,
            unlocked_vault: None,
            unlocked_spaces: Default::default(),
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

    /// Attempts to unlock this User by unlocking its underlying Vault
    pub fn unlock(&mut self, password: String, cx: &mut Context<Self>) -> Task<Result<()>> {
        let vault_id = self.vault_id.clone();
        cx.spawn(async move |this, cx| {
            let vault_handle = cx.vaults().unlock(&vault_id, password).await?;

            this.update(cx, |this, _cx| {
                this.vault_handle = Some(vault_handle);
            })?;

            info!("Unlocked user");
            Ok(())
        })
    }

    /// If the given [`User`] is unlocked, fetch its content and store it in the entity
    ///
    /// This function is async because it drives IO to fetch and decrypt the user's
    /// content from the underlying vault database. Be sure to cache any retrieved data
    /// to store in UI entities.
    ///
    // TODO: Create capability-powered caching mechanism that automatically invalidates
    // fetched cached data on lock.
    pub fn load_content(&mut self, cx: &mut Context<Self>) -> Task<Result<()>> {
        let vault_handle = self.vault_handle.clone().context("user is not unlocked");

        cx.spawn(async move |this, cx| {
            let vault_handle = vault_handle?;

            let user_vault = cx
                .vaults()
                .read(vault_handle, |vault| {
                    serde_json::from_slice::<UserVault>(vault.secret())
                        .context("failed to deserialize UserVault")
                })
                .await??;

            this.update(cx, |this, cx| {
                for space in &user_vault.spaces {
                    if !this.unlocked_spaces.contains_key(&space.id()) {
                        let space_entity = cx.new(|_cx| space.clone());
                        this.unlocked_spaces.insert(space.id(), space_entity);
                    }
                }
                for profile in &user_vault.profiles {
                    if !this.unlocked_profiles.contains_key(&profile.id()) {
                        let profile_entity = cx.new(|_cx| profile.clone());
                        this.unlocked_profiles.insert(profile.id(), profile_entity);
                    }
                }
                this.unlocked_vault = Some(user_vault);
            })?;

            anyhow::Ok(())
        })
    }

    /// Write the user's vault-protected content back to the vault, if it's unlocked
    fn update_content(
        &mut self,
        cx: &mut Context<Self>,
        update_fn: impl 'static + FnOnce(&mut UserVault),
    ) -> Task<Result<()>> {
        let load_content_task = self.load_content(cx);
        let vault_handle = self
            .vault_handle
            .clone()
            .context("user's vault is not unlocked");

        cx.spawn(async move |this, cx| {
            // Reload the user's content from the vault before updating
            load_content_task.await?;
            let vault_handle = vault_handle?;

            let user_vault_content = this
                .update(cx, |this, _cx| {
                    let vault = this.unlocked_vault.as_mut()?;
                    update_fn(vault);
                    let vault_bytes =
                        serde_json::to_vec(vault).context("failed to serialize UserVault");
                    Some(vault_bytes)
                })?
                .transpose()?
                .context("cannot update UserVault, not loaded")?;

            cx.vaults()
                .update(vault_handle, |mut vault| {
                    *vault.secret() = user_vault_content;
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
        f: impl FnOnce(&mut UserContent),
    ) -> Result<()> {
        if let Some(mut vault) = self.unlocked_vault.take() {
            let mut content = UserContent::new(&mut vault);
            f(&mut content);
            self.unlocked_vault = Some(vault);
            return Ok(());
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
    ) -> Task<Result<()>> {
        self.create_space(name, false, cx)
    }

    /// Create a new Owned Space, a Willow Owned Namespace with a display name
    ///
    /// This is side-effectful, after this the space will exist in the user's vault
    pub fn create_owned_space(
        &mut self,
        name: SharedString,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        self.create_space(name, true, cx)
    }

    /// Create an owned or communal Space, a Willow namespace with a display name
    ///
    /// This is side-effectful, after this the space will exist in the user's vault
    fn create_space(
        &mut self,
        name: SharedString,
        owned: bool,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        cx.spawn(async move |this, cx| {
            let namespace = if owned {
                cx.willow().create_owned_namespace().await?
            } else {
                cx.willow().create_communal_namespace().await?
            };
            let space = Space::new(name, namespace);

            this.update(cx, {
                move |this, cx| {
                    let key = space.id();
                    let space_entity = cx.new(|_cx| space.clone());
                    this.unlocked_spaces.insert(key, space_entity);
                    this.update_content(cx, move |vault| {
                        vault.spaces.push(space);
                    })
                }
            })?
            .await?;

            anyhow::Ok(())
        })
    }

    pub fn spaces(&self) -> Vec<Entity<Space>> {
        self.unlocked_spaces.values().cloned().collect()
    }

    /// Create a Profile, a Willow subspace with a display name
    ///
    /// This is side-effectful, after this the Profile will exist in the user's vault
    pub fn create_profile(
        &mut self,
        name: SharedString,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        cx.spawn(async move |this, cx| {
            let subspace = cx.willow().create_subspace().await?;
            let profile = Profile::new(name, subspace);

            this.update(cx, {
                move |this, cx| {
                    this.update_content(cx, move |vault| {
                        vault.profiles.push(profile);
                    })
                }
            })?
            .await?;

            anyhow::Ok(())
        })
    }
}

pub trait UserHandle {
    /// Attempts to unlock this [`User`] by unlocking the underlying Vault
    /// with the given password.
    fn unlock<C: AppContext>(&self, cx: &mut C, password: String) -> Task<Result<()>>;

    fn load_content<C: AppContext>(&self, cx: &mut C) -> Task<Result<()>>;

    /// Utility function to provide access to the protected [`UserContent`] for rendering
    /// only when the [`User`] and its underlying Vault are unlocked.
    fn when_unlocked<C: AppContext, T>(
        &self,
        it: T,
        cx: &mut C,
        f: impl for<'a> FnOnce(T, &mut User, &mut UserContent, &'a mut Context<User>) -> T,
    ) -> T;
}

impl UserHandle for Entity<User> {
    fn unlock<C: AppContext>(&self, cx: &mut C, password: String) -> Task<Result<()>> {
        cx.update_entity(self, |this, cx| this.unlock(password, cx))
    }

    fn load_content<C: AppContext>(&self, cx: &mut C) -> Task<Result<()>> {
        cx.update_entity(self, |this, cx| this.load_content(cx))
    }

    fn when_unlocked<C: AppContext, T>(
        &self,
        item: T,
        cx: &mut C,
        f: impl for<'a> FnOnce(T, &mut User, &mut UserContent, &'a mut Context<User>) -> T,
    ) -> T {
        cx.update_entity(self, |user, cx| {
            // If the user is unlocked and the content is cached, pass it to the caller's function
            //
            // We take the vault and replace it when we're done to avoid double borrowing
            if let Some(mut user_vault) = user.unlocked_vault.take() {
                let mut user_content = UserContent::new(&mut user_vault);
                let item = f(item, user, &mut user_content, cx);
                user.unlocked_vault = Some(user_vault);
                return item;
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

/// API object providing User behavior that is gated behind vault unlock.
pub struct UserContent<'a> {
    vault: &'a mut UserVault,
}

impl<'a> UserContent<'a> {
    fn new(vault: &'a mut UserVault) -> Self {
        Self { vault }
    }

    pub fn namespaces(&self) -> impl IntoIterator<Item = NamespaceId> {
        self.vault.spaces.iter().map(|ns| ns.namespace.id())
    }

    pub fn subspaces(&self) -> impl IntoIterator<Item = SubspaceId> {
        self.vault.profiles.iter().map(|ss| ss.subspace.id())
    }
}

/// Metadata about a user that is visible even when the underlying vault
/// holding the subspace is locked.
#[derive(Debug, Serialize, Deserialize)]
pub struct UserMetadata {
    /// The name of the user, visible on the user login picker
    user_name: SharedString,
}

impl UserMetadata {
    /// Create a new [`UserMetadata`] from the given public user name
    pub fn new(user_name: impl Into<SharedString>) -> Self {
        Self {
            user_name: user_name.into(),
        }
    }
}

/// Data structure to serialize the secret content of a [`User`]
#[derive(derive_more::Debug, Serialize, Deserialize)]
#[debug("UserVault")]
pub struct UserVault {
    spaces: Vec<Space>,
    profiles: Vec<Profile>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Space {
    name: SharedString,
    namespace: Namespace,
}

impl Space {
    pub fn new(name: SharedString, namespace: Namespace) -> Self {
        Self { name, namespace }
    }

    pub fn id(&self) -> NamespaceId {
        self.namespace.id()
    }

    pub fn name(&self) -> SharedString {
        self.name.clone()
    }

    pub fn is_communal(&self) -> bool {
        self.namespace.is_communal()
    }

    pub fn is_owned(&self) -> bool {
        self.namespace.is_owned()
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Profile {
    name: SharedString,
    subspace: Subspace,
}

impl Profile {
    pub fn new(name: SharedString, subspace: Subspace) -> Self {
        Self { name, subspace }
    }

    pub fn id(&self) -> SubspaceId {
        self.subspace.id()
    }
}

impl UserVault {
    pub fn new() -> Self {
        Self {
            spaces: Default::default(),
            profiles: Default::default(),
        }
    }
}

impl<T> UnlockedUserView for T where T: IntoElement {}
pub trait UnlockedUserView {
    fn when_unlocked<C: AppContext>(
        self,
        user: &Entity<User>,
        cx: &mut C,
        f: impl FnOnce(Self, &mut User, &mut UserContent, &mut Context<User>) -> Self,
    ) -> Self
    where
        Self: Sized,
    {
        user.when_unlocked(self, cx, |this, user, user_content, cx| {
            f(this, user, user_content, cx)
        })
    }
}

fn thing(user: &Entity<User>, cx: &mut Context<User>) -> impl IntoElement {
    use zed::unstable::ui::div;

    div()
        //
        .when_unlocked(user, cx, |el, user, content, cx| {
            //
            el
        })
}
