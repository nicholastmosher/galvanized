# 001 — samod-core identity + authorization seams

**Status:** DRAFT
**Depends on:** —

## Why

samod-core is a sans-IO sync engine with **no notion of identity or
authority**. `PeerId` is an ephemeral string (`peer-{u64}`); the `Hub` routes
`Request`/`Sync` by `DocumentId` with no authorizer to consult;
`generate_sync_messages` generates for every connection; `handle_sync_message`
applies every received change. Every other ticket in this arc needs an
**enforcement surface** in samod-core to plug Willow capability logic into.
This ticket defines that surface as **trait seams** that default to "open"
(JS-interop behavior) and admit a Willow enforcement plug-in.

Per Nick's ruling #1: **seam, not fork.** samod-core stays JS-interop-clean;
the seams are no-ops by default. If a seam cannot express a needed enforcement
without baking in Willow specifics, that is a **finding** to surface — not a
reason to silently weaken enforcement or fork.

## Current state (anchors — re-derive before leaning on)

- `samod/samod-core/src/peer_id.rs` — `PeerId(String)`, `new_with_rng` makes
  `peer-{u64}`. Ephemeral, no keypair.
- `samod/samod-core/src/actors/hub.rs:26` — `Hub { state: State }`;
  `handle_event` → `HubResults`. No authorizer field.
- `samod/samod-core/src/actors/document/doc_state.rs:333` —
  `generate_sync_messages` iterates `peer_connections`, generates for all.
- `samod/samod-core/src/actors/document/doc_state.rs:236` —
  `handle_sync_message` applies received changes unconditionally.
- `samod/samod-core/src/actors/document/sync_message_stat.rs` —
  `SyncDirection` is `Received | Generated`, **stats only**, not a control.
- `samod/samod-core/src/network/wire_protocol.rs:26` — `WireMessage` enum:
  `Join`/`Peer` handshake carries `PeerId` + `PeerMetadata`; `Request`/`Sync`
  carry `DocumentId` + `data`.
- `samod/samod-core/src/lib.rs:194` — public exports.

## Scope

1. **`PeerIdentity` type.** A new type representing a verified peer identity,
   carried by the `Hub` per connection after the handshake. On the open path
   it wraps the existing `PeerId` (JS-interop). On the galvanized path (set up
   by ticket 003) it carries a Willow `SubspaceId` + the granted capability
   set. Define it so samod-core does **not** depend on Willow — the
   `SubspaceId` form is provided by the plug-in via a trait or an opaque
   payload.

2. **`PeerAuthenticator` trait.** A hook the `Hub` calls during the
   handshake. Input: the raw handshake material (the `Join`/`Peer` fields plus
   any extension bytes the transport layer carried). Output: either a
   `PeerIdentity` for the connection or a visible `Deny(reason)`. Default
   impl: accept everything, return a `PeerIdentity` wrapping the wire
   `PeerId` (the open path). The galvanized impl (ticket 003) verifies the
   `SubspaceId`, the challenge-response signature, and the presented
   capabilities.

3. **`CapabilityValidator` trait.** A hook the `Hub` (or `DocumentActor`)
   calls before establishing/continuing a per-document sync. Input:
   `(peer_identity, document_id, direction)`. Output: `Allow` | `Deny(reason)`.
   Default impl: always `Allow` (the open path). The galvanized impl (ticket
   004) checks the presented `AutomergeSyncCapability` and the revocation
   view.

4. **`SyncMode` per connection** (`ReadWrite | ReadOnly`), default
   `ReadWrite`. Plumb it through `generate_sync_messages` (skip generation for
   `ReadOnly` conns) and `handle_sync_message` (drop incoming changes for
   `ReadOnly` conns). The mode is set by the authenticator at handshake time
   based on the cap permission. **The plumbing lands here; the galvanized
   read-only *policy* is ticket 007 (fast follow).** Decide with Nick whether
   to land the plumbing now (enabling 007 later) or defer the plumbing with
   007. Recommendation: land the plumbing now — it's cheap and avoids
   re-touching the actor later.

5. **Hub/DocumentActor wiring.** The `Hub` holds an `Arc<dyn
   PeerAuthenticator>` and `Arc<dyn CapabilityValidator>` (or a single
   `AuthorizationPolicy` trait combining both — see design Q1). These are
   consulted at the handshake and at `Request`/`Sync` time. A `Deny` at
   `Request`/`Sync` emits a visible `WireMessage::Error` for that doc and
   skips the sync — it does **not** tear down the whole connection.

6. **No-silent-drop discipline.** Every `Deny` produces a visible artifact
   (an `Error` on the wire + a surfaced result), per `CONVENTIONS.md`. No
   `let _ =` on a fallible auth check.

7. **Open path unchanged.** With the default (no-op) authenticator/validator,
   samod-core behaves exactly as today: any peer syncs any doc. This is the
   JS-interop guarantee. Add a regression test that asserts this.

## Out of scope

- The Willow-specific authenticator/validator impls (tickets 003, 004).
- The `AutomergeSyncCapability` format (ticket 002).
- The extended handshake wire format (ticket 003).
- Read-only *policy* (ticket 007).

## Design questions for Nick

1. **One trait or two?** A single `AuthorizationPolicy` trait with
   `authenticate_handshake` + `authorize_doc` methods, or separate
   `PeerAuthenticator` + `CapabilityValidator` traits? Recommendation:
   **two traits** — they're consulted at different times (handshake vs
   per-message) and a plug-in may implement them with different state;
   separate traits compose better and test independently. Confirm.

2. **Where does the `CapabilityValidator` live, Hub or DocumentActor?** The
   `Hub` routes `Request`/`Sync` and knows the connection; the `DocumentActor`
   knows the doc. Recommendation: **Hub**, because the per-connection
   `PeerIdentity` + granted-scopes lives there, and the Hub already dispatches
   by `DocumentId`. The actor stays doc-focused. Confirm.

3. **`PeerIdentity` shape.** Opaque `Box<dyn Any + Send + Sync>` payload (so
   samod-core stays Willow-free), or a concrete enum
   `PeerIdentity::Open(PeerId) | Willow(WillowPeerId)` that pulls a tiny
   willow-free `SubspaceId`-like type into samod-core? Recommendation:
   **opaque payload** — keeps samod-core substrate-pure; the galvanized
   plug-in downcasts. The cost is a downcast at the validator; the benefit is
   zero Willow coupling. Confirm.

4. **`SyncMode` plumbing now or with 007?** See scope §4. Recommendation: now.

## Acceptance criteria

- `PeerAuthenticator` and `CapabilityValidator` traits exist with default
  no-op impls.
- The `Hub` consults them at handshake and `Request`/`Sync` time; a `Deny`
  emits a visible `Error` and skips the sync without dropping the connection.
- `SyncMode` is plumbed through `generate_sync_messages` and
  `handle_sync_message`.
- With default impls, samod-core behaves exactly as today (regression test).
- samod-core does **not** gain a Willow dependency.

## Verification

- `cargo test` in `samod/samod-core`: a regression test asserting the open
  path syncs as before with default seams.
- A unit test with a stub `Deny`-everything validator asserting that `Request`
  for a doc produces an `Error` and no sync, while the connection stays up.
- State EXECUTED vs RELAYED for anything run in this repo's sandbox.

## Records

- Update `ARCHITECTURE.md` with the seam shapes once ruled on.
- Move the trait-shape ruling (design Q1–Q4) to `DECISIONS.md`.