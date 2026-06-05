use anyhow::{Context as _, Result};
use plugin_vault::{VaultsExt as _, vault_actor::VaultHandle, vault_db::VaultId};
use plugin_willow::willow_serde::{namespace_id::NamespaceIdDef, subspace_id::SubspaceIdDef};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use tracing::info;
use willow25::entry::{NamespaceId, NamespaceSecret, SubspaceId};
use zed::unstable::{
    gpui::{self, AppContext, Entity, Global, Image},
    ui::{App, Context, SharedString},
};

pub fn init(cx: &mut App) {
    let profiles_state = cx.new(|_cx| ProfilesState::new());
    cx.set_global(GlobalProfiles(profiles_state));
}

/// Global wrapper containing the instance state for the Profiles plugin
struct GlobalProfiles(Entity<ProfilesState>);
impl Global for GlobalProfiles {}

/// Profiles plugin instance state
struct ProfilesState {}

impl ProfilesState {
    pub fn new() -> Self {
        Self {}
    }
}

/// API handle to the Profiles plugin, returned by [`cx.profiles()`]
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

impl<C: AppContext> ProfilesCx<'_, C> {
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

        let profile = self.cx.new(|cx| Profile::new(vault_id, display_name, cx));
        Ok(profile)
    }

    /// Attempts to log into this Profile by unlocking its underlying Willow Subspace
    pub async fn login(&mut self, profile: &Entity<Profile>, password: String) -> Result<()> {
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

        let profiles = submetas
            .into_iter()
            .map(|(vault_id, metadata)| {
                self.cx
                    .new(|cx| Profile::from_metadata(vault_id, metadata, cx))
            })
            .collect::<Vec<_>>();

        info!(?profiles, "profiles().list()");
        Ok(profiles)
    }
}

pub trait ProfileHandle {
    async fn login<C: AppContext>(&self, cx: &mut C, password: String) -> Result<()>;
}

impl ProfileHandle for Entity<Profile> {
    async fn login<C: AppContext>(&self, cx: &mut C, password: String) -> Result<()> {
        cx.profiles().login(self, password).await?;
        Ok(())
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

    /// If unlocked, the handle to the Vault.
    ///
    /// The handle carries the capability to read and write to the vault, and
    /// is obtained by unlocking the vault. A handle's capabilities may expire
    /// over time or be revoked.
    vault_handle: Option<VaultHandle>,
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
        }
    }

    /// The name of this [`Profile`].
    ///
    /// The name is public, and visible even when the profile's vault is locked.
    pub fn name(&self) -> SharedString {
        self.metadata.profile_name.clone()
    }
}

/// API object providing Profile behavior that is gated behind vault unlock.
pub struct UnlockedProfile {
    //
}

impl UnlockedProfile {
    //
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

/// Data structure to serialize the secret content of a [`Profile`]
#[serde_as]
#[derive(derive_more::Debug, Serialize, Deserialize)]
#[debug("ProfileVault")]
pub struct ProfileVault {
    #[serde_as(as = "Vec<NamespaceIdDef>")]
    namespaces: Vec<NamespaceId>,
    #[serde_as(as = "Vec<SubspaceIdDef>")]
    subspaces: Vec<SubspaceId>,
}

impl ProfileVault {
    pub fn new() -> Self {
        Self {
            namespaces: Default::default(),
            subspaces: Default::default(),
        }
    }
}
