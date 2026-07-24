//! Contains the [`ContactsProtocol`], which provides p2p functionality for the contacts plugin.
//!
//! The Contacts plugin is responsible for connecting two Profiles to each other over p2p.
//! It's like an address book that also maintains network connections to each contact.
//!
//! The [`ContactsProtocol`] acts both as a receiver that serves incoming requests (as an Iroh
//! [`ProtocolHandler`]) and as a client, providing a programmatic API for initiating requests.
//!
//! On plugin initialization, the [`ContactsProtocol`] is registered with the Iroh runtime and
//! begins listening for requests.
//!
//! The first real unit of work begins with a [`ContactsProtocol::connect`] call, which initiates
//! a connection to a remote endpoint, requesting to connect with a specific remote [`Profile`].

use std::{io::ErrorKind, ops::Deref, sync::Arc};

use anyhow::{Result, anyhow, bail};
use automerge::Automerge;
use autosurgeon::{Hydrate, Reconcile};
use chrono::{DateTime, TimeDelta, Utc};
use dashmap::DashMap;
use futures::{SinkExt, Stream, TryStreamExt as _};
use gpui::{AppContext, Entity, SharedString};
use gpui_util::ResultExt as _;
use iroh::{
    Endpoint,
    endpoint::{Connection, SendStream},
    protocol::{AcceptError, ProtocolHandler},
};
use iroh_ping::Ping;
use plugin_automerge::{Automergable, protocol::AutomergeProtocol};
use plugin_p2p::KeyedProtocolHandler;
use plugin_willow::willow_serde::SubspaceIdSerde;
use samod::DocHandle;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use signature::SignerMut as _;
use tokio::task::LocalSet;
use tokio_util::{
    bytes::BytesMut,
    codec::{Decoder, Framed, LengthDelimitedCodec},
};
use tracing::info;
use ufotofu::{IntoConsumer as _, IntoProducer as _, codec::Decodable as _};
use willow25::entry::{SubspaceId, SubspaceSecret, SubspaceSignature};

use crate::{ContactName, ContactTicket};

/// The [`ContactsProtocol`] object, which is installed into the Iroh protocols
/// stack and is also used as the client-side API object.
#[derive(derive_more::Debug, Clone)]
pub struct ContactsProtocol {
    inner: Arc<ContactsProtocolState>,
}

impl Deref for ContactsProtocol {
    type Target = Arc<ContactsProtocolState>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// The inner shared state for the [`ContactsProtocol`].
#[derive(derive_more::Debug)]
pub struct ContactsProtocolState {
    /// The Automerge protocol instance for this contacts protocol.
    automerge: AutomergeProtocol,

    /// The state of the connection to each contact's peer.
    contact_peers: Arc<DashMap<SubspaceId, ContactPeer>>,

    /// The Iroh Endpoint used for this protocol.
    endpoint: Endpoint,

    /// The display name of the local profile, used when creating new contact docs.
    local_display_name: SharedString,

    /// Profile key for the local profile.
    #[debug("ProfileKey")]
    local_profile_key: SubspaceSecret,

    /// Pending ContactPeers have received a `RequestConnectContact` but have not
    /// been approved by the user.
    pending_contact_peers: Arc<DashMap<SubspaceId, ContactPeer>>,

    /// The ping protocol object, used to check latency
    ping: Ping,
}

/// A shared object containing the p2p connection state for one contact.
#[derive(Debug, Clone)]
pub struct ContactPeer {
    state: Arc<ContactPeerState>,
}

impl Automergable for ContactPeer {
    type Crdt = ContactData;

    fn doc_handle(&self) -> DocHandle {
        self.state.doc_handle.clone()
    }
}

/// The profile data shared via automerge for a contact.
#[derive(Debug, Clone, Hydrate, Reconcile)]
pub struct ContactData {
    /// The display name of the profile this contact represents.
    pub display_name: String,
}

#[derive(Debug)]
pub struct ContactPeerState {
    /// The Iroh Connection established with the remote peer.
    connection: Connection,

    /// The send half of the control stream for this contact connection.
    control_stream_send: SendStream,

    /// The handle to this peer's own contact identity document.
    doc_handle: DocHandle,

    /// The Profile ID and display name of the remote contact.
    name: ContactName,
}

impl ContactPeer {
    /// Create a new `ContactPeer` with a new Automerge document,
    /// initialized with the given profile data.
    pub async fn new(
        connection: Connection,
        control_stream_send: SendStream,
        name: ContactName,
        protocol: ContactsProtocol,
    ) -> Result<Self> {
        let Some(repo) = protocol.automerge.repo() else {
            bail!("Automerge repo not initialized")
        };
        let doc = Automerge::new();
        let doc_handle = repo.create(doc).await?;

        Ok(Self::with_doc_handle(
            connection,
            control_stream_send,
            name,
            doc_handle,
        ))
    }

    /// Create a new `ContactPeer` with an existing Automerge document handle.
    pub fn with_doc_handle(
        connection: Connection,
        control_stream_send: SendStream,
        name: ContactName,
        doc_handle: DocHandle,
    ) -> Self {
        let state = Arc::new(ContactPeerState {
            connection,
            control_stream_send,
            doc_handle,
            name,
        });
        Self { state }
    }

    /// Read the display name from the automerge document.
    pub async fn read_display_name(&self) -> Result<String> {
        let doc_handle = self.state.doc_handle.clone();
        let result = tokio::task::spawn_blocking(move || {
            doc_handle.with_document(|am| {
                autosurgeon::hydrate::<_, ContactData>(am)
                    .map(|data| data.display_name)
                    .map_err(|e| anyhow::anyhow!("failed to hydrate ContactData: {e}"))
            })
        })
        .await?;
        result
    }

    /// Get the profile ID of this contact peer.
    pub fn profile_id(&self) -> &SubspaceId {
        &self.state.name.profile_id
    }
}

impl ContactsProtocol {
    pub fn new(
        endpoint: Endpoint,
        automerge: AutomergeProtocol,
        ping: Ping,
        local_key: SubspaceSecret,
        local_display_name: SharedString,
    ) -> Self {
        let inner = Arc::new(ContactsProtocolState {
            automerge,
            contact_peers: Default::default(),
            endpoint,
            local_display_name,
            local_profile_key: local_key,
            pending_contact_peers: Default::default(),
            ping,
        });

        Self { inner }
    }
}

impl KeyedProtocolHandler for ContactsProtocol {
    const ALPN: &[u8] = b"/galvanized/contacts/1";
}

impl ProtocolHandler for ContactsProtocol {
    fn accept(
        &self,
        connection: Connection,
    ) -> impl Future<Output = Result<(), AcceptError>> + Send {
        async move {
            // NOTE: Requires dialer to send a message on open_bi stream to pass this await
            info!("Contacts ProtocolHandler waiting for handshake");
            let (control_stream_send, mut recv_control_stream) = connection.accept_bi().await?;
            info!("Contacts ProtocolHandler handshake received");

            let mut control_stream =
                Framed::new(&mut recv_control_stream, LengthDelimitedCodec::new());

            // Handle initial message to establish contact state
            let mut control_stream = (async move || {
                let Some(frame) = control_stream.try_next().await? else {
                    bail!("missing expected handshake for ContactsProtocol");
                };

                let signed_envelope = self.try_receive_frame(frame).await?;
                let ContactsMessage::V1(ContactsMessageV1::RequestConnectContact(request)) =
                    signed_envelope.message
                else {
                    bail!(
                        "expected ContactsMessageV1, got {:?}",
                        signed_envelope.message
                    )
                };

                let sender_id = signed_envelope.meta.sender_id;
                let name = ContactName::new(sender_id.clone(), &request.sender_display_name);
                let contact_peer =
                    ContactPeer::new(connection, control_stream_send, name, self.clone()).await?;

                // Add this peer to the pending list, contingent on this user accepting
                self.pending_contact_peers.insert(sender_id, contact_peer);

                anyhow::Ok(control_stream)
            })()
            .await
            .map_err(|error| AcceptError::from_err(std::io::Error::new(ErrorKind::Other, error)))?;

            self.run_loop(&mut control_stream).await;
            Ok(())
        }
    }
}

pub trait ContactsProtocolHandle {
    type Result<T>;
    fn connect<C: AppContext>(
        &self,
        ticket: &ContactTicket,
        cx: &mut C,
    ) -> impl Future<Output = Self::Result<ContactPeer>>;
}

impl ContactsProtocolHandle for Entity<ContactsProtocol> {
    type Result<T> = anyhow::Result<T>;
    fn connect<C: AppContext>(
        &self,
        ticket: &ContactTicket,
        cx: &mut C,
    ) -> impl Future<Output = Self::Result<ContactPeer>> {
        let protocol = cx.read_entity(self, |it, _cx| it.clone());
        async move {
            let peer = protocol.connect(ticket).await?;
            anyhow::Ok(peer)
        }
    }
}

type FrameItem = Result<BytesMut, <LengthDelimitedCodec as Decoder>::Error>;

impl ContactsProtocol {
    /// Given a [`ContactTicket`] received from a remote peer, establishes a connection
    /// to the Iroh endpoint where the target Profile is located.
    ///
    /// Creates an Automerge document with the local profile's data and sends its
    /// `DocumentId` in the handshake so the remote peer can open the same document.
    pub async fn connect(&self, ticket: &ContactTicket) -> Result<ContactPeer> {
        let addrs = ticket.message.addrs.clone();
        let ticket_sender_profile_id = ticket.message.sender_profile_id.clone();
        let ticket_sender_display_name = ticket.message.display_name.clone();
        let name = ContactName::new(
            ticket_sender_profile_id.clone(),
            ticket_sender_display_name.clone(),
        );

        info!(
            ?ticket_sender_profile_id,
            ?addrs,
            "ContactsProtocol::connect"
        );

        let connection = self.endpoint.connect(addrs, Self::ALPN).await?;
        let (mut control_stream_send, control_stream_recv) = connection.open_bi().await?;
        info!("connect: open_bi completed");

        // Spawn this peer's event loop to handle incoming messages
        // Start this before sending the outbound request to avoid
        // missing the response due to a race condition
        let this = self.clone();
        tokio::task::spawn(async move {
            let mut framed_recv = Framed::new(control_stream_recv, LengthDelimitedCodec::new());
            this.run_loop(&mut framed_recv).await;
        });

        // Send outbound request asking for a new connection to a contact
        {
            let mut framed = Framed::new(&mut control_stream_send, LengthDelimitedCodec::default());
            let message = RequestConnectContact::new(
                self.local_display_name.clone(),
                ticket_sender_profile_id.clone(),
            );
            let envelope = SignedEnvelope::new(
                message,
                self.inner.local_profile_key.clone(),
                ticket_sender_profile_id.clone(),
            )
            .await?;
            let envelope_bytes = envelope.to_bytes()?;
            framed.send(envelope_bytes.into()).await?;
        }

        // Store initial state for the contact peer
        let contact_peer =
            ContactPeer::new(connection, control_stream_send, name, self.clone()).await?;
        self.pending_contact_peers
            .insert(ticket_sender_profile_id, contact_peer.clone());

        Ok(contact_peer)
    }

    async fn run_loop(&self, stream: &mut (impl Unpin + Stream<Item = FrameItem>)) {
        self.try_run_loop(stream).await.log_err();
    }

    async fn try_run_loop(
        &self,
        stream: &mut (impl Unpin + Stream<Item = FrameItem>),
    ) -> Result<()> {
        while let Some(frame) = stream.try_next().await? {
            let signed_envelope = self.try_receive_frame(frame).await?;
            self.try_handle_message(&signed_envelope).await?;
        }

        Ok(())
    }

    /// Deserializes and verifies a message from a frame from the control stream.
    async fn try_receive_frame(&self, frame: BytesMut) -> Result<SignedEnvelope<ContactsMessage>> {
        let signed_envelope = serde_json::from_slice::<SignedEnvelope<ContactsMessage>>(&frame)?;

        // Verify the signature of the message, or return error
        signed_envelope.verify().await?;

        let sender_profile_id = &signed_envelope.meta.sender_id;

        // Ensure the message was intended for us
        if sender_profile_id != &self.inner.local_profile_key.corresponding_subspace_id() {
            bail!("handshake message was not intended for us");
        }

        Ok(signed_envelope)
    }

    async fn try_handle_message(&self, envelope: &SignedEnvelope<ContactsMessage>) -> Result<()> {
        match &envelope.message {
            ContactsMessage::V1(v1) => {
                self.try_handle_v1_message(&envelope.meta.sender_id, v1)
                    .await?;
            }
        }

        Ok(())
    }

    async fn try_handle_v1_message(
        &self,
        sender_profile_id: &SubspaceId,
        v1: &ContactsMessageV1,
    ) -> Result<()> {
        match v1 {
            ContactsMessageV1::RequestConnectContact(_request) => {
                bail!("RequestConnectContact must be the first message handled");
            }
            ContactsMessageV1::AcceptConnectContact(request) => {
                self.try_handle_accept_connect_contact(
                    sender_profile_id,
                    &request.requester_profile_id,
                )
                .await?;
            }
            ContactsMessageV1::RejectConnectContact(reject_connect_contact) => {
                info!(?reject_connect_contact, "Received RejectConnectContact");
                self.pending_contact_peers.remove(sender_profile_id);
            }
        }
        Ok(())
    }

    async fn try_handle_accept_connect_contact(
        &self,
        sender_profile_id: &SubspaceId,
        requester_profile_id: &SubspaceId,
    ) -> Result<()> {
        info!(
            ?sender_profile_id,
            ?requester_profile_id,
            "Received AcceptConnectContact, opening remote doc"
        );

        // We were the original "requester", so ensure the requester's profile ID matches our own
        if sender_profile_id != &self.local_profile_key.corresponding_subspace_id() {
            bail!("Requester profile ID does not match our own");
        }

        // If we're receiving this, the recipient of our connection request has accepted it.
        let Some((id, peer)) = self.pending_contact_peers.remove(sender_profile_id) else {
            bail!("No pending contact peer found for sender");
        };

        info!("Contact accepted connection request, moving peer from pending to active");
        self.contact_peers.insert(id, peer);
        Ok(())
    }
}

pub trait Message: Serialize + for<'de> Deserialize<'de> {}
impl<M: Serialize + for<'de> Deserialize<'de>> Message for M {}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignedEnvelope<M> {
    /// Metadata including the sender's profile ID and timestamp of the envelope.
    meta: EnvelopeMetadata,

    /// The message contents, including type and any fields
    message: M,

    /// The signature of the envelope, verifying the sender's identity.
    signature: Vec<u8>,
}

/// Signing an envelope containing the key's own public key authenticates us as the sender.
#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
pub struct EnvelopeMetadata {
    /// Profile/Subspace ID of the sender requesting contact.
    #[serde_as(as = "SubspaceIdSerde")]
    sender_id: SubspaceId,

    /// Profile/Subspace ID of the recipient of the message.
    #[serde_as(as = "SubspaceIdSerde")]
    recipient_id: SubspaceId,

    /// Timestamp when the request was created.
    timestamp: DateTime<Utc>,
}

impl<M: Message> SignedEnvelope<M> {
    /// Create a new signed envelope with the given message and profile key,
    /// timestamped at the current time.
    pub async fn new(
        message: M,
        sender_key: SubspaceSecret,
        recipient_id: SubspaceId,
    ) -> Result<Self> {
        Self::new_at(message, sender_key, recipient_id, Utc::now()).await
    }

    /// Create a new signed envelope with the given message and profile key,
    /// timestamped at the specified time.
    pub async fn new_at(
        message: M,
        mut sender_key: SubspaceSecret,
        recipient_id: SubspaceId,
        timestamp: DateTime<Utc>,
    ) -> Result<Self> {
        let meta = EnvelopeMetadata {
            sender_id: sender_key.corresponding_subspace_id(),
            recipient_id,
            timestamp,
        };
        let envelope_bytes = serde_json::to_vec(&meta).unwrap();
        let subspace_signature = sender_key.try_sign(&envelope_bytes)?;
        let mut signature = Vec::new();
        let mut consumer = (&mut signature).into_consumer();

        use ufotofu::codec::Encodable;
        subspace_signature.encode(&mut consumer).await?;
        drop(consumer);

        Ok(Self {
            meta,
            message,
            signature,
        })
    }

    pub async fn verify(&self) -> Result<()> {
        self.verify_at(Utc::now()).await
    }

    pub async fn verify_at(&self, now: DateTime<Utc>) -> Result<()> {
        use signature::Verifier;
        let envelope_bytes = serde_json::to_vec(&self.meta).unwrap();
        let signature = self.signature.clone();
        let mut sig_producer = signature.into_producer();

        let delta = now - self.meta.timestamp;
        if delta.abs() > TimeDelta::minutes(10) {
            bail!("timestamp is more than 10 minutes old, rejecting as stale");
        }

        let subspace_signature = tokio::task::block_in_place(|| {
            let local = LocalSet::new();
            let runtime = tokio::runtime::Builder::new_current_thread().build()?;
            local.block_on(&runtime, async move {
                let fut = SubspaceSignature::decode(&mut sig_producer);
                let subspace_signature = fut
                    .await
                    .map_err(|_| anyhow!("failed to decode ticket signature"))?;
                anyhow::Ok(subspace_signature)
            })
        })?;

        self.meta
            .sender_id
            .verify(&envelope_bytes, &subspace_signature)?;
        Ok(())
    }

    pub async fn from_bytes(data: &[u8]) -> Result<Self> {
        let envelope: SignedEnvelope<M> =
            serde_json::from_slice(data).map_err(|_| anyhow!("failed to parse envelope"))?;
        envelope.verify().await?;
        Ok(envelope)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        serde_json::to_vec(self).map_err(|_| anyhow!("failed to serialize envelope"))
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "version", content = "content")]
pub enum ContactsMessage {
    //
    V1(ContactsMessageV1),
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "content")]
pub enum ContactsMessageV1 {
    /// A handshake used to complete the `accept_bi` call and open a control stream.
    RequestConnectContact(RequestConnectContact),

    /// Accept a request to connect with a contact.
    AcceptConnectContact(AcceptConnectContact),

    /// Reject a request to connect with a contact.
    RejectConnectContact(RejectConnectContact),
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
pub struct RequestConnectContact {
    /// The display name of the sender Profile.
    sender_display_name: SharedString,

    /// As the sender, the target_contact is the profile ID of the contact I'm trying to connect with.
    ///
    /// As the receiver, the target_contact should by MY profile ID, I need to verify it or discard.
    #[serde_as(as = "SubspaceIdSerde")]
    target_contact: SubspaceId,
}

impl RequestConnectContact {
    pub fn new(sender_display_name: SharedString, target_contact: SubspaceId) -> ContactsMessage {
        ContactsMessage::V1(ContactsMessageV1::RequestConnectContact(Self {
            sender_display_name,
            target_contact,
        }))
    }
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
pub struct AcceptConnectContact {
    /// The Profile ID of the requester who we are accepting the connection from.
    #[serde_as(as = "SubspaceIdSerde")]
    requester_profile_id: SubspaceId,
}

impl AcceptConnectContact {
    pub fn new(requester_profile_id: SubspaceId) -> ContactsMessage {
        ContactsMessage::V1(ContactsMessageV1::AcceptConnectContact(Self {
            requester_profile_id,
        }))
    }
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
pub struct RejectConnectContact {
    /// The Profile ID of the requester who we are rejecting the connection from.
    #[serde_as(as = "SubspaceIdSerde")]
    requester_profile_id: SubspaceId,
}

impl RejectConnectContact {
    pub fn new(requester_profile_id: SubspaceId) -> ContactsMessage {
        ContactsMessage::V1(ContactsMessageV1::RejectConnectContact(Self {
            requester_profile_id,
        }))
    }
}
