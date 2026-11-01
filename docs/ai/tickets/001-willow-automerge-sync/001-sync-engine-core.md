# 001 — `galvanized-sync` engine core + identity + auth seams

**Status:** REFINED (shape signed off by Nick, 2026-11-01; ready to pick up)
**Depends on:** —

## Why

The foundation of the arc. We build a **new crate `galvanized-sync`** — a
from-scratch automerge sync engine inspired by samod's sans-IO actor shape,
with Willow identity and **deny-by-default** capability gating baked in from
line one. `samod`/`samod-core` are a reference, not a dependency. Every other
ticket consumes the identity model and authorization seams defined here.

Per Nick's ruling: privacy is the primary case. The engine has **no ungated
path** — every sync requires positive proof of a valid signed capability.
Trait seams exist for *our own* evolution (refining rules, swapping the
capability store), not to wrap samod; the built-in defaults are the real
Willow implementations, not permissive stubs.

## Reference points (samod — inspiration, not substrate; re-derive if reading)

- `samod/samod-core/src/actors/hub.rs` — `Hub { state }`, `handle_event` →
  `HubResults`. The sans-IO actor shape we adopt.
- `samod/samod-core/src/actors/document/doc_state.rs` — per-document actor;
  `generate_sync_messages` (outgoing) and `handle_sync_message` (incoming).
  We rewrite both with authorization hooks and a real `SyncMode` control
  (samod's `SyncDirection` is stats-only — we do not copy that limitation).
- `samod/samod-core/src/network/wire_protocol.rs` — CBOR `Join`/`Peer`/
  `Request`/`Sync`. Codec inspiration only; our protocol is new (ticket 004).
- `galvanized/crates/galvanized-cap/src/lib.rs` — `WillowScope`,
  `WillowKeyStore` (holds namespace + subspace secrets), `StrictSendCap`. We
  build on this for `AutomergeSyncCapability` (002).

## Scope

1. **New crate `galvanized-sync`** in `crates/galvanized-sync`. Workspace
   member. Depends on `galvanized-cap`, `automerge`, `willow25`, `iroh` (for
   transport traits), `minicbor`. Library root per Galvanized convention:
   `[lib] path = "src/galvanized_sync.rs"` (no `mod.rs`).

2. **Sans-IO actor core.** A `Hub` actor + per-document `DocumentActor`,
  `handle_event` → effects, runtime-agnostic — inspired by samod but written
   from scratch with authorization as a first-class concern, not bolted on.

3. **Peer identity = Willow `SubspaceId`.** No ephemeral `PeerId`. The
   `PeerIdentity` carried per connection holds the verified `SubspaceId` and
   the granted capability set. Built-in (not a plug-in point): identity is
   Willow-native by design.

4. **`PeerAuthenticator` trait.** A seam the `Hub` calls during the handshake
   (ticket 004 implements the real one). Input: raw handshake material.
   Output: `PeerIdentity` | visible `Deny(reason)`. **The built-in default is
   the Willow authenticator** (verify `SubspaceId` + challenge-response +
   presented caps), not a no-op. The seam exists so we can evolve the
   handshake rules, not to allow an open default.

5. **`CapabilityValidator` trait.** A seam consulted at `Request`/`Sync` time
   (ticket 005 implements the real one). Input: `(peer_identity,
   document_id, direction)`. Output: `Allow | Deny(reason)`. **Built-in
   default is the Willow validator** (deny-by-default; require a valid
   `AutomergeSyncCapability`). A `Deny` emits a visible `Error` for that doc
   and skips its sync; the connection stays up for other docs.

6. **`SyncMode` per connection** (`ReadWrite | ReadOnly`), plumbed through
   the outgoing path (skip generation for `ReadOnly`) and incoming path (drop
   pushes from `ReadOnly`). Set from cap permission at handshake time. The
   **plumbing lands here**; the read-only *policy activation* is ticket 008
   (fast follow). In the first deliverable, `SyncMode` is effectively always
   `ReadWrite` because receive is also write-gated (no `Write` cap → no sync
   at all) — but the plumbing is in place for 008.

7. **`CapabilityStore` trait seam** (detailed in ticket 003). Defined here as
   the engine's interface to "where caps live and how they propagate"; the
   self-sufficient impl is 003. The engine depends on the trait, not the impl.

8. **Deny-by-default discipline.** No valid cap → no sync, with a visible
   error. The "no authenticator/validator configured" state is a **visible
   refusal** ("sync refused: authorization not configured"), never a silent
   degrade to open sync. No `let _ =` on auth checks (per `CONVENTIONS.md`).

9. **Storage + Transport traits**, inspired by samod's clean separation
   (KV-store with range queries; stream/sink `Transport` over a dialer). We
   define our own; not bound to samod's types.

## Out of scope

- The `AutomergeSyncCapability` format (002).
- The `CapabilityStore` self-sufficient impl (003).
- The handshake wire format (004).
- Per-doc gating policy (005).
- Read-only *policy* (008).

## Rulings (Nick, 2026-11-01 — RATIFIED)

1. **Crate layout — one crate.** Nick: "One crate is fine." `galvanized-sync`
   ships the sans-IO actor core and the built-in Willow authenticator/validator
   in a single crate. If a future non-Willow consumer appears, split then.

2. **`PeerIdentity` — concrete.** Nick: "Concrete is fine." `PeerIdentity`
   carries a concrete `SubspaceId` + `Vec<AutomergeSyncCapability>` (no opaque
   `Box<dyn Any>` payload, no downcasting at the validator). We are not
   building for arbitrary plug-ins, only our own evolution.

3. **Async runtime coupling — single crate with façade.** Nick: "Single
   crate is fine." One crate: sans-IO actors at the core with a thin async
   façade in the same crate (no separate `galvanized-sync-core` + tokio
   wrapper split). No FFI to other languages yet.

4. **`SyncMode` when a peer holds both `Read` and `Write` for a doc —
   PROPOSED default, not explicitly ruled.** Max wins → `ReadWrite`. Trivial,
   consistent with 008 Q2, non-shape-changing. Carries forward as the default
   unless Nick objects; flag if it ever matters.

## Open design questions for Nick

- None blocking pickup. Q4 above is a trivial default pending explicit
  confirm; everything else is ruled.

## Acceptance criteria

- `galvanized-sync` crate exists, builds, and is wired into the workspace.
- Sans-IO `Hub` + `DocumentActor` with `handle_event` → effects.
- Peer identity is `SubspaceId`; no ephemeral `PeerId`.
- `PeerAuthenticator`, `CapabilityValidator`, `SyncMode`, `CapabilityStore`
  traits defined, with the Willow impls as the built-in defaults.
- Deny-by-default: with no valid cap presented, sync is refused with a visible
  error; no path syncs ungated.
- `SyncMode` plumbing runs through the outgoing and incoming actor paths.
- No `samod` dependency.

## Verification

- `cargo build -p galvanized-sync`.
- Unit tests: a stub validator returning `Deny` → `Request`/`Sync` for that
  doc emits an `Error` and no sync, connection stays up. A connection with no
  authenticator → visible "authorization not configured" refusal.
- `cargo fmt`; no clippy unless Nick asks (ratified 2026-07-30).
- State EXECUTED vs RELAYED.

## Records

- Add `galvanized-sync` to `ARCHITECTURE.md`'s crate map.
- Rulings Q1–Q3 recorded in `DECISIONS.md` (2026-11-01).