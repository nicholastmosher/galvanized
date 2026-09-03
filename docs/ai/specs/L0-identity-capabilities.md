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

## Custody brainstorm — the bootstrap admin-secret (PROPOSED)

> Design exploration. The hybrid below is the direction Nick is **leaning**
> (chat, 2026-09-02) — **PROPOSED, not ratified.** Nothing here is settled policy
> until Nick rules.

### The problem in detail

The bootstrap namespace is an **owned** Willow namespace with a **namespace
secret** (root signing key). That secret is the authority to do one thing in
steady state: **mint new capabilities** (enroll a device, delegate/re-delegate).
Ordinary sync does not need it (devices use delegated read caps); unlocking a
vault does not need it (that is the password). So the secret is an
**enrollment/recovery instrument**, rarely used — but it is needed for **two**
operations, and the second is security-critical:

1. **Enrollment** — mint a read cap for a new device (routine, friendly).
2. **Hostile eviction** — revoke a cap held by a *compromised* device you did
   not delegate to (incident response).

The second case is what raises the stakes: it is not just "how convenient is
enrollment" but "how **fast** can you exercise the root when a device is
compromised." A custody scheme cheap at enrollment but slow under incident is
worse than it looks.

**Tensions:**

- **Availability vs. compromise surface.** Wherever the secret lives *online*,
  that device is a crown-jewel target: holder of the secret → mint caps → enroll
  rogue devices → reach every device's bootstrap namespace → attempt offline
  brute-force on every vault password.
- **No server** to custody, rate-limit, or recover it.
- **Revocation asymmetry.** If every device holds the raw secret, you cannot
  evict a compromised device (it has the root; you must *rotate* — re-enroll
  everyone, and the rogue keeps its old local ciphertext). If devices hold only
  delegated caps, eviction is cheap — but somebody must hold the raw secret to
  mint caps, recreating a privileged custodian.
- **Bootstrap recursion.** The secret cannot live inside a vault (the vault is
  unlocked via this namespace). Whatever protects it is the **new root of trust**.
- **Multi-fleet.** Custody is per bootstrap namespace (compartmentalization),
  bounding blast radius.

**Threat calibration (precise).** First, two axes must not be conflated:

- A **device** is a physical peer that *syncs* the bootstrap namespace; it needs
  a **read capability**. "Enrolling a device" = minting a read cap.
- A **profile** is an identity (a subspace key) stored *inside a vault blob*,
  protected by the **vault password**. Adding a profile to a vault = modifying a
  vault blob's contents = decrypting it = needs the vault password. The namespace
  secret does **not** grant this.

In Willow's model (Meadowcap), the namespace secret is the root authority to **mint
capabilities** (read *and* write) for that namespace, and to **write** to it. So a
holder can: enroll devices to sync (get ciphertext + history); and write garbage /
plant blobs — a *destructive DoS*, mitigated by the static substrate's git-like
history (originals recoverable; tampered blobs fail to decrypt, so detectable).
The secret does **not** grant decrypting vault contents, reading Spaces,
impersonating a profile, or adding profiles.

The namespace secret is **not the final crown jewel** — it protects
*reachability to ciphertext*; the vault passwords protect the *root keys*. The
asymmetry across three threats (Alice: bootstrap namespace `B` with two
password-protected vaults; three devices with read caps; secret on paper):

| Compromise | Yields | Can spread? | Durable fix |
|---|---|---|---|
| Namespace secret (T1) | ciphertext + reachability + write/DoS | **yes** (re-mint) | **rotate the secret** |
| Device stolen (T3) | ciphertext + reachability + maybe live state | no (bounded) | **revoke the cap** |
| Vault password alone (T2) | nothing (no ciphertext) | n/a | rotate the password (cheap) |

Two consequences:

1. **The secret is persistent/spreading** (attacker re-mints after eviction →
   rotation required), while **a device is bounded** (revoke the cap). So custody
   of the secret matters — but the secret is *less immediately catastrophic than
   a stolen unlocked device*: T1 cannot read contents, T3 possibly can. The
   bigger practical risk is **device theft while unlocked**, which the custody
   layer does not address at all — that is the *device/session* layer's job
   (auto-lock, no persistent vault keys on disk, wipe memory on lock). The custody
   ceremony's marginal value is bounded by how well that device layer is
   protected.
2. **Reachability and readability are independent and both required** — T2 is
   impotent alone (password but no ciphertext). This is Q5 made concrete at the
   custody layer: an attacker must compromise *two different things* (a
   reachability root + a password) to get vault contents, except in the
   device-theft case where one device holds both reachability and a live copy.

**Rotation is not a nice-to-have technique — it is the mandatory recovery
operation for namespace-secret compromise (T1).** This raises its priority when
designing the capability lifecycle.

**Cross-cutting wrinkle — who can revoke?** Revoking a device is a **write**
(tombstone), and read-only devices cannot write. Eviction needs the root secret
or a delegated **revocation capability** — but a revocation cap is power: if any
device can revoke any other, a compromised device can revoke all the good ones
(a cheap DoS). The clean corner ("a device can only revoke caps *it* delegated")
handles friendly revocation but **not** hostile eviction. So hostile eviction
likely requires the root — which is *why* the root's latency-to-exercise matters.
**Revocation authority and the DoS tradeoff is an open sub-problem across all
custody schemes; none resolves it alone.**

### Three solutions (three angles: when / where / who the root is)

**Solution A — Cold custody (minimize *when* the root is live).** The root
exists live as rarely as possible; devices hold only delegated read caps. At
creation, mint the first device's read cap, then **seal the secret as an offline
recovery phrase** (BIP-39) — not on any device's disk. Enroll = thaw the phrase,
mint a cap, drop from memory. Recover from total loss = phrase reconstitutes the
secret. Threat: device software compromise gets read caps + local ciphertext,
**not** the secret → cannot enroll rogues; stealing the secret requires physical
access to the phrase. UX: enrollment needs the phrase (rare, heavier); the phrase
is a physical SPOF. **Counter:** this *moves* the crown jewel from a device to a
piece of paper — better (offline, not malware-exfiltrable) but does not
eliminate the SPOF (lose phrase + all devices = irrecoverable; finder can mint).

**Solution B — Hardware-rooted (make the root unstealable by software; *where*
the root lives).** The secret is generated inside a hardware token / OS secure
enclave and is **non-exportable**; the token only signs caps. Enroll = present
token, authorize, sign. Threat: strongest against remote/software compromise of
the root — malware cannot exfiltrate a non-exportable key; theft needs the
physical token + defeating its PIN. UX: a **hardware dependency and cost** — in
tension with the non-technical-organizer audience. Token loss is a steep cliff
without a backup (a cloned token, or a one-time paper export at genesis →
re-introduces A). **Counter:** best against remote attack, weakest against
physical loss/theft, adds gear the audience may lack, and does not solve
recovery alone — it composes with A or C, best as a *component* of a hybrid.

**Solution C — Quorum authority (eliminate the single root; *who* holds it).**
The secret is **Shamir-split** (or **threshold-signed** without reconstruction)
into N shares held by a mix of the user's offsite devices and/or trusted people;
k-of-N mint/evict. Devices hold read caps. Every minted cap is recorded as an
**entry in the bootstrap namespace — an enrollment audit log** visible to all
synced devices. Threat: no single party is root; one compromised share is
useless; attacker must compromise k parties across domains. The audit log
**converts silent compromise into detectable compromise** — the only angle that
gives detection, not just prevention. UX: the **heaviest ceremony**; re-introduces
a **social trust assumption**; users may weaken it (k=1). Two sub-variants:
**social-quorum** (trusted people, slow, trust-dependent) vs **fleet-quorum**
(your own devices co-approve, faster, trust-free but fails if the fleet is small
or co-located). **Counter:** most secure against total loss/SPOF, least usable
for the stated audience.

### Synthesis

| | Live-exposure of root | Remote-attack on root | Physical-loss recovery | Incident latency | Audience fit |
|---|---|---|---|---|---|
| A Cold | minimal (rare thaw) | strong | phrase = SPOF | slow (find paper) | good (familiar) |
| B Hardware | none (non-exportable) | strongest | needs A or C backup | fast (have token) | poor (gear/cost) |
| C Quorum | distributed | strong (k parties) | best (survives loss) | slow (gather k) | poor (ceremony) |

The three **compose**; the honest answer is a hybrid. Evaluative dimensions that
actually matter, given the threat calibration: (1) **incident latency** — how
fast can you evict? (favors B; A/C slow; under-appreciated axis); (2) **SPOF vs
distributed** — does one loss = catastrophe? (favors C); (3) **audience fit** —
no gear, minimal ceremony (favors A).

### The leaning direction (PROPOSED — Nick leaning, 2026-09-02)

**A as the baseline + C's audit log always on + B as an opt-in fast path:**

- **A (cold recovery phrase, devices hold read caps)** as the default —
  familiar, no gear, workable for non-technical organizers.
- **C's enrollment audit log always on** — nearly free, high value: detection of
  rogue enrollments across all configurations.
- **B (hardware token) as an *opt-in* upgrade** for users who have one — fast
  eviction, and the token can *hold* the recovery phrase, mitigating A's paper
  SPOF.

This gives a workable default for the non-technical user while letting the
privacy-power-user upgrade.

### Open sub-problems (not resolved by custody alone)

- **Revocation authority & the DoS tradeoff** — who can write tombstones; can a
  compromised device DoS the fleet by revoking everyone? Hostile eviction likely
  requires the root; friendly revocation might be self-service. A capability-
  delegation-graph design question, not a custody question.
- **Rotation is a first-class operation, not a technique.** It serves two
  roles: (a) *proactive* — rotate the namespace secret per enrollment and destroy
  the old, so a stolen *old* secret cannot mint current caps (limits the value of
  a past theft); (b) *reactive / mandatory* — it is the **only durable recovery
  for namespace-secret compromise (T1)**, since the attacker re-mints after any
  mere eviction. Layers onto any of A/B/C; its cost (re-enroll all surviving
  devices) drives how cheaply we can afford to do it.
- **Fleet-quorum vs social-quorum** (within C) — own devices voting vs trusted
  people voting; different trust and locality assumptions.
