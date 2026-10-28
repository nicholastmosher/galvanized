# 004 — Per-document gating in Hub/DocumentActor

**Status:** DRAFT
**Depends on:** 001 (validator seam), 002 (cap object), 003 (handshake
establishes `PeerIdentity` + granted scopes)

## Why

The message-level gate. The handshake (003) establishes *who the peer is* and
*which caps they presented*; this ticket enforces, **per `Request`/`Sync`
message**, that the peer actually holds a valid capability for the specific
document and direction. It is the runtime enforcement that complements the
handshake's identity-establishment, and it consults the **live revocation
view** so a revocation that lands mid-session takes effect at the next message
without waiting for a reconnect.

This is where T4's core acceptance criterion lands: "a peer without a valid
capability cannot establish an automerge sync for a gated document."

## Current state (anchors — re-derive before leaning on)

- `samod/samod-core/src/actors/document/doc_state.rs:333` —
  `generate_sync_messages` iterates `peer_connections`, generates for all.
  This is the **outgoing** path to gate for push authority.
- `samod/samod-core/src/actors/document/doc_state.rs:236` —
  `handle_sync_message` applies received changes unconditionally. This is the
  **incoming** path to gate for receive authority.
- `samod/samod-core/src/network/wire_protocol.rs:43` — `Request`/`Sync`
  carry `document_id` + `data`; the `Hub` dispatches by `document_id`.

## Scope

1. **`CapabilityValidator` plug-in (galvanized impl).** Implements ticket
   001's `CapabilityValidator` trait. Input: `(peer_identity, document_id,
   direction)`. Output: `Allow | Deny(reason)`. The check:

   - The peer's `PeerIdentity` (from 003) includes a presented
     `AutomergeSyncCapability` whose `document_id` matches.
   - `recipient` is the local `SubspaceId`.
   - `permission` covers the direction:
     - **Push** (`Request`/outgoing `Sync` generation): requires
       `AutomergeSyncPermission::Write`.
     - **Receive** (incoming `Sync` application): requires `Read` *or*
       `Write`. (In the first deliverable, write-gated only — see ruling #5
       and ticket 007. Concretely: first deliverable requires `Write` for
       *any* sync; `Read`-only receive is 007.)
   - Not expired.
   - `nonce` not in the local revocation view (ticket 005).

2. **Hub consults the validator** on `Request`/`Sync` dispatch. On `Deny`:
   emit a visible `WireMessage::Error` for that document and **skip** the sync
   for that doc. The connection stays up — other docs may be authorized.

3. **Outgoing gate.** `generate_sync_messages` (or the Hub's send path)
   suppresses generation for a `(connection, document)` pair the validator
   denies. A peer with no `Write` cap for a doc never has sync messages
   generated for it. (First deliverable: no `Write` cap → no generation at
   all, since receive is also write-gated per ruling #5.)

4. **Incoming gate.** `handle_sync_message` (or the Hub's receive path)
   drops + errors visibly on a `Sync`/`Request` for a doc the validator
   denies. The document actor never sees unauthorized changes.

5. **Live revocation re-check.** The validator reads the revocation view
   *at call time*, not a snapshot from handshake. So a tombstone that syncs
   into the local view mid-session takes effect on the next message — the
   "re-check on reconnect" floor (ruling #4) plus best-effort mid-session
   enforcement for free. (Active in-flight teardown is still 005's fast
   follow.)

6. **Error visibility.** Every `Deny` surfaces a reason (which cap field
   failed: signature / recipient / expiry / permission / revoked). No silent
   drops.

## Out of scope

- Read-only *receive* (a `Read` cap letting a peer receive but not push) —
  ticket 007.
- In-flight session teardown on revocation — ticket 005 fast follow.
- How the revocation view populates — ticket 005.

## Design questions for Nick

1. **First-deliverable direction semantics.** Ruling #5 says write-gate first.
   Does that mean: *no `Write` cap → no sync at all (neither push nor
   receive)*, or *no `Write` cap → no push, but receive still allowed for
   anyone on the galvanized ALPN*? Recommendation: **the former** — no
   `Write` cap → no sync at all. The galvanized ALPN is for controlled docs;
   a peer with no cap has no business receiving either. This makes "read-only
   receive" cleanly the 007 addition (a `Read` cap enables receive-only).
   Confirm.

2. **Validator placement: Hub vs DocumentActor.** Ticket 001 design Q2 leans
   Hub. This ticket implements against that assumption. If Nick rules
   DocumentActor, the per-connection `PeerIdentity` must be reachable from the
   actor — re-confirm here.

3. **Deny granularity.** Per-doc deny (connection stays up, other docs
   proceed) vs whole-connection deny. Recommendation: **per-doc** — a peer
   may be authorized for doc A and not doc B; tearing down the connection
   over doc B is heavy-handed and loses doc A's sync. Confirm.

4. **Error message detail.** How much reason is safe to surface on the wire
   (privacy: does revealing "revoked" vs "expired" leak state)? Recommendation:
   surface a coarse category (`Unauthorized`) on the wire + a detailed reason
   in the local log/surfaced error. Confirm — this is privacy-adjacent.

## Acceptance criteria

- A peer with a valid `Write` cap for doc D can push and receive D.
- A peer with no cap for D (or a cap for a different doc) has D's sync
  refused with a visible per-doc error; the connection stays up for docs it
  is authorized for.
- A peer with an expired or revoked cap for D has D's sync refused.
- A revocation tombstone landing mid-session causes the next `Request`/`Sync`
  for D to be denied (best-effort, no reconnect needed).
- No silent drops.

## Verification

- Two-`Endpoint` in-process test: A mints a `Write` cap for B for doc D1,
  not for D2. B syncs D1; D2 is refused with a visible error; connection
  stays up. A revokes D1's cap (tombstone); after the view propagates, B's
  next D1 sync is refused.
- `cargo fmt`; no clippy unless Nick asks.
- State EXECUTED vs RELAYED.

## Records

- Add a "per-document capability gating" section to `ARCHITECTURE.md`.
- Move design Q1–Q4 rulings to `DECISIONS.md`.