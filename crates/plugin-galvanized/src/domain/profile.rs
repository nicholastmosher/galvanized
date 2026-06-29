use plugin_willow::Subspace;
use serde::{Deserialize, Serialize};
use willow25::entry::SubspaceId;
use zed::unstable::ui::SharedString;

use crate::domain::space::Space;

#[derive(Clone, Serialize, Deserialize)]
pub struct Profile {
    name: SharedString,
    subspace: Subspace,
    home: Space,
}

impl Profile {
    pub fn new(name: SharedString, subspace: Subspace, home: Space) -> Self {
        Self {
            name,
            subspace,
            home,
        }
    }

    pub fn id(&self) -> SubspaceId {
        self.subspace.id()
    }

    pub fn name(&self) -> SharedString {
        self.name.clone()
    }
}
