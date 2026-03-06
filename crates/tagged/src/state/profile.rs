use std::path::{Path, PathBuf};

use zed::unstable::{
    gpui::{AppContext as _, Entity},
    ui::{App, Context, SharedString},
};

use crate::willow::Willow;

pub fn init(cx: &mut App) {
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

// data object only
pub struct Profile {
    /// Path to the avatar image.
    avatar: Option<PathBuf>,
    name: SharedString,
    online: bool,
}

impl Profile {
    pub fn new(name: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        Self {
            //
            avatar: None,
            name: name.into(),
            online: true,
        }
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
