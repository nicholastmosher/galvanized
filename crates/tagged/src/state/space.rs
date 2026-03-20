//! # Spaces
//!
//! > Yes, this is Willow's Namespaces. As part of my experimentation in
//! > this project, I want to consider how to craft an easy-to-follow
//! > terminology and end-user mental model. So I'll be describing the
//! > terms the way I'd want to present the idea to the user.
//!
//! A Space is like a magic folder that's shared between members.
//!
//! There are two kinds of spaces, Owned (invite-only), and Communal
//! (open to anyone). You can be a member of more than one space, and
//! you can have more than one Profile.
//!
//! You can imagine that each Profile that's a member of a space has its
//! own dedicated home directory. You and your apps read and write in
//! your profile's home directory. You can also give other profiles the
//! ability to read and write to your choice of subdirectories in the form
//! of capabilities.
//!
//! Apps are effectively an interface to viewing data.
//!
//! - A photo album is just a list of files rendered as a grid
//! - A calendar is just a list of events, rendered on a week or month view
//! - A chat is just a list of messages and media, rendered as a conversation

use std::path::{Path, PathBuf};

use willow25::entry::{NamespaceId, NamespaceSecret};
use zed::unstable::ui::{App, Context, SharedString};

pub fn init(_cx: &mut App) {
    //
}

pub struct Space {
    key: NamespaceSecret,
    icon_path: Option<PathBuf>,
    name: SharedString,
}

impl Space {
    pub fn new(
        name: impl Into<SharedString>,
        key: NamespaceSecret,
        _cx: &mut Context<Self>,
    ) -> Self {
        Space {
            key,
            icon_path: None,
            name: name.into(),
        }
    }

    pub fn namespace_id(&self) -> NamespaceId {
        self.key.corresponding_namespace_id()
    }

    // TODO before production, find proper secrets solution
    pub fn namespace_secret(&self) -> &NamespaceSecret {
        //
        &self.key
    }

    pub fn parts(&self) -> (NamespaceId, NamespaceSecret) {
        let id = self.key.corresponding_namespace_id();
        (id, self.key.clone())
    }

    pub fn name(&self) -> SharedString {
        self.name.clone()
    }

    pub fn is_communal(&self) -> bool {
        self.key.corresponding_namespace_id().is_communal()
    }

    pub fn icon_path(&self) -> Option<&Path> {
        self.icon_path.as_deref()
    }

    pub fn set_icon_path(&mut self, icon_path: PathBuf) {
        self.icon_path = Some(icon_path);
    }
}
