use std::sync::Arc;

use hex::ToHex;
use plugin_galvanized::{Galvanized, app_behavior::AppBehavior};
use tracing::info;
use willow25::entry::{SubspaceId, randomly_generate_subspace};
use zed::unstable::{
    editor::Editor,
    gpui::{
        self, Action, AppContext as _, Entity, EventEmitter, FocusHandle, Focusable, Image,
        ImageFormat, KeyDownEvent, actions, img, rgba,
    },
    ui::{
        ActiveTheme, App, Context, Div, FluentBuilder as _, Icon, IconName, IconSize,
        InteractiveElement as _, IntoElement, ListSeparator, ParentElement as _, Render,
        SharedString, StatefulInteractiveElement as _, Styled, Tooltip, Window, div, h_flex, px,
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
                Contact::new(ContactId(subspace_id), name.to_string())
            })
            .collect()
    }

    fn add_contact_from_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.input_editor.read(cx).text(cx);
        if text.is_empty() {
            return;
        }

        let bytes = match hex::decode(text.trim()) {
            Ok(bytes) => bytes,
            Err(_) => {
                info!("Failed to parse contact ID: invalid hex");
                return;
            }
        };

        if bytes.len() != 32 {
            info!(
                "Failed to parse contact ID: expected 32 bytes, got {}",
                bytes.len()
            );
            return;
        }

        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        let subspace_id = SubspaceId::from_bytes(&arr);

        if self.contacts.iter().any(|c| c.contact_id.0 == subspace_id) {
            info!("Contact already exists");
            return;
        }

        let short_hex: String = text.trim().chars().take(8).collect();
        let contact = Contact::new(ContactId(subspace_id), format!("Peer {}", short_hex));
        self.contacts.push(contact);
        self.input_editor
            .update(cx, |editor, cx| editor.set_text("", window, cx));
        cx.notify();
    }

    fn remove_contact(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.contacts.len() {
            self.contacts.remove(index);
            cx.notify();
        }
    }
}

impl AppBehavior for Contacts {
    fn id(&self) -> &'static str {
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
                            .on_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                                if e.keystroke.key != "enter" {
                                    return;
                                }
                                this.add_contact_from_input(window, cx);
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
                        let subspace_hex: String = contact.contact_id.0.to_bytes().encode_hex();
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
                                div()
                                    .size(px(32.))
                                    .child(img(contact.avatar.clone()).size(px(32.))),
                            )
                            .child(
                                v_flex().child(contact.display_name.clone()).child(
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
    contact_id: ContactId,
    display_name: SharedString,
    avatar: Arc<Image>,
}

impl Contact {
    pub fn new(contact_id: ContactId, display_name: impl Into<SharedString>) -> Self {
        let identicon_png =
            plot_icon::generate_png(contact_id.0.to_bytes().as_slice(), 512).unwrap();
        let avatar = Image::from_bytes(ImageFormat::Png, identicon_png);
        Self {
            contact_id,
            display_name: display_name.into(),
            avatar: Arc::new(avatar),
        }
    }

    pub fn contact_id(&self) -> &ContactId {
        &self.contact_id
    }

    pub fn display_name(&self) -> &SharedString {
        &self.display_name
    }
}

/// Wrapper around a Willow SubspaceId that identifies a remote Profile.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ContactId(pub SubspaceId);

impl ContactId {
    pub fn from_hex(hex_str: &str) -> Option<Self> {
        let bytes = hex::decode(hex_str).ok()?;
        if bytes.len() != 32 {
            return None;
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Some(ContactId(SubspaceId::from_bytes(&arr)))
    }

    pub fn to_hex(&self) -> String {
        self.0.to_bytes().encode_hex()
    }
}

impl std::fmt::Display for ContactId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hex: String = self.0.to_bytes().encode_hex();
        write!(f, "{}", &hex[..16])
    }
}
