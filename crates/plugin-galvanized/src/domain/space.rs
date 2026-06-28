use plugin_willow::Namespace;
use serde::{Deserialize, Serialize};
use willow25::entry::NamespaceId;
use zed::unstable::ui::SharedString;

#[derive(Clone, Serialize, Deserialize)]
pub struct Space {
    name: SharedString,
    namespace: Namespace,
}

impl Space {
    pub fn new(name: SharedString, namespace: Namespace) -> Self {
        Self { name, namespace }
    }

    pub fn id(&self) -> NamespaceId {
        self.namespace.id()
    }

    pub fn name(&self) -> SharedString {
        self.name.clone()
    }

    pub fn is_communal(&self) -> bool {
        self.namespace.is_communal()
    }

    pub fn is_owned(&self) -> bool {
        self.namespace.is_owned()
    }
}
