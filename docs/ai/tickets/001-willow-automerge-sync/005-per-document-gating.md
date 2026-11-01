# 005 — Per-document gating in the engine

**Status:** REFINED (shape signed off by Nick, 2026-11-01; ready to pick up)
**Depends on:** 001 (validator seam), 002 (cap object), 004 (handshake
establishes `PeerIdentity` + granted scopes)

## Why

The message-level gate. The handshake (004) establishes *who the peer is* and
*which caps they presented*; this ticket enforces, **per `Request`/`Sync`
message**, that the peer holds a valid capability for the specific document
and direction. It is the runtime enforcement that complements the handshake,
and it consults the **live revocation view** (via the `CapabilityStore`, 003)
so a revocation landing mid-session takes effect at the next message without
waiting for a reconnect. This is where T4's core acceptance criterion lands:
"a peer without a valid capability cannot sync a gated document."

## Reference points (inspiration)

- `samod/samod-core/src/actors/document/doc_state.rs:333` —
  `generate_sync_messages` (outgoing path to gate for push authority).
- `samod/samod-core/src/actors/document/doc_state.rs:236` —
  `handle_sync_message` (incoming path to gate for receive authority).
- We rewrite both in `galvanized-sync` with the validator hook; we do **not**
  copy samod's stats-only `SyncDirection`.

## Scope

1. **`CapabilityValidator` impl** (the built-in default behind 001's trait).
   Input: `(peer_identity, document_id, direction)`. Output:
   `Allow | Deny(reason)`. The check:

   - The peer's `PeerIdentity` includes a presented `AutomergeSyncCapability`
     whose `document_id` matches.
   - Signature valid against `signer`'s public key (carried in the cap).
   - `recipient` is the local `SubspaceId` or `Public`.
   - `permission` covers the direction:
     - **Push** (outgoing `Sync` generation / `Request`): requires
       `AutomergeSyncPermission::Write`.
     - **Receive** (incoming `Sync` application): requires `Read` *or*
       `Write`. (In v1, write-gated — see ruling #5 and §2 below.)
   - Not expired.
   - `nonce` not in the live revocation view (`CapabilityStore::is_revoked`).

2. **First-deliverable direction semantics (ruling #5: write-gate first).**
   In v1, **no `Write` cap → no sync at all** (neither push nor receive). The
   galvanized-sync engine is for controlled docs; a peer with no `Write` cap
   has no business receiving either. This makes read-only receive cleanly the
   008 addition (a `Read` cap enables receive-only). The validator's
   receive-branch is present (accepts `Read` or `Write`) but, in v1, a peer
   only reaches it if it also presented a `Write` cap for the doc — so the
   effective v1 rule is "Write required for any sync." Document this clearly
   so 008 is a relaxation, not a re-arch.

3. **Engine consults the validator** on `Request`/`Sync` dispatch. On `Deny`:
   emit a visible `Error { document_id, reason }` for that doc and **skip**
   its sync. The connection stays up — other docs may be authorized.

4. **Outgoing gate.** The actor's generate path suppresses generation for a
   `(connection, document)` pair the validator denies. A peer with no `Write`
   cap for a doc never has sync messages generated for it.

5. **Incoming gate.** The actor's receive path drops + errors visibly on a
   `Sync`/`Request` for a doc the validator denies. The document actor never
   sees unauthorized changes.

6. **Live revocation re-check.** The validator reads
   `CapabilityStore::is_revoked` **at call time**, not a handshake snapshot.
   A tombstone that lands mid-session takes effect on the next message — the
   "re-check on reconnect" floor (ruling #4) plus best-effort mid-session
   enforcement for free. (Active in-flight teardown is 006's fast follow.)

7. **Error visibility.** Every `Deny` surfaces a coarse reason category on the
   wire (per 004 Q4) + a detailed reason locally. No silent drops.

## Out of scope

- Read-only *receive* (a `Read` cap enabling receive-only) — ticket 008.
- In-flight session teardown on revocation — ticket 006 fast follow.
- How the revocation view populates — tickets 003/006.

## Rulings (Nick, 2026-11-01 — RATIFIED)

1. **v1 direction semantics — RATIFIED.** Nick: "Confirmed. No Write cap
   -> no sync at all, to make 008 an easy relaxation." No `Write` cap → no
   sync at all (neither push nor receive). 008 (read-only receive) becomes a
   clean relaxation.

2. **Deny granularity — RATIFIED.** Nick: "Per-doc deny for sure, not worth
   terminating an entire connection for." Per-doc deny; the connection stays up
   for other authorized docs.

3. **Validator placement — RATIFIED.** Nick: "Hub-side validator is fine."
   Hub-side (the `PeerIdentity` + granted scopes live there).

4. **Reason detail on the wire — RATIFIED.** Nick: "Confirm." Coarse category
   on the wire, detailed locally (privacy) — mirroring 004 Q4.

## Open design questions for Nick

- None blocking pickup — all four questions ruled.

## Acceptance criteria

- A peer with a valid `Write` cap for doc D can push and receive D.
- A peer with no cap for D (or a cap for a different doc) has D's sync refused
  with a visible per-doc error; the connection stays up for authorized docs.
- A peer with an expired or revoked cap for D has D's sync refused.
- A revocation tombstone landing mid-session causes the next `Request`/`Sync`
  for D to be denied (best-effort, no reconnect needed).
- No silent drops.

## Verification

- Two-`Endpoint` in-process test: A mints a `Write` cap for B for D1, not for
  D2. B syncs D1; D2 refused visibly; connection stays up. A revokes D1
  (tombstone via 003/006); after the view propagates, B's next D1 sync is
  refused.
- `cargo fmt`; no clippy unless Nick asks.
- State EXECUTED vs RELAYED.

## Records

- Add a "per-document capability gating" section to `ARCHITECTURE.md`.
- Granular rulings (Q1–Q4) kept in-ticket; the arc-level direction is in
  `DECISIONS.md` (2026-11-01).