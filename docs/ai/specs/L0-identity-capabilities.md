# L0 — Identity & Capabilities

> **Status: problem statement (refined).** Solutions to be brainstormed next.
> Background (Willow namespaces/subspaces as cryptographic keys) is a settled
> hard constraint, not an open question. Nothing here is ratified policy until
> Nick rules; the `PROPOSED` / `RATIFIED` markers from the memory system apply.

## What L0 is for

Galvanized has no server and no provider. Its data lives on a peer-to-peer
substrate (Willow) where data is addressed by cryptographic namespaces and
subspaces, and where the *only* access control is capabilities carried and
verified peer-to-peer. L0 is the layer that makes that substrate usable as a
sovereign, private, selectively-shareable system: it defines **who the
participants are**, **what the units of data and sharing are**, **who may do what
to which data**, and **how a user safely holds the cryptographic material that is
their identity and authority** — all without any central authority, and with
privacy/anonymity as a first-class constraint, not an afterthought.

L0 also governs the **cryptographic key layer** holistically: identity keys,
capability keys, and encryption keys are all *keys*, and there is synergy to
harness from that fact — the design treats them as one governed family of
key material rather than three separate key systems. In particular, the
**encryption-at-rest key hierarchy** (whose *mechanism* is applied above Willow)
is designed here, because "permission to *receive* data" (sync, a capability)
and "permission to *read* data" (decrypt, a key) must be independent and must
relate without one subsuming the other.

## Background (settled hard constraints — tools we build on, not open choices)

- **Willow namespaces and subspaces are cryptographic keys.** They are given, not
  chosen by this layer.
- **Subspace = Profile** — an identity / persona. One person may hold many.
- **Namespace** — an identifier for a filesystem of a particular scope and
  sharability; the unit on which sharing and revocation operate.

## The questions L0 must answer

1. **Identity without an authority.** How are participants represented when there
   is no identity provider — such that a person can hold one or more identities
   (personas), identities are *not* bound to a network location, and identity
   material is self-sovereign (held by the user, not a server)?

2. **Data scope as a sharing boundary.** How is "a body of data with a defined
   scope and sharability" identified, such that sharing and revocation can be
   expressed as operations on that unit?

3. **Permission without a referee.** How is "who may read/write/sync which data"
   expressed, granted, revoked, and *enforced* when there is no central
   access-control point — only peers that each decide whether to sync with each
   other?

4. **Two enforcement points, one truth.** Permission must be enforced both (a) at
   **sync time**, where the substrate decides whether to sync a piece of data
   with a peer, efficiently and from serializable evidence; and (b) at **local
   API time**, where the programming interface prevents an operation that lacks
   authority. How do these two stay consistent — especially under revocation,
   across eventually-consistent synced state?

5. **Sync-authority vs. read-authority independence.** "Permission to *receive*
   a piece of data" (sync) and "permission to *read* it" (decrypt) must be
   separable, so capabilities gate propagation while a separate key layer gates
   readability — and neither collapses into the other. How do the two
   authorities relate without one subsuming the other? (This is why the
   encryption-key hierarchy is designed in L0, not deferred to the encryption
   *mechanism* layer.)

6. **Extensible permissions.** How can higher-level, app-specific permissions
   ("can edit this doc", "is admin of this space") be expressed on top of the
   primitive permission system, such that they still reduce to something the
   sync layer enforces efficiently — rather than spawning a second, divergent
   enforcement engine?

7. **Custody of root material without a server.** How does a user safely hold the
   cryptographic material that *is* their identity and authority (subspace keys,
   namespace secrets, capabilities, encryption keys) — protected at rest,
   recoverable across their own devices, enrollable to a new device, and
   revocable on device loss — with no server to gatekeep, rate-limit, or recover
   for them?

8. **Authority in the programming API — dead simple.** How does the
   developer-facing API make "no authority, no operation" a property the type
   system carries, so capability-checking isn't a convention every call site
   must remember — while remaining grounded in the serialized authority the sync
   layer actually checks? The bar is **"baby devs"**: a beginner should be able
   to customize interfaces and program new interactions with data without
   reasoning about capability plumbing.

## The constraints that make this hard

- **No central authority, no provider.** Every permission decision is made
  on-device and reconciled peer-to-peer; revocation is *data* that must propagate
  and win in a merged, eventually-consistent store — there is no central revoke
  call.
- **Privacy and anonymity are the point.** Identity must not bind to network
  location (Endpoint); even intra-user device correlation is a cost to minimize.
  "Who you know" is among the most sensitive data.
- **Selective sharing with revocation, p2p only.** Sharing must not route through
  any cloud; a grant must be revocable and the revocation must hold across
  devices that may be offline.
- **Two enforcement points must agree.** Sync-time and API-time enforcement can
  diverge (a revoked-but-not-yet-propagated authority); the model must define the
  window and close it honestly.
- **No rate-limiting backstop.** Where authority reduces to a user-memorable
  secret, there is no server to throttle guessing; the residual offline
  brute-force threat is inherent and must be owned, not assumed away.
- **Non-technical end users *and* baby-dev API consumers.** The
  custody/enrollment/recovery ceremonies must be usable by community organizers,
  and the programming API must be simple enough for beginner developers to build
  new interfaces and data interactions without capability expertise.

## Out of scope for L0 (boundaries)

- The **content data model** (static blobs vs dynamic docs, content-addressing,
  history) — the substrate layer above L0.
- The **sync transport** (Iroh, rendezvous, hole-punching, relay) — L0 defines
  *what* may sync and *who* may receive it, not the mechanism of moving bytes.
- The **encryption-at-rest *mechanism*** (how ciphertext is laid down on the
  substrate) — that is a layer above Willow. In scope here is only the **key
  hierarchy** and its relationship to capability/identity keys (#5).
- **App-level semantics** — L0 provides the *primitive* permission system that
  apps extend; app behavior is above.

## Open (to resolve in the solution brainstorm)

- All eight questions above are open as *designs*. The background (Willow
  primitives as keys; subspace=Profile; namespace=scope) is settled.
- The custody model for the bootstrap namespace's own admin secret (device
  enrollment, revocation, recovery) is the most foundational open sub-problem
  within Q7.
- The keyring-vs-container question for the vault (does the vault hold only
  keys/caps, or also content?) bears on Q7 and on the sharing model; to be
  settled before or during the custody design.
