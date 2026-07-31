# T1 — Vault as authority root + capability foundation

**Status:** REFINED
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

1. **New `galvanized-cap` crate** for the domain `Permission` and
   `Scope` types, keeping `strict-cap` a general-purpose mechanism
   crate. `galvanized-cap` re-exports the `strict-cap` primitives it
   builds on (`StrictCap`, `StrictSendCap`, `StrictRevoker`, the new
   caching wrapper below) and adds the Galvanized-specific domain types.
   `plugin-willow` and `plugin-vault` both depend on `galvanized-cap`.

2. **Begin with `WillowRead` and `WillowWrite` `Permission` types.**
   (`ProfileAccess` and `AutomergeSync` are *not* in scope for T1 — see
   the note in "Design questions" §2 on why `ProfileAccess` was
   proposed and why it's deferred.) The `Scope` is **namespace +
   subspace + path prefix**, and the **target-string encoding follows
   4. **Scope shape.** — RESOLVED (Nick, this session).
   `NamespaceId + SubspaceId + path prefix`. **Encoding follows the
   Willow URI spec** (`https://willowprotocol.org/specs/uris/`):
   `willow://<namespace_id>.<subspace_id>/<path/components>`, namespace
   first, ids host-safe percent-encoded per the spec's
   `URIEncodeNamespaceId` / `URIEncodeSubspaceId`. The concrete
   `URIEncode*` relations are reused from the `willow25` crate's
   instantiation (define per the spec only if the crate doesn't expose
   them).
   **Matching is structured, not string-prefix.** `WillowScope` holds
   `(Option<NamespaceId>, Option<SubspaceId>, Option<PathPrefix>)`
   with `None` = wildcard, and `capsec::Scope::check` parses the target
   as a Willow URI and matches structurally. Nick confirmed (this
   session) this is acceptable even though the spec's
   `namespace.subspace` ordering means the earlier "same Profile shares
   a string prefix" property no longer holds — structured matching with
   wildcards preserves the attenuation capability (profile-wide cap,
   per-namespace-under-profile cap) and is robust against
   percent-encoding boundaries and dot-segments. Sub-task if needed:
   if `willow25` doesn't expose URI encoders, implement the
   `URIEncode*` relations per the spec as part of T1.

3. **General-purpose caching wrapper in `strict-cap`.** A new type
   `CapCell<T, P, S>` that behaves like an `Option`: it holds an inner
   `T` and only yields `Some(&T)` iff the capability is currently valid
   (not expired, not revoked, target in scope). On lock/timeout/revoke
   the inner value becomes inaccessible via the wrapper. This is the
   mechanism that gates *content held in memory* behind a capability —
   distinct from `StrictCap::try_cap`, which gates the *right to mint a
   Willow cap*. `strict-cap` stays domain-agnostic; `galvanized-cap`
   instantiates it with the Willow permissions/scopes and holds the
   vault's unlocked content (the `vault.rs:41` TODO) behind it.
   **Future (out of scope for T1, but design the wrapper to enable it):**
   GPUI UI wrappers that bind to a `CapCell` and automatically
   hide/invalidate the gated view on lock/timeout — so any content
   gated behind a capability is blanked from the screen on lock. T1
   lands the wrapper primitive; the UI binding is a follow-up ticket.

4. **Mint capabilities on vault unlock, guarded by `capsec::root()`.**
   `capsec::root()` is the device-level root that can mint any
   capability; it stays owned by the vault and is **only invoked when a
   password unlock is provided**. Unlocking a Vault (and a Profile
   within it) produces `StrictCap<Willow{Read,Write}, WillowScope>`
   tokens for the keys the holder is now entitled to use, rather than
   handing out raw subspace/namespace secrets. The
   `secret_todo_consider_access_control` seam gets replaced with a
   capability check: code that needs the subspace secret must present a
   `StrictCap<WillowWrite, WillowScope>` and call `try_cap(target)` to
   obtain the underlying `Cap`, which is then used to mint the Willow
   `WriteCapability`.

5. **Timed + manually revocable capabilities.** Default TTL **15
   minutes**, configurable per mint (and globally configurable later).
   **All capabilities are manually revokable** — locking the
   application (like locking a password manager) revokes the session
   capabilities via their `StrictRevoker`s. The vault's lock action
   calls revoke on every live capability it minted. Combined with the
   `CapCell` caching wrapper (scope §3), revocation blanks any
   in-memory content gated behind those capabilities — the mechanism
   that enforces "content gated behind a capability is blanked from
   the screen on lock or timeout."

6. **Make capabilities the gate for all Willow writes** in
   `plugin-willow`. `create_entry`/future write paths take a
   `StrictCap<WillowWrite, WillowScope>` rather than (or in addition
   to) minting the `WriteCapability` from raw secrets internally. The
   signed-data form (Willow `WriteCapability`) is still minted for the
   actual Willow store operation, but the *right to mint it* is gated
   by the typed token.

7. **Document the dual-form model** in
   [`docs/ai/memory/GLOSSARY.md`](../memory/GLOSSARY.md) (capability:
   signed-data form vs typed-token form; `CapCell` for in-memory
   content) and update `ARCHITECTURE.md`'s "capability flow" section
   once the wiring lands.

## Out of scope

- The on-disk Willow store switch (separate open question; see
  `OPEN_QUESTIONS.md` "Willow persistence"). The capability layer works
  against `MemoryStore` for now.
- The `Willowize`/`EntryHandle` seam (T2) — but T2 depends on the
  contract this ticket settles.
- Capability *sharing* between peers (that's T4's concern for
  automerge, and the contacts handshake's concern for contact
  tickets).
- **`ProfileAccess` and `AutomergeSync` permissions** — deferred (see
  Design questions §2). T1 ships `WillowRead`/`WillowWrite` only.
- **The GPUI UI wrappers that auto-hide/blank `CapCell` content.** T1
  lands the `CapCell` primitive and uses it to hold vault content; the
  view-layer binding (a `CapCell<…>`-aware element that re-renders to
  an empty/redacted state on revoke) is a follow-up ticket (T7). The
  mechanism for blanking on lock is in place via §3 + §5; the UI sugar
  on top is not.

## Design questions for Nick

1. **Where do the `Permission`/`Scope` types live?** — RESOLVED (Nick,
   this session). New `galvanized-cap` crate; `strict-cap` stays
   general-purpose. A general-purpose caching wrapper (`CapCell`) is
   added to `strict-cap`.

2. **`ProfileAccess` — what it was, and why deferred.** I proposed
   `ProfileAccess` as a permission gating "act as this Profile" —
   i.e. the right to use the subspace secret to *sign p2p envelopes*
   and *open connections under that Profile's identity*, distinct from
   the right to *write Willow entries* as that subspace. The subtlety:
   today both operations use the **same subspace secret**, so a single
   `WillowWrite` cap would unlock both — unless we split the secret's
   uses into two permissions. My recommendation: **do not split now**.
   Ship `WillowRead`/`WillowWrite` only; if we later want to separate
   "sign as Profile" (p2p identity) from "write Willow as subspace"
   (data), add `ProfileAct` then. `AutomergeSync` likewise reduces to
   `WillowRead`/`WillowWrite` on the shared object's path (T4), so it
   does not need its own permission. **Confirmed (Nick, this session):
   ship `WillowRead` + `WillowWrite` only, defer the rest.**

3. **Capability TTL defaults and the lock-on-timeout policy.** —
   RESOLVED (Nick, this session). Default 15 min, configurable per mint
   (globally configurable later). **All capabilities manually
   revokable**; the app-lock action revokes every live session
   capability. Combined with the `CapCell` wrapper, lock/timeout
   blanks gated content from memory (and, via the follow-up UI wrapper
   T7, from the screen). **TTL refresh: sliding window — confirmed
   (Nick, this session).** User activity refreshes the capability (on
   successful `try_cap`) so a cap in active use doesn't expire
   mid-operation and interrupt the user at a hard 15m boundary.

4. **Scope shape.** — RESOLVED (Nick, this session).
   `NamespaceId + SubspaceId + path prefix`. **Encoding follows the
   Willow URI spec** (`https://willowprotocol.org/specs/uris/`):
   `willow://<namespace_id>.<subspace_id>/<path/components>`, namespace
   first, ids host-safe percent-encoded per the spec's
   `URIEncodeNamespaceId` / `URIEncodeSubspaceId`. The concrete
   `URIEncode*` relations are reused from the `willow25` crate's
   instantiation (define per the spec only if the crate doesn't expose
   them).
   **Matching is structured, not string-prefix.** `WillowScope` holds
   `(Option<NamespaceId>, Option<SubspaceId>, Option<PathPrefix>)`
   with `None` = wildcard, and `capsec::Scope::check` parses the target
   as a Willow URI and matches structurally. Nick confirmed (this
   session) this is acceptable even though the spec's
   `namespace.subspace` ordering means the earlier "same Profile shares
   a string prefix" property no longer holds — structured matching with
   wildcards preserves the attenuation capability (profile-wide cap,
   per-namespace-under-profile cap) and is robust against
   percent-encoding boundaries and dot-segments. Sub-task if needed:
   if `willow25` doesn't expose URI encoders, implement the
   `URIEncode*` relations per the spec as part of T1.

5. **Relationship to `capsec::root()` + `VaultAccess`.** — RESOLVED
   (Nick, this session). `capsec::root()` is the device root, owned by
   the vault, and only invoked to mint when a password unlock is
   provided. Password-unlock mints session `StrictCap`s (and the
   `CapCell` holders) from that root.

6. **Permission set for T1.** — RESOLVED (Nick, this session). Ship
   `WillowRead` + `WillowWrite` only. Defer `ProfileAccess` and
   `AutomergeSync` (see Q2 for the rationale; add `ProfileAct` later
   only if we need to separate "sign as Profile" from "write Willow as
   subspace").

7. **GPUI `CapCell`-aware view wrapper.** — RESOLVED (Nick, this
   session). Spawn a follow-up ticket (T7) now, refine later. T1 lands
   the `CapCell` primitive; T7 builds the view-layer binding that
   auto-hides/redacts gated content on lock/timeout.

## Acceptance criteria

- `galvanized-cap` crate exists with `WillowRead`, `WillowWrite`, and
  `WillowScope` (= ns + subspace + path prefix); `strict-cap` gains the
  general-purpose `CapCell<T, P, S>` caching wrapper.
- Unlocking a Vault (with password) yields `StrictCap<…>` tokens; no
  Willow key is held as a plain struct field reachable by arbitrary
  code (the `secret_todo_consider_access_control` seam is closed or
  explicitly gated behind a capability).
- A Willow write without a valid `StrictCap<WillowWrite, scope>` is
  impossible through the public API and produces a visible error
  (`CapSecError::Expired`/`Revoked`/scope-mismatch), not a silent skip.
- `CapCell<T, …>` yields `Some` only while the capability is valid;
  after revoke or timeout it yields `None`. Vault content held in a
  `CapCell` becomes inaccessible on lock.
- **Lock blanks gated content:** locking the application (or a
  capability timing out) revokes the session capabilities; any content
  held behind a `CapCell` is no longer accessible via the wrapper.
  (The view-layer auto-blank is a follow-up; T1's bar is that the
  *data* is inaccessible, not that the *view* is hidden — though the
  UI should at minimum surface the re-lock state per `CONVENTIONS.md`.)
- `capsec::root()` is only invoked on a password-unlock path; no other
  code path can mint a top-level capability.
- `cargo fmt` / `cargo clippy` clean on touched crates; `cargo test`
  passes; a manual drive of unlock → write → lock → attempted write
  shows the refusal (and the gated content is inaccessible), not a
  silent skip.

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