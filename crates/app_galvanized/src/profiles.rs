use std::sync::Arc;

use anyhow::{Context as _, Result};
use plugin_vault::{VaultsExt as _, vault_actor::VaultHandle, vault_db::VaultId};
use plugin_willow::{Subspace, SubspaceHandle as _, WillowExt};
use serde::{Deserialize, Serialize};
use tracing::info;
use zed::unstable::{
    gpui::{self, AppContext, Entity, Global, Image},
    ui::{App, Context, SharedString},
    util::ResultExt as _,
};

pub fn init(cx: &mut App) {
    let profiles_state = cx.new(|_cx| ProfilesState::new());
    cx.set_global(GlobalProfiles(profiles_state));
}

struct GlobalProfiles(Entity<ProfilesState>);
impl Global for GlobalProfiles {}

struct ProfilesState {
    //
}

impl ProfilesState {
    pub fn new() -> Self {
        Self {}
    }
}

pub struct ProfilesCx<'a, C: AppContext> {
    cx: &'a mut C,
    state: Entity<ProfilesState>,
}

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

    pub async fn create_subspace(&mut self, profile: &Entity<Profile>) {
        //
    }

    /// Attempts to log into this Profile by unlocking its underlying Willow Subspace
    pub async fn login(&mut self, profile: &Entity<Profile>, password: String) -> Result<()> {
        let (vault_id, vault_handle) = self.cx.read_entity(profile, |profile, cx| {
            (profile.vault_id.clone(), profile.vault_handle.clone())
        });
        self.cx.vaults().unlock(&vault_id, password);

        // let subspace = self
        //     .cx
        //     .read_entity(profile, |profile, _cx| profile.subspace.clone());

        // self.cx
        //     .willow()
        //     .unlock_subspace(&subspace, password)
        //     .await?;

        info!("Unlocked subspace");
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

#[derive(derive_more::Debug)]
pub struct Profile {
    metadata: ProfileMetadata,
    vault_id: VaultId,
    vault_handle: Option<VaultHandle>,
}

/// Private / privileged access to a profile
pub struct UnlockedProfile {
    //
}

impl UnlockedProfile {
    //
}

impl Profile {
    pub fn new(vault_id: VaultId, display_name: String, cx: &mut Context<Self>) -> Self {
        let metadata = ProfileMetadata::new(display_name);
        Self::from_metadata(vault_id, metadata, cx)
    }

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

    pub fn name(&self) -> SharedString {
        SharedString::from(&self.metadata.display_name)
    }
}

/// Metadata about a profile that is visible even when the underlying vault
/// holding the subspace is locked.
#[derive(Debug, Serialize, Deserialize)]
pub struct ProfileMetadata {
    display_name: String,
}

impl ProfileMetadata {
    pub fn new(display_name: String) -> Self {
        Self { display_name }
    }
}

/// Profile's contents that are locked behind the vault
#[derive(derive_more::Debug, Serialize, Deserialize)]
#[debug("ProfileVault")]
pub struct ProfileVault {
    //
}

impl ProfileVault {
    pub fn new() -> Self {
        Self {}
    }
}
