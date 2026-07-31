//! Galvanized domain capability layer.
//!
//! This crate sits on top of the general-purpose [`strict_cap`] primitives and
//! instantiates them with Galvanized's Willow-flavoured domain types:
//!
//! - [`WillowRead`] / [`WillowWrite`] — the typed-token permissions for T1.
//!   `ProfileAccess` and `AutomergeSync` are deliberately deferred (T1 design
//!   question §2); a single `WillowWrite` cap gates both Willow writes and
//!   "sign as Profile" until a future `ProfileAct` splits them.
//! - [`WillowScope`] — `(namespace, subspace, path prefix)` with `None` =
//!   wildcard, matched structurally against a target encoded as a Willow URI
//!   (`willow://<namespace_id>.<subspace_id>/<path/components>`), per the
//!   [Willow URI spec](https://willowprotocol.org/specs/uris/).
//! - [`WillowKeyStore`] — the cap-gated custodian of the vault's Willow
//!   namespace/subspace secrets. It mints the signed-data
//!   [`WriteCapability`](willow25::prelude::WriteCapability) and yields the
//!   `SubspaceSecret` needed to sign an entry, but only on presentation of a
//!   valid [`StrictSendCap`]`<WillowWrite, WillowScope>`. This is the mechanism
//!   that closes the former `secret_todo_consider_access_control` seam: code
//!   that needs a secret must present a capability rather than reaching a
//!   plain struct field.
//!
//! The dual-form capability model — signed-data objects (portable, cross-peer)
//! vs typed programmatic tokens (in-process, revocable, timed, scoped) — is the
//! foundation T2/T4 build on. See `docs/ai/memory/GLOSSARY.md`.

use std::collections::HashMap;

use capsec::{CapSecError, Scope};
use thiserror::Error;
use willow25::entry::{
    NAMESPACE_ID_WIDTH, NamespaceId, NamespaceSecret, SUBSPACE_ID_WIDTH, SubspaceId, SubspaceSecret,
};
use willow25::prelude::{Path, WriteCapability};

// Re-export the strict-cap primitives this crate builds on, so downstream
// crates can depend on `galvanized-cap` alone for the capability surface.
pub use strict_cap::{CapCell, StrictCap, StrictRevoker, StrictSendCap};

/// Permission to read Willow entries within a [`WillowScope`].
#[capsec::permission]
pub struct WillowRead;

/// Permission to write Willow entries within a [`WillowScope`].
///
/// Per T1 design question §2, `WillowWrite` also gates the "sign as Profile"
/// use of the subspace secret (p2p envelope signing), because today both
/// operations share the same subspace secret. A future `ProfileAct` permission
/// may split them.
#[capsec::permission]
pub struct WillowWrite;

/// A Willow capability scope: namespace + subspace + path prefix, any of which
/// may be `None` (wildcard). Matching is structural: the target is parsed as a
/// Willow URI and each component is compared, with `None` matching anything.
///
/// This preserves attenuation (a profile-wide cap, a per-namespace-under-profile
/// cap, a per-path cap) and is robust against percent-encoding boundaries and
/// dot-segments, unlike a naive string-prefix match.
#[derive(Clone, Debug)]
pub struct WillowScope {
    namespace: Option<NamespaceId>,
    subspace: Option<SubspaceId>,
    path_prefix: Option<Path>,
}

impl WillowScope {
    /// Creates a scope. `None` for any field means wildcard.
    pub fn new(
        namespace: Option<NamespaceId>,
        subspace: Option<SubspaceId>,
        path_prefix: Option<Path>,
    ) -> Self {
        Self {
            namespace,
            subspace,
            path_prefix,
        }
    }

    /// The broadest scope — matches every target. This is the session
    /// capability minted on vault unlock; its authority is bounded in practice
    /// by which secrets the [`WillowKeyStore`] actually holds.
    pub fn wildcard() -> Self {
        Self {
            namespace: None,
            subspace: None,
            path_prefix: None,
        }
    }

    pub fn namespace(&self) -> Option<&NamespaceId> {
        self.namespace.as_ref()
    }
    pub fn subspace(&self) -> Option<&SubspaceId> {
        self.subspace.as_ref()
    }
    pub fn path_prefix(&self) -> Option<&Path> {
        self.path_prefix.as_ref()
    }

    /// Checks whether this scope authorizes the given subspace, ignoring
    /// namespace and path. Used by [`WillowKeyStore::subspace_secret_for_signing`]
    /// where there is no Willow path target — the subspace identity is the only
    /// structural constraint that applies.
    pub fn authorizes_subspace(&self, subspace: &SubspaceId) -> Result<(), CapSecError> {
        match &self.subspace {
            None => Ok(()),
            Some(s) if s == subspace => Ok(()),
            Some(_) => Err(CapSecError::OutOfScope {
                target: format!("{subspace:?}"),
                scope: format!("{self:?}"),
            }),
        }
    }
}

impl Scope for WillowScope {
    fn check(&self, target: &str) -> Result<(), CapSecError> {
        let (ns, sub, path_components) =
            parse_target(target).map_err(|_| CapSecError::OutOfScope {
                target: target.to_string(),
                scope: format!("{self:?}"),
            })?;

        if let Some(scope_ns) = &self.namespace {
            if &ns != scope_ns {
                return Err(CapSecError::OutOfScope {
                    target: target.to_string(),
                    scope: format!("{self:?}"),
                });
            }
        }
        if let Some(scope_sub) = &self.subspace {
            if &sub != scope_sub {
                return Err(CapSecError::OutOfScope {
                    target: target.to_string(),
                    scope: format!("{self:?}"),
                });
            }
        }
        if let Some(prefix) = &self.path_prefix {
            let prefix_count = prefix.component_count();
            if path_components.len() < prefix_count {
                return Err(CapSecError::OutOfScope {
                    target: target.to_string(),
                    scope: format!("{self:?}"),
                });
            }
            for (i, prefix_comp) in prefix.components().enumerate() {
                if path_components[i] != prefix_comp.as_bytes() {
                    return Err(CapSecError::OutOfScope {
                        target: target.to_string(),
                        scope: format!("{self:?}"),
                    });
                }
            }
        }
        Ok(())
    }
}

// ── Willow URI encoding ────────────────────────────────────────────────────
//
// We define the `URIEncodeNamespaceId` / `URIEncodeSubspaceId` relations per
// the Willow URI spec (which leaves them as an instantiation parameter,
// requiring only that codes be host-safe). `willow25` does not expose them.
//
// Encoding rule (uniform for ids and path components): keep literal only the
// unreserved set MINUS `.` (`A-Za-z0-9-_~`), and percent-encode every other
// byte with uppercase hex. We additionally encode `.` (which the spec's minimal
// `percent_encode` would leave literal) so that the `.` namespace/subspace
// separator and the `.`/`..` dot-segments remain unambiguous. The result is
// still host-safe and path-safe; encoding more aggressively than the minimum is
// permitted by the spec.

const HEX_UPPER: &[u8; 16] = b"0123456789ABCDEF";

/// Returns true for bytes kept literal in our encoding: unreserved minus `.`.
fn is_literal_byte(b: u8) -> bool {
    matches!(
        b,
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'~'
    )
}

fn percent_encode_bytes(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    for &b in bytes {
        if is_literal_byte(b) {
            out.push(b as char);
        } else {
            out.push('%');
            out.push(HEX_UPPER[(b >> 4) as usize] as char);
            out.push(HEX_UPPER[(b & 0x0f) as usize] as char);
        }
    }
    out
}

fn hex_val(b: u8) -> Result<u8, ParseTargetError> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(ParseTargetError::InvalidPercentEncoding),
    }
}

fn percent_decode_bytes(s: &str) -> Result<Vec<u8>, ParseTargetError> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return Err(ParseTargetError::InvalidPercentEncoding);
            }
            let hi = hex_val(bytes[i + 1])?;
            let lo = hex_val(bytes[i + 2])?;
            out.push((hi << 4) | lo);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    Ok(out)
}

/// Encodes a Willow target URI for `(namespace, subspace, path)`:
/// `willow://<namespace_id>.<subspace_id>/<path/components>`.
///
/// An empty path produces `willow://<ns>.<sub>` (no trailing slash).
pub fn encode_target(namespace: &NamespaceId, subspace: &SubspaceId, path: &Path) -> String {
    let mut s = String::from("willow://");
    s.push_str(&percent_encode_bytes(namespace.as_bytes()));
    s.push('.');
    s.push_str(&percent_encode_bytes(subspace.as_bytes()));
    for comp in path.components() {
        s.push('/');
        s.push_str(&percent_encode_bytes(comp.as_bytes()));
    }
    s
}

#[derive(Debug, Error)]
pub enum ParseTargetError {
    #[error("missing `willow://` scheme")]
    MissingScheme,
    #[error("missing `.` namespace/subspace separator")]
    MissingSeparator,
    #[error("namespace/subspace id is not the expected 32 bytes")]
    InvalidIdLength,
    #[error("invalid percent-encoding")]
    InvalidPercentEncoding,
}

/// Parses a Willow target URI into `(namespace_id, subspace_id, path component bytes)`.
pub fn parse_target(
    uri: &str,
) -> Result<(NamespaceId, SubspaceId, Vec<Vec<u8>>), ParseTargetError> {
    let rest = uri
        .strip_prefix("willow://")
        .ok_or(ParseTargetError::MissingScheme)?;
    let dot = rest.find('.').ok_or(ParseTargetError::MissingSeparator)?;
    let ns_enc = &rest[..dot];
    let after = &rest[dot + 1..];
    let (sub_enc, path_part) = match after.find('/') {
        Some(slash) => (&after[..slash], &after[slash + 1..]),
        None => (after, ""),
    };

    let ns_bytes = percent_decode_bytes(ns_enc)?;
    let sub_bytes = percent_decode_bytes(sub_enc)?;
    if ns_bytes.len() != NAMESPACE_ID_WIDTH {
        return Err(ParseTargetError::InvalidIdLength);
    }
    if sub_bytes.len() != SUBSPACE_ID_WIDTH {
        return Err(ParseTargetError::InvalidIdLength);
    }
    let ns_arr: [u8; NAMESPACE_ID_WIDTH] = ns_bytes
        .as_slice()
        .try_into()
        .map_err(|_| ParseTargetError::InvalidIdLength)?;
    let sub_arr: [u8; SUBSPACE_ID_WIDTH] = sub_bytes
        .as_slice()
        .try_into()
        .map_err(|_| ParseTargetError::InvalidIdLength)?;
    let ns_id = NamespaceId::from_bytes(&ns_arr);
    let sub_id = SubspaceId::from_bytes(&sub_arr);

    let mut components = Vec::new();
    if !path_part.is_empty() {
        for comp in path_part.split('/') {
            components.push(percent_decode_bytes(comp)?);
        }
    }
    Ok((ns_id, sub_id, components))
}

/// A valid Willow URI that any wildcard [`WillowScope`] (all `None`) matches.
/// Intended as the target for a [`CapCell`] gating vault-wide content behind a
/// wildcard session capability: it is a parseable URI, and a wildcard scope
/// accepts any parseable target.
pub fn wildcard_target() -> String {
    // `willow://<zero_ns>.<zero_sub>` (no path = empty path). A wildcard
    // [`WillowScope`] accepts any parseable target, so this serves as the
    // gate target for a [`CapCell`] holding vault-wide content behind a
    // wildcard session capability.
    format!(
        "willow://{}.{}",
        percent_encode_bytes(&[0u8; NAMESPACE_ID_WIDTH]),
        percent_encode_bytes(&[0u8; SUBSPACE_ID_WIDTH]),
    )
}

// ── Cap-gated secret custodian ──────────────────────────────────────────────

/// Errors from cap-gated secret access.
#[derive(Debug, Error)]
pub enum WillowKeyError {
    /// The presented capability was expired, revoked, or out of scope.
    #[error(transparent)]
    Cap(#[from] CapSecError),
    /// The key store does not hold a secret for this namespace.
    #[error("no namespace secret for {0:?}")]
    UnknownNamespace(NamespaceId),
    /// The key store does not hold a secret for this subspace.
    #[error("no subspace secret for {0:?}")]
    UnknownSubspace(SubspaceId),
}

/// The cap-gated custodian of the vault's Willow namespace/subspace secrets.
///
/// Secrets enter the store at construction (from the vault's unlocked content)
/// and never leave it except through a capability check: either
/// [`mint_write_authority`](Self::mint_write_authority) (for Willow writes) or
/// [`subspace_secret_for_signing`](Self::subspace_secret_for_signing) (for the
/// "sign as Profile" p2p use). Both require a valid
/// [`StrictSendCap`]`<WillowWrite, WillowScope>`; a missing/expired/revoked or
/// out-of-scope cap produces a visible [`CapSecError`], never a silent drop.
#[derive(Clone)]
pub struct WillowKeyStore {
    namespaces: HashMap<NamespaceId, NamespaceSecret>,
    subspaces: HashMap<SubspaceId, SubspaceSecret>,
}

impl WillowKeyStore {
    /// Constructs a key store from the given secrets. The secrets are now
    /// gated behind capability checks; callers should not retain separate
    /// references to them.
    pub fn new(
        namespaces: HashMap<NamespaceId, NamespaceSecret>,
        subspaces: HashMap<SubspaceId, SubspaceSecret>,
    ) -> Self {
        Self {
            namespaces,
            subspaces,
        }
    }

    /// Returns `true` if the store holds a secret for this namespace.
    pub fn has_namespace(&self, id: &NamespaceId) -> bool {
        self.namespaces.contains_key(id)
    }

    /// Returns `true` if the store holds a secret for this subspace.
    pub fn has_subspace(&self, id: &SubspaceId) -> bool {
        self.subspaces.contains_key(id)
    }

    /// Inserts a namespace secret, replacing any existing entry for the same id.
    pub fn insert_namespace(&mut self, id: NamespaceId, secret: NamespaceSecret) {
        self.namespaces.insert(id, secret);
    }

    /// Inserts a subspace secret, replacing any existing entry for the same id.
    pub fn insert_subspace(&mut self, id: SubspaceId, secret: SubspaceSecret) {
        self.subspaces.insert(id, secret);
    }

    /// Mints the signed-data [`WriteCapability`] and yields the
    /// [`SubspaceSecret`] needed to sign a Willow entry at `(namespace,
    /// subspace, path)`, gated by a valid `WillowWrite` capability.
    ///
    /// The capability's scope is checked against the Willow URI target for
    /// `(namespace, subspace, path)`; on success the TTL is refreshed (sliding
    /// window). The returned materials are what the Willow store needs to
    /// perform the write — having them is equivalent to holding the write
    /// authority the cap granted.
    pub fn mint_write_authority(
        &self,
        cap: &StrictSendCap<WillowWrite, WillowScope>,
        namespace_id: &NamespaceId,
        subspace_id: &SubspaceId,
        path: &Path,
    ) -> Result<(WriteCapability, SubspaceSecret), WillowKeyError> {
        let target = encode_target(namespace_id, subspace_id, path);
        let _proof = cap.try_cap(&target)?;
        let ns_secret = self
            .namespaces
            .get(namespace_id)
            .ok_or(WillowKeyError::UnknownNamespace(namespace_id.clone()))?;
        let sub_secret = self
            .subspaces
            .get(subspace_id)
            .ok_or(WillowKeyError::UnknownSubspace(subspace_id.clone()))?;
        // Match the existing minting behaviour: an owned-style WriteCapability
        // signed by the namespace secret. (Communal namespaces also carry a
        // secret in the vault; this preserves prior behaviour exactly.)
        let write_cap = WriteCapability::new_owned(ns_secret, subspace_id.clone());
        Ok((write_cap, sub_secret.clone()))
    }

    /// Yields the subspace secret for "sign as Profile" (p2p envelope signing),
    /// gated by a valid `WillowWrite` capability whose subspace scope covers
    /// `subspace_id`.
    ///
    /// This is the capability check that replaces the former
    /// `secret_todo_consider_access_control` seam. Per T1 design question §2,
    /// the "sign as Profile" use shares the subspace secret with `WillowWrite`;
    /// a single cap gates both until a future `ProfileAct` splits them. The
    /// namespace and path-prefix fields of the scope are not consulted here —
    /// signing is a subspace-identity operation, not a Willow-path operation.
    pub fn subspace_secret_for_signing(
        &self,
        cap: &StrictSendCap<WillowWrite, WillowScope>,
        subspace_id: &SubspaceId,
    ) -> Result<SubspaceSecret, WillowKeyError> {
        cap.is_active()?;
        cap.scope().authorizes_subspace(subspace_id)?;
        self.subspaces
            .get(subspace_id)
            .cloned()
            .ok_or(WillowKeyError::UnknownSubspace(subspace_id.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use capsec::Scope;
    use willow25::prelude::*;

    fn fresh_ids() -> (NamespaceId, NamespaceSecret, SubspaceId, SubspaceSecret) {
        let mut rng = rand_core_0_6_4::OsRng;
        let (ns_id, ns_secret) = randomly_generate_owned_namespace(&mut rng);
        let (sub_id, sub_secret) = randomly_generate_subspace(&mut rng);
        (ns_id, ns_secret, sub_id, sub_secret)
    }

    #[test]
    fn target_uri_round_trips() {
        let (ns_id, _, sub_id, _) = fresh_ids();
        let path: Path = path!("/apps/contacts/abc");
        let uri = encode_target(&ns_id, &sub_id, &path);
        assert!(uri.starts_with("willow://"));
        let (ns2, sub2, comps) = parse_target(&uri).expect("parse");
        assert_eq!(ns2, ns_id);
        assert_eq!(sub2, sub_id);
        assert_eq!(
            comps,
            vec![b"apps".to_vec(), b"contacts".to_vec(), b"abc".to_vec()]
        );
    }

    #[test]
    fn target_uri_round_trips_empty_path() {
        let (ns_id, _, sub_id, _) = fresh_ids();
        let path: Path = path!("");
        let uri = encode_target(&ns_id, &sub_id, &path);
        let expected = format!(
            "willow://{}.{}",
            percent_encode_bytes(ns_id.as_bytes()),
            percent_encode_bytes(sub_id.as_bytes())
        );
        assert_eq!(uri, expected);
        let (ns2, sub2, comps) = parse_target(&uri).expect("parse");
        assert_eq!(ns2, ns_id);
        assert_eq!(sub2, sub_id);
        assert!(comps.is_empty());
    }

    #[test]
    fn scope_wildcard_matches_anything() {
        let (ns_id, _, sub_id, _) = fresh_ids();
        let scope = WillowScope::wildcard();
        let path: Path = path!("/any/path");
        let target = encode_target(&ns_id, &sub_id, &path);
        assert!(scope.check(&target).is_ok());
    }

    #[test]
    fn scope_namespace_scoped_rejects_other_namespace() {
        let (ns_a, _, sub_id, _) = fresh_ids();
        let (ns_b, _, _, _) = fresh_ids();
        let scope = WillowScope::new(Some(ns_a.clone()), None, None);
        let path: Path = path!("/x");
        let target_a = encode_target(&ns_a, &sub_id, &path);
        let target_b = encode_target(&ns_b, &sub_id, &path);
        assert!(scope.check(&target_a).is_ok());
        assert!(scope.check(&target_b).is_err());
    }

    #[test]
    fn scope_path_prefix_matches_prefix_only() {
        let (ns_id, _, sub_id, _) = fresh_ids();
        let prefix: Path = path!("/apps/contacts");
        let scope = WillowScope::new(Some(ns_id.clone()), Some(sub_id.clone()), Some(prefix));
        let in_scope = encode_target(&ns_id, &sub_id, &path!("/apps/contacts/abc"));
        let out_of_scope = encode_target(&ns_id, &sub_id, &path!("/apps/calendar/abc"));
        let too_short = encode_target(&ns_id, &sub_id, &path!("/apps"));
        assert!(scope.check(&in_scope).is_ok());
        assert!(scope.check(&out_of_scope).is_err());
        assert!(scope.check(&too_short).is_err());
    }

    #[test]
    fn key_store_mint_requires_valid_cap() {
        let (ns_id, ns_secret, sub_id, sub_secret) = fresh_ids();
        let mut namespaces = HashMap::new();
        namespaces.insert(ns_id.clone(), ns_secret);
        let mut subspaces = HashMap::new();
        subspaces.insert(sub_id.clone(), sub_secret);
        let store = WillowKeyStore::new(namespaces, subspaces);

        let root = capsec::test_root();
        let (cap, revoker) = StrictSendCap::new(
            root.grant::<WillowWrite>().make_send(),
            std::time::Duration::from_secs(60),
            WillowScope::wildcard(),
        );
        let path: Path = path!("/apps/contacts/x");

        let (write_cap, _secret) = store
            .mint_write_authority(&cap, &ns_id, &sub_id, &path)
            .expect("mint with valid cap");
        assert!(write_cap.is_owned());

        revoker.revoke();
        let err = store
            .mint_write_authority(&cap, &ns_id, &sub_id, &path)
            .unwrap_err();
        assert!(matches!(err, WillowKeyError::Cap(CapSecError::Revoked)));
    }

    #[test]
    fn key_store_mint_rejects_out_of_scope_cap() {
        let (ns_id, ns_secret, sub_id, sub_secret) = fresh_ids();
        let (other_ns, _, _, _) = fresh_ids();
        let mut namespaces = HashMap::new();
        namespaces.insert(ns_id.clone(), ns_secret);
        let mut subspaces = HashMap::new();
        subspaces.insert(sub_id.clone(), sub_secret);
        let store = WillowKeyStore::new(namespaces, subspaces);

        let root = capsec::test_root();
        // Cap scoped to a different namespace — must not authorize writes here.
        let (cap, _revoker) = StrictSendCap::new(
            root.grant::<WillowWrite>().make_send(),
            std::time::Duration::from_secs(60),
            WillowScope::new(Some(other_ns), None, None),
        );
        let path: Path = path!("/x");
        let err = store
            .mint_write_authority(&cap, &ns_id, &sub_id, &path)
            .unwrap_err();
        assert!(matches!(
            err,
            WillowKeyError::Cap(CapSecError::OutOfScope { .. })
        ));
    }

    #[test]
    fn key_store_signing_gated_by_subscope() {
        let (ns_id, ns_secret, sub_id, sub_secret) = fresh_ids();
        let (other_sub, _) = randomly_generate_subspace(&mut rand_core_0_6_4::OsRng);
        let mut namespaces = HashMap::new();
        namespaces.insert(ns_id, ns_secret);
        let mut subspaces = HashMap::new();
        subspaces.insert(sub_id.clone(), sub_secret);
        let store = WillowKeyStore::new(namespaces, subspaces);

        let root = capsec::test_root();
        // Cap scoped to a *different* subspace.
        let (cap, _revoker) = StrictSendCap::new(
            root.grant::<WillowWrite>().make_send(),
            std::time::Duration::from_secs(60),
            WillowScope::new(None, Some(other_sub), None),
        );
        let err = store
            .subspace_secret_for_signing(&cap, &sub_id)
            .unwrap_err();
        assert!(matches!(
            err,
            WillowKeyError::Cap(CapSecError::OutOfScope { .. })
        ));

        // Wildcard cap authorizes signing as the subspace.
        let (cap2, _revoker2) = StrictSendCap::new(
            root.grant::<WillowWrite>().make_send(),
            std::time::Duration::from_secs(60),
            WillowScope::wildcard(),
        );
        assert!(store.subspace_secret_for_signing(&cap2, &sub_id).is_ok());
    }
}
