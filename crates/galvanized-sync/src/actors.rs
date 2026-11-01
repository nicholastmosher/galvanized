//! The sans-IO actor core: a [`Hub`] that routes by [`DocumentId`] and a
//! [`DocumentActor`] per document, with the authorization gate and
//! [`SyncMode`] plumbing wired through the outgoing and incoming paths.
//!
//! The engine emits [`IoTask`] effects and the runtime (ticket 007) executes
//! them. Authorization is deny-by-default: no authenticator/validator wired
//! → visible "authorization not configured" refusal; a `Deny` → visible per-doc
//! error and the connection stays up for other docs. There is no ungated
//! path.
//!
//! The automerge sync logic (generate/receive sync messages) is real
//! mechanism via `automerge::sync`; the *wire framing* (CBOR protocol) is
//! ticket 004 — here the sync payload is carried as opaque bytes.

use std::collections::HashMap;
use std::sync::Arc;

use automerge::sync::{Message as AutoSyncMessage, State as SyncState, SyncDoc};
use automerge::{Automerge, AutomergeError};
use thiserror::Error;
use willow25::entry::SubspaceId;

use crate::authorization::{
    AuthenticationError, AuthorizationError, CapabilityValidator, PeerAuthenticator, SyncDirection,
    SyncMode,
};
use crate::document_id::DocumentId;
use crate::identity::PeerIdentity;
use crate::io::{ConnectionId, IoResult, IoTask, IoTaskId, Storage};

/// Identifies a document actor within a [`Hub`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DocumentActorId(pub u64);

/// Identifies an app command awaiting completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CommandId(pub u64);

/// Engine run state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunState {
    Running,
    Stopped,
}

/// A coarse refusal reason sent on the wire (ruling 004 Q4: coarse category on the
/// wire, detailed reason local only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum WireError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("expired")]
    Expired,
    #[error("revoked")]
    Revoked,
    #[error("authorization not configured")]
    NotConfigured,
    #[error("decode error")]
    Decode,
}

impl From<AuthorizationError> for WireError {
    fn from(err: AuthorizationError) -> Self {
        match err {
            AuthorizationError::NotConfigured => WireError::NotConfigured,
            AuthorizationError::NoCap
            | AuthorizationError::WrongDirection
            | AuthorizationError::OutOfScope => WireError::Unauthorized,
            AuthorizationError::Expired => WireError::Expired,
            AuthorizationError::Revoked => WireError::Revoked,
        }
    }
}

impl From<AuthenticationError> for WireError {
    fn from(err: AuthenticationError) -> Self {
        match err {
            AuthenticationError::NotConfigured => WireError::NotConfigured,
            _ => WireError::Unauthorized,
        }
    }
}

/// A visible sync refusal for a (connection, document) pair.
#[derive(Debug, Clone)]
pub struct SyncRefusal {
    pub connection: ConnectionId,
    pub document_id: Option<DocumentId>,
    pub reason: WireError,
}

/// The result of a completed app command.
#[derive(Debug, Clone)]
pub enum CommandResult {
    Created(DocumentId),
    Found(DocumentActorId),
    DocumentUnavailable(DocumentId),
    Stopped,
}

/// A connection lifecycle event the runtime should surface.
#[derive(Debug, Clone)]
pub enum ConnectionEvent {
    /// The connection was refused (handshake failure or not configured) and
    /// should be closed by the runtime.
    Refused {
        connection: ConnectionId,
        reason: WireError,
    },
    /// The connection was lost.
    Closed { connection: ConnectionId },
}

/// An incoming wire message. The handshake carries opaque bytes whose CBOR
/// shape is defined by ticket 004; the sync-phase messages are concrete
/// here. `Request`/`Sync` data is an automerge sync payload (encoded
/// `automerge::sync::Message`), carried opaquely until 004 wraps it in the
/// new CBOR protocol.
#[derive(Debug, Clone)]
pub enum IncomingMessage {
    /// Handshake material (challenge-response + capability presentation);
    /// CBOR shape defined by ticket 004.
    Handshake(Vec<u8>),
    /// The peer requests to begin syncing a document.
    Request {
        document_id: DocumentId,
        data: Vec<u8>,
    },
    /// A sync message for a document.
    Sync {
        document_id: DocumentId,
        data: Vec<u8>,
    },
    /// The peer does not have the document.
    DocUnavailable { document_id: DocumentId },
}

/// An event the engine processes via [`Hub::handle_event`].
#[derive(Debug, Clone)]
pub enum HubEvent {
    /// Stop the engine; completes all pending commands with `Stopped`.
    Stop,
    /// Periodic tick (drives sync generation).
    Tick,
    /// App command: create a new document. The caller supplies the
    /// document id (sans-IO purity: the engine does not generate
    /// randomness).
    CreateDocument {
        command_id: CommandId,
        document_id: DocumentId,
    },
    /// App command: find/load an existing document.
    FindDocument {
        command_id: CommandId,
        document_id: DocumentId,
    },
    /// The runtime reports a new connection (e.g. an accepted Iroh stream).
    ConnectionOpened { connection: ConnectionId },
    /// A connection was lost.
    ConnectionLost { connection: ConnectionId },
    /// A wire message arrived on a connection.
    Receive {
        connection: ConnectionId,
        message: IncomingMessage,
    },
    /// An IO task completed.
    IoComplete { task: IoTaskId, result: IoResult },
}

/// Effects produced by [`Hub::handle_event`].
#[derive(Debug, Clone, Default)]
pub struct HubResults {
    /// IO tasks the runtime must execute.
    pub new_tasks: Vec<IoTask>,
    /// App commands that completed.
    pub completed_commands: Vec<(CommandId, CommandResult)>,
    /// Connection lifecycle events to surface.
    pub connection_events: Vec<ConnectionEvent>,
    /// Visible sync refusals (never silent drops).
    pub errors: Vec<SyncRefusal>,
}

// ── Per-connection state ──────────────────────────────────────────────────

struct ConnectionState {
    /// The peer's verified identity, set after a successful handshake.
    /// `None` until the handshake completes.
    peer_identity: Option<PeerIdentity>,
    /// Per-document sync mode derived from the presented cap's permission.
    /// Plumbing lands here; the read-only *policy* activates in ticket 008.
    doc_modes: HashMap<DocumentId, SyncMode>,
}

impl ConnectionState {
    fn new() -> Self {
        Self {
            peer_identity: None,
            doc_modes: HashMap::new(),
        }
    }

    fn sync_mode_for(&self, document_id: &DocumentId) -> SyncMode {
        // Default to ReadWrite when unknown — the v1 rule is "no Write cap → no
        // sync at all" (ticket 005 Q1), enforced by the validator *before*
        // a doc reaches this lookup. So a present entry implies an authorized
        // peer; ReadWrite is the safe default for v1 (read-only activates in 008).
        self.doc_modes
            .get(document_id)
            .copied()
            .unwrap_or(SyncMode::ReadWrite)
    }
}

// ── DocumentActor ───────────────────────────────────────────────────────

/// Manages one automerge document: holds the doc and per-peer sync state,
/// generates and receives real automerge sync messages, gated by `SyncMode`.
pub struct DocumentActor {
    document_id: DocumentId,
    doc: Automerge,
    peer_connections: HashMap<ConnectionId, PeerDocConnection>,
}

struct PeerDocConnection {
    sync_state: SyncState,
    sync_mode: SyncMode,
}

/// A generated sync message for a connection.
struct GeneratedSync {
    connection: ConnectionId,
    bytes: Vec<u8>,
}

impl DocumentActor {
    /// Constructs a new document actor for an empty document.
    pub fn new(document_id: DocumentId) -> Self {
        Self {
            document_id,
            doc: Automerge::new(),
            peer_connections: HashMap::new(),
        }
    }

    /// Constructs a document actor around an existing automerge document.
    pub fn from_automerge(document_id: DocumentId, doc: Automerge) -> Self {
        Self {
            document_id,
            doc,
            peer_connections: HashMap::new(),
        }
    }

    pub fn document_id(&self) -> &DocumentId {
        &self.document_id
    }

    pub fn actor_id(&self) -> DocumentActorId {
        DocumentActorId(self.document_id.as_bytes()[0] as u64)
    }

    /// Access the underlying automerge document.
    pub fn with_document<R>(&mut self, f: impl FnOnce(&mut Automerge) -> R) -> R {
        f(&mut self.doc)
    }

    /// Register a peer connection with its authorized sync mode.
    fn add_peer(&mut self, connection: ConnectionId, sync_mode: SyncMode) {
        self.peer_connections.insert(
            connection,
            PeerDocConnection {
                sync_state: SyncState::new(),
                sync_mode,
            },
        );
    }

    /// Apply an incoming sync message from a peer. Read-only peers' pushes are
    /// dropped with a visible error (SyncMode plumbing; policy in 008).
    fn handle_sync_message(
        &mut self,
        connection: ConnectionId,
        data: &[u8],
    ) -> Result<(), SyncApplyError> {
        let Some(peer_conn) = self.peer_connections.get_mut(&connection) else {
            return Err(SyncApplyError::UnknownPeer);
        };
        if peer_conn.sync_mode == SyncMode::ReadOnly {
            // Read-only peers must not push changes. Drop the push with a
            // visible error rather than applying it.
            return Err(SyncApplyError::ReadOnlyPush);
        }
        let msg = match AutoSyncMessage::decode(data) {
            Ok(m) => m,
            Err(e) => return Err(SyncApplyError::Decode(e.to_string())),
        };
        self.doc
            .receive_sync_message(&mut peer_conn.sync_state, msg)
            .map_err(SyncApplyError::Automerge)?;
        Ok(())
    }

    /// Generate outgoing sync messages for all connected peers. Read-only
    /// connections are skipped (SyncMode plumbing; policy in 008).
    fn generate_sync_messages(&mut self) -> Vec<GeneratedSync> {
        let mut out = Vec::new();
        for (conn_id, peer_conn) in self.peer_connections.iter_mut() {
            if peer_conn.sync_mode == SyncMode::ReadOnly {
                continue;
            }
            if let Some(msg) = self.doc.generate_sync_message(&mut peer_conn.sync_state) {
                out.push(GeneratedSync {
                    connection: *conn_id,
                    bytes: msg.encode(),
                });
            }
        }
        out
    }
}

#[derive(Debug, Error)]
enum SyncApplyError {
    #[error("unknown peer for this document")]
    UnknownPeer,
    #[error("read-only peer attempted to push changes")]
    ReadOnlyPush,
    #[error("sync message decode error: {0}")]
    Decode(String),
    #[error("automerge error: {0}")]
    Automerge(AutomergeError),
}

// ── Hub ───────────────────────────────────────────────────────────────

/// The central sans-IO actor: routes by [`DocumentId`], consults the
/// authorization seams at handshake and per-message time, and emits
/// [`IoTask`] effects.
pub struct Hub {
    peer_id: SubspaceId,
    authenticator: Option<Arc<dyn PeerAuthenticator>>,
    validator: Option<Arc<dyn CapabilityValidator>>,
    storage: Option<Arc<dyn Storage>>,
    connections: HashMap<ConnectionId, ConnectionState>,
    documents: HashMap<DocumentId, DocumentActor>,
    next_command_id: u64,
    next_task_id: u64,
    run_state: RunState,
}

impl Hub {
    /// Constructs a new engine for the local peer with the given authorization
    /// seams. Pass `None` for either seam to get the deny-by-default
    /// behavior (visible "authorization not configured" refusal).
    pub fn new(peer_id: SubspaceId) -> Self {
        Self {
            peer_id,
            authenticator: None,
            validator: None,
            storage: None,
            connections: HashMap::new(),
            documents: HashMap::new(),
            next_command_id: 0,
            next_task_id: 0,
            run_state: RunState::Running,
        }
    }

    pub fn with_authenticator(mut self, authenticator: Arc<dyn PeerAuthenticator>) -> Self {
        self.authenticator = Some(authenticator);
        self
    }

    pub fn with_validator(mut self, validator: Arc<dyn CapabilityValidator>) -> Self {
        self.validator = Some(validator);
        self
    }

    pub fn with_storage(mut self, storage: Arc<dyn Storage>) -> Self {
        self.storage = Some(storage);
        self
    }

    pub fn peer_id(&self) -> &SubspaceId {
        &self.peer_id
    }

    pub fn is_stopped(&self) -> bool {
        matches!(self.run_state, RunState::Stopped)
    }

    fn next_command_id(&mut self) -> CommandId {
        let id = CommandId(self.next_command_id);
        self.next_command_id += 1;
        id
    }

    fn next_task_id(&mut self) -> IoTaskId {
        let id = IoTaskId(self.next_task_id);
        self.next_task_id += 1;
        id
    }

    /// Process an event and return the effects the runtime must execute.
    pub fn handle_event(&mut self, event: HubEvent) -> HubResults {
        let mut results = HubResults::default();
        match event {
            HubEvent::Stop => {
                self.run_state = RunState::Stopped;
                // Completing pending commands is out of scope for the 001
                // scaffold; the real stop draining lands with the full engine.
            }
            HubEvent::Tick => self.tick(&mut results),
            HubEvent::CreateDocument {
                command_id,
                document_id,
            } => {
                let actor = DocumentActor::new(document_id.clone());
                let actor_id = actor.actor_id();
                self.documents.insert(document_id.clone(), actor);
                results
                    .completed_commands
                    .push((command_id, CommandResult::Created(document_id)));
                // Suppress unused-warning while storage wiring is optional.
                let _ = &actor_id;
            }
            HubEvent::FindDocument {
                command_id,
                document_id,
            } => {
                if let Some(actor) = self.documents.get(&document_id) {
                    results
                        .completed_commands
                        .push((command_id, CommandResult::Found(actor.actor_id())));
                } else {
                    results
                        .completed_commands
                        .push((command_id, CommandResult::DocumentUnavailable(document_id)));
                }
            }
            HubEvent::ConnectionOpened { connection } => {
                self.connections.insert(connection, ConnectionState::new());
            }
            HubEvent::ConnectionLost { connection } => {
                self.connections.remove(&connection);
                for doc in self.documents.values_mut() {
                    doc.peer_connections.remove(&connection);
                }
                results
                    .connection_events
                    .push(ConnectionEvent::Closed { connection });
            }
            HubEvent::Receive {
                connection,
                message,
            } => {
                self.receive(connection, message, &mut results);
            }
            HubEvent::IoComplete { task: _, result } => {
                // Storage/sent confirmations feed back into document actors in
                // the full engine; the 001 scaffold records completion without
                // further routing. Real wiring lands with 004/005.
                let _ = result;
            }
        }
        results
    }

    fn receive(
        &mut self,
        connection: ConnectionId,
        message: IncomingMessage,
        results: &mut HubResults,
    ) {
        match message {
            IncomingMessage::Handshake(material) => match &self.authenticator {
                None => {
                    results.errors.push(SyncRefusal {
                        connection,
                        document_id: None,
                        reason: WireError::NotConfigured,
                    });
                    results.connection_events.push(ConnectionEvent::Refused {
                        connection,
                        reason: WireError::NotConfigured,
                    });
                }
                Some(authenticator) => match authenticator.authenticate(&material) {
                    Ok(identity) => {
                        if let Some(conn) = self.connections.get_mut(&connection) {
                            conn.peer_identity = Some(identity);
                        }
                    }
                    Err(err) => {
                        let reason = WireError::from(err);
                        results.errors.push(SyncRefusal {
                            connection,
                            document_id: None,
                            reason,
                        });
                        results
                            .connection_events
                            .push(ConnectionEvent::Refused { connection, reason });
                    }
                },
            },
            IncomingMessage::Request { document_id, data } => {
                self.authorize_and_route(
                    connection,
                    &document_id,
                    &data,
                    SyncDirection::Push,
                    results,
                );
            }
            IncomingMessage::Sync { document_id, data } => {
                self.authorize_and_route(
                    connection,
                    &document_id,
                    &data,
                    SyncDirection::Receive,
                    results,
                );
            }
            IncomingMessage::DocUnavailable { document_id } => {
                // Advisory; no authorization needed. The document actor notes
                // the peer doesn't have the doc.
                let _ = document_id;
            }
        }
    }

    /// The per-message authorization gate: consult the validator, then route
    /// to the document actor on `Allow`. On `Deny`, emit a visible per-doc
    /// error and skip — the connection stays up for other docs.
    fn authorize_and_route(
        &mut self,
        connection: ConnectionId,
        document_id: &DocumentId,
        data: &[u8],
        direction: SyncDirection,
        results: &mut HubResults,
    ) {
        let peer_identity = {
            let Some(conn_state) = self.connections.get(&connection) else {
                results.errors.push(SyncRefusal {
                    connection,
                    document_id: Some(document_id.clone()),
                    reason: WireError::Unauthorized,
                });
                return;
            };
            conn_state.peer_identity.clone()
        };
        let Some(validator) = &self.validator else {
            results.errors.push(SyncRefusal {
                connection,
                document_id: Some(document_id.clone()),
                reason: WireError::NotConfigured,
            });
            return;
        };
        let Some(peer_identity) = peer_identity else {
            results.errors.push(SyncRefusal {
                connection,
                document_id: Some(document_id.clone()),
                reason: WireError::Unauthorized,
            });
            return;
        };
        match validator.authorize(&peer_identity, document_id, direction) {
            Ok(sync_mode) => {
                if let Some(conn_state) = self.connections.get_mut(&connection) {
                    conn_state.doc_modes.insert(document_id.clone(), sync_mode);
                }
                let actor = self
                    .documents
                    .entry(document_id.clone())
                    .or_insert_with(|| DocumentActor::new(document_id.clone()));
                actor.add_peer(connection, sync_mode);
                if let Err(err) = actor.handle_sync_message(connection, data) {
                    results.errors.push(SyncRefusal {
                        connection,
                        document_id: Some(document_id.clone()),
                        reason: match err {
                            SyncApplyError::ReadOnlyPush => WireError::Unauthorized,
                            _ => WireError::Decode,
                        },
                    });
                }
            }
            Err(err) => {
                results.errors.push(SyncRefusal {
                    connection,
                    document_id: Some(document_id.clone()),
                    reason: WireError::from(err),
                });
            }
        }
    }

    /// Drive outgoing sync generation for all documents, honoring each
    /// connection's `SyncMode` (read-only connections are skipped).
    fn tick(&mut self, results: &mut HubResults) {
        // Collect generated messages first, then assign task ids, to avoid
        // borrowing `self.documents` mutably while calling `next_task_id`.
        let mut generated = Vec::new();
        for actor in self.documents.values_mut() {
            generated.extend(actor.generate_sync_messages());
        }
        for g in generated {
            let task_id = self.next_task_id();
            results.new_tasks.push(IoTask::SendMsg {
                connection: g.connection,
                bytes: g.bytes,
            });
            let _ = task_id;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authorization::{DenyAllValidator, EmptyRevocationView, NotConfiguredAuthenticator};
    use crate::identity::PeerIdentity;
    use galvanized_cap::{AutomergeSyncCapability, AutomergeSyncPermission, Recipient, Signer};
    use rand::Rng;
    use rand::RngCore;

    fn fresh_subspace_id() -> SubspaceId {
        use rand_core_0_6_4::OsRng;
        use willow25::entry::randomly_generate_subspace;
        let (id, _) = randomly_generate_subspace(&mut OsRng);
        id
    }

    /// A validator that denies every document with `NoCap`.
    struct DenyAllNoCap;

    impl CapabilityValidator for DenyAllNoCap {
        fn authorize(
            &self,
            _peer: &PeerIdentity,
            _document_id: &DocumentId,
            _direction: SyncDirection,
        ) -> Result<SyncMode, AuthorizationError> {
            Err(AuthorizationError::NoCap)
        }
    }

    /// A validator that always allows `ReadWrite`.
    struct AllowAllValidator;

    impl CapabilityValidator for AllowAllValidator {
        fn authorize(
            &self,
            _peer: &PeerIdentity,
            _document_id: &DocumentId,
            _direction: SyncDirection,
        ) -> Result<SyncMode, AuthorizationError> {
            Ok(SyncMode::ReadWrite)
        }
    }

    fn make_cap(recipient: SubspaceId) -> AutomergeSyncCapability<DocumentId> {
        let owner = fresh_subspace_id();
        let mut nonce = [0u8; 32];
        rand::rng().fill_bytes(&mut nonce);
        AutomergeSyncCapability::new(
            Signer::Subspace(owner),
            Recipient::Subspace(recipient),
            DocumentId::new(&mut rand::rng()),
            galvanized_cap::WillowScope::wildcard(),
            AutomergeSyncPermission::Write,
            u64::MAX,
            nonce,
        )
    }

    fn handshake_ok_for(recipient: SubspaceId) -> Vec<u8> {
        // The real handshake CBOR shape is ticket 004. For the 001 engine
        // test we feed a stand-in payload that the test authenticator (below)
        // accepts; the engine treats handshake bytes as opaque.
        let _ = recipient;
        Vec::new()
    }

    /// Test authenticator that establishes a PeerIdentity from the stand-in
    /// handshake payload, carrying one granted cap for a doc.
    struct TestAuthenticator {
        local: SubspaceId,
    }

    impl PeerAuthenticator for TestAuthenticator {
        fn authenticate(&self, _handshake: &[u8]) -> Result<PeerIdentity, AuthenticationError> {
            let peer = fresh_subspace_id();
            Ok(PeerIdentity::new(peer, vec![make_cap(self.local.clone())]))
        }
    }

    #[test]
    fn no_authenticator_refuses_handshake_visibly() {
        let local = fresh_subspace_id();
        let mut hub = Hub::new(local.clone());
        let conn = ConnectionId(1);
        hub.handle_event(HubEvent::ConnectionOpened { connection: conn });
        let results = hub.handle_event(HubEvent::Receive {
            connection: conn,
            message: IncomingMessage::Handshake(vec![]),
        });

        // Visible "authorization not configured" refusal + connection refused.
        assert!(
            results
                .errors
                .iter()
                .any(|r| r.connection == conn && r.reason == WireError::NotConfigured)
        );
        assert!(
            results
                .connection_events
                .iter()
                .any(|e| matches!(e, ConnectionEvent::Refused { connection: c, .. } if *c == conn))
        );
    }

    #[test]
    fn deny_validator_emits_per_doc_error_and_keeps_connection() {
        let local = fresh_subspace_id();
        let mut hub = Hub::new(local.clone())
            .with_authenticator(Arc::new(TestAuthenticator {
                local: fresh_subspace_id(),
            }))
            .with_validator(Arc::new(DenyAllNoCap));
        let conn = ConnectionId(1);
        hub.handle_event(HubEvent::ConnectionOpened { connection: conn });
        hub.handle_event(HubEvent::Receive {
            connection: conn,
            message: IncomingMessage::Handshake(handshake_ok_for(local)),
        });
        let doc = DocumentId::new(&mut rand::rng());
        let results = hub.handle_event(HubEvent::Receive {
            connection: conn,
            message: IncomingMessage::Request {
                document_id: doc.clone(),
                data: vec![],
            },
        });

        // Per-doc error, no sync send for that doc, connection stays up.
        assert!(
            results
                .errors
                .iter()
                .any(|r| r.connection == conn && r.document_id == Some(doc.clone()))
        );
        assert!(!results.new_tasks.iter().any(|t| matches!(
            t,
            IoTask::SendMsg { connection: c, .. } if *c == conn
        )));
        assert!(
            !results
                .connection_events
                .iter()
                .any(|e| matches!(e, ConnectionEvent::Refused { connection: c, .. } if *c == conn))
        );
    }

    #[test]
    fn allow_validator_routes_to_document_actor() {
        let local = fresh_subspace_id();
        let mut hub = Hub::new(local.clone())
            .with_authenticator(Arc::new(TestAuthenticator {
                local: fresh_subspace_id(),
            }))
            .with_validator(Arc::new(AllowAllValidator));
        let conn = ConnectionId(1);
        hub.handle_event(HubEvent::ConnectionOpened { connection: conn });
        hub.handle_event(HubEvent::Receive {
            connection: conn,
            message: IncomingMessage::Handshake(handshake_ok_for(local)),
        });
        let doc = DocumentId::new(&mut rand::rng());
        let results = hub.handle_event(HubEvent::Receive {
            connection: conn,
            message: IncomingMessage::Sync {
                document_id: doc.clone(),
                data: vec![],
            },
        });

        // Authorization passed (AllowAllValidator) and routing reached the
        // document actor. A decode error from empty data is a data concern,
        // not an auth refusal — assert the actor exists and no auth refusal
        // (Unauthorized/NotConfigured) occurred.
        assert!(hub.documents.contains_key(&doc));
        assert!(
            !results.errors.iter().any(
                |r| r.reason == WireError::Unauthorized || r.reason == WireError::NotConfigured
            )
        );
    }

    #[test]
    fn tick_generates_sync_for_readwrite_only() {
        let local = fresh_subspace_id();
        let mut hub = Hub::new(local.clone())
            .with_authenticator(Arc::new(TestAuthenticator {
                local: fresh_subspace_id(),
            }))
            .with_validator(Arc::new(AllowAllValidator));
        let conn = ConnectionId(1);
        hub.handle_event(HubEvent::ConnectionOpened { connection: conn });
        hub.handle_event(HubEvent::Receive {
            connection: conn,
            message: IncomingMessage::Handshake(handshake_ok_for(local)),
        });
        let doc = DocumentId::new(&mut rand::rng());
        hub.handle_event(HubEvent::Receive {
            connection: conn,
            message: IncomingMessage::Request {
                document_id: doc.clone(),
                data: vec![],
            },
        });
        let results = hub.handle_event(HubEvent::Tick);
        assert!(
            results
                .new_tasks
                .iter()
                .any(|t| matches!(t, IoTask::SendMsg { connection: c, .. } if *c == conn))
        );
    }

    #[test]
    fn read_only_connection_skips_generation() {
        let local = fresh_subspace_id();
        // A validator that authorizes a specific doc as ReadOnly.
        struct ReadOnlyValidator {
            doc: DocumentId,
        }
        impl CapabilityValidator for ReadOnlyValidator {
            fn authorize(
                &self,
                _peer: &PeerIdentity,
                document_id: &DocumentId,
                _direction: SyncDirection,
            ) -> Result<SyncMode, AuthorizationError> {
                if document_id == &self.doc {
                    Ok(SyncMode::ReadOnly)
                } else {
                    Err(AuthorizationError::NoCap)
                }
            }
        }
        let mut hub = Hub::new(local.clone()).with_authenticator(Arc::new(TestAuthenticator {
            local: fresh_subspace_id(),
        }));
        let conn = ConnectionId(1);
        hub.handle_event(HubEvent::ConnectionOpened { connection: conn });
        hub.handle_event(HubEvent::Receive {
            connection: conn,
            message: IncomingMessage::Handshake(handshake_ok_for(local)),
        });
        let doc = DocumentId::new(&mut rand::rng());
        hub.validator = Some(Arc::new(ReadOnlyValidator { doc: doc.clone() }));
        hub.handle_event(HubEvent::Receive {
            connection: conn,
            message: IncomingMessage::Request {
                document_id: doc.clone(),
                data: vec![],
            },
        });
        let results = hub.handle_event(HubEvent::Tick);
        // Read-only connection: no outgoing sync generated for it.
        assert!(!results.new_tasks.iter().any(|t| matches!(
            t,
            IoTask::SendMsg { connection: c, .. } if *c == conn
        )));
    }

    #[test]
    fn deny_all_validator_default_refuses() {
        // The built-in default validator (DenyAllValidator) refuses everything.
        let local = fresh_subspace_id();
        let mut hub = Hub::new(local.clone())
            .with_authenticator(Arc::new(TestAuthenticator {
                local: fresh_subspace_id(),
            }))
            .with_validator(Arc::new(DenyAllValidator));
        let conn = ConnectionId(1);
        hub.handle_event(HubEvent::ConnectionOpened { connection: conn });
        hub.handle_event(HubEvent::Receive {
            connection: conn,
            message: IncomingMessage::Handshake(handshake_ok_for(local)),
        });
        let doc = DocumentId::new(&mut rand::rng());
        let results = hub.handle_event(HubEvent::Receive {
            connection: conn,
            message: IncomingMessage::Request {
                document_id: doc.clone(),
                data: vec![],
            },
        });
        assert!(
            results
                .errors
                .iter()
                .any(|r| r.connection == conn && r.reason == WireError::NotConfigured)
        );
    }

    // Suppress unused warnings for the deny-by-default types brought in by the
    // `use` above but not exercised in every test.
    #[test]
    fn _deny_by_default_types_compile() {
        let _ = NotConfiguredAuthenticator;
        let _ = DenyAllValidator;
        let _ = EmptyRevocationView;
    }
}
