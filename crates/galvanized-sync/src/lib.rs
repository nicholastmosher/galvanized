//! # `galvanized-sync`
//!
//! A from-scratch automerge sync engine that only syncs documents between
//! peers who hold and prove valid signed Willow capabilities. Privacy is the
//! primary case: every sync requires positive proof of a valid signed
//! capability; there is no ungated path. "Public" sharing is a granted
//! capability, not a mode of the engine.
//!
//! `samod`/`samod-core` are a reference implementation we are inspired by
//! (sans-IO actor shape, CBOR codec ideas), not a dependency and not a
//! compatibility target. No `automerge-repo`/JS interop is preserved.
//!
//! ## Layout
//!
//! - [`DocumentId`] — a fresh newtype identifying an automerge document
//!   (wire form: deterministic CBOR bytes; human-facing string form: base58,
//!   landed with the full 002 work).
//! - [`PeerIdentity`] — the post-handshake per-connection handle: a
//!   verified Willow `SubspaceId` plus the granted capability set. Concrete
//!   (no opaque payload) — we build for our own evolution, not arbitrary
//!   plug-ins.
//! - [`PeerAuthenticator`] / [`CapabilityValidator`] / [`SyncMode`] /
//!   [`CapabilityStore`] — the authorization seams the engine consults at
//!   handshake and per-message time. Built-in defaults are deny-by-default
//!   (no no-op open path); the real Willow implementations (challenge
//!   response, cap verification, revocation view) land in tickets 004/005/006.
//! - [`Hub`] / [`DocumentActor`] — the sans-IO actor core (`handle_event` →
//!   effects), with the authorization gate and `SyncMode` plumbing wired through
//!   the outgoing and incoming paths.
//!
//! See `docs/ai/tickets/001-willow-automerge-sync/` (INDEX, SPEC, BRAINSTORM,
//! tickets 001–008) for the design and ratified rulings.

mod actors;
mod authorization;
mod document_id;
mod identity;
mod io;

pub use actors::{DocumentActor, DocumentActorId, Hub, HubEvent, HubResults, RunState};
pub use authorization::{
    AuthenticationError, AuthorizationError, CapabilityStore, CapabilityValidator,
    PeerAuthenticator, RevocationView, SyncMode,
};
pub use document_id::{BadDocumentId, DocumentId};
pub use identity::PeerIdentity;
pub use io::{ConnectionId, IoResult, IoTask, IoTaskId, Storage, StorageTask, Transport};
