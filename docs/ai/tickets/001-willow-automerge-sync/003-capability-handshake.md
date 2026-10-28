# 003 — Capability handshake + galvanized ALPN

**Status:** DRAFT
**Depends on:** 001 (seams), 002 (cap object)

## Why

The connection-level gate. Before samod's normal `Join`/`Peer` sync proceeds,
the galvanized ALPN handshake establishes **cryptographic peer identity**
(Willow `SubspaceId`), **proves key possession** (challenge-response, the
leaked-token mitigation), and **exchanges capabilities** so each side knows
which of the other's docs it is authorized to sync. This is where ruling #2
(caps presented in the handshake), #3 (challenge-response), and #8 (separate
ALPN) become concrete.

## Current state (anchors — re-derive before leaning on)

- `galvanized/crates/plugin-automerge/src/protocol.rs:38` —
  `AutomergeProtocol::ALPN = b"iroh/automerge-repo/1"`; `accept` opens a bidi
  stream and hands it straight to `repo.make_acceptor` → `Transport`. No
  handshake of any kind before samod takes over.
- `galvanized/crates/plugin-p2p/src/iroh_repo.rs:106` — `IrohSamod::accept`,
  same pattern (older copy).
- `samod/samod-core/src/network/wire_protocol.rs:26` — `Join`/`Peer` carry
  `PeerId` + `PeerMetadata` only.
- `galvanized/crates/plugin-contacts/src/protocol.rs:445` — `SignedEnvelope`
  with `sender_id`/`recipient_id`/`timestamp`, signed by the local subspace
  key, verified on receive. The reference for "sign as Profile."

## Scope

1. **Galvanized ALPN.** A new ALPN, tentatively `iroh/automerge-repo-galvanized/1`,
   registered by `AutomergeProtocol` (or a new `GalvanizedAutomergeProtocol`).
   The vanilla ALPN stays for the open path. Decide with Nick whether one
   protocol handler serves both ALPNs (branching internally) or two handlers
   exist. Recommendation: **two handlers** — cleaner, the open path stays
   untouched, and the galvanized handler owns the extended handshake. Confirm.

2. **Extended handshake.** On the galvanized ALPN, before handing the stream
   to samod's `Transport`, run:

   1. Exchange `SubspaceId`s + fresh challenge nonces (32-byte random).
   2. Each side signs the other's nonce with its `SubspaceSecret` (via
      `WillowKeyStore::subspace_secret_for_signing`, gated by a
      `StrictSendCap<WillowWrite, WillowScope>` — reusing the existing "sign
      as Profile" path). Send the signature.
   3. Each side verifies the signature against the peer's claimed
      `SubspaceId`. **Failure → refuse with a visible error, close the
      stream.** This is ruling #3 (leaked-token mitigation): a cap names the
      recipient `SubspaceId`, and the peer must prove they hold that key.
   4. Each side presents the `AutomergeSyncCapability` object(s) it holds for
      docs the other side owns (ticket 002). Each side verifies: signature
      valid, `recipient` matches local `SubspaceId`, not expired, nonce not in
      the local revocation view.
   5. On success, construct a `PeerIdentity` (carrying the verified
      `SubspaceId` + the granted cap set) and hand the stream to samod's
      `Transport` with the `PeerAuthenticator` (ticket 001) pre-populated.

3. **Handshake wire encoding.** PROPOSED: a small CBOR-framed
   `GalvanizedHandshake` message set (`Hello { subspace_id, nonce }`,
   `Proof { signature }`, `Caps { capabilities: Vec<AutomergeSyncCapability> }`,
   `Reject { reason }`) exchanged on the bidi stream before samod's
   `Join`/`Peer`. This keeps the handshake **outside** samod's wire protocol
   (samod-core stays untouched; ruling #1). Confirm the encoding shape with
   Nick (design Q1).

4. **`PeerAuthenticator` plug-in (galvanized impl).** The galvanized
   authenticator (in `samod-willow` or `plugin-automerge`) implements ticket
   001's `PeerAuthenticator` trait: it runs the handshake above and yields a
   `PeerIdentity` with the `SubspaceId` + granted scopes, or a visible
   `Deny(reason)`.

5. **Error visibility.** Every refusal (bad signature, wrong recipient,
   expired, revoked) produces a visible `Reject { reason }` on the wire and a
   surfaced error to the caller — never a silent close.

6. **No silent fallback to open.** If the galvanized handshake fails, the
   connection does **not** fall back to ungated sync. It refuses. (This is the
   "Willow enforcement wins" half of ruling #1.)

## Out of scope

- The per-document runtime gating (ticket 004) — the handshake establishes
  identity + granted scopes; 004 enforces per message.
- Revocation view sync (ticket 005) — the handshake *consults* the local
  revocation view; how that view populates is 005.
- Read-only policy (ticket 007) — the handshake may set `SyncMode` from cap
  permission, but the enforcement plumbing is 001 and the policy is 007.

## Design questions for Nick

1. **Handshake encoding shape.** The CBOR `GalvanizedHandshake` message set
   above, or extend samod's `Join`/`Peer` with capability fields (which would
   touch samod-core, against ruling #1's seam preference)? Recommendation:
   **the out-of-band CBOR set** — keeps samod-core pure; the handshake is a
   galvanized-layer concern. Confirm.

2. **One handler or two for the ALPNs?** See scope §1. Recommendation: two.

3. **Cap presentation granularity.** Does each side present *all* caps it
   holds for the other's docs (could be large), or only announce `document_id`
   + nonce and present the full signed object lazily on first `Request`?
   Recommendation: **present full caps upfront** — the handshake is the
   natural trust-establishment moment, and the per-doc gate (004) can then
   decide from in-memory state without another round-trip. Re-evaluate if cap
   sets get large. Confirm.

4. **Re-handshake on reconnection.** Ruling #4 says re-check on reconnect.
   Does a reconnect re-run the full handshake (yes, naturally — it's a new
   stream)? Confirm that "reconnect" = new ALPN stream = full handshake +
   revocation-view re-sync (005).

## Acceptance criteria

- A galvanized-ALPN connection completes the handshake: both peers verify the
  other's `SubspaceId` signature and presented caps, then proceed to sync.
- A peer presenting a cap without proving key possession (stolen token) is
  rejected with a visible error.
- A peer presenting a cap whose `recipient` is not the local `SubspaceId` is
  rejected.
- A peer presenting an expired or revoked cap is rejected for that cap's docs.
- The vanilla ALPN open path still works unchanged.
- No silent fallback to ungated sync on galvanized-ALPN failure.

## Verification

- Two-`Endpoint` in-process test: A and B exchange a valid cap → handshake
  succeeds → sync proceeds. A presents a stolen cap (no key) → rejected. A
  presents a cap for the wrong recipient → rejected.
- `cargo fmt`; no clippy unless Nick asks.
- State EXECUTED vs RELAYED.

## Records

- Add a "galvanized sync handshake" section to `ARCHITECTURE.md`.
- Move design Q1–Q4 rulings to `DECISIONS.md`.