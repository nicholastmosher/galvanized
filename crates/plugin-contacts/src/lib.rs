use std::path::PathBuf;

use anyhow::{Context as _, Result, anyhow, bail};
use hex::ToHex;
use iroh::{EndpointAddr, EndpointId};
use plugin_galvanized::{
    Galvanized,
    app_behavior::AppBehavior,
    domain::{
        profile::{Profile, ProfileHandle},
        space::Space,
    },
};
use plugin_p2p::P2pHandle as _;
use plugin_willow::WillowExt as _;
use plugin_willow::willow_serde::SubspaceIdSerde;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use signature::SignerMut as _;
use tracing::info;
use ufotofu::{IntoConsumer as _, IntoProducer, codec::Decodable};
use willow25::entry::{SubspaceId, SubspaceSecret, SubspaceSignature};
use zed::unstable::{
    editor::Editor,
    gpui::{
        self, Action, AppContext as _, ClipboardItem, Entity, EventEmitter, FocusHandle, Focusable,
        KeyDownEvent, Task, WeakEntity, actions,
    },
    ui::{
        ActiveTheme, App, Context, FluentBuilder as _, Icon, IconName, IconSize,
        InteractiveElement as _, IntoElement, ListSeparator, ParentElement as _, Render,
        SharedString, StatefulInteractiveElement as _, Styled, Tooltip, Window, div, h_flex,
        v_flex,
    },
    util::ResultExt as _,
};

use crate::protocol::{ContactPeer, ContactsProtocol};

pub mod protocol;

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
        let galvanized_weak = cx.weak_entity();
        let contacts = cx.new(|cx| Contacts::new(galvanized_weak, window, cx));
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

    // Each Profile has its own p2p stack, here we register the Contacts protocol for each
    cx.observe_new::<Profile>(|profile, _window, cx| {
        let local_key = profile.subspace().secret_todo_consider_access_control();
        let endpoint = profile.p2p().endpoint(cx);
        let protocol = ContactsProtocol::new(endpoint, local_key.clone());
        profile.p2p().register_protocol(protocol, cx);
    })
    .detach();
}

/// The main Contacts entity, holding the list of contacts for the current
/// profile in the current space.
pub struct Contacts {
    contacts: Vec<Entity<Contact>>,
    copy_ticket: Option<String>,
    focus_handle: FocusHandle,
    galvanized: WeakEntity<Galvanized>,
    input_editor: Entity<Editor>,
}

impl Contacts {
    pub fn new(
        galvanized: WeakEntity<Galvanized>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input_editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text("Paste Profile Ticket", window, cx);
            editor
        });

        Self {
            contacts: Default::default(),
            copy_ticket: None,
            focus_handle: cx.focus_handle(),
            galvanized,
            input_editor,
        }
    }

    /// Load Contacts from the given Profile and Space.
    pub fn load(
        &mut self,
        space: Entity<Space>,
        profile: Entity<Profile>,
        cx: &mut Context<Self>,
    ) -> Task<Result<Vec<Entity<Contact>>>> {
        let path = PathBuf::from(format!("/apps/{}", Self::id()));
        let namespace = space.read(cx).namespace().clone();
        let subspace = profile.read(cx).subspace().clone();

        cx.spawn(async move |_this, cx| {
            use futures_concurrency::prelude::*;

            let entries = cx
                .willow()
                .list_entries(&namespace, &subspace, &path)
                .await?;

            // Concurrently read entries from Willow store
            let entries_bytes = entries
                .into_iter()
                .map(|entry| cx.willow().read_entry(&namespace, entry))
                .collect::<Vec<_>>()
                .join()
                .await;

            let contact_data = entries_bytes
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .map(|entry_bytes| {
                    serde_json::from_slice::<ContactData>(&entry_bytes)
                        .context("failed to deserialize ContactData")
                })
                .collect::<Result<Vec<_>, _>>()?;

            let contacts = contact_data
                .into_iter()
                .map(|data| cx.new(|cx| Contact::from_data(data, cx)))
                .collect::<Vec<_>>();

            anyhow::Ok(contacts)
        })
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
        if let Some(copy_ticket) = self.copy_ticket.take() {
            cx.write_to_clipboard(ClipboardItem::new_string(copy_ticket));
            info!("Copied ContactTicket");
        }

        let active_profile = self
            .galvanized
            .read_with(cx, |it, cx| it.active_profile(cx))
            .log_err()
            .flatten();

        v_flex()
            .size_full()
            .p_2()
            .gap_2()
            .when_none(&active_profile, |el| {
                //
                el.child("No active profile")
            })
            .when_some(active_profile.as_ref(), |el, profile| {
                //
                el.child(self.render_header(profile.clone(), window, cx))
                    .child(ListSeparator)
                    .child(self.render_contact_list(profile.clone(), window, cx))
            })
    }
}

impl Contacts {
    /// User has pasted a contact ticket into the input editor and submitted it.
    fn submit_create_contact(
        &mut self,
        profile: Entity<Profile>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ticket_string = self.input_editor.read(cx).text(cx);
        let Some(ticket) = ticket_string.parse::<ContactTicket>().log_err() else {
            return;
        };

        cx.spawn(async move |_this, cx| {
            ticket.verify().await?;
            let p2p = profile.p2p(cx);
            let contacts_protocol = p2p.protocol::<ContactsProtocol, _>(cx);
            let Some(contacts_protocol) = contacts_protocol else {
                bail!("ContactsProtocol not installed when trying to create Contact");
            };

            let contact_peer = contacts_protocol.connect(&ticket, cx).await?;
            anyhow::Ok(())
        })
        .detach_and_log_err(cx);
    }

    fn copy_contact_ticket(
        &mut self,
        profile: Entity<Profile>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let p2p = profile.p2p(cx);
        let profile_key = profile
            .read(cx)
            .subspace()
            .secret_todo_consider_access_control()
            .clone();
        let display_name = profile.read(cx).name();

        cx.spawn(async move |this, cx| {
            let endpoint_id = p2p.endpoint_id(cx).await.context("iroh not initialized")?;

            let addr = EndpointAddr {
                id: endpoint_id,
                addrs: Default::default(),
            };
            let ticket = ContactTicket::new(profile_key, display_name, addr).await?;
            let ticket_string = ticket.to_string();
            this.update(cx, |this, _cx| {
                this.copy_ticket = Some(ticket_string);
            })?;

            anyhow::Ok(())
        })
        .detach_and_log_err(cx);
    }

    fn render_header(
        &mut self,
        profile: Entity<Profile>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
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
                        //
                        h_flex()
                            .id("copy-profile-id")
                            .gap_2()
                            .text_xs()
                            .text_color(cx.theme().colors().text_muted)
                            .hover(|style| style.text_color(cx.theme().colors().text_placeholder))
                            .tooltip(Tooltip::text("Send this to a friend to paste in their app"))
                            .child(Icon::new(IconName::Copy).size(IconSize::Small))
                            .child("Copy Profile Ticket")
                            .on_click(cx.listener({
                                let profile = profile.clone();
                                move |this, _e, window, cx| {
                                    this.copy_contact_ticket(profile.clone(), window, cx);
                                }
                            })),
                    ),
            )
            .child(
                h_flex()
                    .flex_grow()
                    .border_1()
                    .border_color(cx.theme().colors().border)
                    .rounded_md()
                    .on_key_down(cx.listener(move |this, e: &KeyDownEvent, window, cx| {
                        if e.keystroke.key != "enter" {
                            return;
                        }

                        this.submit_create_contact(profile.clone(), window, cx);
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
            )
    }

    fn render_contact_list(
        &mut self,
        profile: Entity<Profile>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p2p = profile.p2p(cx);
        let protocol = p2p.protocol::<ContactsProtocol, _>(cx);

        div()
            .id("contacts-list")
            .size_full()
            .overflow_y_scroll()
            .child(
                div()
                    .when_none(&protocol, |el| {
                        el.p_2().child(
                            div()
                                .text_color(cx.theme().colors().text_muted)
                                .child("Paste a Profile ticket to add a contact"),
                        )
                    })
                    .when_some(protocol, |el, _protocol| {
                        // TODO
                        _protocol.contacts();
                        el
                    }),
                // .children(self.contacts.iter().enumerate().map(|(i, contact)| {
                //     let subspace_hex: String =
                //         contact.read(cx).profile_id().as_bytes().encode_hex();
                //     let short_id: SharedString = {
                //         let mut s = subspace_hex.clone();
                //         s.truncate(16);
                //         SharedString::from(s)
                //     };

                //     h_flex()
                //         .id(SharedString::from(format!("contact-{i}")))
                //         .p_2()
                //         .gap_2()
                //         .rounded_md()
                //         .hover(|style| style.bg(cx.theme().colors().ghost_element_hover))
                //         .active(|style| style.bg(cx.theme().colors().ghost_element_active))
                //         .child(
                //             v_flex().child(contact.read(cx).display_name()).child(
                //                 div()
                //                     .text_sm()
                //                     .text_color(cx.theme().colors().text_muted)
                //                     .child(short_id),
                //             ),
                //         )
                //         .child(div().flex_grow())
                //         .child(
                //             div()
                //                 .id(SharedString::from(format!("remove-contact-{i}")))
                //                 .hover(|style| style.text_color(rgba(0xef4444ff)))
                //                 .on_click(cx.listener({
                //                     let index = i;
                //                     move |this, _e, _window, cx| {
                //                         this.remove_contact(index, cx);
                //                     }
                //                 }))
                //                 .child(div().px_1().child("✕")),
                //         )
                // })),
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
    /// The persistent data for this Contact.
    data: ContactData,

    /// The peer connection for this Contact, if one exists.
    peer: Option<ContactPeer>,
}

/// Serializable, persistent data for a Contact.
#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
pub struct ContactData {
    /// Profile ID of the remote Profile this Contact represents.
    #[serde_as(as = "SubspaceIdSerde")]
    profile_id: SubspaceId,

    /// The display name of the remote Profile.
    display_name: SharedString,

    /// The last EndpointId seen for this Contact.
    endpoint_id: Option<EndpointId>,
}

impl Contact {
    pub fn new(
        profile_id: SubspaceId,
        display_name: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) -> Self {
        let data = ContactData {
            profile_id,
            display_name: display_name.into(),
            endpoint_id: None,
        };
        Self::from_data(data, cx)
    }

    pub fn from_data(data: ContactData, _cx: &mut Context<Self>) -> Self {
        Self { data, peer: None }
    }

    pub fn from_profile(profile: &Entity<Profile>, cx: &mut Context<Self>) -> Self {
        let data = ContactData {
            profile_id: profile.read(cx).id(),
            display_name: profile.read(cx).name(),
            endpoint_id: None,
        };
        Self::from_data(data, cx)
    }

    pub fn from_ticket(ticket: ContactTicket, cx: &mut Context<Self>) -> Self {
        let data = ContactData {
            profile_id: ticket.message.profile_id,
            display_name: ticket.message.display_name.clone(),
            endpoint_id: None,
        };
        Self::from_data(data, cx)
    }

    pub fn profile_id(&self) -> &SubspaceId {
        &self.data.profile_id
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
        let profile_id = self.profile_id().as_bytes().encode_hex::<String>();
        let path = PathBuf::from(format!("/apps/{}/{}", Contacts::id(), profile_id));
        let data = serde_json::to_vec(&self.data).context("failed to serialize ContactData")?;

        cx.willow()
            .create_entry(&namespace, &subspace, &path, data)
            .await?;

        Ok(())
    }
}

// /// Wrapper around a Willow SubspaceId that identifies a remote Profile.
// #[serde_as]
// #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
// pub struct ContactId {
//     #[serde_as(as = "SubspaceIdSerde")]
//     subspace_id: SubspaceId,
// }

// impl ContactId {
//     pub fn from_hex(hex_str: &str) -> Option<Self> {
//         let bytes = hex::decode(hex_str).ok()?;
//         if bytes.len() != 32 {
//             return None;
//         }
//         let mut arr = [0u8; 32];
//         arr.copy_from_slice(&bytes);
//         Some(ContactId {
//             subspace_id: SubspaceId::from_bytes(&arr),
//         })
//     }

//     pub fn to_hex(&self) -> String {
//         self.subspace_id.to_bytes().encode_hex()
//     }
// }

// impl std::fmt::Display for ContactId {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         let hex: String = self.subspace_id.to_bytes().encode_hex();
//         write!(f, "{}", &hex[..16])
//     }
// }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactTicket {
    message: ContactTicketMessage,
    signature: Vec<u8>,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactTicketMessage {
    #[serde_as(as = "SubspaceIdSerde")]
    profile_id: SubspaceId,
    display_name: SharedString,
    addrs: EndpointAddr,
}

impl ContactTicketMessage {
    pub fn new(profile_id: SubspaceId, display_name: SharedString, addrs: EndpointAddr) -> Self {
        Self {
            profile_id,
            display_name,
            addrs,
        }
    }
}

impl ContactTicket {
    /// Create a new ticket for the given profile key and endpoints.
    ///
    /// On construction, the ticket message is signed using the profile key
    /// and the signature is stored in the ticket alongside the message.
    async fn new(
        mut profile_key: SubspaceSecret,
        display_name: SharedString,
        addrs: EndpointAddr,
    ) -> Result<Self> {
        let message =
            ContactTicketMessage::new(profile_key.corresponding_subspace_id(), display_name, addrs);
        let message_bytes =
            serde_json::to_vec(&message).context("failed to serialize TicketMessage")?;

        let subspace_signature = profile_key.try_sign(&message_bytes)?;
        let mut signature = Vec::new();
        let mut consumer = (&mut signature).into_consumer();

        use ufotofu::codec::Encodable;
        subspace_signature.encode(&mut consumer).await?;
        drop(consumer);

        Ok(Self { message, signature })
    }

    /// Verifies that the signature of the ticket matches the profile ID in the message.
    async fn verify(&self) -> Result<()> {
        use signature::Verifier;
        let message_bytes = serde_json::to_vec(&self.message).unwrap();
        let signature = self.signature.clone();
        let mut sig_producer = signature.into_producer();
        let subspace_signature = SubspaceSignature::decode(&mut sig_producer)
            .await
            .map_err(|_| anyhow!("failed to decode ticket signature"))?;
        self.message
            .profile_id
            .verify(&message_bytes, &subspace_signature)?;
        Ok(())
    }

    /// Deserialize from a slice of bytes to a Ticket.
    fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        serde_json::from_slice(bytes).map_err(Into::into)
    }

    /// Serialize from a `Ticket` to a `Vec` of bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("serde_json::to_vec is infallible")
    }
}

// The `Display` trait allows us to use the `to_string`
// method on `Ticket`.
impl std::fmt::Display for ContactTicket {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let mut text = data_encoding::BASE32_NOPAD.encode(&self.to_bytes()[..]);
        text.make_ascii_lowercase();
        write!(f, "{}", text)
    }
}

// The `FromStr` trait allows us to turn a `str` into
// a `Ticket`
impl std::str::FromStr for ContactTicket {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = data_encoding::BASE32_NOPAD.decode(s.to_ascii_uppercase().as_bytes())?;
        Self::from_bytes(&bytes)
    }
}

#[cfg(test)]
mod tests {
    use willow25::entry::randomly_generate_subspace;

    use super::*;

    #[tokio::test]
    async fn test_sign_verify_ticket() {
        let (_id, secret) = randomly_generate_subspace(&mut rand_core_0_6_4::OsRng);
        let id = EndpointId::from_bytes(&[0u8; 32]).unwrap();
        let endpoints = EndpointAddr {
            id,
            addrs: Default::default(),
        };
        let display_name = "Fred".into();
        let ticket = ContactTicket::new(secret, display_name, endpoints)
            .await
            .expect("failed to create ticket");

        ticket
            .verify()
            .await
            .expect("ticket should verify successfully");
    }
}
