# T7 — GPUI `CapCell`-aware view wrapper (auto-hide on lock/timeout)

**Status:** DRAFT
**Depends on:** T1 (the `CapCell<T, P, S>` primitive in `strict-cap`)

> **Note:** Spawned at T1 refinement (Nick, this session) to keep the
> "blank gated content on lock" story from being lost. This is a DRAFT
> to refine later — scope/design questions are deliberately open.

## Why

T1 lands the `CapCell<T, P, S>` caching wrapper: an `Option`-like holder
that yields `Some(&T)` only while its capability is valid (not expired,
not revoked, target in scope). On lock or timeout the inner value
becomes inaccessible. That protects the *data*, but not the *view*: a UI
that rendered gated content before lock would keep showing stale pixels
unless it re-renders in response to the capability invalidating.

This ticket builds the GPUI binding that makes `CapCell`-gated views
**automatically hide / redact / invalidate on lock or timeout**, so
every app plugin that gates content behind a capability gets the
"blank on lock" behavior for free — the same way locking a password
manager blanks every password field at once.

## Current state (anchors — re-derive before leaning on)

- T1 (REFINED) defines `CapCell<T, P, S>` in `strict-cap`, instantiated
  by `galvanized-cap` with the Willow permissions/scopes. The vault
  holds unlocked content behind it.
- GPUI rendering in this repo: app plugins implement `Render`
  (`crates/plugin-contacts/src/lib.rs:219`, etc.) and use
  `cx.notify()` to trigger re-render. `cx.observe(entity, …)` is the
  pattern for re-rendering on entity change.
- Theming via `gpui_component::ActiveTheme` + `cx.theme()` (never
  Zed's `ActiveTheme` — see `CONVENTIONS.md` / `KNOWN_TRAPS.md`).
- The vault's lock action (T1 scope §5) revokes every live session
  capability. The mechanism for "something changed" exists; this
  ticket is about turning that into a view-layer event.

## Scope (provisional — to refine with Nick)

1. **A `CapCell` invalidation signal reachable from GPUI.** `CapCell`
   today (T1) is a plain `Option`-like accessor; it has no notification
   stream. To drive UI re-render, the cell needs to emit an event (or
   expose an observable flag) when its capability is revoked/expired and
   when it's refreshed (sliding-window refresh — T1 §3). Decide the
   shape with Nick: (a) `CapCell` holds a GPUI `Observer`/emits on a
   `flume`; (b) `CapCell` is wrapped in an `Entity<CapCell<…>>` so
   `cx.observe` works; (c) a separate `CapCellHandle` the view holds.
   Recommendation: (b) — `Entity<CapCell<T, P, S>>` slots into the
   existing `cx.observe` / `cx.notify` pattern with no new primitive.

2. **A view wrapper element** (working name `cap_cell_view` / a
   `CapView` component) that takes a `CapCell` (or `Entity<CapCell>`)
   and two render branches: `Some` (the gated content) and `None` (the
   redacted/empty state). The wrapper re-renders automatically when the
   cell invalidates. Decide with Nick whether this is a gpui-component
   widget or a small in-repo helper.

3. **Honest redacted state.** When the cell is `None` due to lock, the
   view shows an honest state ("Locked" / redacted placeholder), not a
   lying placeholder that implies the data is present. Per
   `CONVENTIONS.md`: honest empty states are fine; lying ones are not.

4. **At least one consumer migrated** as the reference (Contacts is the
   candidate — its contact list is gated content). Confirm with Nick
   which app to migrate first.

## Out of scope (provisional)

- The `CapCell` primitive itself (T1).
- Policy for *which* content is gated (that's per-app; T7 provides the
  mechanism).
- Animations / transitions on redact (nice-to-have; defer unless Nick
  wants it).

## Design questions for Nick (to refine later)

1. **Invalidation signal shape** — `Entity<CapCell<…>>` + `cx.observe`
   (my rec) vs. a `flume`/event stream vs. a `CapCellHandle`. Decide at
  refinement time.
2. **Wrapper as gpui-component widget vs. in-repo helper** — does this
   belong in `gpui-component` (reusable beyond Galvanized) or in
   `galvanized-core`? Recommendation: `galvanized-core` for now;
   promote to gpui-component if it proves general.
3. **Redacted-state look** — blank/empty, a "🔒 Locked" chip, or a
   blurred placeholder? Recommendation: a clear "Locked" state, no
   fake-content blur (blur can leak shape). Confirm at refinement.
4. **First consumer** — Contacts contact list, or a simpler synthetic
   view first? Recommendation: a synthetic view to prove the
   mechanism, then Contacts.

## Acceptance criteria (provisional)

- A view bound to a `CapCell` re-renders to the redacted state within
  one frame of the cell being revoked/expired.
- Locking the application blanks every `CapCell`-gated view on screen
  (no stale pixels, no lying placeholders).
- Unlocking re-populates the gated views (round-trip works).

## Verification

- `cargo fmt && cargo clippy && cargo test` for `galvanized-core`
  (or wherever the wrapper lives) and the migrated consumer.
- `cargo run`: open a gated view, lock the app, confirm the view
  redacts; unlock, confirm it re-populates.