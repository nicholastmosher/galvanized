use std::{
    fmt::LowerHex,
    path::{Path, PathBuf},
};

use hex::ToHex as _;
use willow25::entry::{SubspaceId, SubspaceSecret, randomly_generate_subspace};
use zed::unstable::ui::{App, Context, SharedString};

pub fn init(_cx: &mut App) {
    //
}

pub struct ProfileKey(SubspaceSecret);
impl ProfileKey {
    // TODO: plumb RNG? or is this fine
    pub fn generate() -> Self {
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
    key: ProfileKey,
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
            key: ProfileKey(key),
            name: name.into(),
            online: true,
        }
    }

    // TODO: make crate-owned wrapper type
    pub fn subspace_id(&self) -> SubspaceId {
        self.key.0.corresponding_subspace_id()
    }

    // TODO before production, find proper secrets solution
    pub fn subspace_secret(&self) -> &SubspaceSecret {
        &self.key.0
    }

    pub fn parts(&self) -> (SubspaceId, SubspaceSecret) {
        let id = self.key.0.corresponding_subspace_id();
        (id, self.key.0.clone())
    }

    pub fn name(&self) -> SharedString {
        self.name.clone()
    }

    pub fn set_avatar(&mut self, avatar: impl Into<PathBuf>) {
        self.avatar = Some(avatar.into());
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
