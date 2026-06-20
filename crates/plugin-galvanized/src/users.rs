use std::collections::BTreeMap;

use anyhow::{Context as _, Result, bail};
use plugin_vault::{VaultsExt as _, vault_actor::VaultHandle, vault_db::VaultId};
use plugin_willow::{Namespace, Subspace};
use serde::{Deserialize, Serialize};
use tracing::info;
use willow25::entry::{NamespaceId, SubspaceId};
use zed::unstable::{
    gpui::{AppContext, Entity, Global, Task},
    ui::{App, Context, IntoElement, SharedString},
};

pub fn init(cx: &mut App) {
    let users_state = cx.new(|_cx| UsersState::new());
    cx.set_global(GlobalUsers(users_state));
}

/// Global wrapper containing the instance state for the Users plugin
struct GlobalUsers(Entity<UsersState>);
impl Global for GlobalUsers {}

/// Users plugin instance state
struct UsersState {
    users: BTreeMap<VaultId, Entity<User>>,
}

impl UsersState {
    pub fn new() -> Self {
        Self {
            users: Default::default(),
        }
    }
}

/// API handle to the Users plugin, returned by [`cx.users()`]
///
/// This API may be used to create a new [`Users`] from any [`AppContext`] type,
/// and provides general user-related functionality.
///
/// See [`UserHandle`] for a convenience API attached directly to [`Entity<User>`].
pub struct UsersCx<'a, C: AppContext> {
    cx: &'a mut C,
    state: Entity<UsersState>,
}

/// Extension trait for App context objects, provides [`cx.users()`]
pub trait UsersExt {
    type Context: AppContext;
    fn users(&mut self) -> UsersCx<'_, Self::Context>;
}

impl<C: AppContext> UsersExt for C {
    type Context = C;
    fn users(&mut self) -> UsersCx<'_, Self::Context> {
        let state = self.read_global::<GlobalUsers, _>(|it, _cx| it.0.clone());
        UsersCx { cx: self, state }
    }
}

impl<'a, C: AppContext> UsersCx<'a, C> {
    /// Creates a new User from the given display name and password.
    ///
    /// This creates a new backing Vault with the display name used as
    /// metadata to identify the vault, and the password used to encrypt
    /// the future content of the vault, which begins empty.
    pub async fn create(&mut self, display_name: String, password: String) -> Result<Entity<User>> {
        let vault_id = self.cx.vaults().create(password.to_string()).await?;
        let vault_handle = self.cx.vaults().unlock(&vault_id, password).await?;

        let user_metadata = UserMetadata::new(display_name.clone());
        let user_metadata_bytes =
            serde_json::to_vec(&user_metadata).context("failed to serialize user metadata")?;

        let user_vault = UserVault::new();
        let user_vault_bytes =
            serde_json::to_vec(&user_vault).context("failed to serialize user vault content")?;

        // Initial vault should be empty, so we just write to the vault buffers
        self.cx
            .vaults()
            .update(vault_handle, |mut vault| {
                *vault.metadata() = user_metadata_bytes;
                *vault.secret() = user_vault_bytes;
            })
            .await
            .context("failed to write new subspace to vault")?;
        info!(?vault_id, ?display_name, "Wrote user to vault");

        let user = self
            .cx
            .new(|cx| User::new(vault_id.clone(), display_name, cx));
        self.cx.update_entity(&self.state, |state, _cx| {
            state.users.insert(vault_id, user.clone());
        });

        Ok(user)
    }

    /// Attempts to unlock this User by unlocking its underlying Vault
    pub async fn unlock(&mut self, user: &Entity<User>, password: String) -> Result<()> {
        let vault_id = self.cx.read_entity(user, |user, _cx| user.vault_id.clone());

        let vault_handle = self.cx.vaults().unlock(&vault_id, password).await?;
        self.cx.update_entity(user, |user, _cx| {
            user.vault_handle = Some(vault_handle);
        });

        info!("Unlocked user");
        Ok(())
    }

    /// Return a list of Users stored in the underlying vault
    ///
    /// This is async as it drives IO to query the vaults from the underlying database.
    ///
    /// UI elements above this should spawn this only when they need a fresh view of
    /// users, such as after a new [`User`] is created. Otherwise, they should
    /// cache the entities to use while rendering.
    // Implementation notes:
    //
    // - We want to only ever create one `Entity<User>` per vault, ever.
    // - To do this, when listing users by vaults, we check and only create
    //   a new `Entity<User>` for users that don't already exist in our list
    pub async fn list(&mut self) -> Result<Vec<Entity<User>>> {
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
                            serde_json::from_slice::<UserMetadata>(vault.metadata());

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

        // Get the list of Entity<User>, creating new entities ONLY for users that exist in the vault but not yet in memory.
        let users = self.cx.update_entity(&self.state, |state, cx| {
            for (vault_id, metadata) in submetas {
                if !state.users.contains_key(&vault_id) {
                    let user = cx.new(|cx| User::from_metadata(vault_id.clone(), metadata, cx));

                    state.users.insert(vault_id, user);
                }
            }

            state.users.values().cloned().collect()
        });

        info!(?users, "users().list()");
        Ok(users)
    }

    /// If the given [`User`] is unlocked, fetch its content and provide it
    /// to the given function.
    ///
    /// This function is async because it drives IO to fetch and decrypt the user's
    /// content from the underlying vault database. Be sure to cache any retrieved data
    /// to store in UI entities.
    ///
    /// TODO: Create capability-powered caching mechanism that automatically invalidates
    /// fetched cached data on lock.
    async fn load_content(&mut self, user: &Entity<User>) -> Result<()> {
        let vault_handle = self
            .cx
            .read_entity(user, |user, _cx| user.vault_handle.clone())
            .context("user is not unlocked")?;

        let user_vault = self
            .cx
            .vaults()
            .read(vault_handle, |vault| {
                serde_json::from_slice::<UserVault>(vault.secret())
                    .context("failed to deserialize UserVault")
            })
            .await??;

        self.cx.update_entity(user, |user, _cx| {
            user.unlocked_vault = Some(user_vault);
        });

        Ok(())
    }

    /// Write the user's vault-protected content back to the vault, if it's unlocked
    async fn update_content(
        &mut self,
        user: &Entity<User>,
        update_fn: impl FnOnce(&mut UserVault),
    ) -> Result<()> {
        // Reload the user's content from the vault before updating
        self.load_content(user).await?;

        let vault_handle = self
            .cx
            .read_entity(user, |user, _cx| user.vault_handle.clone())
            .context("user's vault is not unlocked")?;

        let user_vault_content = self
            .cx
            .update_entity(user, |user, _cx| {
                let vault = user.unlocked_vault.as_mut()?;
                update_fn(vault);
                let vault_bytes =
                    serde_json::to_vec(vault).context("failed to serialize UserVault");
                Some(vault_bytes)
            })
            .transpose()?
            .context("cannot update UserVault, not loaded")?;

        self.cx
            .vaults()
            .update(vault_handle, |mut vault| {
                *vault.secret() = user_vault_content;
            })
            .await?;

        Ok(())
    }
}

pub trait UserHandle {
    /// Attempts to unlock this [`User`] by unlocking the underlying Vault
    /// with the given password.
    async fn unlock<C: AppContext>(&self, cx: &mut C, password: String) -> Result<()>;

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
    async fn unlock<C: AppContext>(&self, cx: &mut C, password: String) -> Result<()> {
        cx.users().unlock(self, password).await?;
        Ok(())
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

                cx.users().load_content(&user).await?;
                anyhow::Ok(())
            });
            user.unlock_task = Some(task);

            item
        })
    }
}

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

    // TODO: Create a capability-powered caching wrapper, with timeout and revoke
    unlocked_vault: Option<UserVault>,

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
            unlock_task: None,
        }
    }

    /// The name of this [`User`].
    ///
    /// The name is public, and visible even when the user's vault is locked.
    pub fn name(&self) -> SharedString {
        self.metadata.user_name.clone()
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

            cx.users().load_content(&user).await?;
            anyhow::Ok(())
        });
        self.unlock_task = Some(task);

        Ok(())
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
        self.vault.namespaces.iter().map(|ns| ns.namespace.id())
    }

    pub fn subspaces(&self) -> impl IntoIterator<Item = SubspaceId> {
        self.vault.subspaces.iter().map(|ss| ss.subspace.id())
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
    namespaces: Vec<UserNamespace>,
    subspaces: Vec<UserSubspace>,
}

#[derive(Serialize, Deserialize)]
struct UserNamespace {
    name: SharedString,
    namespace: Namespace,
}

#[derive(Serialize, Deserialize)]
struct UserSubspace {
    name: SharedString,
    subspace: Subspace,
}

impl UserVault {
    pub fn new() -> Self {
        Self {
            namespaces: Default::default(),
            subspaces: Default::default(),
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
