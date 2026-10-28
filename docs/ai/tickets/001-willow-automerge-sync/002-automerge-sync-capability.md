# 002 — `AutomergeSyncCapability` signed object + type-token

**Status:** DRAFT
**Depends on:** 001 (soft — the validator consumes this; the type itself is
independent)

## Why

L0's capability duality requires a **serialized signed object** that the sync
layer checks, linked to a **Rust type-token** that gates the local API. For
automerge sync specifically, the capability must bind a `DocumentId` to a
Willow scope and name the recipient Profile, so the sync engine can answer
"may this peer sync this doc in this direction?" from signed evidence alone.
This ticket defines that object in `galvanized-cap` and its type-token,
reusing the existing `WillowScope` + `StrictSendCap` machinery.

## Current state (anchors — re-derive before leaning on)

- `galvanized/crates/galvanized-cap/src/lib.rs:42` — `WillowRead`, `WillowWrite`
  permissions.
- `galvanized/crates/galvanized-cap/src/lib.rs:61` — `WillowScope` (namespace
  + subspace + path prefix, structured matching against Willow URIs).
- `galvanized/crates/galvanized-cap/src/lib.rs:344` — `WillowKeyStore` with
  `mint_write_authority` (mints Willow `WriteCapability` gated by
  `StrictSendCap<WillowWrite, WillowScope>`) and `subspace_secret_for_signing`
  (gated "sign as Profile").
- `samod/samod-core/src/document_id.rs` — `DocumentId(Uuid)`, bs58 on the wire.
- Willow: `willow25::entry::{SubspaceId, SubspaceSecret, NamespaceSecret}`.

## Scope

1. **`AutomergeSyncCapability` signed object** (in `galvanized-cap`):

   ```
   AutomergeSyncCapability {
     owner: SubspaceId,            // doc owner's Profile (minter)
     recipient: SubspaceId,        // peer's Profile (granted to)
     document_id: DocumentId,      // the automerge doc authorized
     willow_scope: WillowScope,    // (namespace, subspace, path_prefix) binding
     permission: AutomergeSyncPermission,  // Read | Write
     expiry: UnixTimestamp,
     nonce: [u8; 32],              // revocation tracking id
   }
   signed by owner's SubspaceSecret
   ```

   - Serialization: CBOR (consistent with samod's wire codec and Willow's
     signing). Define a canonical encoding so signatures are reproducible.
   - Verification: `verify(public_key=&owner, message=canonical_bytes)` →
     bool; plus the structural checks (recipient match, expiry, scope) that
     the validator (ticket 004) performs.

2. **`AutomergeSyncPermission`** (`Read | Write`). Distinct from
   `WillowRead`/`WillowWrite`? Or alias them? See design Q1. Recommendation:
   **distinct type** — `WillowRead/Write` gate Willow *store* operations;
   `AutomergeSyncPermission` gates *sync participation*. They align today but
   may diverge (e.g. a future `Admin` sync permission). Cheap to keep
   separate now; expensive to split later.

3. **Minting**, gated by `StrictSendCap<WillowWrite, WillowScope>` reusing
   the `WillowKeyStore` pattern: the owner's right to mint a sync cap for
   `(namespace, subspace, path)` is itself gated by a typed cap, so "no
   authority, no mint." The mint yields the serialized
   `AutomergeSyncCapability` signed by the owner's `SubspaceSecret`.

4. **Type-token dual.** A `StrictCap<AutomergeSync, WillowScope>` (or a new
   `AutomergeSync` permission in `strict-cap`) that is **minted from** a
   verified `AutomergeSyncCapability` object. The local programming API
   (plugin-automerge's sync-establishment) requires this token to initiate a
   gated sync — "no token, no call." The serialized object is the minting
   oracle. This is the L0 duality made concrete for sync.

5. **Binding independence (ruling #6).** `document_id` and `willow_scope`
   are independent fields; nothing derives one from the other. The signed
   claims are the only link. Document this explicitly in the type's doc
   comment so a future reader doesn't "optimize" the binding away.

6. **Multi-owner / communal namespaces.** For a communal namespace, any
   subspace holder may write; each writer mints caps as `owner = their
   SubspaceId` for their own subspace. Document the convention; do not
   special-case in the type. See design Q2.

## Out of scope

- The validator that *consumes* this (ticket 004).
- The handshake that *presents* this (ticket 003).
- Delegation/chaining (mint-by-owner only in the first deliverable).
- A capability-management UI.

## Design questions for Nick

1. **`AutomergeSyncPermission` distinct from `WillowRead/Write`?** See scope
   §2. Recommendation: distinct. Confirm.

2. **Communal-namespace cap minting convention.** Is "each writer mints as
   `owner = their SubspaceId`, scoped to their own subspace" the right rule,
   or should communal namespaces have a different cap shape? Recommendation:
   the former — it falls out of Meadowcap naturally and needs no special
   type. Confirm; this affects the validator (004).

3. **Canonical encoding.** CBOR with a fixed key order, or a Willow-style
   length-prefixed encoding? Recommendation: **CBOR with deterministic
   encoding** (RFC 8949) — consistent with samod's codec and easy to verify
   across implementations. Confirm.

4. **Nonce source.** Random 32 bytes at mint, or derived from
   `(owner, recipient, document_id, expiry)`? Recommendation: **random** —
   derivation would prevent re-minting an identical cap (which is sometimes
   useful) and complicates revocation identity. Confirm.

## Acceptance criteria

- `AutomergeSyncCapability` round-trips through its canonical encoding;
  signatures verify; tampering any field invalidates the signature.
- Minting requires a valid `StrictSendCap<WillowWrite, WillowScope>`; absent
  or out-of-scope → visible error, not a silent drop.
- The type-token is mintable only from a verified serialized cap.
- `galvanized-cap` tests cover: mint+verify, tamper-detection, expiry,
  recipient-mismatch, scope-mismatch.

## Verification

- `cargo test -p galvanized-cap`.
- `cargo fmt`; do **not** run clippy unless Nick asks (per ratified ruling
  2026-07-30).
- State EXECUTED vs RELAYED.

## Records

- Add `AutomergeSyncCapability` to `GLOSSARY.md` under "Live collaboration."
- Move design Q1–Q4 rulings to `DECISIONS.md`.