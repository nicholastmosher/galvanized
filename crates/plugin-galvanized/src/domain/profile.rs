use plugin_willow::Subspace;
use plugin_willow::willow_serde::NamespaceIdSerde;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use willow25::entry::{NamespaceId, SubspaceId};
use zed::unstable::{
    gpui::Entity,
    ui::{Context, SharedString},
};

use crate::domain::space::Space;

#[derive(Clone)]
pub struct Profile {
    data: ProfileData,
    home: Entity<Space>,
}

#[serde_as]
#[derive(Clone, Serialize, Deserialize)]
pub struct ProfileData {
    name: SharedString,
    subspace: Subspace,
    #[serde_as(as = "NamespaceIdSerde")]
    home_space: NamespaceId,
}

impl ProfileData {
    pub fn id(&self) -> SubspaceId {
        self.subspace.id()
    }

    pub fn home_space_id(&self) -> NamespaceId {
        self.home_space.clone()
    }
}

impl Profile {
    pub fn new(
        name: SharedString,
        subspace: Subspace,
        home: Entity<Space>,
        cx: &mut Context<Self>,
    ) -> Self {
        let home_space = home.read(cx).id();
        Self {
            home,
            data: ProfileData {
                name,
                subspace,
                home_space,
            },
        }
    }

    pub fn from_data(data: ProfileData, home: Entity<Space>, _cx: &mut Context<Self>) -> Self {
        Self { home, data }
    }

    pub fn data(&self) -> &ProfileData {
        &self.data
    }

    pub fn subspace(&self) -> &Subspace {
        &self.data.subspace
    }

    pub fn id(&self) -> SubspaceId {
        self.data.subspace.id()
    }

    pub fn name(&self) -> SharedString {
        self.data.name.clone()
    }

    pub fn home_space(&self) -> Entity<Space> {
        self.home.clone()
    }
}
