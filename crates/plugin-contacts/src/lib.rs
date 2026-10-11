//! The Contacts app maintains a list of remote Profiles
//! and shares public identity information with them.
//!
//! # In this crate
//!
//! - We have [`Contacts`], an Entity that implements [`AppBehavior`]
//!   to integrate with Galvanized.
//!
//! - We have [`Contact`] which is an Entity representing one remote [`Profile`],
//!   i.e. a profile controlled by a remote peer.
//!
//!   - [`Contact`] is the UI / domain object for rendering any contents of
//!     the given contact. It contains both a data object for persistence and
//!     a copy of this contact's [`ContactPeer`] object, representing a live
//!     connection to the given contact, if one exists.
//!
//! - We have [`ContactsProtocol`], which impls Iroh's [`ProtocolHandler`]
//!   to define inbound connection behavior and also provides client APIs
//!   to define outbound behavior.
//!
//! - We have [`ContactPeer`], which represents and manages a live connection
//!   with a given remote [`Contact`]. If the ContactPeer exists, it means we
//!   have a live Iroh Connection to the remote contact.

use std::path::PathBuf;

use crate::protocol::{ContactPeer, ContactsProtocol};
use anyhow::{Context as _, Result, anyhow, bail};
use futures::stream::StreamExt as _;
use gpui::prelude::FluentBuilder as _;
use gpui::{
    self, Action, App, AppContext as _, ClipboardItem, Context, Entity, EventEmitter, FocusHandle,
    Focusable, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Render,
    SharedString, StatefulInteractiveElement as _, Styled as _, Task, TaskExt as _, WeakEntity,
    Window, actions, div,
};
use gpui_kit::component::{
    ActiveTheme as _, h_flex,
    input::{Input, InputState},
    tooltip::Tooltip,
    v_flex,
};
use gpui_util::ResultExt as _;
use hex::ToHex;
use iroh::{EndpointAddr, EndpointId};
use iroh_ping::Ping;
use plugin_automerge::{Automergable as _, protocol::AutomergeProtocol};
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
use tracing::{info, warn};
use ufotofu::{IntoConsumer as _, IntoProducer, codec::Decodable};
use willow25::entry::{SubspaceId, SubspaceSecret, SubspaceSignature};

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
        let local_display_name = profile.name();
        let endpoint = profile.p2p().endpoint(cx);
        let Some(automerge) = profile.p2p().protocol::<AutomergeProtocol, _>(cx) else {
            warn!("Failed to get AutomergeProtocol for ContactsProtocol");
            return;
        };
        // Retrieve the subspace secret through the capability check so the
        // secret must come from the vault's `WillowKeyStore`, gated by the
        // Profile's WillowWrite session cap. If the vault is locked (no cap),
        // the Contacts protocol is not registered for this Profile.
        let Some(write_cap) = profile.willow_write_cap() else {
            warn!("No WillowWrite cap for profile; skipping ContactsProtocol");
            return;
        };
        let Some(key_store) = profile.key_store() else {
            warn!("No WillowKeyStore for profile; skipping ContactsProtocol");
            return;
        };
        let profile_id = profile.id();
        let local_key = match key_store.subspace_secret_for_signing(&write_cap, &profile_id) {
            Ok(secret) => secret,
            Err(error) => {
                warn!(
                    ?error,
                    "subspace_secret_for_signing refused; skipping ContactsProtocol"
                );
                return;
            }
        };
        // AutomergeProtocol is already a shared object with Arc-like semantics
        // So here we escape the Arc by cloning the object so we can use it directly, without Arc
        let automerge = AutomergeProtocol::clone(&*automerge);
        let ping = Ping::new();
        let protocol = ContactsProtocol::new(
            endpoint,
            automerge,
            ping,
            local_key.clone(),
            local_display_name.clone(),
        );
        profile.p2p().register_protocol(protocol, cx);
        info!(
            id = ?local_key.corresponding_subspace_id(),
            name = %local_display_name,
            "Registered ContactsProtocol with profile"
        );
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
    // input_editor: Entity<Editor>,
    input: Entity<InputState>,
}

impl Contacts {
    pub fn new(
        galvanized: WeakEntity<Galvanized>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx));

        Self {
            contacts: Default::default(),
            copy_ticket: None,
            focus_handle: cx.focus_handle(),
            galvanized,
            input,
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
                    .child(self.render_contact_list(profile.clone(), window, cx))
            })
    }
}

impl Contacts {
    /// User has pasted a contact ticket into the input editor and submitted it.
    fn submit_contact_ticket(
        &mut self,
        profile: Entity<Profile>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(ticket_string) = self.input.read(cx).text().as_str() else {
            return;
        };
        let Some(ticket) = ticket_string.parse::<ContactTicket>().log_err() else {
            return;
        };

        // Create the Contact entity immediately (data-only, no peer yet)
        let contact = cx.new(|cx| Contact::from_ticket(ticket.clone(), cx));
        self.contacts.push(contact.clone());

        // Observe the Contact so the Contacts list re-renders when it changes
        cx.observe(&contact, |_this, _contact, cx| {
            cx.notify();
        })
        .detach();

        cx.notify();

        cx.spawn(async move |_this, cx| {
            ticket.verify().await?;
            let p2p = profile.p2p(cx);
            let contacts_protocol = p2p.protocol::<ContactsProtocol, _>(cx);
            let Some(contacts_protocol) = contacts_protocol else {
                bail!("ContactsProtocol not installed when trying to connect Contact");
            };

            let contact_peer = contacts_protocol.connect(&ticket).await?;

            // Attach the peer to the Contact entity, which emits a ContactEvent
            let _ = contact.update(cx, |contact, cx| {
                contact.set_peer(contact_peer, cx);
            });

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
        // Retrieve the subspace secret through the capability check (T1) instead
        // of the former `secret_todo_consider_access_control` seam.
        let write_cap = profile.read(cx).willow_write_cap();
        let key_store = profile.read(cx).key_store();
        let profile_id = profile.read(cx).id();
        let display_name = profile.read(cx).name();

        cx.spawn(async move |this, cx| {
            let write_cap = write_cap.context("No WillowWrite cap; vault is locked")?;
            let key_store = key_store.context("No WillowKeyStore; vault is locked")?;
            let profile_key = key_store
                .subspace_secret_for_signing(&write_cap, &profile_id)
                .context("refused to yield subspace secret for signing")?;
            let endpoint_id = p2p.endpoint_id(cx);
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
            .min_w_0()
            .px_2()
            .gap_2()
            .child(
                //
                h_flex()
                    .min_w_0()
                    .flex_wrap()
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
                            .min_w_0()
                            .gap_2()
                            .text_xs()
                            .text_color(cx.theme().colors.muted_foreground)
                            .hover(|style| style.text_color(cx.theme().colors.muted_foreground))
                            .tooltip(|window, cx| {
                                Tooltip::new("Send this to a friend to paste in their app")
                                    .build(window, cx)
                            })
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
                    .min_w_0()
                    .on_key_down(cx.listener(move |this, e: &KeyDownEvent, window, cx| {
                        if e.keystroke.key != "enter" {
                            return;
                        }

                        this.submit_contact_ticket(profile.clone(), window, cx);
                    }))
                    .child(
                        //
                        div()
                            //
                            .min_w_0()
                            .flex_grow_1()
                            .bg(cx.theme().colors.background)
                            .child(Input::new(&self.input)),
                    ),
            )
    }

    fn render_contact_list(
        &mut self,
        _profile: Entity<Profile>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("contacts-list")
            .size_full()
            .min_w_0()
            .overflow_y_scroll()
            .child(
                div()
                    .min_w_0()
                    .when(self.contacts.is_empty(), |el| {
                        el.p_2().child(
                            div()
                                .text_color(cx.theme().colors.muted_foreground)
                                .child("Paste a Profile ticket to add a contact"),
                        )
                    })
                    .children(self.contacts.iter().enumerate().map(|(i, contact)| {
                        let contact_ref = contact.read(cx);
                        let display_name = contact_ref.display_name();
                        let has_peer = contact_ref.peer.is_some();
                        let status_icon = if has_peer { "🟢" } else { "⚪" };
                        let status_text = if has_peer {
                            "Connected"
                        } else {
                            "No connection"
                        };

                        h_flex()
                            .id(SharedString::from(format!("contact-{i}")))
                            .p_2()
                            .gap_2()
                            .min_w_0()
                            .rounded_md()
                            .hover(|style| style.bg(cx.theme().colors.list_hover))
                            .active(|style| style.bg(cx.theme().colors.list_active))
                            .child(
                                v_flex()
                                    .min_w_0()
                                    .child(
                                        h_flex()
                                            .gap_1()
                                            .child(SharedString::from(status_icon))
                                            .child(display_name),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().colors.muted_foreground)
                                            .child(status_text),
                                    ),
                            )
                    })),
            )
    }
}

impl Focusable for Contacts {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

type ContactsEvent = ();
impl EventEmitter<ContactsEvent> for Contacts {}

/// A Contact represents a remote Profile, identified by its SubspaceId.
pub struct Contact {
    /// The persistent, serializable data for this Contact.
    data: ContactData,

    /// The peer connection for this Contact, if one exists.
    peer: Option<ContactPeer>,
}

/// Events emitted by a [`Contact`] entity when its state changes.
///
/// These events allow any GPUI entity to react to changes in a Contact's
/// live protocol state without knowing about the protocol layer.
#[derive(Clone, Debug)]
pub enum ContactEvent {
    /// A peer connection was established for this Contact.
    PeerConnected,
    /// The peer connection for this Contact was lost.
    PeerDisconnected,
}
impl EventEmitter<ContactEvent> for Contact {}

/// Serializable, persistent data for a Contact.
#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
pub struct ContactData {
    /// The ID and name of the remote Profile.
    name: ContactName,

    /// The last EndpointId seen for this Contact.
    endpoint_id: Option<EndpointId>,
}

impl ContactData {
    /// Creates a new `ContactData` with the given name and endpoint ID.
    ///
    /// # Example
    ///
    /// Using `endpoint_id: impl Into<Option<EndpointId>>` allows passing an `EndpointId` value or `None`.
    ///
    /// ```
    /// # fn example(name: ContactName, endpoint_id: EndpointId) {
    /// let contact_data = ContactData::new(name.clone(), endpoint_id);
    /// let contact_data = ContactData::new(name, None);
    /// # }
    /// ```
    pub fn new(name: ContactName, endpoint_id: impl Into<Option<EndpointId>>) -> Self {
        Self {
            name,
            endpoint_id: endpoint_id.into(),
        }
    }
}

/// The name of a Contact, including the profile ID and display name.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactName {
    /// Profile ID of the remote Profile this Contact represents.
    #[serde_as(as = "SubspaceIdSerde")]
    profile_id: SubspaceId,

    /// The display name of the remote Profile.
    display_name: SharedString,
}

impl ContactName {
    /// Creates a new `ContactName` with the given profile ID and display name.
    pub fn new(profile_id: SubspaceId, display_name: impl Into<SharedString>) -> Self {
        Self {
            profile_id,
            display_name: display_name.into(),
        }
    }
}

impl Contact {
    pub fn new(name: ContactName, cx: &mut Context<Self>) -> Self {
        let data = ContactData {
            name,
            endpoint_id: None,
        };
        Self::from_data(data, cx)
    }

    pub fn from_data(data: ContactData, _cx: &mut Context<Self>) -> Self {
        Self { data, peer: None }
    }

    pub fn from_profile(profile: &Entity<Profile>, cx: &mut Context<Self>) -> Self {
        let data = ContactData {
            name: ContactName::new(profile.read(cx).id(), profile.read(cx).name()),
            endpoint_id: None,
        };
        Self::from_data(data, cx)
    }

    pub fn from_ticket(ticket: ContactTicket, cx: &mut Context<Self>) -> Self {
        let name = ContactName::new(
            ticket.message.sender_profile_id,
            ticket.message.display_name.clone(),
        );
        let data = ContactData {
            name,
            endpoint_id: None,
        };
        Self::from_data(data, cx)
    }

    pub fn profile_id(&self) -> &SubspaceId {
        &self.data.name.profile_id
    }

    pub fn display_name(&self) -> SharedString {
        self.data.name.display_name.clone()
    }

    /// Sets the peer connection for this Contact.
    ///
    /// Emits a [`ContactEvent::PeerConnected`] and spawns a listener
    /// that updates the display name from the automerge document.
    pub fn set_peer(&mut self, peer: ContactPeer, cx: &mut Context<Self>) {
        self.peer = Some(peer.clone());

        // Spawn a task that listens for automerge doc changes and updates the display name
        let entity = cx.entity().downgrade();
        cx.spawn(async move |_this, cx| {
            let doc_handle = peer.doc_handle();
            let mut change_stream = doc_handle.changes();

            // Read the initial display name from the doc
            if let Ok(display_name) = peer.read_display_name().await {
                entity.update(cx, |contact, cx| {
                    contact.data.name.display_name = display_name.into();
                    cx.notify();
                })?;
            }

            // Listen for subsequent changes
            while change_stream.next().await.is_some() {
                if let Ok(display_name) = peer.read_display_name().await {
                    entity.update(cx, |contact, cx| {
                        contact.data.name.display_name = display_name.into();
                        cx.notify();
                    })?;
                }
            }

            anyhow::Ok(())
        })
        .detach();

        cx.emit(ContactEvent::PeerConnected);
        cx.notify();
    }

    /// Removes the peer connection for this Contact.
    ///
    /// Emits a [`ContactEvent::PeerDisconnected`] so subscribers can react.
    pub fn clear_peer(&mut self, cx: &mut Context<Self>) {
        self.peer = None;
        cx.emit(ContactEvent::PeerDisconnected);
        cx.notify();
    }

    pub async fn save(
        &self,
        profile: Entity<Profile>,
        space: Entity<Space>,
        cx: &mut App,
    ) -> Result<()> {
        let namespace_id = space.read(cx).id();
        let subspace_id = profile.read(cx).id();
        let write_cap = profile.read(cx).willow_write_cap();
        let key_store = profile.read(cx).key_store();
        let profile_id = self.profile_id().as_bytes().encode_hex::<String>();
        let path = PathBuf::from(format!("/apps/{}/{}", Contacts::id(), profile_id));
        let data = serde_json::to_vec(&self.data).context("failed to serialize ContactData")?;

        let write_cap = write_cap.context("No WillowWrite cap; vault is locked")?;
        let key_store = key_store.context("No WillowKeyStore; vault is locked")?;
        cx.willow()
            .create_entry(
                &write_cap,
                &key_store,
                &namespace_id,
                &subspace_id,
                &path,
                data,
            )
            .await?;

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactTicket {
    message: ContactTicketMessage,
    signature: Vec<u8>,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactTicketMessage {
    #[serde_as(as = "SubspaceIdSerde")]
    sender_profile_id: SubspaceId,
    display_name: SharedString,
    addrs: EndpointAddr,
}

impl ContactTicketMessage {
    pub fn new(
        sender_profile_id: SubspaceId,
        display_name: SharedString,
        addrs: EndpointAddr,
    ) -> Self {
        Self {
            sender_profile_id,
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
        // TODO verify hash
        self.message
            .sender_profile_id
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
