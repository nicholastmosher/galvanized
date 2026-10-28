# 005 — Revocation via Willow tombstones + reconnect re-check

**Status:** DRAFT
**Depends on:** 002 (cap nonce), 004 (gating consults the revocation view)

## Why

Revocation in a serverless p2p system is **data that must propagate and win**
in an eventually-consistent store — there is no central revoke call (L0 §Q3,
§constraints). Ruling #4 sets the floor: **re-check on reconnect**. Ruling #7
sets the location: **tombstones under the doc's own namespace**, so syncing
the namespace propagates revocations to all peers for free. This ticket builds
the tombstone convention, the revocation view the validator (004) consults,
and the reconnect re-check into the handshake (003).

## Current state (anchors — re-derive before leaning on)

- `galvanized/crates/plugin-willow/src/lib.rs:170` — `WillowCx::create_entry`
  (the write path; gated by `StrictCap<WillowWrite, …>` per T1).
- `galvanized/crates/plugin-willow/src/lib.rs:209` — `list_entries`;
  `:234` — `read_entry`. The read path for the revocation view.
- Willow: `willow25::entry::Entry`, `Area`, `AuthorisedEntry`.
- No revocation mechanism exists today.

## Scope

1. **Tombstone convention.** A revoked capability is recorded as a Willow
   entry in the doc's namespace at path `/{owner_subspace}/revoked/{cap_nonce}`,
   where `cap_nonce` is the 32-byte nonce from the `AutomergeSyncCapability`
   (ticket 002). The entry's presence = revoked. Content: minimal (a timestamp
   + optionally the revoking `SubspaceId`), since the path *is* the identity.

   - Path encoding: the nonce as hex or base58 in the final path component.
     Decide with Nick (design Q1).
   - The tombstone is a **write** to the doc's namespace, so it requires the
     owner's `WriteCapability` (minted via `WillowKeyStore::mint_write_authority`,
     gated by `StrictSendCap<WillowWrite, …>`). Only the owner (or a delegated
     revoker — open sub-problem, see L0 custody) can revoke.

2. **Revocation view.** A local, per-namespace set of revoked nonces the
   validator (004) consults at call time. Populated by:

   - **Syncing the doc's namespace** — tombstones arrive as Willow entries
     and are indexed into the view. This is the primary propagation path:
     no separate revocation channel.
   - **Local revocation** — when the local owner revokes, write the tombstone
     and index it locally immediately.

   The view is a `HashSet<[u8; 32]>` (or `HashMap<nonce, revocation_meta>`)
   per namespace, kept in sync with the Willow store. PROPOSED: the
   `WillowCx` exposes a `revocation_view(namespace) -> RevocationView`
   accessor; the validator holds it.

3. **Reconnect re-check (ruling #4 floor).** On a new galvanized-ALPN
   connection (003), before accepting the peer's presented caps, sync the
   revocation view for the relevant namespace(s) and reject any cap whose
   nonce is tombstoned. This is the minimum guaranteed enforcement: a revoked
   peer cannot re-establish sync after the tombstone has propagated to the
   local view.

4. **Mid-session best-effort.** Because the validator (004) reads the view at
   call time, a tombstone that syncs in mid-session takes effect on the next
   `Request`/`Sync` — without a reconnect. This is a free improvement over
   the floor; document it as best-effort (depends on Willow sync latency).

5. **In-flight teardown (fast follow, NOT in this ticket).** Actively killing
   a live session the instant a tombstone lands (pushing a notification over
   the control stream, or the Hub polling the view between messages) is
   explicitly deferred per ruling #4. Tracked as a sub-item; do not implement
   here.

6. **No silent ignore.** A revocation that cannot be written (missing write
   cap) surfaces a visible error — do not `let _ =` it.

## Out of scope

- In-flight session teardown (fast follow).
- Revocation *authority* design (who may revoke beyond the owner — the L0
  custody "revocation authority & DoS tradeoff" open sub-problem). This
  ticket assumes the owner revokes; delegated revocation is a follow-up.
- A revocation-management UI.

## Design questions for Nick

1. **Tombstone path encoding.** `/{owner_subspace}/revoked/{hex_nonce}` vs
   base58 vs a fixed-width binary path component. Recommendation: **base58**
   (consistent with `DocumentId`'s wire encoding). Confirm.

2. **Tombstone content.** Bare presence (empty value) vs. a signed
   revocation object (revoking `SubspaceId` + timestamp + signature). A
   signed object lets a third party verify a revocation was issued by the
   owner, not planted — relevant if the namespace ever admits delegated
   revokers. Recommendation: **signed revocation object** now — cheap, and
   avoids a format change when delegated revocation lands. Confirm.

3. **View freshness vs the handshake.** Does the handshake (003) block on a
   full namespace sync before accepting caps (slow but safe), or accept caps
   optimistically and re-check as the view catches up? Recommendation:
   **block on the revocation-view sync for the relevant namespace(s) before
   accepting** — a revoked peer must not get a window of sync. Confirm; this
   is security-shaped.

4. **Garbage collection.** Tombstones for expired caps (already past
   `expiry`) can be pruned. When? Recommendation: **never prune in the first
   deliverable** — keep it simple; pruning is a follow-up once volume
   matters. Confirm.

## Acceptance criteria

- An owner can revoke a cap: writing the tombstone to
  `/{owner_subspace}/revoked/{nonce}` succeeds (gated by the write cap) and
  indexes the nonce locally.
- After the tombstone propagates to a peer's view, that peer's next sync
  attempt for the revoked doc is refused with a visible error.
- On reconnect, a peer presenting a revoked cap is refused at the handshake
  (003) once the local view has the tombstone.
- A revocation landing mid-session best-effort denies the next
  `Request`/`Sync` (documented as best-effort).
- No silent ignores on revocation write failure.

## Verification

- Two-`Endpoint` in-process test: A mints a cap for B for D; B syncs; A
  revokes (tombstone); after Willow sync, B's next D sync is refused; B
  reconnects and is refused at the handshake.
- `cargo fmt`; no clippy unless Nick asks.
- State EXECUTED vs RELAYED.

## Records

- Add a "revocation via Willow tombstones" section to `ARCHITECTURE.md`.
- Move design Q1–Q4 rulings to `DECISIONS.md`.
- Note the L0 custody "revocation authority" sub-problem as still open (it
  composes on top of this, not resolved by it).