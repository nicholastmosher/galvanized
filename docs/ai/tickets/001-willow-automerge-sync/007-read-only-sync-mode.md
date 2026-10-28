# 007 — Read-only `SyncMode` (fast follow)

**Status:** DRAFT (deferred — explicit fast follow per ruling #5)
**Depends on:** 001 (`SyncMode` plumbing lands with the seams)

## Why

The first deliverable is **write-gated** (ruling #5): no `Write` cap → no
sync at all. The natural follow-up is **read-only sync**: a peer with a
`Read` cap may *receive* changes but must not *push*. This needs a
per-connection `SyncMode::ReadOnly` that the `DocumentActor` honors on both
the outgoing and incoming paths. Samod has no directional control today
(`SyncDirection` is stats only — verified in
`samod/samod-core/src/actors/document/sync_message_stat.rs`), so this is new
actor work. Ticket 001 lands the `SyncMode` plumbing so this is a policy flip,
not a re-arch.

## Current state (anchors — re-derive before leaning on)

- `samod/samod-core/src/actors/document/sync_message_stat.rs:22` —
  `SyncDirection` (`Received` | `Generated`), **stats only**.
- `samod/samod-core/src/actors/document/doc_state.rs:333` —
  `generate_sync_messages` (outgoing path — to suppress for `ReadOnly`).
- `samod/samod-core/src/actors/document/doc_state.rs:236` —
  `handle_sync_message` (incoming path — `ReadOnly` peers' pushes dropped).
- `galvanized/crates/galvanized-cap/src/lib.rs:42` — `WillowRead` permission
  (the cap permission that selects `ReadOnly`).

## Scope

1. **`SyncMode::ReadOnly` enforcement.** With the plumbing from 001:
   - `generate_sync_messages` skips generation for `ReadOnly` connections
     (a read-only peer sends no sync messages outward).
   - `handle_sync_message` drops incoming `Sync`/`Request` *data* from
     `ReadOnly` peers (they cannot push changes), but still processes
     *metadata* (heads, subscriptions) so the read-only peer stays informed.
   - Decide with Nick: does a read-only peer still *request* a doc
     (`Request`) to pull it, or only receive passive `Sync`? Recommendation:
     a read-only peer may `Request` (to pull) but the response carries
     receive-only data; it must not generate `Sync` data with local changes.

2. **Cap permission → `SyncMode` mapping.** The handshake (003) sets
   `SyncMode` from the presented cap's `AutomergeSyncPermission`:
   `Write → ReadWrite`, `Read → ReadOnly`. A peer holding both a `Read` and a
   `Write` cap for the same doc → `ReadWrite` (max).

3. **`AutomergeHandle` read-only enforcement (T4 §5).** `Entity<T:
   GatedAutomergable>`'s `set`/`update` refuse local writes when the handle's
   cap is `Read`-only, with a visible error — so a read-only peer can't even
   *attempt* a push locally. (Plumbing for this lands in 006; this ticket
   activates it.)

4. **Visible state.** A read-only sync state is surfaced (UI/log) so it's
   clear the peer is receive-only, not broken.

## Out of scope

- A read-only *UI* for documents (app-level, future).
- Partial-write scopes (e.g. read-only on a path subrange) — Meadowcap
  supports area attenuation; whether to expose it is a later question.

## Design questions for Nick

1. **Read-only peer `Request` semantics.** See scope §1. Recommendation: a
   read-only peer may `Request` to pull but must not push `Sync` data.
   Confirm.

2. **Mixed caps.** A peer holds `Read` for doc D1 and `Write` for D2.
   `SyncMode` is per-`(connection, document)`, not per-connection. Confirm
   (this follows from 004's per-doc gating).

3. **When to start this.** Explicitly after the first deliverable (001–006)
   is verified. Confirm it stays off the critical path.

## Acceptance criteria

- A peer with a `Read` cap for D receives D's changes but cannot push; its
  `Sync` data is dropped with a visible per-doc error.
- A peer with a `Write` cap behaves as today (`ReadWrite`).
- `AutomergeHandle::set`/`update` on a read-only handle fails visibly.
- A peer holding `Read` for D1 and `Write` for D2 syncs D2 both ways and D1
  receive-only.

## Verification

- Two-`Endpoint` in-process test: A mints `Read` for B for D; B receives A's
  changes; B's push to D is refused; B's `set`/`update` on D's handle fails
  visibly.
- `cargo fmt`; no clippy unless Nick asks.
- State EXECUTED vs RELAYED.

## Records

- Update `ARCHITECTURE.md` and the gating section.
- Move design Q1–Q3 rulings to `DECISIONS.md`.