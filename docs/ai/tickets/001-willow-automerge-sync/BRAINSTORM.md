# Brainstorm — Willow-gated automerge sync

> **Status: exploration + RATIFIED rulings.** Design-space exploration is
> here; the settled target-state is in [`SPEC.md`](SPEC.md). Rulings are
> Nick's and marked RATIFIED. Sub-decisions still open are PROPOSED and live
> as design questions in the tickets.

## What we're building

**`galvanized-sync`**: a new, from-scratch automerge sync engine in this repo
that only syncs documents between peers who hold and prove valid signed
Willow capabilities. Privacy is the primary case; public sharing is a granted
exception (a kind of capability), never the default. No `automerge-repo` / JS
interop is preserved. `samod`/`samod-core` are a reference we're inspired by
(sans-IO actor shape, CBOR codec), not a dependency.

## What samod is, and what we take from it (reference, not substrate)

- **Sans-IO actor model** — a `Hub` routes by `DocumentId`; one
  `DocumentActor` per document; `handle_event` → effects. Runtime-agnostic,
  testable, FFI-friendly. **We take the shape; we rewrite the code.**
- **CBOR wire codec** with a `Join`/`Peer` handshake then `Request`/`Sync`
  per document. **We take the codec idea; the protocol is new** (carries
  Willow identity + capabilities natively).
- **What we explicitly drop:**
  - `PeerId` as an ephemeral string (`peer-{u64}`) — replaced by Willow
    `SubspaceId`.
  - `SyncDirection` as stats-only — we build a real `SyncMode` control.
  - The "any peer syncs any doc" default — replaced by deny-by-default.
  - JS `automerge-repo` wire compatibility — not a goal.
- **Storage/Transport abstraction** — samod's KV-store + stream/sink traits
  are a clean separation worth keeping in spirit.

**Conclusion:** samod is a good architectural reference; we are not bound by
its data types, wire format, or defaults. Everything about identity and
authority is ours to define.

## What galvanized already has (grounded in the code)

- **`galvanized-cap`** (`crates/galvanized-cap/src/lib.rs`): `WillowRead` /
  `WillowWrite` permissions, `WillowScope` (namespace + subspace + path
  prefix, structured matching), `WillowKeyStore` holding both namespace and
  subspace secrets, minting Willow `WriteCapability` gated by
  `StrictSendCap<WillowWrite, WillowScope>`, and `subspace_secret_for_signing`
  ("sign as Profile"). The signed-data + type-token duality is partly built.
  We extend this with `AutomergeSyncCapability` (ticket 002).
- **`plugin-willow`**: `WillowCx` over a global `WillowState` (`MemoryStore`
  today). No syncing Willow store exists yet — which is why `galvanized-sync`
  must be **self-sufficient** for capability distribution (ticket 003), not
  dependent on a syncing Willow store.
- **Current samod glue** (`plugin-automerge/src/protocol.rs`,
  `plugin-p2p/src/iroh_repo.rs`): dial Iroh, hand the bidi stream to samod,
  sync with zero auth. **This gets replaced**, not extended (ticket 007).

## The goals (from L0 + T4 + user stories, privacy-primary)

1. **Cryptographic peer identity.** Peers are Willow `SubspaceId`s; the
   handshake proves key possession.
2. **Per-document capability gating, deny-by-default.** No valid signed cap
   → no sync. No ungated path exists.
3. **Sync-authority ≠ read-authority.** The cap gates *propagation*; decrypt
   is a separate key (L0 §5). The engine syncs ciphertext to a sync-authorized
   peer who may lack the read key — correct and intended.
4. **Revocation that holds across offline peers.** Revocation is data; re-check
   on reconnect; best-effort mid-session via the live store view.
5. **Read-only vs read-write** (deferred — write-gate first).
6. **"Leaked token isn't enough."** Challenge-response per session.
7. **Two public modes.** Anonymous-public (hard requirement) and
   attributable-public, both as granted capabilities.
8. **Self-sufficient capability distribution now;** swappable to a syncing
   Willow store later.

## The design space (and why we landed where we did)

### Fork samod-core vs. extend-with-seams vs. new crate

- **Fork:** breaks any future samod upstream; we own a divergent engine.
- **Extend samod-core with seams (the prior direction):** preserved JS interop
  as a bonus, but forced a no-op "open default" that contradicts the
  privacy-primary stance, and constrained the wire format to stay
  `automerge-repo`-shaped.
- **New crate (RATIFIED):** samod as reference only; we bake in Willow
  identity + deny-by-default + the new wire protocol from line one. Seams
  still exist, but for *our* evolution (refining rules, swapping the
  capability store), not to keep samod wrapped. Privacy wins cleanly.

### Why deny-by-default, no ungated path

A privacy-first system cannot default to "sync with anyone who dials."
Nick's ruling: every sync requires positive proof of a valid signed cap; even
"public" docs carry a signed owner capability approving public sharing. The
"open path" concept is gone — what remains is a *public-sharing cap* (wildcard
recipient) vs a *recipient-specific cap*. Both are signed; both are verified.

### The hardest sub-problem: who signs an anonymous-public cap?

A public cap must be signed by an authority the validator can check, but for
anonymous-public that authority must **not** be the owner's Profile
`SubspaceSecret` (that names the owner). Resolution (RATIFIED, ruling #11):
reuse Willow's key duality —

- **Recipient-specific & attributable-public:** signed by the owner's
  `SubspaceSecret` → names the Profile. Validator checks against `SubspaceId`.
- **Anonymous-public:** signed by the **namespace secret** (owned namespace)
  → the `NamespaceId` is the "owner" identity, decoupled from any Profile.
  Validator checks against `NamespaceId`. The owner-Profile is not revealed.
- Communal namespaces are attributable by nature (any subspace holder signs →
  reveals their subspace), so anonymous-public is an owned-namespace facility.

This is L0 "key synergy" made concrete: identity, capability, and namespace
keys are one family. All three cap modes fall out of the same
`galvanized-cap` machinery (`WillowKeyStore` already holds both secrets).

## The capability object

```
AutomergeSyncCapability {
  signer: Signer,                // Subspace(SubspaceId) | Namespace(NamespaceId)
  recipient: Recipient,          // Subspace(SubspaceId) | Public
  document_id: DocumentId,       // the automerge doc authorized
  willow_scope: WillowScope,     // (namespace, subspace, path_prefix) binding
  permission: AutomergeSyncPermission,  // Read | Write
  expiry: UnixTimestamp,
  nonce: [u8; 32],               // revocation tracking id
}
signed by the signer's secret (subspace secret or namespace secret)
```

- **Binding (ruling #6):** `document_id` and `willow_scope` are independent;
  the signed claims bind them. No external registry.
- **Public caps (ruling #9):** `permission = Read`, tight TTL, revokable.
  Public *write* is not minted (anyone-push is almost never wanted).
- **Honest limit:** revoking a public cap stops *future* sync; peers who
  already synced keep the bytes. Public sharing is an irrevocable-to-that-point
  visibility grant. Stated explicitly, not implied away.

## The handshake + per-doc flow

1. **Connect** (Iroh QUIC, new galvanized ALPN).
2. **Identity + capability handshake** (new protocol):
   - Exchange `SubspaceId`s + fresh challenge nonces.
   - Each side signs the other's nonce with its `SubspaceSecret` → proves key
     possession. For a recipient-specific cap, the recipient must prove *the
     named* `SubspaceId`; for a public cap, *some* `SubspaceId`
     (sybil-resistant identity; the cap doesn't restrict *which*).
   - Each side presents the `AutomergeSyncCapability`(s) it holds (sourced via
     the `CapabilityStore`, ticket 003). Each side verifies: signature valid
     against the signer's public key, recipient matches (or is `Public`),
     permission covers the direction, not expired, nonce not revoked.
3. **Sync proceeds**, but the engine re-validates per `Request`/`Sync` against
   the live revocation view (ticket 005) — so a revocation landing mid-session
   takes effect at the next message.
4. **On reconnect:** re-run the handshake + re-sync the revocation view.

The handshake establishes *who you are + what you may sync*; the per-doc gate
(ticket 005) is the runtime enforcement, re-checked per message.

## Capability distribution — the `CapabilityStore` seam (ticket 003)

No syncing Willow store exists yet, so the engine must be self-sufficient.
The `CapabilityStore` trait abstracts *where caps live and how they propagate*,
with an initial self-sufficient impl and a future Willow-namespace-path impl.

**The per-peer automerge "index doc" idea — critique and scoped adoption.**

The proposal: each peer has a public-read-only automerge doc indexing the
caps it has issued. Critique:

- **Strongest counter (privacy):** a public index of *all* a peer's caps
  publishes their entire sharing graph — every doc, every recipient — to
  anyone who reads it. "Who you know" is maximally sensitive. So
  **recipient-specific caps must never go in a public index.** They stay
  out-of-band (contact ticket / direct message / the handshake itself).
- **Public caps in a per-peer index is fine *for attributable-public only*:**
  a public cap already declares the doc public and is already signed by an
  authority, so listing it leaks no more than the cap does itself.
- **Anonymous-public caps cannot live in an identity-bound index** — the
  index associates the (anonymous) doc with the (identified) indexing peer,
  de-anonymizing it. Anonymous-public needs an *identity-less* store (future
  Willow namespace path, or travel-with-the-doc). Behind the same seam.
- **CRDT invariant:** caps in an automerge doc are **immutable append-only
  entries + tombstones**, never editable fields (editing a signed cap
  invalidates its signature; an editable field is a forge path).
- **Bootstrap doesn't vanish, it shrinks:** a peer still needs the index doc's
  `DocumentId` out-of-band first; thereafter the index is the source of
  further public caps.

**Adopted scope:** the self-sufficient `CapabilityStore` impl uses a per-peer
automerge index doc for *attributable-public* caps + their tombstones;
recipient-specific caps are out-of-band and held locally; anonymous-public
awaits the identity-less store impl. All behind the seam, swappable later.

## Read-only (ruling #5, deferred)

First deliverable is write-gated: no `Write` cap → no sync. Read-only
(`Read` = receive but not push) is ticket 008: a `SyncMode::ReadOnly` the
actor honors (skip `generate` for read-only conns; drop incoming pushes).
New actor work — samod's `SyncDirection` is stats-only, and we're rewriting
the actor anyway, so we design `SyncMode` in from the start (plumbing lands
in 001; policy activates in 008).

## What "Willow rules" means here

Willow's Meadowcap determines, via signed capabilities, which peers may sync
which data; the sync layer checks the capability; Willow does **not** encrypt
(read-authority is a separate key layer above). So "sync according to Willow
rules" = **the engine treats the signed capability as ground truth for
propagation authority**, and **read-authority is independent and not the
engine's concern**.

## Honest gaps / unresolved sub-decisions (surfaced in tickets, not buried)

- **Exact wire encoding of the new handshake/protocol.** PROPOSED in 004.
- **Tombstone entry shape** (bare nonce vs signed revocation object).
  PROPOSED in 006.
- **Public-cap TTL value** and whether it's per-mint or global. PROPOSED in 002.
- **Anonymous-public distribution today.** The identity-less store is future
  work; until then anonymous-public caps travel out-of-band like
  recipient-specific ones. Stated honestly in 003.
- **Multi-owner / communal docs.** Each writer mints as `signer = their
  SubspaceId` for their own subspace; communal = attributable by nature.
  PROPOSED in 002.
- **Capability delegation/chaining.** Mint-by-signer only in the first
  deliverable; delegation is a follow-up.
- **Session-long grant set vs per-message re-validation.** PROPOSED:
  session-long identity + per-message cap check against the live revocation
  view, so revocation takes effect mid-session once the view updates.