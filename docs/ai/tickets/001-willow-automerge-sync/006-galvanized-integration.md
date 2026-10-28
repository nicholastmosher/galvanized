# 006 — Galvanized integration + contacts migration

**Status:** DRAFT
**Depends on:** 003 (handshake), 004 (per-doc gating), 005 (revocation)

## Why

The consumer-side wiring that proves the seam end-to-end. Tickets 001–005
build the mechanism in samod-core + `samod-willow` + `galvanized-cap`; this
ticket wires them into `plugin-automerge` / `plugin-p2p`, registers the
galvanized ALPN, plugs the `PeerAuthenticator` and `CapabilityValidator` into
the `Hub`, and **migrates the contacts shared-display-name doc** to the gated
path as the reference consumer. This is where T4's "migrate `ContactPeer` to
the gated path" acceptance criterion lands.

## Current state (anchors — re-derive before leaning on)

- `galvanized/crates/plugin-automerge/src/lib.rs:15` — `init` observes new
  `Profile`s and registers an `AutomergeProtocol` per Profile's endpoint,
  building a `samod::Repo` with `InMemoryStorage`.
- `galvanized/crates/plugin-automerge/src/protocol.rs:38` —
  `AutomergeProtocol::ALPN = b"iroh/automerge-repo/1"`; `accept` →
  `repo.make_acceptor` → `Transport`, no auth.
- `galvanized/crates/plugin-contacts/src/protocol.rs:130` — `ContactPeer::new`
  creates an automerge doc, hands the `DocumentId` to the remote over the
  control stream (`RequestConnectContact`/`AcceptConnectContact`). The
  contact handshake authenticates the peer's Profile via `SignedEnvelope`
  but does **not** gate the automerge sync on a capability.
- `galvanized/crates/galvanized-cap/src/lib.rs:344` — `WillowKeyStore`
  (mint + sign-as-Profile, both gated).

## Scope

1. **Register the galvanized ALPN.** A `GalvanizedAutomergeProtocol` (or
   extend `AutomergeProtocol`) registered on the galvanized ALPN (003), in
   addition to the vanilla ALPN. The `Repo` is constructed with the
   `PeerAuthenticator` + `CapabilityValidator` (001) plugged in, sourced from
   the Profile's `WillowKeyStore` + the revocation view (005).

2. **`Automergable` + cap binding.** When a plugin creates a shared doc, it
   binds the `DocumentId` to a Willow `(namespace, subspace, path)` and mints
   an `AutomergeSyncCapability` (002) for the recipient. The `Automergable`
   trait (or a new `GatedAutomergable`) carries the binding so the sync
   engine knows the scope. Decide with Nick whether `Automergable` grows a
   cap-binding method or a separate `GatedAutomergable: Automergable` trait
   exists. Recommendation: **separate trait** — keeps ungated automerge
   simple; gated sync opts in explicitly. Confirm.

3. **Migrate `ContactPeer`.** The contacts shared-display-name doc becomes
   the first gated consumer:

   - `ContactPeer::new` mints a `WriteCapability`-bearing
     `AutomergeSyncCapability` for the peer's `SubspaceId`, scoped to the
     contacts namespace + path.
   - The cap is carried to the peer in the existing
     `AcceptConnectContact` envelope (or a new field) — out-of-band of the
     sync handshake, but presented by the peer at handshake time (003).
   - The peer's `AutomergeProtocol` registers the cap, and the gated sync
     proceeds only if the cap is valid.
   - Revocation: a contact removal writes a tombstone (005) so a dropped
     contact cannot re-sync.

4. **`AutomergeHandle` direction awareness (T4 §5).** `Entity<T:
   Automergable>`'s `set`/`update` (`plugin-automerge/src/lib.rs:97`/`:111`)
   should refuse writes when the local holder only has a read cap. In the
   first deliverable (write-gated only) this is moot (no cap = no sync at
   all), but plumb the cap permission into the handle so 007's read-only
   mode is a flip, not a re-arch. Recommendation: land the plumbing now.

5. **Storage.** The migrated `ContactPeer` keeps `InMemoryStorage` for now
   (on-disk storage is a separate open question, out of scope here).

6. **Error surfacing.** A refused sync (handshake or per-doc) surfaces to the
   contacts UI as a visible state ("sync refused: unauthorized"), not a
   silent failure.

## Out of scope

- On-disk automerge storage (the commented `TokioFilesystemStorage`).
- A capability-management UI (caps are minted programmatically here).
- Migrating other plugins (chat, form) — they follow the same pattern once
  contacts proves it.
- Read-only policy (ticket 007).

## Design questions for Nick

1. **`Automergable` vs `GatedAutomergable`.** See scope §2. Recommendation:
   separate trait. Confirm.

2. **Cap transport in the contacts handshake.** Carry the
   `AutomergeSyncCapability` inside the existing `SignedEnvelope`-wrapped
   `AcceptConnectContact`, or a dedicated `PresentCapability` message?
   Recommendation: **inside the existing envelope** — the contacts handshake
   already signs by Profile; piggybacking avoids a new message and keeps the
   cap exchange on a signed channel. Confirm.

3. **Which namespace holds the contacts doc + its tombstones?** The
   Profile's Home Space namespace (per GLOSSARY: Contacts live in the Home
   Space). Recommendation: **Home Space namespace**, path
   `/apps/contacts/{contact_id}`, tombstones at
   `/{owner_subspace}/revoked/{nonce}` within the same namespace. Confirm.

4. **Revocation on contact removal.** Should removing a contact auto-revoke
   its sync cap (write the tombstone), or is revocation a separate explicit
   action? Recommendation: **auto-revoke on removal** — removal means "this
   peer should no longer sync," which is exactly revocation. Confirm
   (privacy-shaped).

## Acceptance criteria

- A `GalvanizedAutomergeProtocol` (or extended `AutomergeProtocol`) serves
  the galvanized ALPN with the `PeerAuthenticator` + `CapabilityValidator`
  plugged in; the vanilla ALPN still works.
- `ContactPeer` mints and presents a cap; gated sync succeeds between two
  peers with valid caps.
- A peer without a cap for the contacts doc cannot sync it; the contacts UI
  shows a visible "sync refused" state.
- Removing a contact revokes its cap; the dropped contact cannot re-sync.
- `AutomergeHandle` carries the cap permission (plumbing for 007).

## Verification

- Two-`Endpoint` in-process test mirroring the contacts handshake: A invites
  B (mints cap), B accepts (presents cap), contacts doc syncs; C is not
  invited, C's sync is refused visibly; A removes B, B cannot re-sync.
- `cargo fmt`; `cargo test` for `plugin-automerge`, `plugin-contacts`,
  `plugin-willow`, `galvanized-cap`.
- State EXECUTED vs RELAYED.

## Records

- Update `ARCHITECTURE.md`'s capability-flow and plugin-automerge sections.
- Move design Q1–Q4 rulings to `DECISIONS.md`.
- Mark T4 (`000-initial-design/004`) superseded with a pointer to this arc.