use plugin_willow::Subspace;
use serde::{Deserialize, Serialize};
use willow25::entry::SubspaceId;
use zed::unstable::ui::SharedString;

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

    pub fn name(&self) -> SharedString {
        self.name.clone()
    }
}
