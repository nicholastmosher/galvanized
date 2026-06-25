use std::collections::BTreeSet;

use plugin_galvanized::{Galvanized, app_behavior::AppBehavior};
use willow25::entry::{SubspaceId, SubspaceSecret};
use zed::unstable::{
    gpui::{AppContext, rgb},
    ui::{App, Context, IntoElement, Render, SharedString, Styled, Window, div},
};

pub fn init(cx: &mut App) {
    cx.observe_new::<Galvanized>(|galvanized, _window, cx| {
        let contacts = cx.new(|cx| Contacts::new(cx));
        galvanized.register_app(contacts);
    })
    .detach();
}

pub struct Contacts {
    //
}

impl Contacts {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {}
    }
}

impl AppBehavior for Contacts {
    fn id(&self) -> &'static str {
        "contacts"
    }

    fn icon(&self) -> SharedString {
        "📒".into()
    }

    fn title(&self) -> SharedString {
        "Contacts".into()
    }
}

// Rendered under the home screen, should be easily accessible
impl Render for Contacts {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            //
            .bg(rgb(0xaa55ee))
    }
}

pub struct Contact {
    //
    contact_id: ContactId,
}

pub trait ContactsRepository {
    /// The ContactId is the public key of the remote Profile
    fn add_contact(&mut self, contact_id: impl Into<ContactId>) -> Contact;
    fn remove_contact(&mut self, contact_id: impl Into<ContactId>);
}

struct SimpleRepo {
    contacts: BTreeSet<ContactId>,
}

impl ContactsRepository for SimpleRepo {
    fn add_contact(&mut self, contact_id: impl Into<ContactId>) -> Contact {
        // let contact_id = contact_id.into();
        // self.contacts.push(contact_id.clone());
        Contact {
            contact_id: contact_id.into(),
        }
    }

    fn remove_contact(&mut self, contact_id: impl Into<ContactId>) {
        let id = contact_id.into();
        self.contacts.remove(&id);
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ContactId(SubspaceId);
impl From<SubspaceId> for ContactId {
    fn from(id: SubspaceId) -> Self {
        Self(id)
    }
}
pub struct ContactKey(SubspaceSecret);

pub struct ContactsBuilder {
    /// Remote Profile's public key
    contact_id: ContactId,
    display_name: Option<String>,
}

// impl<C: AppContext> ContactsRepository for C {
//     fn create(&mut self) {
//         todo!()
//     }
// }
