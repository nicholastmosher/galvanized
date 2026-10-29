# 003 — `CapabilityStore` trait seam + self-sufficient impl

**Status:** DRAFT
**Depends on:** 002 (stores `AutomergeSyncCapability` objects)

## Why

The engine needs to know where capabilities live and how they propagate — to
present caps at handshake (004), to consult the revocation view at gating
time (005), and to write tombstones on revocation (006). **No syncing Willow
store exists yet**, so the engine must be **self-sufficient** for capability
distribution today. But the design must not bake in the self-sufficient
mechanism permanently: a future syncing Willow store (caps at a designated
namespace path) is the natural evolution. So we define a `CapabilityStore`
trait seam now and ship a self-sufficient impl behind it.

Privacy is the primary case (Nick's ruling): the store design must not
publish a peer's sharing graph. This drives the split between attributable-public caps (in a per-peer public index doc) and recipient-specific caps (out-of-band, never public).

## Reference points

- `galvanized/crates/plugin-willow/src/lib.rs:170` — `WillowCx::create_entry`
  / `list_entries` / `read_entry` over `MemoryStore`. A future
  `CapabilityStore` impl can sit on these once Willow syncs.
- `galvanized/crates/plugin-contacts/src/protocol.rs:445` — `SignedEnvelope`
  (sender_id/recipient_id/timestamp, signed by subspace key). The model for
  out-of-band cap delivery in a contact ticket.
- samod's storage trait (KV + range queries) — inspiration for the local
  store part of the self-sufficient impl.

## Scope

1. **`CapabilityStore` trait** (in `galvanized-sync`, referenced by 001's
   seam). The engine's interface to caps + revocation. PROPOSED shape:

   ```rust
   trait CapabilityStore: Send + Sync {
       /// Caps the local peer holds that authorize it to sync docs
       /// owned by `peer` (or public caps relevant to `peer`).
       fn caps_to_present(&self, peer: &SubspaceId) -> Vec<AutomergeSyncCapability>;

       /// Record a cap the local peer received out-of-band (recipient-specific).
       fn store_received(&self, cap: AutomergeSyncCapability) -> Result<()>;

       /// Record a tombstone revoking a cap by nonce.
       fn revoke(&self, nonce: [u8; 32]) -> Result<()>;

       /// Whether a nonce is revoked (consulted at gating time, live).
       fn is_revoked(&self, nonce: [u8; 32]) -> bool;

       /// A live view the validator (005) reads at call time.
       fn revocation_view(&self) -> RevocationView;
   }
   ```

   Exact shape to rule on (design Q1). The key property: the engine depends
   on the **trait**, not any impl.

2. **Self-sufficient impl** (the v1 default, behind the seam):

   - **Attributable-public caps + tombstones:** a per-peer public-read-only
     automerge doc — the "index doc" — synced by `galvanized-sync` itself.
     Caps are **immutable append-only entries**; tombstones are appends.
     Invariant: never store a signed cap as an editable CRDT field (editing
     invalidates the signature; an editable field is a forge path). The doc
     is a log, not a map.
   - **Recipient-specific caps:** out-of-band (contact ticket / direct
     message / presented in the handshake). Held in a **local** store
     (in-memory or on-disk KV). **Never** written to the public index doc
     (would publish the sharing graph — the privacy counter in the
     brainstorm).
   - **Anonymous-public caps:** need an identity-less store (future Willow
     namespace path, or travel-with-the-doc). **Until that exists,
     anonymous-public caps travel out-of-band like recipient-specific ones.**
     State this limitation honestly; the trait seam lets us plug the
     identity-less impl later without touching the engine.

3. **Index-doc bootstrap.** A peer needs the index doc's `DocumentId`
   out-of-band before it can read the index. This shrinks "exchange N caps"
   to "exchange 1 doc id" but does not eliminate out-of-band exchange. The
   doc id travels in the contact ticket; thereafter the index is the source of
   further attributable-public caps. Document the bootstrap.

4. **Index doc synced by the engine itself.** The index doc is a public doc
   → it carries its own attributable-public cap (signed by the peer's
   subspace, or the namespace). Mild bootstrap recursion (the index doc's cap
   is carried out-of-band first), workable. Surface as a design note, not a
   blocker.

5. **Privacy review (load-bearing).** Confirm in review:
   - Recipient-specific caps never appear in any public store.
   - The public index doc reveals only attributable-public caps (which already
     declare themselves public + signed) — no recipient set, no private doc
     inventory.
   - Anonymous-public caps are not placed in an identity-bound index
     (de-anonymizing); they use the identity-less path when it exists.

6. **No silent drops.** A revocation that fails to write surfaces a visible
   error. A cap that can't be stored surfaces a visible error.

## Out of scope

- The syncing Willow store itself (the future impl behind the seam).
- The handshake that *presents* caps from the store (004).
- The validator that *consults* the store (005).
- A capability-management UI.

## Design questions for Nick

1. **`CapabilityStore` trait shape.** The PROPOSED shape above, or
   narrower/wider? Specifically: should `caps_to_present` take a `peer` and
   filter, or return all held caps and let the handshake filter? Recommendation:
   **take `peer` and filter** — keeps the handshake simple and avoids
   over-sharing. Confirm.

2. **Index doc as automerge.** The per-peer index doc is itself an automerge
   doc synced by `galvanized-sync`. Confirm this recursion is acceptable
   (it's the most self-sufficient option given no Willow sync yet). Alternative:
   a plain local KV + out-of-band for everything public too — but that loses
   automatic propagation of public caps across the peer's own devices and to
   others. Recommendation: **automerge index doc**. Confirm.

3. **Index doc cap signer.** Is the index doc's own public cap
   `Signer::Subspace` (attributable to the peer) or `Signer::Namespace`?
   Recommendation: **`Subspace`** — the index doc is intentionally
   attributable (it's the peer's public face); anonymous-public docs live
   elsewhere. Confirm.

4. **Revocation for recipient-specific caps across the owner's devices.** When
   the owner revokes a recipient-specific cap, the tombstone must reach the
   owner's other devices so they also refuse. The local store + the engine's
   own sync can carry tombstones, or a separate small synced doc. Recommendation:
   **tombstones ride in the same per-peer index doc** (append-only log) even
   for recipient-specific revocations — the tombstone is just a nonce, it
   doesn't reveal *which* recipient. Confirm (this keeps one propagation
   channel for all tombstones without leaking recipients).

## Acceptance criteria

- `CapabilityStore` trait defined; the engine depends on it, not on the impl.
- Self-sufficient impl: attributable-public caps + tombstones in a per-peer
  automerge index doc (append-only); recipient-specific caps in a local
  store, delivered out-of-band.
- Recipient-specific caps never appear in a public store (asserted by test).
- `is_revoked` / `revocation_view` reflect appends live.
- Anonymous-public caps travel out-of-band until the identity-less store
  exists (documented limitation).
- No silent drops on revocation/store failure.

## Verification

- `cargo test -p galvanized-sync`: append a public cap to the index doc →
  retrievable; append a tombstone → `is_revoked` true; a recipient-specific
  cap stored locally never appears in the index doc's contents.
- `cargo fmt`; no clippy unless Nick asks.
- State EXECUTED vs RELAYED.

## Records

- Add a "CapabilityStore" section to `ARCHITECTURE.md`.
- Move design Q1–Q4 rulings to `DECISIONS.md`.
- Note the anonymous-public identity-less store as a follow-up (open).