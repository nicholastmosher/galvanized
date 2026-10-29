# 002 — `AutomergeSyncCapability` signed object

**Status:** DRAFT
**Depends on:** 001 (soft — the engine consumes this; the type itself is
independent)

## Why

L0's capability duality requires a **serialized signed object** the sync
engine checks, linked to a **Rust type-token** that gates the local API. For
automerge sync, the capability binds a `DocumentId` to a Willow scope and
names the recipient (or `Public`), so the engine can answer "may this peer
sync this doc in this direction?" from signed evidence alone. This ticket
defines that object in `galvanized-cap`, covering all three modes
(recipient-specific, attributable-public, anonymous-public) per Nick's
rulings.

## Current state (anchors — re-derive before leaning on)

- `galvanized/crates/galvanized-cap/src/lib.rs:42` — `WillowRead`, `WillowWrite`.
- `galvanized/crates/galvanized-cap/src/lib.rs:61` — `WillowScope` (namespace
  + subspace + path prefix, structured matching against Willow URIs).
- `galvanized/crates/galvanized-cap/src/lib.rs:344` — `WillowKeyStore`:
  `mint_write_authority` (mints Willow `WriteCapability` from the namespace
  secret, gated by `StrictSendCap<WillowWrite, WillowScope>`) and
  `subspace_secret_for_signing` (gated "sign as Profile"). Holds both
  namespace and subspace secrets — exactly what the anonymous-public
  (namespace-secret) signing needs.
- `automerge` crate — `DocumentId` is whatever `galvanized-sync` adopts (samod
  uses a UUID; we may reuse or redefine — see design Q5).

## Scope

1. **`AutomergeSyncCapability` signed object** (in `galvanized-cap`):

   ```
   AutomergeSyncCapability {
     signer: Signer,                // Subspace(SubspaceId) | Namespace(NamespaceId)
     recipient: Recipient,          // Subspace(SubspaceId) | Public
     document_id: DocumentId,       // the automerge doc authorized
     willow_scope: WillowScope,     // (namespace, subspace, path_prefix) binding
     permission: AutomergeSyncPermission,  // Read | Write
     expiry: UnixTimestamp,
     nonce: [u8; 32],               // revocation tracking id
   }
   signed by the signer's secret (subspace secret or namespace secret)
   ```

   - Canonical CBOR encoding (deterministic, RFC 8949) so signatures reproduce
     across implementations. Verification = `verify(public_key, canonical)`.
   - The validator (005) performs signature check + structural checks
     (recipient match, expiry, scope, revocation).

2. **`Signer` enum** (`Subspace(SubspaceId) | Namespace(NamespaceId)`) —
   ruling #11. The validator selects the public key to verify against from
   the variant. `Subspace` names the owner Profile (attributable);
   `Namespace` names only the namespace (anonymous at the Profile level).

3. **`Recipient` enum** (`Subspace(SubspaceId) | Public`). `Public` =
   wildcard; anyone presenting it + key proof may sync (read-only, see below).

4. **`AutomergeSyncPermission`** (`Read | Write`). Distinct from
   `WillowRead/Write` (which gate Willow store ops); this gates sync
   participation. Recommendation: distinct type (cheap now, costly to split
   later). See design Q1.

5. **Public-cap constraints (ruling #9).** A cap with `recipient = Public`:
   - `permission` MUST be `Read` (no public `Write` — anyone-push is almost
     never wanted). Enforce at mint.
   - Tight TTL (PROPOSED default; see design Q3). Expiry is load-bearing for
     public caps because revocation stops future sync, not already-synced
     bytes — a short TTL bounds exposure.
   - Revokable via the `nonce` tombstone (ticket 006).

6. **Minting**, gated by `StrictSendCap<WillowWrite, WillowScope>` reusing
   the `WillowKeyStore` pattern:
   - Recipient-specific / attributable-public: sign with the owner's
     `SubspaceSecret` (`subspace_secret_for_signing`).
   - Anonymous-public: sign with the namespace secret
     (`mint_write_authority`-adjacent path — the namespace secret is already
     held). The `signer = Namespace(namespace_id)`.
   - "No authority, no mint" — absent or out-of-scope `StrictSendCap` →
     visible error, not a silent drop.

7. **Type-token dual (L0).** A `StrictCap<AutomergeSync, WillowScope>` (or a
   new `AutomergeSync` permission in `strict-cap`) **minted from** a verified
   `AutomergeSyncCapability`. The local API (plugin-automerge sync
   establishment) requires this token to initiate a gated sync. The
   serialized object is the minting oracle.

8. **Binding independence (ruling #6).** `document_id` and `willow_scope`
   are independent fields; nothing derives one from the other. Document this
   in the type's doc comment so a future reader doesn't "optimize" the
   binding away.

9. **Multi-owner / communal namespaces.** For a communal namespace, any
   subspace holder writes; each mints as `signer = Subspace(their SubspaceId)`
   scoped to their own subspace. Communal = attributable by nature;
   anonymous-public is owned-namespace only. Document; no special type. See
   design Q2.

10. **Honest limit (ruling #9).** State in the type docs: revoking a public
    cap stops *future* sync; peers who already synced keep the bytes. Public
    sharing is an irrevocable-to-that-point visibility grant.

## Out of scope

- The validator that *consumes* this (005).
- The handshake that *presents* this (004).
- The `CapabilityStore` that *holds/distributes* this (003).
- Delegation/chaining (mint-by-signer only in v1).
- A capability-management UI.

## Design questions for Nick

1. **`AutomergeSyncPermission` distinct from `WillowRead/Write`?** See scope
   §4. Recommendation: distinct. Confirm.

2. **Communal-namespace minting convention.** "Each writer mints as
   `signer = their SubspaceId`, scoped to their own subspace" — confirm, or a
   different rule? Affects the validator (005).

3. **Public-cap TTL default.** PROPOSED: 15 minutes (matching T1's session-cap
   default), configurable per mint. Public-cap exposure is bounded by TTL
   since revocation can't recall bytes. Confirm or pick a value.

4. **Nonce source.** Random 32 bytes at mint (allows re-minting an identical
   cap; simple revocation identity), or derived from
   `(signer, recipient, document_id, expiry)`? Recommendation: **random**.
   Confirm.

5. **`DocumentId` type.** Reuse samod's UUID shape, or define a fresh one in
   `galvanized-sync`? Recommendation: **fresh in `galvanized-sync`** (we're
   not bound to samod's types); a `DocumentId` newtype over a fixed byte
   array, base58 on the wire. Confirm — this is a 001/002 shared decision.

6. **Canonical encoding.** Deterministic CBOR (RFC 8949) vs a
   length-prefixed encoding. Recommendation: **deterministic CBOR**.
   Confirm.

## Acceptance criteria

- `AutomergeSyncCapability` round-trips through canonical encoding;
  signatures verify; tampering any field invalidates the signature.
- All three modes mint + verify: recipient-specific (`Signer::Subspace`,
  `Recipient::Subspace`), attributable-public (`Signer::Subspace`,
  `Recipient::Public`, `Read`), anonymous-public (`Signer::Namespace`,
  `Recipient::Public`, `Read`).
- Public caps with `permission = Write` are refused at mint.
- Minting requires a valid `StrictSendCap<WillowWrite, WillowScope>`; absent
  or out-of-scope → visible error.
- The type-token is mintable only from a verified serialized cap.
- Tests: mint+verify, tamper-detection, expiry, recipient-mismatch,
  scope-mismatch, public-Write-refused, anonymous-public hides the Profile.

## Verification

- `cargo test -p galvanized-cap`.
- `cargo fmt`; no clippy unless Nick asks.
- State EXECUTED vs RELAYED.

## Records

- Add `AutomergeSyncCapability` (and the three modes) to `GLOSSARY.md` under
  "Live collaboration."
- Move design Q1–Q6 rulings to `DECISIONS.md`.