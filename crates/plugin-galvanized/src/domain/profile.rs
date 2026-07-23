use gpui::{AppContext, Context, Entity, SharedString, WeakEntity};
use iroh::Endpoint;
use plugin_p2p::P2p;
use plugin_willow::Subspace;
use plugin_willow::willow_serde::NamespaceIdSerde;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use willow25::entry::{NamespaceId, SubspaceId};

use crate::domain::space::Space;

#[derive(Clone)]
pub struct Profile {
    data: ProfileData,
    home: Entity<Space>,
    p2p: Entity<P2p>,
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
        endpoint: Endpoint,
        subspace: Subspace,
        name: SharedString,
        home: Entity<Space>,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let data = ProfileData {
            name,
            subspace,
            home_space: home.read(cx).id(),
        };
        Self::from_parts(endpoint, data, home, cx)
    }

    pub fn from_parts(
        endpoint: Endpoint,
        data: ProfileData,
        home: Entity<Space>,
        cx: &mut Context<Self>,
    ) -> Self {
        let p2p = cx.new(|cx| P2p::new(endpoint, cx));
        Self { data, home, p2p }
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

    pub fn p2p(&self) -> Entity<P2p> {
        self.p2p.clone()
    }
}

pub trait ProfileHandle {
    type Result<T>;
    fn p2p<C: AppContext>(&self, cx: &mut C) -> Self::Result<Entity<P2p>>;
}

impl ProfileHandle for Entity<Profile> {
    type Result<T> = T;

    fn p2p<C: AppContext>(&self, cx: &mut C) -> Self::Result<Entity<P2p>> {
        cx.read_entity(self, |it, _cx| it.p2p())
    }
}

impl ProfileHandle for WeakEntity<Profile> {
    type Result<T> = anyhow::Result<T>;

    fn p2p<C: AppContext>(&self, cx: &mut C) -> Self::Result<Entity<P2p>> {
        self.read_with(cx, |it, _cx| it.p2p())
    }
}
