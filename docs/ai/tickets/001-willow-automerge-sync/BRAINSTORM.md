# Brainstorm — Willow-gated automerge sync

> **Status: exploration + RATIFIED rulings.** The design-space exploration is
> here; the settled target-state is in [`SPEC.md`](SPEC.md). The rulings are
> Nick's (this session) and are marked RATIFIED. Sub-decisions still open are
> marked PROPOSED and live as design questions in the tickets.

## What we're trying to do

A next-generation samod that **only syncs automerge documents between peers
who hold and prove valid signed Willow capabilities**. Today samod syncs with
any peer that dials it; there is no notion of identity or authority anywhere
in the engine. The requirement (from L0, from T4, from the user stories U1–U7)
is that a shared object must only sync with a peer who can present a valid
capability for that object, enforced by the sync engine itself — not by an
outer wrapper that can be bypassed.

## What samod is today (grounded in the code)

- `samod-core` is a **sans-IO actor model**: a `Hub` actor routes by
  `DocumentId`, and one `DocumentActor` per document. `Hub::handle_event`
  takes events and returns `HubResults` (effects to execute). Pure state
  machine, runtime-agnostic, FFI-friendly.
- **Wire protocol** (`network/wire_protocol.rs`): CBOR-encoded `Join`/`Peer`
  handshake exchanging `PeerId` + `PeerMetadata` (storage id, is_ephemeral),
  then `Request`/`Sync`/`DocUnavailable` per document, plus `Ephemeral`,
  `RemoteSubscriptionChange`, `RemoteHeadsChanged`.
- **`PeerId`** (`peer_id.rs`) is `peer-{u64}` — an **ephemeral string**. No
  cryptographic identity, no binding to a keypair, no proof of possession.
  Generated fresh per process.
- **`DocumentId`** (`document_id.rs`) is a random UUID (bs58-encoded on the
  wire). No binding to any Willow path or namespace.
- **`SyncDirection`** (`actors/document/sync_message_stat.rs`) is **stats
  only** (`Received` vs `Generated`). It is *not* a sync-control mechanism.
  `generate_sync_messages` (`doc_state.rs:333`) iterates `peer_connections`
  and generates for all of them; `handle_sync_message` applies all received
  changes. **There is no hook to make a session directional.**
- **No authorization seams.** The `Hub` has no trait hook to consult an
  authorizer before establishing or continuing a doc sync. The `DocumentActor`
  has no notion of per-connection permissions.
- **Storage** is abstracted as a key-value store with range queries
  (`StorageKey` = list of strings). **Transport** is abstracted as a
  `Transport` (stream + sink) over a `Dialer` trait. Both are clean seams —
  the gap is purely on the *authorization* axis.

**Conclusion:** samod is well-shaped for extension (sans-IO, trait-based
transport/storage) but has *zero* enforcement surface for identity or
capabilities. A next-gen samod must *add* that surface.

## What galvanized already has (grounded in the code)

- **`galvanized-cap`** (`crates/galvanized-cap/src/lib.rs`): `WillowRead` /
  `WillowWrite` `Permission` types, `WillowScope` (namespace + subspace +
  path prefix, structured matching against Willow URIs), `WillowKeyStore`
  that mints Willow `WriteCapability` gated by `StrictSendCap<WillowWrite,
  WillowScope>`. The **signed-data + type-token duality** from L0 is already
  partially built: the serialized `WriteCapability` is the sync-checked form,
  `StrictSendCap` is the type-token gated by `capsec::root()`.
- **`plugin-willow`** (`crates/plugin-willow/src/lib.rs`): `WillowCx` over a
  global `WillowState` (`MemoryStore` today), `create_entry` / `list_entries`
  / `read_entry`. `Willowize` / `EntryHandle` are a commented-out skeleton
  (T2's job).
- **Current samod glue** (`plugin-automerge/src/protocol.rs`:
  `AutomergeProtocol`; `plugin-p2p/src/iroh_repo.rs`: `IrohSamod`): dial an
  Iroh endpoint, open a bidi stream, hand it to samod's `Transport`, sync with
  **no auth check**. The contacts handshake authenticates the peer's Profile
  via `SignedEnvelope` but does **not** gate the automerge sync on a
  capability.

**Conclusion:** the capability *machinery* exists in `galvanized-cap`; what's
missing is the *bridge* from that machinery into samod's sync decisions.

## The goals (from L0 + T4 + user stories)

1. **Cryptographic peer identity.** Replace ephemeral `PeerId` with Willow
   `SubspaceId`. A peer *is* their Profile.
2. **Per-document capability gating at sync time.** Verify the peer holds a
   valid signed cap for the doc before/while syncing.
3. **Sync-authority ≠ read-authority.** The cap gates *propagation*; decrypt
   is a separate key. Samod handles only sync-authority and must not collapse
   the two (it syncs ciphertext to a sync-authorized peer who may lack the
   read key).
4. **Revocation that holds across offline peers.** Revocation is data in an
   eventually-consistent store; re-check on reconnect.
5. **Read-only vs read-write** (deferred — write-gate first).
6. **"Leaked token isn't enough."** Prove key possession per session.
7. **Interop posture.** JS `automerge-repo` interop is a bonus, not a
   constraint (Nick's ruling).

## The design space

### Option A — Gate as a wrapper above samod (don't touch samod)

A capability guard in `plugin-automerge` wraps the `Transport` before handing
it to samod; runs a cap handshake on the bidi stream first.

- **Strongest counter first:** per-document gating is **hard from outside the
  wire protocol**. The guard would have to parse/intercept samod's
  `Request`/`Sync` frames to check caps per doc — fragile, version-coupled,
  and read-only enforcement is impossible without dropping frames mid-stream.
  You get connection-level gating but not clean per-doc gating.
- **Pro:** samod untouched.
- **Verdict:** rejected for the core requirement. Per-doc gating *is* the
  requirement, and A can't deliver it cleanly.

### Option B — Fork samod-core into a galvanized sync engine

Branch samod-core; replace `PeerId` with `SubspaceId`; extend the handshake;
add per-doc cap checks in `Hub`/`DocumentActor`; add read-only enforcement.

- **Strongest counter first:** breaks JS interop permanently; galvanized owns
  a divergent sync engine; upstream samod improvements stop flowing in. Nick
  values JS interop as a *bonus*, and a fork throws that bonus away for a
  simplification that the seam approach can achieve with discipline.
- **Pro:** cleanest integration; the gate lives at the right layer.
- **Verdict:** rejected by Nick's ruling #1 (seam, not fork) — *unless* the
  seam approach turns out to be unable to express the enforcement. That's a
  finding to surface, not a reason to preemptively fork.

### Option C — Extend samod-core with trait seams (RATIFIED direction)

Keep samod-core pure and JS-interop-clean. Add no-op-by-default extension
points; a sibling crate plugs in the real Willow logic.

- **Strongest counter first:** designing the seams *well* is the hard part.
  They must be general enough not to bake in Willow specifics (samod-core
  stays substrate), but tight enough to actually enforce (no bypass). A loose
  seam is a false sense of security; a tight seam risks coupling samod-core
  to Willow concepts. The handshake extension is the trickiest: it changes
  the wire format unless carried by a separate ALPN — which Nick's ruling #8
  resolves (separate ALPN).
- **Pro:** samod stays interop-compatible (seams default to open); the gate is
  at the right layer (`Hub` already routes by `DocumentId`); read-only is a
  first-class mode; the sans-IO design makes the validator a pure function
  (testable, no I/O in the auth path).
- **Verdict:** **RATIFIED.** This is the direction.

The seams (to be finalized in ticket 001):

- `PeerAuthenticator` — hook into the handshake: verify identity + capability
  presentation, yield a `PeerIdentity` (carrying `SubspaceId` + granted
  scopes) or a visible refusal.
- `CapabilityValidator` — given `(peer_identity, document_id, direction)`,
  return `Allow | Deny(reason)`. Consulted at `Request`/`Sync` time in the
  `Hub`.
- `SyncMode` per connection (`ReadWrite | ReadOnly`) that
  `generate_sync_messages` / `handle_sync_message` consult (ticket 007, fast
  follow).

## The capability object (signed-data form)

Grounded in `galvanized-cap`'s existing `WillowScope` + `WriteCapability`,
extended for per-doc sync:

```
AutomergeSyncCapability {
  owner: SubspaceId,            // doc owner's Profile (minter)
  recipient: SubspaceId,        // peer's Profile (who it's granted to)
  document_id: DocumentId,      // the automerge doc being authorized
  willow_scope: WillowScope,    // (namespace, subspace, path_prefix) — the binding
  permission: WillowRead | WillowWrite,
  expiry: UnixTimestamp,
  nonce: [u8; 32],              // uniqueness for revocation tracking
}
signed by owner's SubspaceSecret
```

Per ruling #6: `document_id` and `willow_scope` are **independent**, bound
together by the signed claims. The engine trusts the binding because the owner
signed it; no separate doc→path registry needed. Per ruling #7: the `nonce`
identifies the cap for revocation tombstones under the doc's own namespace.

## The handshake + per-doc flow

1. **Connect** (Iroh QUIC link established, galvanized ALPN negotiated).
2. **Identity + capability handshake** (extended `Join`/`Peer` on the
   galvanized ALPN):
   - Exchange `SubspaceId`s + challenge nonces.
   - Each side signs the other's challenge with its `SubspaceSecret` → proves
     key possession (ruling #3: leaked-token mitigation).
   - Each side presents the capability object(s) it holds for the other's
     docs; the other verifies (signature, recipient match, expiry, revocation
     view).
3. **samod normal sync** proceeds, but the `Hub`'s `CapabilityValidator` is
   consulted on every `Request`/`Sync` for a doc — refuse with a visible
   `Error` if the peer lacks a valid cap.
4. **On reconnect:** re-run the handshake + re-sync the revocation view
   (ruling #4).

The handshake establishes *peer identity + which docs each side may sync*; the
per-doc gate (ticket 004) is the runtime enforcement that the peer actually
has a cap for each doc it touches.

## Revocation (ruling #4 + #7)

- The owner writes tombstones to a path in the doc's namespace:
  `/{owner_subspace}/revoked/{cap_nonce}`.
- Syncing the namespace propagates revocations to all peers for free — no
  separate channel.
- On reconnect, the engine syncs revocation entries and checks the peer's cap
  nonces against them (ticket 005).
- **In-flight revocation** (tear down a live session mid-sync) is harder:
  needs an active notification over the control stream or periodic re-checking.
  Explicitly a **fast follow**, not in the first deliverable (ruling #4).

## Read-only (ruling #5)

Deferred. The first deliverable ships **write-gated**: you either have a
`WillowWrite` cap or you can't sync. Read-only (`WillowRead` = receive but
don't push) requires the new `SyncMode::ReadOnly` mechanism in the
`DocumentActor` (skip `generate_sync_messages` for read-only conns, or drop
incoming changes from them). Samod has no directional control today
(`SyncDirection` is stats only — verified in `sync_message_stat.rs`), so this
is new actor work. Tracked as ticket 007, explicitly a fast follow.

## What "Willow rules" means here

Willow's Meadowcap determines, via capabilities, which peers may sync which
data. The capability is a signed object; the sync layer checks it. Willow
**does not encrypt** — read-authority is a separate key layer above Willow
(L0 §5). So "sync according to Willow rules" means: **the sync engine treats
the signed capability as the ground truth for propagation authority**, and
**read-authority (decryption) is independent and not samod's concern**. Samod
syncs ciphertext to a sync-authorized peer who may or may not be able to
decrypt it; that's correct and intended.

## Honest gaps / things this brainstorm does not resolve

- **The exact wire encoding of the extended handshake.** CBOR map extending
  `Join`/`Peer`, or a distinct capability-exchange frame post-handshake?
  PROPOSED in ticket 003; Nick to rule.
- **The tombstone entry format.** Bare nonce vs. a signed revocation object?
  PROPOSED in ticket 005.
- **Multi-owner docs.** A doc whose namespace is communal (any subspace holder
  writes) — who mints caps? Likely each writer mints for their own subspace;
  the cap's `owner` is the writer's `SubspaceId`. PROPOSED; surface in 002.
- **Capability chaining / delegation.** Can a recipient further delegate
  (attenuate)? Meadowcap supports it; whether samod-willow exposes it in the
  first deliverable is open. PROPOSED: ship mint-by-owner only; delegation as
  a follow-up.
- **Ephemeral messages and subscription changes.** The wire protocol has
  `Ephemeral` and `RemoteSubscriptionChange` — do caps gate these too, or only
  `Request`/`Sync`? PROPOSED: caps gate `Request`/`Sync` (the document path);
  ephemeral messaging is a separate concern (T3's envelope already signs by
  subspace).
- **The `PeerIdentity` carrying granted scopes vs. per-message re-validation.**
  Does the handshake establish a session-long grant set, or is every message
  re-checked against the validator (which may consult a fresh revocation view)?
  PROPOSED: session-long identity + per-message cap check against the live
  revocation view, so revocation takes effect at the next message even
  mid-session once the view updates.

These are surfaced as design questions in the relevant tickets, not buried.