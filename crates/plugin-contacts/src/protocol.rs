use std::{io::ErrorKind, sync::Arc};

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, TimeDelta, Utc};
use dashmap::DashMap;
use futures::{SinkExt, Stream};
use iroh::{
    Endpoint, EndpointAddr,
    endpoint::{Connection, SendStream},
    protocol::{AcceptError, ProtocolHandler},
};
use plugin_willow::willow_serde::SubspaceIdSerde;
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
use zed::unstable::{db::smol::stream::StreamExt, util::ResultExt as _};

#[derive(derive_more::Debug, Clone)]
pub struct ContactsProtocol {
    inner: Arc<ContactsProtocolState>,
}

#[derive(derive_more::Debug)]
pub struct ContactsProtocolState {
    /// The state of the connection to each contact.
    contacts_state: Arc<DashMap<SubspaceId, ContactState>>,

    /// Profile key for the local profile.
    #[debug("ProfileKey")]
    local_profile_key: SubspaceSecret,
}

#[derive(Debug)]
pub struct ContactState {
    /// The Iroh Connection established with the remote peer.
    connection: Connection,

    /// The send half of the control stream for this contact connection.
    control_stream_send: SendStream,
}

impl ContactsProtocol {
    pub const ALPN: &[u8] = b"/galvanized/contacts/1";

    pub fn new(local_key: SubspaceSecret) -> Self {
        let inner = Arc::new(ContactsProtocolState {
            contacts_state: Default::default(),
            local_profile_key: local_key,
        });

        Self { inner }
    }
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

            let contact_state = ContactState {
                connection,
                control_stream_send,
            };

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
                    != self.inner.local_profile_key.corresponding_subspace_id()
                {
                    bail!("handshake message was not intended for us");
                }

                #[allow(unreachable_patterns)]
                match &envelope.meta.message {
                    ContactsMessage::V1(ContactsMessageV1::RequestConnectContact {
                        target_contact,
                    }) => {
                        self.inner
                            .contacts_state
                            .insert(target_contact.clone(), contact_state);
                        info!(?target_contact, "Received handshake, added Contact state");
                    }
                    _ => {
                        bail!("expected handshake as the first message in ContactsProtocol");
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

type FrameItem = Result<BytesMut, <LengthDelimitedCodec as Decoder>::Error>;

impl ContactsProtocol {
    /// Returns a list of Contacts that are currently connected.
    pub fn contacts(&self) {
        // TODO
    }

    pub async fn connect(
        &self,
        endpoint: &Endpoint,
        target_profile_id: &SubspaceId,
        addrs: EndpointAddr,
    ) -> Result<()> {
        info!(?target_profile_id, ?addrs, "ContactsProtocol::connect");

        let connection = endpoint.connect(addrs, Self::ALPN).await?;
        let (mut control_stream_send, mut control_stream_recv) = connection.open_bi().await?;
        info!("connect: open_bi completed");

        {
            let mut framed = Framed::new(&mut control_stream_send, LengthDelimitedCodec::default());
            let message = ContactsMessage::V1(ContactsMessageV1::RequestConnectContact {
                target_contact: target_profile_id.clone(),
            });
            let envelope =
                SignedEnvelope::new(message, self.inner.local_profile_key.clone()).await?;
            let envelope_bytes = envelope.to_bytes()?;
            framed.send(envelope_bytes.into()).await?;
            info!("connect: Sent handshake");
        }

        let contact_state = ContactState {
            connection,
            control_stream_send,
        };
        self.inner
            .contacts_state
            .insert(target_profile_id.clone(), contact_state);

        let mut framed_recv =
            Framed::new(&mut control_stream_recv, LengthDelimitedCodec::default());
        info!("connect: Starting control loop");
        self.run_loop(&mut framed_recv).await;
        Ok(())
    }

    async fn run_loop(&self, stream: &mut (impl Unpin + Stream<Item = FrameItem>)) {
        loop {
            self.try_run_loop(stream).await.log_err();
        }
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

        match &signed_envelope.meta.message {
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
        //
        Ok(())
    }
}

pub trait Message: Serialize + for<'de> Deserialize<'de> {}
impl<M: Serialize + for<'de> Deserialize<'de>> Message for M {}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignedEnvelope<M> {
    meta: MetaEnvelope<M>,
    signature: Vec<u8>,
}

/// Signing an envelope containing the key's own public key authenticates us as the sender.
#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
pub struct MetaEnvelope<M> {
    /// Profile/Subspace ID of the sender requesting contact.
    #[serde_as(as = "SubspaceIdSerde")]
    sender_id: SubspaceId,

    /// Timestamp when the request was created.
    timestamp: DateTime<Utc>,

    /// Message contents.
    message: M,
}

impl<M: Message> SignedEnvelope<M> {
    /// Create a new signed envelope with the given message and profile key,
    /// timestamped at the current time.
    pub async fn new(message: M, profile_key: SubspaceSecret) -> Result<Self> {
        Self::new_at(message, profile_key, Utc::now()).await
    }

    /// Create a new signed envelope with the given message and profile key,
    /// timestamped at the specified time.
    pub async fn new_at(
        message: M,
        mut profile_key: SubspaceSecret,
        timestamp: DateTime<Utc>,
    ) -> Result<Self> {
        let meta = MetaEnvelope {
            sender_id: profile_key.corresponding_subspace_id(),
            timestamp,
            message,
        };
        let envelope_bytes = serde_json::to_vec(&meta).unwrap();
        let subspace_signature = profile_key.try_sign(&envelope_bytes)?;
        let mut signature = Vec::new();
        let mut consumer = (&mut signature).into_consumer();

        use ufotofu::codec::Encodable;
        subspace_signature.encode(&mut consumer).await?;
        drop(consumer);

        Ok(Self { meta, signature })
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
    /// Request to connect with a contact by that contact's Profile ID
    RequestConnectContact {
        #[serde_as(as = "SubspaceIdSerde")]
        target_contact: SubspaceId,
    },
    AcceptConnectContact {
        //
    },
}
