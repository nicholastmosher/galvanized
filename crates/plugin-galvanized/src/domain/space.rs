use plugin_willow::Namespace;
use serde::{Deserialize, Serialize};
use willow25::entry::NamespaceId;
use zed::unstable::ui::{Context, SharedString};

#[derive(Clone)]
pub struct Space {
    data: SpaceData,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SpaceData {
    name: SharedString,
    namespace: Namespace,
}

impl SpaceData {
    pub fn id(&self) -> NamespaceId {
        self.namespace.id()
    }
}

impl Space {
    pub fn new(name: SharedString, namespace: Namespace, _cx: &mut Context<Self>) -> Self {
        Self {
            data: SpaceData { name, namespace },
        }
    }

    pub fn from_data(data: SpaceData, _cx: &mut Context<Self>) -> Self {
        Self { data }
    }

    pub fn data(&self) -> &SpaceData {
        &self.data
    }

    pub fn namespace(&self) -> &Namespace {
        &self.data.namespace
    }

    pub fn id(&self) -> NamespaceId {
        self.data.namespace.id()
    }

    pub fn name(&self) -> SharedString {
        self.data.name.clone()
    }

    pub fn is_communal(&self) -> bool {
        self.data.namespace.is_communal()
    }

    pub fn is_owned(&self) -> bool {
        self.data.namespace.is_owned()
    }
}
