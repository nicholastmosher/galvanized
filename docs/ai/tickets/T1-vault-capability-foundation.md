# T1 — Vault as authority root + capability foundation

**Status:** DRAFT
**Depends on:** —
**Blocks:** T2, T4 (capability contract must settle first)

## Why

Every authority in Galvanized flows from unlocking a Vault with a
password. The Vault holds Willow keys (subspace secrets = Profile
identifiers, namespace secrets = filesystem auth/authz roots). All
operations should happen within the scope of a Profile and be gated by
explicit capabilities. Capabilities are manifested twofold:

1. **Signed data objects** — portable, transferable authority to perform
   an action (the existing Willow `WriteCapability`/read capabilities,
   minted from namespace + subspace secrets). This is the form that
   crosses the peer boundary.
2. **Typed programmatic tokens** — in-process authority that unlocks
   specific APIs. This is `strict-cap`'s `StrictCap<P, S>`: revocable,
   timed, scoped. This is the form that gates local code.

This dual form must be **foundational to all higher-level plugins**.
Higher-level plugins should not be able to reach Willow keys or perform
a write without holding (and proving) a capability.

## Current state (anchors — re-derive before leaning on)

- Vault: `crates/plugin-vault/src/lib.rs`. `init` mints a `capsec::root()`
  `VaultAccess` capability, spawns a `VaultActor` on a background
  thread; all reads/writes go through `VaultToken`s. `cx.vaults()` is
  the extension-trait API (`VaultsCx`).
- Vault domain model: `crates/plugin-galvanized/src/domain/vault.rs`.
  `Vault` caches `unlocked_vault`/`unlocked_spaces`/`unlocked_profiles`
  as plain `Option`s. **TODO at line 41**: the timed/revocable
  capability wrapper is not built.
- Profile: `crates/plugin-galvanized/src/domain/profile.rs`. `Profile`
  holds a `Subspace` (whose `secret_todo_consider_access_control`
  (`crates/plugin-willow/src/lib.rs:116`) is the known-open seam — keys
  currently exposed as plain struct fields).
- Willow capability minting: `crates/plugin-willow/src/lib.rs:182` —
  `WriteCapability` minted from namespace + subspace secrets inside
  `create_entry`. There is no "just write directly" path today; do not
  add one.
- `strict-cap` crate: `crates/strict-cap/src/lib.rs`. `StrictCap<P, S>`
  wraps `capsec::Cap<P>` with revocation (Arc<AtomicBool>), TTL
  (`Instant`), and `Scope` check. `StrictSendCap` is the `Send+Sync`
  form via `make_send()`. `StrictRevoker` is the cross-thread revoker.
- `capsec` is the underlying capability system (used by the vault).

## Scope

1. **Standardize the typed-token capability layer** on top of the vault.
   Define `Permission` marker types for the operation classes the
   platform needs (e.g. `WillowRead`, `WillowWrite`, `ProfileAccess`,
   maybe `AutomergeSync`). Define a `Scope` type that captures the
   target (e.g. a Willow namespace + subspace + path prefix, or a
   profile id). These live in a crate reachable by both `plugin-willow`
   and `plugin-vault` — likely `strict-cap` itself, or a new
   `galvanized-cap` crate if `strict-cap` should stay generic. Decide
   the home with Nick.

2. **Mint capabilities on vault unlock.** Unlocking a Vault (and a
   Profile within it) produces `StrictCap` tokens for the keys the
   holder is now entitled to use, rather than handing out raw
   subspace/namespace secrets. The `secret_todo_consider_access_control`
   seam gets replaced with a capability check: code that needs the
   subspace secret must present a `StrictCap<WillowWrite, SubspaceScope>`
   and call `try_cap(target)` to obtain the underlying `Cap`, which is
   then used to mint the Willow `WriteCapability`.

3. **Build the timed/revocable caching wrapper** (the `vault.rs:41`
   TODO) so unlocked Vault content is held behind a capability that
   expires/revokes — the lock-on-timeout design from dev log 2026-05-19.
   This is the model for "defense in depth" per `CONVENTIONS.md`.

4. **Make capabilities the gate for all Willow writes** in
   `plugin-willow`. `create_entry`/future write paths take a
   `StrictCap<WillowWrite, …>` rather than (or in addition to) minting
   the `WriteCapability` from raw secrets internally. The signed-data
   form (Willow `WriteCapability`) is still minted for the actual Willow
   store operation, but the *right to mint it* is gated by the typed
   token.

5. **Document the dual-form model** in
   [`docs/ai/memory/GLOSSARY.md`](../memory/GLOSSARY.md) (capability:
   signed-data form vs typed-token form) and update
   `ARCHITECTURE.md`'s "capability flow" section once the wiring lands.

## Out of scope

- The on-disk Willow store switch (separate open question; see
  `OPEN_QUESTIONS.md` "Willow persistence"). The capability layer works
  against `MemoryStore` for now.
- The `Willowize`/`EntryHandle` seam (T2) — but T2 depends on the
  contract this ticket settles.
- Capability *sharing* between peers (that's T4's concern for
  automerge, and the contacts handshake's concern for contact
  tickets).

## Design questions for Nick

1. **Where do the `Permission`/`Scope` types live?** Options: (a) extend
   `strict-cap` with Galvanized-specific permission types; (b) new
   `galvanized-cap` crate that re-exports `strict-cap` primitives and
   adds the domain types. Recommendation: (b) — keeps `strict-cap` as a
   reusable mechanism crate and collects domain perms in one place.

2. **What are the operation classes / `Permission` types?** Propose
   `WillowRead`, `WillowWrite`, `ProfileAccess` to start; add more as
   the platform grows. Is `AutomergeSync` a distinct permission or does
   it reduce to `WillowRead`/`WillowWrite` on a capability object?

3. **Capability TTL defaults and the lock-on-timeout policy.** Dev log
   2026-05-19 names timed/revocable capabilities; what are the concrete
   defaults (e.g. 15 min unlock TTL, refresh on activity)? Is lock
   manual + on-timeout, or purely timeout?

4. **Scope shape.** Is the `Scope` a Willow path prefix
   (`namespace_id + subspace_id + path_prefix`), or finer/coarser?
   Recommendation: path-prefix scoped, so a capability can be attenuated
   to a subtree.

5. **Relationship to the existing `capsec::root()` + `VaultAccess`.** Do
   we keep `capsec::root()` as the ultimate root and derive everything
   else, or does the password-unlock become the minting root for
   session capabilities? Recommendation: keep `capsec::root()` as the
   device root; password-unlock mints session `StrictCap`s from it.

## Acceptance criteria

- Unlocking a Vault yields `StrictCap<…>` tokens; no Willow key is held
  as a plain struct field reachable by arbitrary code (the
  `secret_todo_consider_access_control` seam is closed or explicitly
  gated behind a capability).
- A Willow write without a valid `StrictCap<WillowWrite, scope>` is
  impossible through the public API and produces a visible error
  (`CapSecError::Expired`/`Revoked`/scope-mismatch), not a silent skip.
- Lock-on-timeout works: an expired/revoked capability cannot be used to
  mint a Willow `WriteCapability`; the UI surfaces the re-lock.
- `cargo fmt` / `cargo clippy` clean on touched crates; `cargo test`
  passes; a manual drive of unlock → write → lock → attempted write
  shows the refusal in the UI.

## Verification

- `cargo fmt && cargo clippy && cargo test` for `plugin-vault`,
  `plugin-willow`, `strict-cap` (and any new cap crate).
- `cargo run`: create/unlock a vault, create a Profile, attempt a
  Willow write; lock the vault; confirm the next write is refused and
  surfaced (status chip / notification), not silent.

## Records

- Update `GLOSSARY.md` with the dual-form capability model.
- Move the "Capability-powered caching wrapper" open question in
  `OPEN_QUESTIONS.md` to `DECISIONS.md` once Nick rules on TTL/scope.
- Update `ARCHITECTURE.md` "capability flow" section.