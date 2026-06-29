use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use hex::ToHex;
use plugin_galvanized::{
    Galvanized,
    app_behavior::AppBehavior,
    domain::{profile::Profile, space::Space},
};
use plugin_willow::willow_serde::SubspaceIdSerde;
use plugin_willow::{Namespace, Subspace, WillowExt as _};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use tracing::info;
use willow25::entry::{SubspaceId, randomly_generate_subspace};
use zed::unstable::{
    editor::Editor,
    gpui::{
        self, Action, AppContext as _, Entity, EventEmitter, FocusHandle, Focusable, KeyDownEvent,
        actions, rgba,
    },
    ui::{
        ActiveTheme, App, Context, Div, FluentBuilder as _, Icon, IconName, IconSize,
        InteractiveElement as _, IntoElement, ListSeparator, ParentElement as _, Render,
        SharedString, StatefulInteractiveElement as _, Styled, Tooltip, Window, div, h_flex,
        v_flex,
    },
};

actions!(
    contacts,
    [
        //
        OpenContacts,
    ]
);

pub fn init(cx: &mut App) {
    cx.observe_new::<Galvanized>(|galvanized, window, cx| {
        let Some(window) = window else { return };
        let contacts = cx.new(|cx| Contacts::new(window, cx));
        galvanized.register_app(contacts.clone(), cx);
        galvanized
            .panel()
            .update(cx, |panel, cx| panel.set_active_app(contacts.clone(), cx));
        galvanized.register_action(
            cx,
            move |this, _workspace, _: &OpenContacts, _window, cx| {
                let contacts = contacts.clone();
                this.panel()
                    .update(cx, move |panel, cx| panel.set_active_app(contacts, cx));
            },
        );
    })
    .detach();
}

/// The main Contacts entity, holding the list of contacts for the current
/// profile in the current space.
pub struct Contacts {
    contacts: Vec<Contact>,
    input_editor: Entity<Editor>,
    focus_handle: FocusHandle,
}

impl Contacts {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input_editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text("Paste remote Profile ID", window, cx);
            editor
        });

        let contacts = Self::mock_contacts();

        Self {
            contacts,
            input_editor,
            focus_handle: cx.focus_handle(),
        }
    }

    /// Generate mock contacts for development purposes.
    fn mock_contacts() -> Vec<Contact> {
        let names = ["Alice", "Bob", "Charlie", "Diana"];
        names
            .iter()
            .map(|name| {
                let (subspace_id, _secret) =
                    randomly_generate_subspace(&mut rand_core_0_6_4::OsRng);
                Contact::new(ContactId { subspace_id }, name.to_string())
            })
            .collect()
    }

    fn remove_contact(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.contacts.len() {
            self.contacts.remove(index);
            cx.notify();
        }
    }
}

impl AppBehavior for Contacts {
    fn id() -> &'static str {
        "contacts"
    }

    fn icon(&self) -> SharedString {
        "🧑‍🧑‍🧒‍🧒".into()
    }

    fn title(&self) -> SharedString {
        "Contacts".into()
    }

    fn open_action(&self) -> Box<dyn Action> {
        Box::new(OpenContacts)
    }
}

impl Render for Contacts {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .p_2()
            .gap_2()
            .child(self.render_header(window, cx))
            .child(ListSeparator)
            .child(self.render_contact_list(window, cx))
    }
}

impl Contacts {
    fn render_header(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            //
            .px_2()
            .gap_2()
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        //
                        v_flex()
                            //
                            .text_lg()
                            .child("Contacts"),
                    )
                    .child(
                        h_flex()
                            .flex_grow()
                            .border_1()
                            .border_color(cx.theme().colors().border)
                            .rounded_md()
                            .on_key_down(cx.listener(|_this, e: &KeyDownEvent, _window, _cx| {
                                if e.keystroke.key != "enter" {
                                    return;
                                }

                                info!("TODO Create Contact");
                            }))
                            .child(
                                //
                                div()
                                    //
                                    .flex_grow()
                                    .bg(cx.theme().colors().editor_background)
                                    .px_2()
                                    .child(self.input_editor.clone()),
                            ),
                    ),
            )
            .child(
                //
                h_flex()
                    .id("copy-profile-id")
                    .gap_2()
                    .text_xs()
                    .text_color(cx.theme().colors().text_muted)
                    .hover(|style| style.text_color(cx.theme().colors().text_placeholder))
                    .tooltip(Tooltip::text("Send this to a friend to paste in their app"))
                    .child(Icon::new(IconName::Copy).size(IconSize::Small))
                    .child("Copy My ID: 0xdeadbeefdeadbeefdeadbeef"),
            )
    }

    fn render_contact_list(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("contacts-list")
            .size_full()
            .overflow_y_scroll()
            .child(
                div()
                    .when(self.contacts.is_empty(), |el: Div| {
                        el.p_4().child(
                            div()
                                .text_color(cx.theme().colors().text_muted)
                                .child("No contacts yet. Paste a Profile ID above to add one."),
                        )
                    })
                    .children(self.contacts.iter().enumerate().map(|(i, contact)| {
                        let subspace_hex: String = contact.contact_id().to_hex();
                        let short_id: SharedString = {
                            let mut s = subspace_hex.clone();
                            s.truncate(16);
                            SharedString::from(s)
                        };

                        h_flex()
                            .id(SharedString::from(format!("contact-{i}")))
                            .p_2()
                            .gap_2()
                            .rounded_md()
                            .hover(|style| style.bg(cx.theme().colors().ghost_element_hover))
                            .active(|style| style.bg(cx.theme().colors().ghost_element_active))
                            .child(
                                v_flex().child(contact.display_name()).child(
                                    div()
                                        .text_sm()
                                        .text_color(cx.theme().colors().text_muted)
                                        .child(short_id),
                                ),
                            )
                            .child(div().flex_grow())
                            .child(
                                div()
                                    .id(SharedString::from(format!("remove-contact-{i}")))
                                    .hover(|style| style.text_color(rgba(0xef4444ff)))
                                    .on_click(cx.listener({
                                        let index = i;
                                        move |this, _e, _window, cx| {
                                            this.remove_contact(index, cx);
                                        }
                                    }))
                                    .child(div().px_1().child("✕")),
                            )
                    })),
            )
    }
}

impl Focusable for Contacts {
    fn focus_handle(&self, _cx: &App) -> zed::unstable::gpui::FocusHandle {
        self.focus_handle.clone()
    }
}

type ContactsEvent = ();
impl EventEmitter<ContactsEvent> for Contacts {}

/// A Contact represents a remote Profile, identified by its SubspaceId.
pub struct Contact {
    data: ContactData,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
pub struct ContactData {
    contact_id: ContactId,
    display_name: SharedString,
}

impl Contact {
    pub fn new(contact_id: ContactId, display_name: impl Into<SharedString>) -> Self {
        let data = ContactData {
            contact_id,
            display_name: display_name.into(),
        };
        Self { data }
    }

    pub fn contact_id(&self) -> &ContactId {
        &self.data.contact_id
    }

    pub fn display_name(&self) -> SharedString {
        self.data.display_name.clone()
    }

    pub async fn save(
        &self,
        profile: Entity<Profile>,
        space: Entity<Space>,
        cx: &mut App,
    ) -> Result<()> {
        let namespace = space.read(cx).namespace().clone();
        let subspace = profile.read(cx).subspace().clone();
        let path = PathBuf::from(format!(
            "/apps/{}/{}",
            Contacts::id(),
            self.contact_id().to_hex()
        ));
        let data = serde_json::to_vec(&self.data).context("failed to serialize ContactData")?;

        cx.willow()
            .create_entry(&namespace, &subspace, &path, data)
            .await?;

        Ok(())
    }

    pub async fn load(
        namespace: &Namespace,
        subspace: &Subspace,
        path: &Path,
        cx: &mut App,
    ) -> Result<Self> {
        todo!()
    }
}

/// Wrapper around a Willow SubspaceId that identifies a remote Profile.
#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ContactId {
    #[serde_as(as = "SubspaceIdSerde")]
    subspace_id: SubspaceId,
}

impl ContactId {
    pub fn from_hex(hex_str: &str) -> Option<Self> {
        let bytes = hex::decode(hex_str).ok()?;
        if bytes.len() != 32 {
            return None;
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Some(ContactId {
            subspace_id: SubspaceId::from_bytes(&arr),
        })
    }

    pub fn to_hex(&self) -> String {
        self.subspace_id.to_bytes().encode_hex()
    }
}

impl std::fmt::Display for ContactId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hex: String = self.subspace_id.to_bytes().encode_hex();
        write!(f, "{}", &hex[..16])
    }
}
