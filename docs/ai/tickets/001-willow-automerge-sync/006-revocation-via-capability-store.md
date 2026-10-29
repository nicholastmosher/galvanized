# 006 — Revocation via `CapabilityStore` tombstones

**Status:** DRAFT
**Depends on:** 002 (cap nonce), 003 (`CapabilityStore` + index doc), 005
(gating consults the revocation view)

## Why

Revocation in a serverless p2p system is **data that must propagate and win**
in an eventually-consistent store — there is no central revoke call (L0 §Q3).
Ruling #4 sets the floor: **re-check on reconnect**. Ruling #7 + #12 set the
mechanism: **tombstones via the `CapabilityStore` seam**, with the
self-sufficient per-peer automerge index doc (003) as today's propagation
channel. This ticket builds the tombstone write + the reconnect re-check into
the handshake (004), and states the honest limits (already-synced bytes;
in-flight teardown deferred).

## Reference points

- `galvanized/crates/galvanized-cap/src/lib.rs:344` — `WillowKeyStore` (the
  mint + sign paths the revoker reuses for signing a revocation object, if
  we adopt one — design Q2).
- The per-peer automerge index doc (003) — the propagation channel for
  attributable-public tombstones (append-only log).
- The `CapabilityStore::revoke` / `is_revoked` / `revocation_view` seam (003).

## Scope

1. **Tombstone convention.** A revoked capability is recorded by its `nonce`
   (the 32-byte field from `AutomergeSyncCapability`, 002). A tombstone =
   "this nonce is revoked." The validator (005) calls
   `CapabilityStore::is_revoked(nonce)` at gating time.

2. **Tombstone shape (design Q2).** PROPOSED: a **signed revocation object**
   (`{ nonce, revoked_by: Signer, timestamp }` signed by the revoking secret),
   not a bare nonce. A signed object lets a third party verify the revocation
   was issued by an authority (the original `signer` or a delegated revoker),
   not planted — relevant once delegated revocation lands, and cheap now.
   Recommendation: signed object. Confirm.

3. **Propagation by cap mode:**
   - **Attributable-public & all tombstones for the owner's docs:** appended
     to the owner's per-peer automerge index doc (003). Syncing that doc
     carries tombstones to all peers for free. **One propagation channel for
     all tombstones** — and because a tombstone is just a nonce (+ signed
     object), it reveals *which cap* was revoked, not *which recipient* — so
     placing recipient-specific revocations here does **not** leak the
     sharing graph. (Per 003 Q4.)
   - **Recipient-specific caps received locally:** tombstoned locally on
     revocation by the *owner*; the owner's tombstone append propagates to the
     owner's other devices via the index doc, so all the owner's devices
     refuse the revoked peer. The *recipient* learns of revocation on next
     reconnect (the owner's index doc syncs the tombstone to them too, since
     they already sync that doc for attributable-public caps — or via the
     handshake re-check).

4. **Reconnect re-check (ruling #4 floor).** On a new connection (004), before
   accepting the peer's presented caps, ensure the revocation view for the
   relevant docs is fresh (the index doc has synced) and reject any cap whose
   nonce is tombstoned. Decide blocking semantics with Nick (design Q3) —
   recommendation: **block on the revocation-view sync before accepting caps**
   (a revoked peer must not get a sync window).

5. **Mid-session best-effort.** The validator (005) reads the view at call
   time, so a tombstone syncing in mid-session denies the next
   `Request`/`Sync` without a reconnect. Document as best-effort (depends on
   sync latency).

6. **In-flight teardown (fast follow, NOT here).** Actively killing a live
   session the instant a tombstone lands (pushing a notification over the
   control stream, or the engine polling the view between messages) is
   explicitly deferred per ruling #4. Tracked as a sub-item.

7. **Honest limits (ruling #9), stated in docs:**
   - Revoking stops *future* sync; a peer that already synced keeps the bytes.
   - For public caps especially: a public doc is an irrevocable-to-that-point
     visibility grant. Short TTL (002) bounds exposure.
   - In-flight teardown is not in v1; a revoked peer may sync until the next
     message-level re-check denies them (best-effort window).

8. **No silent ignore.** A revocation that can't be written (missing signing
   authority) surfaces a visible error — no `let _ =`.

## Out of scope

- In-flight session teardown (fast follow).
- Delegated revocation authority (who may revoke beyond the `signer`) — the
  L0 custody "revocation authority & DoS tradeoff" open sub-problem. This
  ticket assumes the `signer` (owner) revokes; delegated revocation is a
  follow-up.
- A revocation-management UI.

## Design questions for Nick

1. **Tombstone shape.** Signed revocation object vs bare nonce. See scope §2.
   Recommendation: **signed object**. Confirm.

2. **Revoking authority.** v1: only the cap's `signer` (owner) may revoke
   (sign the tombstone). Confirm this is the v1 scope; delegated revocation
   is follow-up.

3. **Handshake blocking on revocation-view sync.** Block until the relevant
   index doc has synced (safe, slow) vs accept caps optimistically and re-check
   as the view catches up (fast, risky). Recommendation: **block** — a
   revoked peer must not get a sync window. Confirm (security-shaped).

4. **Garbage collection of tombstones.** Tombstones for expired caps can be
   pruned. When? Recommendation: **never prune in v1** — keep it simple;
   pruning is a follow-up once volume matters. Confirm.

## Acceptance criteria

- An owner can revoke a cap: `CapabilityStore::revoke(nonce)` writes a signed
  tombstone (attributable-public: appended to the index doc; recipient-specific:
  local + index-doc append).
- After the tombstone propagates to a peer's view, that peer's next sync for
  the revoked doc is refused with a visible error.
- On reconnect, a peer presenting a revoked cap is refused at the handshake
  (004) once the local view has the tombstone.
- A revocation landing mid-session best-effort denies the next
  `Request`/`Sync`.
- Tombstones do not leak the recipient set (a tombstone is a nonce + signed
  object, no recipient field) — asserted by test.
- No silent ignores on revocation write failure.

## Verification

- Two-`Endpoint` in-process test: A mints a cap for B for D; B syncs; A
  revokes (tombstone via 003); after sync, B's next D sync is refused; B
  reconnects and is refused at the handshake. Assert the index doc contents
  reveal no recipient identity.
- `cargo fmt`; no clippy unless Nick asks.
- State EXECUTED vs RELAYED.

## Records

- Add a "revocation via CapabilityStore tombstones" section to
  `ARCHITECTURE.md`.
- Move design Q1–Q4 rulings to `DECISIONS.md`.
- Note the L0 custody "revocation authority" sub-problem as still open (it
  composes on top of this, not resolved by it).