//! Post-handshake peer identity.
//!
//! Concrete (ruling 001 Q2): a verified Willow [`SubspaceId`] plus the granted
//! capability set the peer presented. No opaque `Box<dyn Any>` payload — we
//! build for our own evolution, not arbitrary plug-ins, so there is no
//! downcasting at the validator.

use galvanized_cap::AutomergeSyncCapability;
use willow25::entry::SubspaceId;

/// The verified identity of a connected peer, plus the capabilities it
/// presented at handshake time. Carried by the [`crate::Hub`] per connection
/// and consulted by the [`crate::CapabilityValidator`] on every
/// `Request`/`Sync`.
#[derive(Debug, Clone)]
pub struct PeerIdentity {
    /// The peer's Profile (a Willow subspace), proven by challenge-response
    /// during the handshake.
    pub subspace_id: SubspaceId,
    /// The capabilities the peer presented that authorize it to sync
    /// particular documents. Sourced via [`crate::CapabilityStore`].
    pub granted: Vec<AutomergeSyncCapability<crate::DocumentId>>,
}

impl PeerIdentity {
    /// Constructs a verified peer identity from its proven subspace and the
    /// granted capability set.
    pub fn new(
        subspace_id: SubspaceId,
        granted: Vec<AutomergeSyncCapability<crate::DocumentId>>,
    ) -> Self {
        Self {
            subspace_id,
            granted,
        }
    }

    /// The peer's verified subspace (Profile).
    pub fn subspace_id(&self) -> &SubspaceId {
        &self.subspace_id
    }

    /// The granted capability set.
    pub fn granted(&self) -> &[AutomergeSyncCapability<crate::DocumentId>] {
        &self.granted
    }
}
