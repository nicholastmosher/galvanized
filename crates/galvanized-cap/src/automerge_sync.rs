//! The `AutomergeSyncCapability` signed object + its type-token dual (full 002).
//!
//! The signed-data capability object that `galvanized-sync` checks at sync time,
//! plus the canonical CBOR encoding, signing/verification, minting gated by
//! `StrictSendCap`, and the type-token (`StrictCap<AutomergeSync,
//! WillowScope>`) minted from a verified serialized cap.
//!
//! `AutomergeSyncCapability<D>` is generic over the document-id type `D`
//! (owned by `galvanized-sync` as `DocumentId`) to avoid a
//! `galvanized-cap → galvanized-sync` crate dependency cycle.
//!
//! The canonical CBOR encoding is hand-rolled (RFC 8949 deterministic: map keys
//! emitted in bytewise-sorted order, signer/recipient as ordered arrays so
//! no nested-map key sorting, minimal-length integers/lengths). This avoids a
//! minicbor dependency and its Result-chaining for a fixed struct shape.
//!
//! Rulings baked in (Nick, 2026-11-01): `Signer::Subspace` for
//! recipient-specific & attributable-public (names the owner Profile);
//! `Signer::Namespace` for anonymous-public (names only the namespace, an
//! owned-namespace facility); `Recipient::Public` caps are read-only, tight
//! TTL (15 min default), revokable; `document_id` and `willow_scope` are
//! independent, bound by the signed claims (ruling #6); public caps with
//! `permission = Write` are refused at mint.

use std::time::Duration;

use capsec::{Cap, CapSecError, Permission};
use signature::{SignerMut as _, Verifier as _};
use willow25::entry::{
    NamespaceId, NamespaceSecret, NamespaceSignature, SubspaceId, SubspaceSecret, SubspaceSignature,
};

use crate::{WillowKeyError, WillowKeyStore, WillowScope, WillowWrite, encode_target};

/// The type-token permission for automerge sync. A `StrictCap<AutomergeSync,
/// WillowScope>` is minted from a verified `AutomergeSyncCapability` (see
/// [`mint_type_token`]) and is required to call gated sync APIs. The
/// read/write distinction lives in the serialized cap and is enforced by the
/// sync validator (tickets 005/008), not by this permission.
#[capsec::permission]
pub struct AutomergeSync;

/// Who minted the capability and whose signature the validator checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signer {
    /// The owner's Profile (a Willow subspace). Names the owner —
    /// attributable. Recipient-specific and attributable-public caps use this.
    Subspace(SubspaceId),
    /// The namespace itself (an owned namespace's id). Names only the
    /// namespace, not a Profile — anonymous at the Profile level. Available
    /// only for owned namespaces (communal namespaces have no namespace
    /// secret, so they cannot mint anonymous-public caps).
    Namespace(NamespaceId),
}

/// Who the capability authorizes to sync.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recipient {
    /// A specific peer's Profile (a Willow subspace). The peer must prove
    /// it holds *that* subspace's key.
    Subspace(SubspaceId),
    /// Anyone holding the cap and proving *some* subspace key. Public
    /// sharing; the cap does not restrict *which* peer. Public caps are
    /// read-only, tight-TTL, revokable.
    Public,
}

/// What the capability permits.
///
/// Distinct from `WillowRead`/`WillowWrite` (which gate Willow store
/// operations); this gates sync participation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutomergeSyncPermission {
    Read,
    Write,
}

/// The signature over [`AutomergeSyncCapability::canonical_bytes`],
/// stored as the willow25 signature type matching [`Signer`] (no byte
/// conversion in this sync crate; the wire encoding in ticket 004
/// re-encodes via ufotofu in the async runtime).
#[derive(Debug, Clone)]
pub enum AutomergeSignature {
    Subspace(SubspaceSignature),
    Namespace(NamespaceSignature),
}

/// A signed capability authorizing a peer to sync an automerge document.
///
/// `document_id` and `willow_scope` are independent (ruling #6); the signed
/// claims bind them. The `nonce` identifies the cap for revocation
/// tombstones. The `signature` is excluded from the canonical encoding
/// (it is over the canonical bytes of the other fields).
#[derive(Debug, Clone)]
pub struct AutomergeSyncCapability<D> {
    signer: Signer,
    recipient: Recipient,
    document_id: D,
    willow_scope: WillowScope,
    permission: AutomergeSyncPermission,
    expiry: u64,
    nonce: [u8; 32],
    signature: Option<AutomergeSignature>,
}

/// The default public-cap TTL (ruling 002 Q3), in milliseconds.
pub const DEFAULT_PUBLIC_TTL_MS: u64 = 15 * 60 * 1000;

impl<D> AutomergeSyncCapability<D>
where
    D: Clone + AsRef<[u8]>,
{
    /// Constructs an **unsigned** capability from its fields. Use [`sign`] to
    /// attach a signature, or [`WillowKeyStore::mint_sync_capability`] for
    /// the `StrictSendCap`-gated mint path.
    pub fn new(
        signer: Signer,
        recipient: Recipient,
        document_id: D,
        willow_scope: WillowScope,
        permission: AutomergeSyncPermission,
        expiry: u64,
        nonce: [u8; 32],
    ) -> Self {
        Self {
            signer,
            recipient,
            document_id,
            willow_scope,
            permission,
            expiry,
            nonce,
            signature: None,
        }
    }

    pub fn signer(&self) -> &Signer {
        &self.signer
    }
    pub fn recipient(&self) -> &Recipient {
        &self.recipient
    }
    pub fn document_id(&self) -> &D {
        &self.document_id
    }
    pub fn willow_scope(&self) -> &WillowScope {
        &self.willow_scope
    }
    pub fn permission(&self) -> AutomergeSyncPermission {
        self.permission
    }
    pub fn expiry(&self) -> u64 {
        self.expiry
    }
    pub fn nonce(&self) -> [u8; 32] {
        self.nonce
    }
    pub fn signature(&self) -> Option<&AutomergeSignature> {
        self.signature.as_ref()
    }

    /// Whether the cap carries a signature.
    pub fn is_signed(&self) -> bool {
        self.signature.is_some()
    }

    /// The canonical (deterministic, RFC 8949) CBOR encoding of the cap,
    /// **excluding the signature**. This is the byte sequence the signature
    /// is computed over. Map keys are emitted in bytewise-sorted order;
    /// signer/recipient are ordered arrays (no nested-map key sorting);
    /// willow_scope wildcards are `null`.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        cbor_map_header(&mut buf, 7);
        cbor_str(&mut buf, "document_id");
        cbor_bytes(&mut buf, self.document_id.as_ref());
        cbor_str(&mut buf, "expiry");
        cbor_uint(&mut buf, self.expiry);
        cbor_str(&mut buf, "nonce");
        cbor_bytes(&mut buf, &self.nonce);
        cbor_str(&mut buf, "permission");
        cbor_uint(
            &mut buf,
            match self.permission {
                AutomergeSyncPermission::Read => 0,
                AutomergeSyncPermission::Write => 1,
            },
        );
        // recipient: array(2)[type, bstr|nil]  (0 = Subspace, 1 = Public)
        cbor_str(&mut buf, "recipient");
        cbor_array_header(&mut buf, 2);
        match &self.recipient {
            Recipient::Subspace(id) => {
                cbor_uint(&mut buf, 0);
                cbor_bytes(&mut buf, id.as_bytes());
            }
            Recipient::Public => {
                cbor_uint(&mut buf, 1);
                cbor_null(&mut buf);
            }
        }
        // signer: array(2)[type, bstr]  (0 = Subspace, 1 = Namespace)
        cbor_str(&mut buf, "signer");
        cbor_array_header(&mut buf, 2);
        match &self.signer {
            Signer::Subspace(id) => {
                cbor_uint(&mut buf, 0);
                cbor_bytes(&mut buf, id.as_bytes());
            }
            Signer::Namespace(id) => {
                cbor_uint(&mut buf, 1);
                cbor_bytes(&mut buf, id.as_bytes());
            }
        }
        // willow_scope: map(3) with sorted keys, null for wildcards.
        cbor_str(&mut buf, "willow_scope");
        cbor_map_header(&mut buf, 3);
        cbor_str(&mut buf, "namespace");
        match self.willow_scope.namespace() {
            Some(id) => cbor_bytes(&mut buf, id.as_bytes()),
            None => cbor_null(&mut buf),
        }
        cbor_str(&mut buf, "path_prefix");
        match self.willow_scope.path_prefix() {
            Some(path) => {
                let comps: Vec<&[u8]> = path.components().map(|c| c.as_bytes()).collect();
                cbor_array_header(&mut buf, comps.len() as u64);
                for comp in comps {
                    cbor_bytes(&mut buf, comp);
                }
            }
            None => cbor_null(&mut buf),
        }
        cbor_str(&mut buf, "subspace");
        match self.willow_scope.subspace() {
            Some(id) => cbor_bytes(&mut buf, id.as_bytes()),
            None => cbor_null(&mut buf),
        }
        buf
    }

    /// Attach a signature over [`canonical_bytes`] using the signer's secret.
    /// The signer secret must correspond to [`signer`]'s public id. The
    /// willow25 signature type is stored in-memory; the wire encoding
    /// (ticket 004) re-encodes it via ufotofu in the async runtime.
    pub fn sign(&mut self, mut secret: SigningSecret) {
        let bytes = self.canonical_bytes();
        match &mut secret {
            SigningSecret::Subspace(s) => {
                if let Ok(sig) = s.try_sign(&bytes) {
                    self.signature = Some(AutomergeSignature::Subspace(sig));
                }
            }
            SigningSecret::Namespace(s) => {
                if let Ok(sig) = s.try_sign(&bytes) {
                    self.signature = Some(AutomergeSignature::Namespace(sig));
                }
            }
        }
    }

    /// Verify the signature against the signer's public id, using the
    /// in-memory willow25 signature type.
    pub fn verify(&self) -> bool {
        let bytes = self.canonical_bytes();
        let Some(sig) = &self.signature else {
            return false;
        };
        match (&self.signer, sig) {
            (Signer::Subspace(id), AutomergeSignature::Subspace(s)) => id.verify(&bytes, s).is_ok(),
            (Signer::Namespace(id), AutomergeSignature::Namespace(s)) => {
                id.verify(&bytes, s).is_ok()
            }
            _ => false,
        }
    }
}

/// The signing secret for a given [`Signer`]: the subspace secret for
/// `Signer::Subspace`, the namespace secret for `Signer::Namespace`.
/// Owned (cloned from the key store) so each variant can satisfy
/// `SignerMut` for its own signature type.
pub enum SigningSecret {
    Subspace(SubspaceSecret),
    Namespace(NamespaceSecret),
}

// ── Hand-rolled canonical CBOR (RFC 8949) ──────────────────────────────────
// Minimal-length integers and lengths; definite-length containers; map
// keys are emitted in sorted order by the caller.

fn cbor_uint(buf: &mut Vec<u8>, n: u64) {
    match n {
        0..=23 => buf.push(n as u8),
        24..=255 => {
            buf.push(0x18);
            buf.push(n as u8);
        }
        256..=65535 => {
            buf.push(0x19);
            buf.extend_from_slice(&(n as u16).to_be_bytes());
        }
        65536..=4294967295 => {
            buf.push(0x1A);
            buf.extend_from_slice(&(n as u32).to_be_bytes());
        }
        _ => {
            buf.push(0x1B);
            buf.extend_from_slice(&n.to_be_bytes());
        }
    }
}

fn cbor_map_header(buf: &mut Vec<u8>, n: u64) {
    buf.push(0xA0);
    cbor_uint(buf, n);
}

fn cbor_array_header(buf: &mut Vec<u8>, n: u64) {
    buf.push(0x80);
    cbor_uint(buf, n);
}

fn cbor_str(buf: &mut Vec<u8>, s: &str) {
    buf.push(0x60);
    cbor_uint(buf, s.len() as u64);
    buf.extend_from_slice(s.as_bytes());
}

fn cbor_bytes(buf: &mut Vec<u8>, b: &[u8]) {
    buf.push(0x40);
    cbor_uint(buf, b.len() as u64);
    buf.extend_from_slice(b);
}

fn cbor_null(buf: &mut Vec<u8>) {
    buf.push(0xF6);
}

impl WillowKeyStore {
    /// Mints a signed `AutomergeSyncCapability`, gated by a valid
    /// `StrictSendCap<WillowWrite, WillowScope>` for the cap's
    /// `(namespace, subspace, path)` scope. The signer secret is selected
    /// by the requested [`Signer`]: the subspace secret for
    /// `Signer::Subspace`, the namespace secret for `Signer::Namespace`
    /// (anonymous-public). Public caps (`Recipient::Public`) are forced to
    /// `AutomergeSyncPermission::Read` and refused if `Write` is
    /// requested. The cap's scope must be concrete (not wildcard) — a sync
    /// cap is minted for a specific doc location; wildcard-scope attenuation
    /// is a separate operation.
    pub fn mint_sync_capability<D>(
        &self,
        cap: &strict_cap::StrictSendCap<WillowWrite, WillowScope>,
        signer: Signer,
        recipient: Recipient,
        document_id: D,
        scope: WillowScope,
        permission: AutomergeSyncPermission,
        expiry: u64,
        nonce: [u8; 32],
    ) -> Result<AutomergeSyncCapability<D>, WillowKeyError>
    where
        D: Clone + AsRef<[u8]>,
    {
        // Public caps are read-only (ruling 002 §5). Refuse public Write at mint.
        if matches!(
            (&recipient, permission),
            (Recipient::Public, AutomergeSyncPermission::Write)
        ) {
            return Err(WillowKeyError::Cap(CapSecError::OutOfScope {
                target: "public Write cap".to_string(),
                scope: "public caps are read-only".to_string(),
            }));
        }

        // Authorize the mint via the typed cap: checks scope + active + not revoked.
        // The target is the cap's own willow scope — concrete for a concrete
        // scope, the wildcard target (`willow://<zero_ns>.<zero_sub>`) for any
        // wildcard, which a wildcard-scoped cap accepts and a concrete-scoped
        // cap rejects (correctly).
        let target = match (scope.namespace(), scope.subspace(), scope.path_prefix()) {
            (Some(ns), Some(sub), Some(path)) => encode_target(ns, sub, path),
            (Some(ns), Some(sub), None) => encode_target(ns, sub, &willow25::prelude::Path::new()),
            _ => crate::wildcard_target(),
        };
        let _proof = cap.try_cap(&target)?;

        let mut signed = AutomergeSyncCapability::new(
            signer.clone(),
            recipient,
            document_id,
            scope,
            permission,
            expiry,
            nonce,
        );

        match &signer {
            Signer::Subspace(subspace_id) => {
                let secret = self.subspace_secret_for_signing(cap, subspace_id)?;
                signed.sign(SigningSecret::Subspace(secret));
            }
            Signer::Namespace(namespace_id) => {
                let secret = self.namespace_secret_for_signing(cap, namespace_id)?;
                signed.sign(SigningSecret::Namespace(secret));
            }
        }
        Ok(signed)
    }

    /// Yields the namespace secret for signing, gated by a valid
    /// `WillowWrite` cap whose scope covers `namespace_id`. Mirrors
    /// [`subspace_secret_for_signing`] for the namespace path.
    fn namespace_secret_for_signing(
        &self,
        cap: &strict_cap::StrictSendCap<WillowWrite, WillowScope>,
        namespace_id: &NamespaceId,
    ) -> Result<NamespaceSecret, WillowKeyError> {
        cap.is_active()?;
        cap.scope().authorizes_namespace(namespace_id)?;
        self.namespaces
            .get(namespace_id)
            .cloned()
            .ok_or(WillowKeyError::UnknownNamespace(namespace_id.clone()))
    }
}

/// Mints a type-token [`strict_cap::StrictCap<AutomergeSync, WillowScope>`] from a
/// **verified** serialized [`AutomergeSyncCapability`].
///
/// This is the L0 capability duality: the serialized object is the
/// *portable, sync-checked* ground truth; the type-token is the
/// *in-process, revocable, timed, scoped* programming authority. You cannot
/// call a gated sync operation without the token, and you can only mint it
/// by presenting a valid serialized cap.
///
/// `root_cap` is a `Cap<AutomergeSync>` granted by the local root (the
/// vault, at the application layer — see `docs/ai/tickets/000-initial-
/// design/001-vault-capability-foundation.md` §4). This function verifies the
/// serialized cap (signature, expiry) and, on success, wraps the root cap
/// into a `StrictCap` attenuated to the cap's `willow_scope`, with the
/// cap's TTL. On failure, returns a visible error — never a silent
/// type-token.
pub fn mint_type_token<D>(
    cap: &AutomergeSyncCapability<D>,
    root_cap: Cap<AutomergeSync>,
    ttl: Duration,
) -> Result<
    (
        strict_cap::StrictCap<AutomergeSync, WillowScope>,
        strict_cap::StrictRevoker,
    ),
    CapSecError,
>
where
    D: Clone + AsRef<[u8]>,
{
    if !cap.is_signed() || !cap.verify() {
        return Err(CapSecError::Revoked);
    }
    if cap.expiry <= now_millis() {
        return Err(CapSecError::Expired);
    }
    let scope = cap.willow_scope.clone();
    Ok(strict_cap::StrictCap::new(root_cap, ttl, scope))
}

/// Current milliseconds since the Unix epoch (for expiry checks). Uses
/// `std::time`; the vault layer passes the authoritative clock where
/// it matters.
fn now_millis() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{WillowKeyError, WillowScope};
    use rand_core_0_6_4::{OsRng, RngCore};
    use strict_cap::StrictSendCap;
    use willow25::entry::{randomly_generate_owned_namespace, randomly_generate_subspace};

    fn fresh_ids() -> (NamespaceId, NamespaceSecret, SubspaceId, SubspaceSecret) {
        let (ns_id, ns_secret) = randomly_generate_owned_namespace(&mut OsRng);
        let (sub_id, sub_secret) = randomly_generate_subspace(&mut OsRng);
        (ns_id, ns_secret, sub_id, sub_secret)
    }

    fn store_with(ns: NamespaceSecret, sub: SubspaceSecret) -> WillowKeyStore {
        let mut namespaces = std::collections::HashMap::new();
        namespaces.insert(ns.corresponding_namespace_id(), ns);
        let mut subspaces = std::collections::HashMap::new();
        subspaces.insert(sub.corresponding_subspace_id(), sub);
        WillowKeyStore::new(namespaces, subspaces)
    }

    fn root_cap() -> (
        StrictSendCap<WillowWrite, WillowScope>,
        strict_cap::StrictRevoker,
    ) {
        let root = capsec::test_root();
        StrictSendCap::new(
            root.grant::<WillowWrite>().make_send(),
            Duration::from_secs(60),
            WillowScope::wildcard(),
        )
    }

    fn fresh_nonce() -> [u8; 32] {
        let mut nonce = [0u8; 32];
        OsRng.fill_bytes(&mut nonce);
        nonce
    }

    #[test]
    fn mint_and_verify_roundtrips() {
        let (ns_id, ns_secret, sub_id, _sub_secret) = fresh_ids();
        let store = store_with(ns_secret, _sub_secret);
        let (cap, _revoker) = root_cap();

        let scope = WillowScope::new(Some(ns_id.clone()), Some(sub_id.clone()), None);
        let signed = store
            .mint_sync_capability(
                &cap,
                Signer::Subspace(sub_id.clone()),
                Recipient::Subspace(sub_id),
                [0u8; 16],
                scope,
                AutomergeSyncPermission::Write,
                u64::MAX,
                fresh_nonce(),
            )
            .unwrap();
        assert!(signed.is_signed());
        assert!(signed.verify());
    }

    #[test]
    fn tampering_any_field_invalidates_signature() {
        let (ns_id, _ns_secret, sub_id, sub_secret) = fresh_ids();
        let store = store_with(_ns_secret, sub_secret);
        let (cap, _revoker) = root_cap();

        let scope = WillowScope::new(Some(ns_id.clone()), Some(sub_id.clone()), None);
        let mut signed = store
            .mint_sync_capability(
                &cap,
                Signer::Subspace(sub_id.clone()),
                Recipient::Subspace(sub_id),
                [0u8; 16],
                scope,
                AutomergeSyncPermission::Write,
                u64::MAX,
                fresh_nonce(),
            )
            .unwrap();
        signed.document_id = [1u8; 16];
        assert!(!signed.verify());
    }

    #[test]
    fn tampering_recipient_invalidates_signature() {
        let (_ns_id, _ns_secret, sub_id, sub_secret) = fresh_ids();
        let store = store_with(_ns_secret, sub_secret);
        let (cap, _revoker) = root_cap();
        let other = randomly_generate_subspace(&mut OsRng).0;
        let mut signed = store
            .mint_sync_capability(
                &cap,
                Signer::Subspace(sub_id.clone()),
                Recipient::Subspace(other),
                [0u8; 16],
                WillowScope::wildcard(),
                AutomergeSyncPermission::Write,
                u64::MAX,
                fresh_nonce(),
            )
            .unwrap();
        signed.recipient = Recipient::Subspace(sub_id);
        assert!(!signed.verify());
    }

    #[test]
    fn public_write_is_refused_at_mint() {
        let (_ns_id, _ns_secret, sub_id, sub_secret) = fresh_ids();
        let store = store_with(_ns_secret, sub_secret);
        let (cap, _revoker) = root_cap();
        let err = store
            .mint_sync_capability(
                &cap,
                Signer::Subspace(sub_id),
                Recipient::Public,
                [0u8; 16],
                WillowScope::wildcard(),
                AutomergeSyncPermission::Write,
                u64::MAX,
                fresh_nonce(),
            )
            .unwrap_err();
        assert!(matches!(
            err,
            WillowKeyError::Cap(CapSecError::OutOfScope { .. })
        ));
    }

    #[test]
    fn anonymous_public_hides_the_profile() {
        let (ns_id, ns_secret, _sub_id, _sub_secret) = fresh_ids();
        let store = store_with(ns_secret, _sub_secret);
        let (cap, _revoker) = root_cap();
        let signed = store
            .mint_sync_capability(
                &cap,
                Signer::Namespace(ns_id.clone()),
                Recipient::Public,
                [0u8; 16],
                WillowScope::new(Some(ns_id.clone()), None, None),
                AutomergeSyncPermission::Read,
                u64::MAX,
                fresh_nonce(),
            )
            .unwrap();
        assert!(matches!(signed.signer(), Signer::Namespace(id) if *id == ns_id));
        assert!(signed.verify());
    }

    #[test]
    fn type_token_mint_requires_verified_cap() {
        let (_ns_id, _ns_secret, sub_id, sub_secret) = fresh_ids();
        let store = store_with(_ns_secret, sub_secret);
        let (cap, _revoker) = root_cap();
        let signed = store
            .mint_sync_capability(
                &cap,
                Signer::Subspace(sub_id.clone()),
                Recipient::Subspace(sub_id),
                [0u8; 16],
                WillowScope::wildcard(),
                AutomergeSyncPermission::Write,
                u64::MAX,
                fresh_nonce(),
            )
            .unwrap();

        let root = capsec::test_root();
        let root_cap = root.grant::<AutomergeSync>();
        let result = mint_type_token(&signed, root_cap, Duration::from_secs(60));
        assert!(result.is_ok());
    }

    #[test]
    fn type_token_refuses_unsigned_cap() {
        let signed = AutomergeSyncCapability::<[u8; 16]>::new(
            Signer::Subspace(randomly_generate_subspace(&mut OsRng).0),
            Recipient::Subspace(randomly_generate_subspace(&mut OsRng).0),
            [0u8; 16],
            WillowScope::wildcard(),
            AutomergeSyncPermission::Write,
            u64::MAX,
            [0u8; 32],
        );
        let root = capsec::test_root();
        let root_cap = root.grant::<AutomergeSync>();
        let result = mint_type_token(&signed, root_cap, Duration::from_secs(60));
        assert!(matches!(result, Err(CapSecError::Revoked)));
    }
}
