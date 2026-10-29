# 008 — Read-only `SyncMode` (fast follow)

**Status:** DRAFT (deferred — explicit fast follow per ruling #5)
**Depends on:** 001 (`SyncMode` plumbing lands with the engine core)

## Why

The first deliverable is **write-gated** (ruling #5): no `Write` cap → no sync
at all. The natural follow-up is **read-only sync**: a peer with a `Read` cap
may *receive* changes but must not *push*. This needs a per-connection
`SyncMode::ReadOnly` that the actor honors on both the outgoing and incoming
paths. `galvanized-sync` is written from scratch with `SyncMode` plumbed in
from the start (001), so this ticket is a **policy activation**, not a
re-arch — samod's stats-only `SyncDirection` limitation does not carry over.

## Reference points

- `galvanized/docs/ai/tickets/001-willow-automerge-sync/001-sync-engine-core.md`
  — the `SyncMode` plumbing (outgoing + incoming paths) lands there.
- `galvanized/crates/galvanized-cap/src/lib.rs:42` — `WillowRead` / the
  `AutomergeSyncPermission::Read` (002) that selects `ReadOnly`.

## Scope

1. **`SyncMode::ReadOnly` enforcement.** With the plumbing from 001:
   - The actor's generate path skips generation for `ReadOnly` connections
     (a read-only peer sends no sync messages outward).
   - The actor's receive path drops incoming `Sync`/`Request` *data* from
     `ReadOnly` peers (they cannot push changes), but still processes
     *metadata* (heads, subscriptions) so the read-only peer stays informed.
   - Decide with Nick: does a read-only peer still `Request` a doc (to pull),
     or only receive passive `Sync`? Recommendation: a read-only peer may
     `Request` to pull; it must not generate `Sync` data carrying local
     changes.

2. **Cap permission → `SyncMode` mapping.** The handshake (004) sets
   `SyncMode` from the presented cap's `AutomergeSyncPermission`:
   `Write → ReadWrite`, `Read → ReadOnly`. A peer holding both `Read` and
   `Write` for the same doc → `ReadWrite` (max). `SyncMode` is
   per-`(connection, document)`, not per-connection (a peer may be read-only
   on D1 and read-write on D2).

3. **`AutomergeHandle` read-only enforcement (T4 §5).** `Entity<T:
   GatedAutomergable>`'s `set`/`update` refuse local writes when the handle's
   cap is `Read`-only, with a visible error — so a read-only peer can't even
   *attempt* a push locally. (Plumbing lands in 007; this ticket activates it.)

4. **Visible state.** A read-only sync state is surfaced (UI/log) so it's
   clear the peer is receive-only, not broken.

## Out of scope

- A read-only *UI* for documents (app-level, future).
- Partial-write / area-attenuated scopes (Meadowcap supports area attenuation;
  whether to expose it is a later question).

## Design questions for Nick

1. **Read-only peer `Request` semantics.** See scope §1. Recommendation: a
   read-only peer may `Request` to pull but must not push `Sync` data.
   Confirm.

2. **Mixed caps per doc.** Confirm `SyncMode` is per-`(connection, document)`
   (a peer with `Read` for D1 and `Write` for D2 syncs D2 both ways, D1
   receive-only). Follows from 005's per-doc gating.

3. **When to start.** Explicitly after the first deliverable (001–007) is
   verified. Confirm it stays off the critical path.

## Acceptance criteria

- A peer with a `Read` cap for D receives D's changes but cannot push; its
  `Sync` data is dropped with a visible per-doc error.
- A peer with a `Write` cap behaves as today (`ReadWrite`).
- `AutomergeHandle::set`/`update` on a read-only handle fails visibly.
- A peer holding `Read` for D1 and `Write` for D2 syncs D2 both ways and D1
  receive-only, over one connection.

## Verification

- Two-`Endpoint` in-process test: A mints `Read` for B for D; B receives A's
  changes; B's push to D is refused; B's `set`/`update` on D's handle fails
  visibly. Mixed-cap test: B has `Read` for D1 + `Write` for D2 over one
  connection → D2 both ways, D1 receive-only.
- `cargo fmt`; no clippy unless Nick asks.
- State EXECUTED vs RELAYED.

## Records

- Update `ARCHITECTURE.md` and the gating section.
- Move design Q1–Q3 rulings to `DECISIONS.md`.