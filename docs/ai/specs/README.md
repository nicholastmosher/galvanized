# Galvanized target-state specifications

This directory holds the **target-state** design for Galvanized — a description of
the system we are building *toward*, which differs from the current code and
from the historical intent recorded in the dev log (`README.md`) and memory
(`docs/ai/memory/`). Where the spec and the code disagree, the spec is the
*goal*; the code is the *present*. We iterate on the spec here, and the code
moves toward it.

## How these specs are written

- **Requirements are user stories.** Every design choice traces to one or more
  user stories (below). A choice with no story behind it is scope creep.
- **Decisions are Nick's.** Shape-shaped, mechanism-shaped, and
  security-shaped calls are framed as questions with a recommendation, not
  pre-made picks. The `INFERRED` / `PROPOSED` / `RATIFIED` markers from the
  memory system apply here: nothing is settled policy until Nick ratifies it.
- **The strongest counter goes first.** For any proposal, raise its best
  counter-argument before anyone else has to, then resolve.
- **Honest gaps.** An unresolved question is written as a question, never papered
  over with a plausible-sounding answer.
- **Don't stub areas before the shape is settled.** We create a spec file only
  when its scope is nailed down. Premature stubs ossify a wrong decomposition.

## The load-bearing requirements (user stories)

- **U1 — Data sovereignty.** As a user, I want all of my data to live on my own
  devices and be controlled by me.
- **U2 — Self-sync, no gatekeeper.** As a user, I want to sync my data between my
  own devices securely and privately, with no centralized provider gatekeeping
  me.
- **U3 — Selective p2p sharing.** As a user, I want to selectively share some of
  my files or content with other users, via direct peer-to-peer connection, not
  relying on any cloud or third-party provider.
- **U4 — Static blob sync.** As a user, I want to sync static, unchanging,
  blob-like files among my devices and to users I've shared with.
- **U5 — Dynamic doc sync.** As a user, I want to sync dynamic, updatable,
  document-like files, updatable one keystroke at a time (Google-Docs-style
  multi-cursor collaboration).
- **U6 — New interfaces.** As a developer, I want to quickly build apps that
  organize data in new ways and offer new interfaces for presenting rich shared data.
- **U7 — Cross-app data.** As a developer, I want to write apps that interact
  with data from other apps if I have the right permissions.
- **U8 — Easy API, automatic sync.** As a developer, I want an easy-to-use
  programming API for interacting with user data that automatically gains the
  sync and sharing benefits of the platform.
- **U9 — Password-manager unlock.** As a user, I want a password-manager-like
  experience: a password unlocks a vault, and that vault gatekeeps access to the
  entire app.
- **U10 — Vault sync, same password.** As a user with a vault, I want to sync my
  vault between multiple devices and use the same password to unlock the app and
  use it on each device.

## The vault, and the bootstrap recursion (U9, U10)

The vault is the gatekeeper for the entire app: unlock it (with a password) to
use anything. **Target state:** the vault is no longer a bespoke store — it is
implemented *on* the encrypted Willow filesystem, so **Willow sync becomes the
vault's own sync primitive**. This unifies vault sync (U10) with content sync:
the vault is just the first, most important customer of the encrypted static
substrate.

**Current state (code truth):** the vault is a bespoke implementation on top of
sqlite (`plugin-vault`), and therefore has **no sync primitives**. Moving it onto
encrypted Willow is a target-state change, not a refactor of what exists.

**The bootstrap namespace (revised).** The bootstrap namespace is a
**purpose-built *owned* namespace** — you hold its namespace key, and it is
**not itself encrypted**. Its *content* is a set of **vaults**, where each vault
is a **password-encrypted blob of subspace and namespace keys** (the Profile and
Space keys that vault grants access to). So:

- Reaching the bootstrap namespace = a **device-enrollment** matter (you hold
  its namespace key / a sync cap, granted device-to-device by invitation),
  **not** a password matter.
- Unlocking a *vault* = enter that vault's password → decrypt its blob → recover
  the subspace/namespace keys for the Profiles and Spaces that vault contains.
- **Multiple vaults** coexist in the one bootstrap namespace, each with its own
  password (a "daily" vault and a "sensitive" vault, say).

This cleanly separates two secrets that the earlier "password-derived bootstrap
key" idea wrongly conflated: a **high-entropy, machine-managed** device secret
(the bootstrap namespace key / sync cap — gates *reachability*) and a
**memorable, user-chosen** password (gates *decryption of vault contents*). A
new device needs *both* an invite (reachability) and the right password
(decryption); a stolen device exposes only password-encrypted blobs; a forgotten
password leaves reachability intact (but useless). This is the same factoring a
password manager uses, and it is the right one.

**Security validation (see turn discussion):** sound, and a better factoring than
a password-derived bootstrap key. Surfaced problems: (a) bootstrap-namespace
*key custody* is the new root-of-trust question; (b) a stolen/revoked device
*retains* the password-encrypted blobs, so offline password brute-force is the
residual threat; (c) vaults hold *root keys* for owned spaces/profiles, so a
password compromise is catastrophic and non-revocable short of namespace
rotation; (d) the bootstrap namespace's structure (vault count, timing) is
visible to sync-capable peers, so vault *existence* is not deniable to them.

This makes the encryption-at-rest layer **load-bearing for the content
substrates** (the Spaces where files/docs live), while the bootstrap namespace
uses **per-entry password-encryption** over the *same* crypto primitives (a
password-derived KEK, rather than a separate crypto path). One crypto core, two
KEK regimes.

## The layered model (proposed — awaiting Nick)

The system is a stack. We design **bottom-up**, because each layer's contract is
load-bearing for the one above it. Nothing above is specified until the layer
below is settled.

```
┌─────────────────────────────────────────────────────────────┐
│ Apps & UI shell  (chat, calendar, files, contacts, …)         │  spec later
├─────────────────────────────────────────────────────────────┤
│ Dynamic data substrate  (live docs, keystroke-updatable)      │  U5
│   layered ON TOP of the static substrate                       │
├─────────────────────────────────────────────────────────────┤
│ Static data substrate  (content-addressed, git-like history)  │  U4
├─────────────────────────────────────────────────────────────┤
│ Encryption at rest  (Willow does not encrypt; we add this)     │  U1
├─────────────────────────────────────────────────────────────┤
│ Willow  (namespaces + subspaces = keys; capabilities gate sync)│
├─────────────────────────────────────────────────────────────┤
│ Identity & capability system  ← THE FOUNDATION                │  U1,U2,U3,U7,U9
│   • Profiles = Willow subspaces (identities)                   │
│   • Namespaces = filesystem-scope + sharability identifiers    │
│   • The vault, implemented on the encrypted Willow FS (U9,U10) │
│     - bootstrap namespace = purpose-built owned namespace holding │
│       password-encrypted vault blobs (multiple vaults, own pwds) │
│     - device secret (reachability) ≠ user password (decryption)    │
│   • Capability duality (below)                                 │
└─────────────────────────────────────────────────────────────┘
```

### L0 — Identity & capability system (the foundation)

This is where capabilities live, and it sits **below** the data substrates. We
expound it at length before anything else, because every layer above assumes its
contracts.

**Willow primitives as keys.** Willow **namespaces** and **subspaces** are
cryptographic keys. Galvanized uses them as:

- **Subspace = Profile** — an identity / persona. (One Vault holds many
  Profiles = many personas.)
- **Namespace** — an identifier for a *filesystem of a particular scope and
  sharability*. I.e. it names a unit of data whose sharing/boundary is defined
  once.

**What Willow does and does not do.** Willow determines, via capabilities,
which peers may or may not sync particular data to and from one another. It
does **not** encrypt. Therefore an **encryption-at-rest layer** sits on top of
Willow (between Willow and the static substrate) so data is encrypted at rest.

**The capability duality.** Capabilities exist in two layers, deliberately
linked:

1. **Serialized capability objects** — like Willow's native ones. Serializable,
   usable *by Willow* to determine sync permissions efficiently. Extended with
   **custom capability objects** that describe higher-level, app-specific
   capabilities. These are data: they can be stored, shared, and verified by the
   sync layer.
2. **Rust type-level capabilities** — a serialized capability object (layer 1)
   *authorizes minting* a **type-token** expressing the same capability in the
   programming API, for Willow operations or higher-level apps. You cannot call
   a gated operation without the type-token, and you can only mint the token by
   presenting a valid serialized capability object. (This is the concrete form
   of the "parallel between capabilities-as-signed-objects and type-level
   capabilities" the dev log reached for on 2026-07-11.)

The link between the two is the load-bearing mechanism: the serialized object is
the *ground truth* the sync layer checks; the type-token is the *programming
ergonomic* that makes "no capability, no call" a compile-time matter, with the
serialized object as the minting oracle.

### L1 — Static data substrate

Content-addressable, with **git-like history** (commits). Built on Willow +
the encryption layer. Satisfies U4 (static, unchanging, blob-like files). This
is the durable, addressable, history-tracked base layer.

### L2 — Dynamic data substrate

Layered **on top of** the static substrate — not a peer substrate. Updatable one
keystroke at a time; live multi-cursor (U5). It composes over the static layer
rather than replacing it.

### Above (spec later)

Apps, the UI shell, and the sync/sharing policy sit on top of the substrates.
We do not spec them until L0–L2 are settled.

## Guiding conclusions to validate (not yet ratified)

- **The dynamic substrate is layered on the static one** — one substrate, not
  two peers. Static is the foundation; dynamic composes over it.
- **The static substrate is content-addressed with git-like history.**
- **Identity & capabilities are the foundation**, below the data substrates, and
  must be settled first.
- **Willow namespaces/subspaces are keys**; subspaces = Profiles, namespaces =
  filesystem-scope/sharability identifiers.
- **Willow does not encrypt** — an encryption-at-rest layer is required on top
  of it.
- **Capabilities are dual**: serializable objects (Willow-sync-checked,
  extensible to app-specific) + Rust type-tokens (minted from a serialized
  object, required to call gated APIs).
- **The vault is implemented on the encrypted Willow filesystem** (not bespoke
  sqlite), so Willow sync is the vault's sync primitive (U9, U10). The
  **bootstrap namespace** is a purpose-built *owned* namespace (not encrypted)
  holding **multiple password-encrypted vault blobs**; device reachability
  (namespace key / sync cap, high-entropy) is separated from the user password
  (decryption, memorable).
- **Encryption-at-rest is load-bearing** for the content substrates; the
  bootstrap namespace uses per-entry password-encryption over the *same* crypto
  primitives (password-derived KEK).
- **Transport is p2p-only (Iroh); no cloud control plane** (U2/U3). A relay may
  be a transport fallback, never a control plane.

## Plan

**Iterate on L0 (the low-level identity & capability system) before spec'ing
anything else.** No other area files are created until L0 is nailed down. The
first spec document we write will be the identity & capability layer.
