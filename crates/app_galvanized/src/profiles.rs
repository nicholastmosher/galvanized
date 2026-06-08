use std::collections::BTreeMap;

use anyhow::{Context as _, Result, bail};
use plugin_vault::{VaultsExt as _, vault_actor::VaultHandle, vault_db::VaultId};
use plugin_willow::willow_serde::{NamespaceSecretSerde, SubspaceSecretSerde};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use tracing::info;
use willow25::entry::{NamespaceId, NamespaceSecret, SubspaceId, SubspaceSecret};
use zed::unstable::{
    gpui::{AppContext, Entity, Global, Task},
    ui::{App, Context, IntoElement, SharedString},
};

pub fn init(cx: &mut App) {
    let profiles_state = cx.new(|_cx| ProfilesState::new());
    cx.set_global(GlobalProfiles(profiles_state));
}

/// Global wrapper containing the instance state for the Profiles plugin
struct GlobalProfiles(Entity<ProfilesState>);
impl Global for GlobalProfiles {}

/// Profiles plugin instance state
struct ProfilesState {
    profiles: BTreeMap<VaultId, Entity<Profile>>,
}

impl ProfilesState {
    pub fn new() -> Self {
        Self {
            profiles: Default::default(),
        }
    }
}

/// API handle to the Profiles plugin, returned by [`cx.profiles()`]
///
/// This API may be used to create a new [`Profile`] from any [`AppContext`] type,
/// and provides general profile-related functionality.
///
/// See [`ProfileHandle`] for a convenience API attached directly to [`Entity<Profile>`].
pub struct ProfilesCx<'a, C: AppContext> {
    cx: &'a mut C,
    state: Entity<ProfilesState>,
}

/// Extension trait for App context objects, provides [`cx.profiles()`]
pub trait ProfilesExt {
    type Context: AppContext;
    fn profiles(&mut self) -> ProfilesCx<'_, Self::Context>;
}

impl<C: AppContext> ProfilesExt for C {
    type Context = C;
    fn profiles(&mut self) -> ProfilesCx<'_, Self::Context> {
        let state = self.read_global::<GlobalProfiles, _>(|it, _cx| it.0.clone());
        ProfilesCx { cx: self, state }
    }
}

impl<'a, C: AppContext> ProfilesCx<'a, C> {
    /// Creates a new Profile from the given display name and password.
    ///
    /// This creates a new backing Vault with the display name used as
    /// metadata to identify the vault, and the password used to encrypt
    /// the future content of the vault, which begins empty.
    pub async fn create(
        &mut self,
        display_name: String,
        password: String,
    ) -> Result<Entity<Profile>> {
        let vault_id = self.cx.vaults().create(password.to_string()).await?;
        let vault_handle = self.cx.vaults().unlock(&vault_id, password).await?;

        let profile_metadata = ProfileMetadata::new(display_name.clone());
        let profile_metadata_bytes = serde_json::to_vec(&profile_metadata)
            .context("failed to serialize profile metadata")?;

        let profile_vault = ProfileVault::new();
        let profile_vault_bytes = serde_json::to_vec(&profile_vault)
            .context("failed to serialize profile vault content")?;

        // Initial vault should be empty, so we just write to the vault buffers
        self.cx
            .vaults()
            .update(vault_handle, |mut vault| {
                *vault.metadata() = profile_metadata_bytes;
                *vault.secret() = profile_vault_bytes;
            })
            .await
            .context("failed to write new subspace to vault")?;
        info!(?vault_id, ?display_name, "Wrote profile to vault");

        let profile = self
            .cx
            .new(|cx| Profile::new(vault_id.clone(), display_name, cx));
        self.cx.update_entity(&self.state, |state, _cx| {
            state.profiles.insert(vault_id, profile.clone());
        });

        Ok(profile)
    }

    /// Attempts to unlock this Profile by unlocking its underlying Vault
    pub async fn unlock(&mut self, profile: &Entity<Profile>, password: String) -> Result<()> {
        let vault_id = self
            .cx
            .read_entity(profile, |profile, _cx| profile.vault_id.clone());

        let vault_handle = self.cx.vaults().unlock(&vault_id, password).await?;
        self.cx.update_entity(profile, |profile, _cx| {
            profile.vault_handle = Some(vault_handle);
        });

        info!("Unlocked profile");
        Ok(())
    }

    /// Return a list of Profiles stored in the underlying vault
    ///
    /// This is async as it drives IO to query the vaults from the underlying database.
    ///
    /// UI elements above this should spawn this only when they need a fresh view of
    /// profiles, such as after a new [`Profile`] is created. Otherwise, they should
    /// cache the entities to use while rendering.
    // Implementation notes:
    //
    // - We want to only ever create one `Entity<Profile>` per vault, ever.
    // - To do this, when listing profiles by vaults, we check and only create
    //   a new `Entity<Profile>` for profiles that don't already exist in our list
    pub async fn list(&mut self) -> Result<Vec<Entity<Profile>>> {
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
                            serde_json::from_slice::<ProfileMetadata>(vault.metadata());

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

        // Get the list of Entity<Profile>, creating new entities ONLY for profiles that exist in the vault but not yet in memory.
        let profiles = self.cx.update_entity(&self.state, |state, cx| {
            for (vault_id, metadata) in submetas {
                if !state.profiles.contains_key(&vault_id) {
                    let profile =
                        cx.new(|cx| Profile::from_metadata(vault_id.clone(), metadata, cx));

                    state.profiles.insert(vault_id, profile);
                }
            }

            state.profiles.values().cloned().collect()
        });

        info!(?profiles, "profiles().list()");
        Ok(profiles)
    }

    /// If the given [`Profile`] is unlocked, fetch its content and provide it
    /// to the given function.
    ///
    /// This function is async because it drives IO to fetch and decrypt the profile's
    /// content from the underlying vault database. Be sure to cache any retrieved data
    /// to store in UI entities.
    ///
    /// TODO: Create capability-powered caching mechanism that automatically invalidates
    /// fetched cached data on lock.
    async fn load_content(&mut self, profile: &Entity<Profile>) -> Result<()> {
        let vault_handle = self
            .cx
            .read_entity(profile, |profile, _cx| profile.vault_handle.clone())
            .context("profile is not unlocked")?;

        let profile_vault = self
            .cx
            .vaults()
            .read(vault_handle, |vault| {
                serde_json::from_slice::<ProfileVault>(vault.secret())
                    .context("failed to deserialize ProfileVault")
            })
            .await??;

        self.cx.update_entity(profile, |profile, _cx| {
            profile.unlocked_vault = Some(profile_vault);
        });

        Ok(())
    }
}

pub trait ProfileHandle {
    /// Attempts to unlock this [`Profile`] by unlocking the underlying Vault
    /// with the given password.
    async fn unlock<C: AppContext>(&self, cx: &mut C, password: String) -> Result<()>;

    /// Utility function to provide access to the protected [`ProfileContent`] for rendering
    /// only when the [`Profile`] and its underlying Vault are unlocked.
    fn when_unlocked<C: AppContext, T>(
        &self,
        it: T,
        cx: &mut C,
        f: impl for<'a> FnOnce(T, &mut Profile, &mut ProfileContent, &'a mut Context<Profile>) -> T,
    ) -> T;
}

impl ProfileHandle for Entity<Profile> {
    async fn unlock<C: AppContext>(&self, cx: &mut C, password: String) -> Result<()> {
        cx.profiles().unlock(self, password).await?;
        Ok(())
    }

    fn when_unlocked<C: AppContext, T>(
        &self,
        item: T,
        cx: &mut C,
        f: impl for<'a> FnOnce(T, &mut Profile, &mut ProfileContent, &'a mut Context<Profile>) -> T,
    ) -> T {
        cx.update_entity(self, |profile, cx| {
            // If the profile is unlocked and the content is cached, pass it to the caller's function
            //
            // We take the vault and replace it when we're done to avoid double borrowing
            if let Some(mut profile_vault) = profile.unlocked_vault.take() {
                let mut profile_content = ProfileContent::new(&mut profile_vault);
                let item = f(item, profile, &mut profile_content, cx);
                profile.unlocked_vault = Some(profile_vault);
                return item;
            }

            let task = cx.spawn(async move |this, cx| {
                let Some(profile) = this.upgrade() else {
                    bail!("Profile weak handle has been dropped");
                };

                cx.profiles().load_content(&profile).await?;
                anyhow::Ok(())
            });
            profile.unlock_task = Some(task);

            item
        })
    }
}

/// A Profile is conceptually the representation of a user that owns a Vault.
///
/// A Profile represents a user and their access to Willow data.
///
/// A Profile is backed by a vault which stores Willow namespace and subspace
/// keys. One Profile may control any number of namespace and/or subspace keys.
///
/// A Profile also provides protected access to the Willow data it owns.
/// Willow data access is gated behind the Profile's vault being unlocked.
#[derive(derive_more::Debug)]
pub struct Profile {
    /// Public metadata about the profile, visible while the vault is locked.
    metadata: ProfileMetadata,

    /// The ID of the vault underlying this Profile.
    vault_id: VaultId,

    /// If unlocked, the handle to the Vault that stores this Profile's data.
    ///
    /// The handle carries the capability to read and write to the vault, and
    /// is obtained by unlocking the vault. A handle's capabilities may expire
    /// over time or be revoked.
    vault_handle: Option<VaultHandle>,

    // TODO: Create a capability-powered caching wrapper, with timeout and revoke
    unlocked_vault: Option<ProfileVault>,

    unlock_task: Option<Task<Result<()>>>,
}

impl Profile {
    /// Create a new Profile from the given [`VaultId`] and profile name.
    pub fn new(vault_id: VaultId, profile_name: String, cx: &mut Context<Self>) -> Self {
        let metadata = ProfileMetadata::new(profile_name);
        Self::from_metadata(vault_id, metadata, cx)
    }

    /// Create a new Profile using the given vault and profile metadata.
    ///
    /// Profile metadata is public, and visible when the profile's vault is locked.
    pub fn from_metadata(
        vault_id: VaultId,
        metadata: ProfileMetadata,
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

    /// The name of this [`Profile`].
    ///
    /// The name is public, and visible even when the profile's vault is locked.
    pub fn name(&self) -> SharedString {
        self.metadata.profile_name.clone()
    }

    /// Call the give function with the profile's secure content, if loaded
    ///
    /// If the content is not loaded, spawns a task to load it, after which subsequent
    /// calls will provide the content.
    pub fn with_content(
        &mut self,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut ProfileContent),
    ) -> Result<()> {
        if let Some(mut vault) = self.unlocked_vault.take() {
            let mut content = ProfileContent::new(&mut vault);
            f(&mut content);
            self.unlocked_vault = Some(vault);
            return Ok(());
        }

        let task = cx.spawn(async move |this, cx| {
            let Some(profile) = this.upgrade() else {
                bail!("Profile weak handle has been dropped");
            };

            cx.profiles().load_content(&profile).await?;
            anyhow::Ok(())
        });
        self.unlock_task = Some(task);

        Ok(())
    }
}

/// API object providing Profile behavior that is gated behind vault unlock.
pub struct ProfileContent<'a> {
    vault: &'a mut ProfileVault,
}

impl<'a> ProfileContent<'a> {
    fn new(vault: &'a mut ProfileVault) -> Self {
        Self { vault }
    }

    pub fn namespaces(&self) -> Vec<NamespaceId> {
        self.vault.namespaces.iter().map(|ns| ns.id()).collect()
    }

    pub fn subspaces(&self) -> Vec<SubspaceId> {
        self.vault.subspaces.iter().map(|s| s.id()).collect()
    }
}

/// Metadata about a profile that is visible even when the underlying vault
/// holding the subspace is locked.
#[derive(Debug, Serialize, Deserialize)]
pub struct ProfileMetadata {
    /// The name of the profile, visible on the profile login picker
    profile_name: SharedString,
}

impl ProfileMetadata {
    /// Create a new [`ProfileMetadata`] from the given public profile name
    pub fn new(profile_name: impl Into<SharedString>) -> Self {
        Self {
            profile_name: profile_name.into(),
        }
    }
}

/// Namespace data that gets seriazlied and stored in a user's vault
#[serde_as]
#[derive(derive_more::Debug, Serialize, Deserialize)]
pub struct Namespace {
    name: SharedString,
    #[debug("NamespaceSecret")]
    #[serde_as(as = "NamespaceSecretSerde")]
    secret: NamespaceSecret,
}

impl Namespace {
    /// Returns the [`NamespaceId`] of this namespace
    pub fn id(&self) -> NamespaceId {
        self.secret.corresponding_namespace_id()
    }
}

#[serde_as]
#[derive(derive_more::Debug, Serialize, Deserialize)]
pub struct Subspace {
    name: SharedString,
    #[debug("SubspaceSecret")]
    #[serde_as(as = "SubspaceSecretSerde")]
    secret: SubspaceSecret,
}

impl Subspace {
    /// Returns the [`SubspaceId`] of this subspace
    pub fn id(&self) -> SubspaceId {
        self.secret.corresponding_subspace_id()
    }
}

/// Data structure to serialize the secret content of a [`Profile`]
#[derive(derive_more::Debug, Serialize, Deserialize)]
#[debug("ProfileVault")]
pub struct ProfileVault {
    namespaces: Vec<Namespace>,
    subspaces: Vec<Subspace>,
}

impl ProfileVault {
    pub fn new() -> Self {
        Self {
            namespaces: Default::default(),
            subspaces: Default::default(),
        }
    }
}

impl<T> UnlockedProfileView for T where T: IntoElement {}
pub trait UnlockedProfileView {
    fn when_unlocked<C: AppContext>(
        self,
        profile: &Entity<Profile>,
        cx: &mut C,
        f: impl FnOnce(Self, &mut Profile, &mut ProfileContent, &mut Context<Profile>) -> Self,
    ) -> Self
    where
        Self: Sized,
    {
        profile.when_unlocked(self, cx, |this, profile, profile_content, cx| {
            f(this, profile, profile_content, cx)
        })
    }
}

fn thing(profile: &Entity<Profile>, cx: &mut Context<Profile>) -> impl IntoElement {
    use zed::unstable::ui::div;

    div()
        //
        .when_unlocked(profile, cx, |el, profile, content, cx| {
            //
            el
        })
}
