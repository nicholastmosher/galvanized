//! Authorization seams: the four traits the engine consults to decide
//! whether a peer may sync, plus the deny-by-default implementations
//! that ship as the built-in defaults.
//!
//! Privacy is the primary case: every sync requires positive proof of a valid
//! signed capability, and there is no ungated path. The defaults here are
//! **deny-by-default** — `NotConfiguredAuthenticator` refuses with a
//! visible "authorization not configured" error, and `DenyAllValidator`
//! refuses every document with a visible per-doc error (the connection
//! stays up for other docs). Tickets 004/005 replace these with the real
//! Willow implementations (challenge-response, cap verification, revocation
//! view). The seams exist for *our own* evolution, not to wrap samod, and
//! the defaults are privacy-first, not permissive stubs.

use std::sync::Arc;

use galvanized_cap::AutomergeSyncCapability;
use thiserror::Error;
use willow25::entry::SubspaceId;

use crate::document_id::DocumentId;
use crate::identity::PeerIdentity;

/// The direction of a sync operation the validator is asked about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncDirection {
    /// The peer is pushing changes to us (incoming `Sync`/`Request` data).
    Push,
    /// We would generate changes for the peer (outgoing `Sync`).
    Receive,
}

/// Per-connection sync mode, derived from the presented cap's permission.
/// `Write` → `ReadWrite`; `Read` → `ReadOnly`. Plumbing lands in 001;
/// the read-only *policy* activates in ticket 008.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMode {
    ReadWrite,
    ReadOnly,
}

impl SyncMode {
    /// The effective mode when a peer holds both `Read` and `Write` for a doc:
    /// max wins → `ReadWrite`.
    pub fn max(self, other: SyncMode) -> SyncMode {
        match (self, other) {
            (SyncMode::ReadWrite, _) | (_, SyncMode::ReadWrite) => SyncMode::ReadWrite,
            (SyncMode::ReadOnly, SyncMode::ReadOnly) => SyncMode::ReadOnly,
        }
    }
}

/// A live view over revocation tombstones, read at gating *call time* so a
/// tombstone landing mid-session takes effect on the next message without
/// waiting for a reconnect. Provided by [`CapabilityStore::revocation_view`].
pub trait RevocationView: Send + Sync {
    /// Whether the capability with this nonce has been revoked.
    fn is_revoked(&self, nonce: [u8; 32]) -> bool;
}

/// Verifies a peer's identity and presented capabilities during the
/// handshake. Returns a [`PeerIdentity`] on success or a visible
/// [`AuthenticationError`] on refusal. The engine never falls back to
/// ungated sync on refusal.
pub trait PeerAuthenticator: Send + Sync {
    fn authenticate(&self, handshake_material: &[u8]) -> Result<PeerIdentity, AuthenticationError>;
}

/// Decides, per `Request`/`Sync`, whether a peer may sync a given
/// document in a given direction. `Deny` produces a visible per-doc error
/// and skips that doc's sync; the connection stays up for other docs.
pub trait CapabilityValidator: Send + Sync {
    fn authorize(
        &self,
        peer: &PeerIdentity,
        document_id: &DocumentId,
        direction: SyncDirection,
    ) -> Result<SyncMode, AuthorizationError>;
}

/// Where capabilities live and how they propagate. Today's self-sufficient
/// impl (ticket 003) uses a per-peer automerge index doc for
/// attributable-public caps + tombstones and a local store for
/// recipient-specific caps. A future impl backs this with Willow namespace
/// paths. The engine depends on the trait, not the impl.
pub trait CapabilityStore: Send + Sync {
    /// Caps the local peer holds that authorize it to sync docs owned by
    /// `peer` (or public caps relevant to `peer`).
    fn caps_to_present(&self, peer: &SubspaceId) -> Vec<AutomergeSyncCapability<DocumentId>>;

    /// Record a cap the local peer received out-of-band (recipient-specific).
    fn store_received(&self, cap: AutomergeSyncCapability<DocumentId>) -> Result<(), StoreError>;

    /// Record a tombstone revoking a cap by nonce.
    fn revoke(&self, nonce: [u8; 32]) -> Result<(), StoreError>;

    /// A live view the validator reads at gating call time.
    fn revocation_view(&self) -> Arc<dyn RevocationView>;
}

#[derive(Debug, Clone, Error)]
pub enum AuthenticationError {
    /// No authenticator is wired. The engine surfaces this as a visible
    /// "sync refused: authorization not configured" refusal, never a silent
    /// degrade to open sync.
    #[error("authorization not configured")]
    NotConfigured,
    #[error("handshake signature invalid")]
    InvalidSignature,
    #[error("challenge response invalid")]
    ChallengeFailed,
    #[error("capability recipient does not match local subspace")]
    WrongRecipient,
    #[error("no valid capabilities presented")]
    NoCapsPresented,
    #[error("handshake decode error: {0}")]
    Decode(String),
}

#[derive(Debug, Clone, Error)]
pub enum AuthorizationError {
    /// No validator is wired. Visible "authorization not configured" refusal.
    #[error("authorization not configured")]
    NotConfigured,
    #[error("no valid capability for document")]
    NoCap,
    #[error("capability expired")]
    Expired,
    #[error("capability revoked")]
    Revoked,
    #[error("capability does not permit this direction")]
    WrongDirection,
    #[error("capability out of scope")]
    OutOfScope,
}

#[derive(Debug, Clone, Error)]
pub enum StoreError {
    #[error("store error: {0}")]
    Io(String),
    #[error("capability already stored")]
    Duplicate,
}

// ── Deny-by-default implementations (the built-in defaults) ──────────────────

/// The default authenticator when no real one (ticket 004) is wired yet.
/// Always refuses with a visible "authorization not configured" error.
#[derive(Debug, Clone, Default)]
pub struct NotConfiguredAuthenticator;

impl PeerAuthenticator for NotConfiguredAuthenticator {
    fn authenticate(&self, _handshake: &[u8]) -> Result<PeerIdentity, AuthenticationError> {
        Err(AuthenticationError::NotConfigured)
    }
}

/// The default validator when no real one (ticket 005) is wired yet. Refuses
/// every document with a visible per-doc error; the connection stays up.
#[derive(Debug, Clone, Default)]
pub struct DenyAllValidator;

impl CapabilityValidator for DenyAllValidator {
    fn authorize(
        &self,
        _peer: &PeerIdentity,
        _document_id: &DocumentId,
        _direction: SyncDirection,
    ) -> Result<SyncMode, AuthorizationError> {
        Err(AuthorizationError::NotConfigured)
    }
}

/// An empty revocation view (nothing revoked). Used by the deny-by-default
/// path and by tests that don't exercise revocation.
#[derive(Debug, Clone, Default)]
pub struct EmptyRevocationView;

impl RevocationView for EmptyRevocationView {
    fn is_revoked(&self, _nonce: [u8; 32]) -> bool {
        false
    }
}
