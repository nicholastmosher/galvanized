use std::sync::Arc;

use anyhow::Result;
use plugin_willow::{Subspace, SubspaceHandle as _, WillowExt};
use serde::{Deserialize, Serialize};
use tracing::info;
use zed::unstable::{
    gpui::{self, AppContext, Entity, Global, Image},
    ui::{App, Context, SharedString},
    util::ResultExt as _,
};

pub fn init(cx: &mut App) {
    let profiles_state = cx.new(|_cx| ProfilesState {});
    cx.set_global(GlobalProfiles(profiles_state));
}

struct GlobalProfiles(Entity<ProfilesState>);
impl Global for GlobalProfiles {}

struct ProfilesState {
    //
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
    pub async fn create(
        &mut self,
        display_name: String,
        password: String,
    ) -> Result<Entity<Profile>> {
        let profile_metadata = ProfileMetadata::new(display_name);
        let subspace = self
            .cx
            .willow()
            .create_subspace(password, Some(&profile_metadata))
            .await?;
        let profile = self
            .cx
            .new(|cx| Profile::from_metadata(profile_metadata, subspace, cx));
        Ok(profile)
    }

    /// Return a list of Profiles stored in the underlying vault
    pub async fn list(&mut self) -> Result<Vec<Entity<Profile>>> {
        let subspaces = self.cx.willow().list_subspaces().await?;

        let profile_metadatas = subspaces
            .into_iter()
            // Skip and log any subspaces that don't have metadata matching Profile
            .filter_map(|subspace| {
                let profile_meta = subspace
                    .read_metadata(&*self.cx, |meta, _cx| {
                        meta.extra()
                            .cloned()
                            .map(|extra| serde_json::from_value::<ProfileMetadata>(extra))
                    })? // ? -> None
                    .log_err()?; // ? -> None
                Some((subspace, profile_meta))
            })
            .collect::<Vec<_>>();

        let profiles = profile_metadatas
            .into_iter()
            .map(|(subspace, metadata)| {
                self.cx
                    .new(|cx| Profile::from_metadata(metadata, subspace, cx))
            })
            .collect::<Vec<_>>();
        info!(?profiles, "profiles.list()");

        Ok(profiles)
    }
}

pub trait ProfileHandle {
    fn with_secrets<C: AppContext, F>(&self, cx: &C, f: F)
    where
        F: FnOnce(&ProfileKey);
}

impl ProfileHandle for Entity<Profile> {
    fn with_secrets<C: AppContext, F>(&self, cx: &C, f: F)
    where
        F: FnOnce(&ProfileKey),
    {
        cx.read_entity(self, |profile, cx| {
            profile.subspace.in_unlock_scope(cx, |subspace| {
                //
            });
        })
    }
}

#[derive(derive_more::Debug)]
pub struct Profile {
    #[debug("Avatar")]
    avatar: Arc<Image>,
    metadata: ProfileMetadata,
    subspace: Entity<Subspace>,
}

/// Private / privileged access to a profile
pub struct ProfileKey {
    //
}

impl Profile {
    pub fn new(display_name: String, subspace: Entity<Subspace>, cx: &mut Context<Self>) -> Self {
        let metadata = ProfileMetadata { display_name };
        Self::from_metadata(metadata, subspace, cx)
    }

    pub fn from_metadata(
        metadata: ProfileMetadata,
        subspace: Entity<Subspace>,
        cx: &mut Context<Self>,
    ) -> Self {
        let id = subspace.id(cx);
        let profile_identicon = plot_icon::generate_png(id.as_bytes(), 512).unwrap();
        let profile_identicon_image = Image::from_bytes(gpui::ImageFormat::Png, profile_identicon);
        let avatar = Arc::new(profile_identicon_image);

        Self {
            avatar,
            metadata,
            subspace,
        }
    }

    pub fn name(&self) -> SharedString {
        SharedString::from(&self.metadata.display_name)
    }

    pub(crate) fn avatar(&self) -> Arc<Image> {
        self.avatar.clone()
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
