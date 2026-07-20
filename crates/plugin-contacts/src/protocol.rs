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

use anyhow::{Context, Result, anyhow, bail};
use automerge::Automerge;
use autosurgeon::{Hydrate, Reconcile};
use chrono::{DateTime, TimeDelta, Utc};
use dashmap::DashMap;
use futures::{SinkExt, Stream};
use iroh::{
    Endpoint,
    endpoint::{Connection, SendStream},
    protocol::{AcceptError, ProtocolHandler},
};
use iroh_ping::Ping;
use plugin_automerge::{Automergable, protocol::AutomergeProtocol};
use plugin_p2p::KeyedProtocolHandler;
use plugin_willow::willow_serde::SubspaceIdSerde;
use samod::{DocHandle, DocumentId, PeerId};
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
use zed::unstable::{
    db::smol::stream::StreamExt,
    gpui::{AppContext, Entity},
    ui::SharedString,
    util::ResultExt as _,
};

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
    contact_peer_state: Arc<DashMap<SubspaceId, ContactPeer>>,

    /// The Iroh Endpoint used for this protocol.
    endpoint: Endpoint,

    /// The display name of the local profile, used when creating new contact docs.
    local_display_name: SharedString,

    /// Profile key for the local profile.
    #[debug("ProfileKey")]
    local_profile_key: SubspaceSecret,

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
    /// The Automerge protocol instance for this contact peer.
    automerge: AutomergeProtocol,

    /// The Iroh Connection established with the remote peer.
    connection: Connection,

    /// The send half of the control stream for this contact connection.
    control_stream_send: SendStream,

    /// The handle to this peer's own contact identity document, if we have it.
    doc_handle: DocHandle,

    /// The Profile ID and display name of the remote contact.
    name: ContactName,

    /// Iroh protocol pinging with the remote Contact.
    ///
    /// Used to measure the latency of connection between two Contacts/Peers.
    ping: Ping,
}

impl ContactPeer {
    /// Create a new `ContactPeer` with a new Automerge document,
    /// initialized with the given profile data.
    pub async fn new(
        connection: Connection,
        control_stream_send: SendStream,
        name: ContactName,
        automerge: AutomergeProtocol,
        ping: Ping,
        initial_data: ContactData,
    ) -> Result<Self> {
        let Some(repo) = automerge.repo() else {
            bail!("Automerge protocol repo not initialized");
        };

        let doc = Automerge::new();
        let doc_handle = repo.create(doc).await?;

        // Write the initial profile data into the document
        doc_handle.with_document(|am| {
            let mut tx = am.transaction();
            autosurgeon::reconcile(&mut tx, &initial_data)?;
            let (_change_hash, _patch_log) = tx.commit();
            anyhow::Ok(())
        })?;

        Self::with_doc_handle(
            connection,
            control_stream_send,
            name,
            automerge,
            ping,
            doc_handle,
        )
        .await
    }

    pub async fn is_connected(&self) -> bool {
        //
        todo!()
    }

    /// Create a new `ContactPeer` with an existing Automerge document handle.
    pub async fn with_doc_handle(
        connection: Connection,
        control_stream_send: SendStream,
        name: ContactName,
        automerge: AutomergeProtocol,
        ping: Ping,
        doc_handle: DocHandle,
    ) -> Result<Self> {
        Ok(Self {
            state: Arc::new(ContactPeerState {
                automerge,
                connection,
                control_stream_send,
                doc_handle,
                name,
                ping,
            }),
        })
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

    /// Get the automerge doc handle for this contact peer.
    pub fn doc_handle(&self) -> DocHandle {
        self.state.doc_handle.clone()
    }

    /// Get the automerge protocol instance.
    pub fn automerge(&self) -> &AutomergeProtocol {
        &self.state.automerge
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
            contact_peer_state: Default::default(),
            endpoint,
            local_display_name,
            local_profile_key: local_key,
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
        let automerge = self.automerge.clone();
        let ping = self.ping.clone();
        let local_display_name = self.inner.local_display_name.clone();
        let local_profile_key = self.inner.local_profile_key.clone();
        let contact_peer_state = self.inner.contact_peer_state.clone();
        async move {
            // NOTE: Requires dialer to send a message on open_bi stream to pass this await
            info!("Contacts ProtocolHandler waiting for handshake");
            let (mut control_stream_send, mut recv_control_stream) = connection.accept_bi().await?;
            info!("Contacts ProtocolHandler handshake received");

            let remote_endpoint_id = connection.remote_id();
            let remote_addr = iroh::EndpointAddr::from(remote_endpoint_id);

            let mut control_stream =
                Framed::new(&mut recv_control_stream, LengthDelimitedCodec::new());

            // Handle initial message to establish contact state
            let mut stream = (async move || {
                let Some(frame) = control_stream.try_next().await? else {
                    bail!("missing expected handshake for ContactsProtocol");
                };

                let envelope = serde_json::from_slice::<SignedEnvelope<ContactsMessage>>(&frame)
                    .context("failed to parse ContactsProtocol handshake")?;

                // Verify the signature of the message, or return error
                envelope
                    .verify()
                    .await
                    .context("failed to verify handshake message for ContactsProtocol")?;

                // Ensure the message was intended for us
                if envelope.meta.sender_id
                    != local_profile_key.corresponding_subspace_id()
                {
                    bail!("handshake message was not intended for us");
                }

                let remote_profile_id = envelope.meta.sender_id.clone();

                #[allow(unreachable_patterns)]
                match &envelope.message {
                    ContactsMessage::V1(ContactsMessageV1::EstablishControlStream {
                        doc_id,
                    }) => {
                        // Dial the automerge protocol to connect to the dialer
                        info!("accept: dialing automerge peer");
                        automerge.dial_peer(remote_addr).await?;

                        let Some(repo) = automerge.repo() else {
                            bail!("Automerge protocol repo not initialized");
                        };

                        // Wait for the automerge connection to be established
                        repo.when_connected(PeerId::from_string(
                                remote_endpoint_id.to_string(),
                            ))
                            .await?;
                        info!("accept: connected to automerge peer");

                        // Find the remote's profile document
                        let remote_doc_handle = repo
                            .find(doc_id.clone())
                            .await?
                            .context("failed to find remote ContactData document")?;
                        info!("accept: found remote ContactData document");

                        // Create a local doc with our profile data
                        let local_doc = Automerge::new();
                        let local_doc_handle = repo.create(local_doc).await?;
                        let local_data = ContactData {
                            display_name: local_display_name.to_string(),
                        };
                        local_doc_handle.with_document(|am| {
                            let mut tx = am.transaction();
                            autosurgeon::reconcile(&mut tx, &local_data)
                                .context("failed to write local ContactData")?;
                            let (_change_hash, _patch_log) = tx.commit();
                            anyhow::Ok(())
                        })?;
                        let local_doc_id = local_doc_handle.document_id().clone();

                        // Send AcceptConnectContact with our doc_id
                        {
                            let mut framed = Framed::new(
                                &mut control_stream_send,
                                LengthDelimitedCodec::default(),
                            );
                            let response =
                                ContactsMessage::V1(ContactsMessageV1::AcceptConnectContact {
                                    doc_id: local_doc_id,
                                });
                            let response_envelope = SignedEnvelope::new(
                                response,
                                local_profile_key.clone(),
                                remote_profile_id.clone(),
                            )
                            .await?;
                            let envelope_bytes = response_envelope.to_bytes()?;
                            framed.send(envelope_bytes.into()).await?;
                            info!("accept: sent AcceptConnectContact with doc_id");
                        }

                        let name = ContactName::new(
                            remote_profile_id.clone(),
                            SharedString::default(),
                        );
                        let contact_peer = ContactPeer::with_doc_handle(
                            connection,
                            control_stream_send,
                            name,
                            automerge,
                            ping,
                            remote_doc_handle,
                        )
                        .await?;
                        contact_peer_state
                            .insert(remote_profile_id.clone(), contact_peer);
                    }
                    _ => {
                        bail!("expected EstablishControlStream handshake as the first message in ContactsProtocol");
                    }
                }

                anyhow::Ok(control_stream)
            })()
            .await
            .map_err(|error| AcceptError::from_err(std::io::Error::new(ErrorKind::Other, error)))?;

            self.run_loop(&mut stream).await;
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
    /// Returns a list of Contacts that are currently connected.
    pub fn contacts(&self) {
        // TODO
    }

    /// Given a [`ContactTicket`] received from a remote peer, establishes a connection
    /// to the Iroh endpoint where the target Profile is located.
    ///
    /// Creates an Automerge document with the local profile's data and sends its
    /// `DocumentId` in the handshake so the remote peer can open the same document.
    pub async fn connect(&self, ticket: &ContactTicket) -> Result<ContactPeer> {
        let addrs = ticket.message.addrs.clone();
        let automerge_addrs = addrs.clone();
        let ticket_sender_profile_id = ticket.message.sender_profile_id.clone();
        let ticket_sender_display_name = ticket.message.display_name.clone();

        info!(
            ?ticket_sender_profile_id,
            ?addrs,
            "ContactsProtocol::connect"
        );

        let connection = self.endpoint.connect(addrs, Self::ALPN).await?;
        let (mut control_stream_send, mut control_stream_recv) = connection.open_bi().await?;
        info!("connect: open_bi completed");

        // Create a doc with the local profile's display name, before sending the handshake
        // so we can include the doc_id in the EstablishControlStream message.
        let automerge = self.automerge.clone();
        let ping = self.ping.clone();
        let Some(repo) = automerge.repo() else {
            bail!("Automerge protocol repo not initialized");
        };

        let initial_data = ContactData {
            display_name: self.inner.local_display_name.to_string(),
        };
        let doc = Automerge::new();
        let doc_handle = repo.create(doc).await?;
        doc_handle.with_document(|am| {
            let mut tx = am.transaction();
            autosurgeon::reconcile(&mut tx, &initial_data)
                .context("failed to write initial ContactData to doc")?;
            let (_change_hash, _patch_log) = tx.commit();
            anyhow::Ok(())
        })?;
        let doc_id = doc_handle.document_id().clone();

        // Send the handshake with our doc_id
        {
            let mut framed = Framed::new(&mut control_stream_send, LengthDelimitedCodec::default());
            let message = ContactsMessage::V1(ContactsMessageV1::EstablishControlStream {
                doc_id: doc_id.clone(),
            });
            let envelope = SignedEnvelope::new(
                message,
                self.inner.local_profile_key.clone(),
                ticket_sender_profile_id.clone(),
            )
            .await?;
            let envelope_bytes = envelope.to_bytes()?;
            framed.send(envelope_bytes.into()).await?;
            info!("connect: Sent handshake with doc_id");
        }

        // Dial the automerge protocol to start syncing the document
        automerge.dial_peer(automerge_addrs).await?;

        let name = ContactName::new(
            ticket_sender_profile_id.clone(),
            ticket_sender_display_name.clone(),
        );
        let contact_peer = ContactPeer::with_doc_handle(
            connection,
            control_stream_send,
            name,
            automerge,
            ping,
            doc_handle,
        )
        .await?;
        self.inner
            .contact_peer_state
            .insert(ticket_sender_profile_id.clone(), contact_peer.clone());

        let protocol = self.clone();
        tokio::task::spawn(async move {
            let mut framed_recv =
                Framed::new(&mut control_stream_recv, LengthDelimitedCodec::default());
            info!("connect: Starting control loop");
            protocol.run_loop(&mut framed_recv).await;
        });

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
            self.try_handle_frame(frame).await?;
        }

        Ok(())
    }

    async fn try_handle_frame(&self, frame: BytesMut) -> Result<()> {
        let signed_envelope = serde_json::from_slice::<SignedEnvelope<ContactsMessage>>(&frame)?;

        // Verify the signature of the message, or return error
        signed_envelope.verify().await?;

        // Ensure the message was intended for us
        if signed_envelope.meta.sender_id
            != self.inner.local_profile_key.corresponding_subspace_id()
        {
            bail!("handshake message was not intended for us");
        }

        match &signed_envelope.message {
            ContactsMessage::V1(v1) => {
                let sender_profile_id = &signed_envelope.meta.sender_id;
                self.try_handle_v1_message(sender_profile_id, v1).await?;
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
            ContactsMessageV1::AcceptConnectContact { doc_id } => {
                info!(
                    ?sender_profile_id,
                    ?doc_id,
                    "Received AcceptConnectContact, opening remote doc"
                );

                let Some(repo) = self.automerge.repo() else {
                    bail!("Automerge protocol repo not initialized");
                };

                // Find the remote's profile document via automerge sync
                let _remote_doc_handle = repo
                    .find(doc_id.clone())
                    .await?
                    .context("failed to find remote ContactData document")?;
                info!("Opened remote ContactData document");

                // Update the ContactPeer in our state to use the remote's doc handle
                if let Some(_peer) = self.contact_peer_state.get_mut(sender_profile_id) {
                    // We can't easily swap the doc_handle since it's behind an Arc,
                    // but the remote doc is now in the repo and will sync automatically.
                    // The ContactPeer already has its own doc_handle for the local profile.
                    // For now, we log that the remote doc is available.
                    info!(
                        "Remote doc {} is now syncing for contact {:?}",
                        doc_id, sender_profile_id
                    );
                }
            }
            _ => {}
        }
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
    /// Carries the `DocumentId` of the sender's profile data document.
    EstablishControlStream {
        /// The Automerge document ID containing the sender's profile data.
        doc_id: DocumentId,
    },
    /// Request to connect with a contact by that contact's Profile ID
    RequestConnectContact {
        #[serde_as(as = "SubspaceIdSerde")]
        target_contact: SubspaceId,
    },
    /// Accept a connection request, carrying the responder's profile document ID.
    AcceptConnectContact {
        /// The Automerge document ID containing the responder's profile data.
        doc_id: DocumentId,
    },
}
