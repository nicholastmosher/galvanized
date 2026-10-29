# 007 — Galvanized integration + contacts migration

**Status:** DRAFT
**Depends on:** 004 (handshake), 005 (per-doc gating), 006 (revocation)

## Why

The consumer-side wiring that proves the engine end-to-end. Tickets 001–006
build `galvanized-sync` + the cap model + the store; this ticket wires the
engine into `plugin-automerge` / `plugin-p2p`, registers the new ALPN, plugs
in the `CapabilityStore` (003), and **migrates the contacts
shared-display-name doc** to the gated path as the reference consumer. This
is where T4's "migrate `ContactPeer` to the gated path" acceptance criterion
lands. The current `IrohSamod`/`AutomergeProtocol` glue is **replaced**, not
extended.

## Current state (anchors — re-derive before leaning on)

- `galvanized/crates/plugin-automerge/src/lib.rs:15` — `init` observes new
  `Profile`s, registers an `AutomergeProtocol` per Profile's endpoint,
  builds a `samod::Repo` with `InMemoryStorage`. **Replaced**: the protocol
  handler becomes a `galvanized-sync` engine instance, not a samod repo.
- `galvanized/crates/plugin-automerge/src/protocol.rs:38` —
  `AutomergeProtocol::ALPN = b"iroh/automerge-repo/1"`; `accept` → samod
  `Transport`, no auth. **Replaced** by the new ALPN (004) + the
  `galvanized-sync` actor driving the stream.
- `galvanized/crates/plugin-p2p/src/iroh_repo.rs` — `IrohSamod`, an older
  copy of the same pattern. **Removed or retired** (decide with Nick, design
  Q4).
- `galvanized/crates/plugin-contacts/src/protocol.rs:130` — `ContactPeer::new`
  creates an automerge doc, hands the `DocumentId` over the control stream
  (`RequestConnectContact`/`AcceptConnectContact`). The contact handshake
  authenticates the peer's Profile via `SignedEnvelope` but does **not** gate
  the automerge sync on a capability. **Migrated** to mint + present a cap.
- `galvanized/crates/galvanized-cap/src/lib.rs:344` — `WillowKeyStore` (mint
  + sign-as-Profile, both gated) — reused for minting the contacts cap.

## Scope

1. **Register the new ALPN.** A protocol handler (new, in `plugin-automerge`
   or a thin `plugin-sync` wiring module) serves `iroh/galvanized-sync/1`
   (004). Per-Profile, like today. The handler constructs a `galvanized-sync`
   engine instance with the built-in Willow `PeerAuthenticator` +
   `CapabilityValidator` (001) + the self-sufficient `CapabilityStore` (003),
   sourced from the Profile's `WillowKeyStore` + the index doc.

2. **`Automergable` + cap binding.** When a plugin creates a shared doc, it
   binds the `DocumentId` to a Willow `(namespace, subspace, path)` and mints
   an `AutomergeSyncCapability` (002) for the recipient. Decide with Nick
   whether `Automergable` grows a cap-binding method or a separate
   `GatedAutomergable: Automergable` trait exists. Recommendation: **separate
   trait** — keeps ungated-style usage simple; gated sync opts in explicitly.
   Confirm.

3. **Migrate `ContactPeer`:**
   - `ContactPeer::new` mints an `AutomergeSyncCapability` (`Write` for a
     contact, `signer = owner's SubspaceSecret`, `recipient = contact's
     SubspaceId`), scoped to the contacts namespace + path.
   - The cap is carried to the peer in the existing `SignedEnvelope`-wrapped
     `AcceptConnectContact` (or a new field) — out-of-band of the sync
     handshake, then presented by the peer at handshake time (004) and stored
     via `CapabilityStore::store_received` (003).
   - The peer's engine registers the cap; gated sync proceeds only if valid.
   - Revocation: removing a contact writes a tombstone (006) so a dropped
     contact cannot re-sync.

4. **`AutomergeHandle` direction awareness (T4 §5).** `Entity<T:
   Automergable>`'s `set`/`update` (`plugin-automerge/src/lib.rs:97`/`:111`)
   refuse writes when the local holder only has a `Read` cap, with a visible
   error. In v1 (write-gated) this is moot (no cap = no sync at all), but
   plumb the cap permission into the handle so 008 is a flip, not a re-arch.

5. **Storage.** The migrated `ContactPeer` keeps in-memory storage for now
   (on-disk storage is a separate open question, out of scope here).

6. **Error surfacing.** A refused sync (handshake or per-doc) surfaces to the
   contacts UI as a visible state ("sync refused: unauthorized"), not a
   silent failure.

## Out of scope

- On-disk automerge storage.
- A capability-management UI (caps minted programmatically here).
- Migrating other plugins (chat, form) — they follow the same pattern once
  contacts proves it.
- Read-only *policy activation* (008).

## Design questions for Nick

1. **`Automergable` vs `GatedAutomergable`.** See scope §2. Recommendation:
   separate trait. Confirm.

2. **Cap transport in the contacts handshake.** Carry the
   `AutomergeSyncCapability` inside the existing `SignedEnvelope`-wrapped
   `AcceptConnectContact`, or a dedicated `PresentCapability` message?
   Recommendation: **inside the existing envelope** — the contacts handshake
   already signs by Profile; piggybacking keeps the cap exchange on a signed
   channel and avoids a new message. Confirm.

3. **Which namespace holds the contacts doc + its tombstones?** The Profile's
   Home Space namespace (GLOSSARY: Contacts live in the Home Space).
   Recommendation: **Home Space namespace**, path `/apps/contacts/{contact_id}`,
   tombstones appended to the per-peer index doc (003/006). Confirm.

4. **Retire `plugin-p2p/src/iroh_repo.rs` (`IrohSamod`)?** It's an older copy
   of the same samod glue. Recommendation: **remove it** once
   `galvanized-sync` wiring lands — dead code rots and confuses. Confirm.

## Acceptance criteria

- A new-ALPN handler serves `iroh/galvanized-sync/1` with the
  `PeerAuthenticator` + `CapabilityValidator` + `CapabilityStore` plugged in.
- `ContactPeer` mints and presents a cap; gated sync succeeds between two
  peers with valid caps.
- A peer without a cap for the contacts doc cannot sync it; the contacts UI
  shows a visible "sync refused" state.
- Removing a contact revokes its cap; the dropped contact cannot re-sync.
- `AutomergeHandle` carries the cap permission (plumbing for 008).
- No `samod` dependency remains in `plugin-automerge`/`plugin-p2p` (the
  `Cargo.toml` `samod` lines are removed).

## Verification

- Two-`Endpoint` in-process test mirroring the contacts handshake: A invites
  B (mints cap), B accepts (presents cap), contacts doc syncs; C is not
  invited, C's sync is refused visibly; A removes B, B cannot re-sync.
- `cargo fmt`; `cargo test` for `plugin-automerge`, `plugin-contacts`,
  `plugin-willow`, `galvanized-cap`, `galvanized-sync`.
- State EXECUTED vs RELAYED.

## Records

- Update `ARCHITECTURE.md`'s capability-flow and plugin-automerge sections.
- Move design Q1–Q4 rulings to `DECISIONS.md`.
- Mark T4 (`000-initial-design/004`) superseded with a pointer to this arc.
- Remove the `samod` workspace dependency line once `plugin-automerge`/
  `plugin-p2p` no longer use it (note in the PR; verify no other crate pulls
  `samod`).