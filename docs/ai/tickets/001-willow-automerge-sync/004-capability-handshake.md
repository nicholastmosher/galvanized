# 004 — Capability handshake + new protocol/ALPN

**Status:** DRAFT
**Depends on:** 001 (engine + authenticator seam), 002 (cap object), 003
(store sources caps)

## Why

The connection-level gate. On the new galvanized ALPN, before normal sync,
the handshake establishes **cryptographic peer identity** (Willow
`SubspaceId`), **proves key possession** (challenge-response — the
leaked-token mitigation), and **exchanges capabilities** so each side knows
which of the other's docs it may sync. This is where rulings #2, #3, and #8
become concrete. The wire protocol is **entirely new** — inspired by samod's
codec, not compatible with `automerge-repo`.

## Reference points (inspiration, not substrate)

- `samod/samod-core/src/network/wire_protocol.rs:26` — `Join`/`Peer`/
  `Request`/`Sync` CBOR framing. We adopt the codec idea, not the messages.
- `galvanized/crates/plugin-automerge/src/protocol.rs:38` — current
  `AutomergeProtocol::ALPN = b"iroh/automerge-repo/1"`; `accept` hands the
  stream straight to samod. **This gets replaced** (ticket 007), not extended.
- `galvanized/crates/plugin-contacts/src/protocol.rs:445` — `SignedEnvelope`
  signed by subspace key. The model for the challenge-response signature.

## Scope

1. **New ALPN.** Tentatively `iroh/galvanized-sync/1`. Registered by the new
   engine's protocol handler (007 wires it). No vanilla ALPN; no
   `automerge-repo` compat (ruling #8).

2. **New wire protocol.** A CBOR-framed message set, distinct from samod's.
   PROPOSED initial shape (to rule on, design Q1):

   - Handshake phase: `Hello { subspace_id, nonce }`, `Proof { signature }`,
     `Caps { capabilities: Vec<AutomergeSyncCapability> }`, `Reject { reason }`.
   - Sync phase: `Request { document_id, data }`, `Sync { document_id, data }`,
     `DocUnavailable { document_id }`, `Error { document_id, reason }`.

   The handshake messages ride the bidi stream *before* the sync phase. The
   engine's actor core consumes sync-phase messages; the handshake is run by
   the `PeerAuthenticator` (001) before handing the stream to the actor.

3. **Handshake sequence:**

   1. Exchange `Hello { subspace_id, nonce }`.
   2. Each side signs the other's nonce with its `SubspaceSecret` (via
      `WillowKeyStore::subspace_secret_for_signing`, gated by a
      `StrictSendCap<WillowWrite, WillowScope>`). Send `Proof { signature }`.
   3. Verify the signature against the peer's claimed `subspace_id`.
      **Failure → `Reject { reason }`, close, visible error.** This is ruling
      #3: a stolen cap without the key is useless.
   4. Each side sends `Caps { ... }` — the caps it holds that are relevant to
      the peer, sourced via `CapabilityStore::caps_to_present(peer)` (003).
      Each side verifies every cap: signature valid against `signer`'s public
      key, `recipient` is the local `SubspaceId` or `Public`, `permission`
      covers intended direction, not expired, `nonce` not in the live
      revocation view.
   5. On success, construct a `PeerIdentity` (verified `SubspaceId` + granted
      cap set + `SyncMode` derived from cap permissions) and hand the stream
      to the actor core for the sync phase.

4. **Challenge-response semantics by cap mode (ruling #3):**
   - **Recipient-specific cap:** the recipient must prove *the named*
     `SubspaceId` (the one in `recipient`). A different identity → reject.
   - **Public cap (`recipient = Public`):** the peer proves *some*
     `SubspaceId` (sybil-resistant identity for accountability); the cap
     doesn't restrict *which*. A stolen public cap + no key → reject.

5. **`SyncMode` from caps.** `Write` cap → `ReadWrite`; `Read`-only →
   `ReadOnly` (active in 008; in v1 receive is write-gated so this is moot,
   but the mode is set correctly for 008).

6. **No fallback.** A failed handshake **never** degrades to ungated sync.
   It refuses (ruling: privacy wins). Visible `Reject` on the wire + surfaced
   error.

7. **Error visibility.** Every refusal (bad signature, wrong recipient,
   expired, revoked, no cap for a doc) produces a visible reason on the wire
   (coarse category, per 005 Q4) + a detailed local surfaced error. No silent
   closes.

## Out of scope

- Per-document runtime gating (005) — the handshake establishes identity +
  granted scopes; 005 enforces per message.
- Revocation view sync (003/006) — the handshake *consults* the local view;
  populating it is 003/006.
- Read-only *policy activation* (008).

## Design questions for Nick

1. **Wire encoding shape.** The PROPOSED message set above, or fold cap
   exchange into extended `Hello`/`Proof` maps? Recommendation: **distinct
   `Caps` frame** — cleaner, lets a peer stream many caps without bloating
   `Hello`, and keeps each message's purpose single. Confirm.

2. **Cap presentation granularity.** Present all relevant caps upfront
   (could be large) vs announce `document_id`+`nonce` and present full signed
   objects lazily on first `Request`? Recommendation: **upfront** — the
   handshake is the trust-establishment moment; the per-doc gate (005) then
   decides from in-memory state without round-trips. Re-evaluate if cap sets
   get large. Confirm.

3. **Re-handshake on reconnect.** Confirm: a new ALPN stream = full
   handshake + revocation-view re-sync (006). No partial resumption in v1.

4. **Privacy: reason detail on the wire.** How much refusal detail is safe to
   send to an untrusted peer (does "revoked" vs "expired" leak state)?
   Recommendation: **coarse category on the wire** (`Unauthorized`,
   `Expired`, `Revoked`), **detailed reason local only**. Confirm —
   privacy-shaped.

## Acceptance criteria

- A new-ALPN connection completes the handshake: both peers verify the other's
  `SubspaceId` signature and presented caps, then proceed to sync.
- A peer presenting a recipient-specific cap without proving the named
  `SubspaceId` is rejected.
- A peer presenting a public cap with no key proof is rejected.
- A peer presenting a cap whose `recipient` is not the local `SubspaceId`
  (and not `Public`) is rejected.
- An expired or revoked cap is rejected for its docs.
- No fallback to ungated sync on any failure.
- The wire protocol is distinct from `automerge-repo` (no shared message
  types).

## Verification

- Two-`Endpoint` in-process test: valid cap → handshake succeeds → sync
  proceeds. Stolen recipient-specific cap (wrong key) → rejected. Public cap
  + no key → rejected. Wrong recipient → rejected. Expired/revoked → rejected.
- `cargo fmt`; no clippy unless Nick asks.
- State EXECUTED vs RELAYED.

## Records

- Add a "galvanized-sync handshake" section to `ARCHITECTURE.md`.
- Move design Q1–Q4 rulings to `DECISIONS.md`.