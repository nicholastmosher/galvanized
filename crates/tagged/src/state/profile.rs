use std::{
    fmt::LowerHex,
    path::{Path, PathBuf},
};

use hex::ToHex as _;
use willow25::entry::{SubspaceId, SubspaceSecret, randomly_generate_subspace};
use zed::unstable::{
    gpui::Entity,
    ui::{App, Context, SharedString},
};

pub fn init(_cx: &mut App) {
    //
}

/// Data-only manager for profiles
pub struct ProfileManager {
    //
    profiles: Vec<Entity<Profile>>,
}

impl ProfileManager {
    pub fn new() -> Self {
        Self {
            profiles: Vec::new(),
        }
    }

    pub fn add_profile(&mut self, profile: Entity<Profile>) {
        self.profiles.push(profile);
    }
}

pub struct ProfileKey(SubspaceSecret);
impl ProfileKey {
    // TODO: plumb RNG? or is this fine
    pub fn new() -> Self {
        let (_subspace_id, sub_secret) = randomly_generate_subspace(&mut rand_core_0_6_4::OsRng);
        ProfileKey(sub_secret)
    }

    pub fn id(&self) -> ProfileId {
        ProfileId(self.0.corresponding_subspace_id())
    }
}
impl From<SubspaceSecret> for ProfileKey {
    fn from(value: SubspaceSecret) -> Self {
        Self(value)
    }
}
pub struct ProfileId(SubspaceId);
impl LowerHex for ProfileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hex: String = self.0.to_bytes().encode_hex();
        write!(f, "{hex}")
    }
}

// data object only
pub struct Profile {
    /// Path to the avatar image.
    avatar: Option<PathBuf>,

    // TODO: Need a protected wrapper API, like `SecretEntity<T>` or such
    key: SubspaceSecret,
    name: SharedString,
    online: bool,
}

impl Profile {
    pub fn new(
        name: impl Into<SharedString>,
        key: SubspaceSecret,
        _cx: &mut Context<Self>,
    ) -> Self {
        Self {
            //
            avatar: None,
            key,
            name: name.into(),
            online: true,
        }
    }

    pub fn id(&self) -> ProfileId {
        ProfileId(self.key.corresponding_subspace_id())
    }

    pub fn name(&self) -> SharedString {
        self.name.clone()
    }

    pub fn with_avatar(mut self, avatar: impl Into<PathBuf>) -> Self {
        self.avatar = Some(avatar.into());
        self
    }

    pub fn avatar(&self) -> Option<&Path> {
        self.avatar.as_deref()
    }

    pub fn online(&self) -> bool {
        self.online
    }

    pub fn toggle_online(&mut self) {
        self.online = !self.online;
    }
}

// pub trait ProfileExt {
//     //
//     fn create_profile(&self, name: impl Into<SharedString>, cx: &mut App) -> Entity<Profile>;
//     fn profiles(&self, cx: &mut App) -> Vec<Entity<Profile>>;
// }

// // TODO: Need external building blocks on Willow
// impl ProfileExt for Willow {
//     fn create_profile(&self, name: impl Into<SharedString>, cx: &mut App) -> Entity<Profile> {
//         // TODO: Store the profile in the database
//         let profile = cx.new(|cx| Profile::new(name, cx));
//         profile
//     }

//     fn profiles(&self, cx: &mut App) -> Vec<Entity<Profile>> {
//         Vec::new()
//     }
// }
