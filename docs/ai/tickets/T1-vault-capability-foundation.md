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

---

## Iteration 2 — 2026-08-08 (spec review after partial implementation)

> A review of the partially-implemented T1 surfaced that the capability
> *check* exists but the *secret* still leaks out of the key store into
> long-lived application state. This iteration tightens the spec so keys
> are not merely cap-gated at access time but **non-extractable** — the
> raw secret never leaves the custodian. Decisions below are
> `RATIFIED (Nick, 2026-08-08)` where marked; a few are `PROPOSED` and
> still open. Append-only: earlier scope/Q&A above still stand unless a
> later entry explicitly supersedes.

### The leak that motivated this iteration

`plugin-contacts::init` (`crates/plugin-contacts/src/lib.rs:108-143`) does
present a `WillowWrite` cap to `WillowKeyStore::subspace_secret_for_signing`
— but only **once**, at Profile-construction time. The returned
`SubspaceSecret` is then **cloned** (`local_key.clone()`) and handed to
`ContactsProtocol::new`, which stores it as
`ContactsProtocolState.local_profile_key: SubspaceSecret`
(`crates/plugin-contacts/src/protocol.rs:81`) for the **lifetime of the
protocol**. After that the protocol never re-presents a capability; locking
the vault revokes the `write_cap` but the protocol still holds the raw key
and can keep signing. The cap check became a one-time door the secret walked
out of.

`local_profile_key` is used in three places (`protocol.rs:330`, `:373`,
`:427`):
- `:330` (`connect`) — the only real *signing* site (hands the clone to
  `SignedEnvelope::new`).
- `:373` (`try_receive_frame`) and `:427` (`try_handle_accept_connect_contact`)
  — only call `corresponding_subspace_id()`, i.e. they need the **public**
  `SubspaceId`, not the secret.

A wider sweep found the same class of leak in several other places (see
"Leak inventory" below). The common root: `WillowKeyStore`'s accessors
return **owned `SubspaceSecret`/`NamespaceSecret` clones**, and the store is
`#[derive(Clone)]`, so secrets get duplicated and held ungated across the
app. T1's "no Willow key held as a plain struct field" acceptance bar is
not met as long as a copyable secret can exit the store.

### Leak inventory (anchors — re-derive before leaning on)

1. **`ContactsProtocolState.local_profile_key: SubspaceSecret`** — held
   forever, no re-check (`plugin-contacts/src/protocol.rs:81`, consumed at
   `:330`). Closed by §8 below.
2. **`WillowKeyStore` is `#[derive(Clone)]`** (`galvanized-cap/src/lib.rs:343`).
   `Profile::key_store()` (`plugin-galvanized/src/domain/profile.rs:121`)
   returns a clone, so every plugin that asks gets a full duplicate of every
   namespace/subspace secret in its own memory. The cap still gates *use*,
   but the secrets are duplicated in memory. Closed by §9 below.
3. **`subspace_secret_for_signing` / `mint_write_authority` return owned
   `SubspaceSecret` (clones)** (`galvanized-cap/src/lib.rs:398, :430`).
   Even on-demand callers (`Contacts::copy_contact_ticket`
   `plugin-contacts/src/lib.rs:334`, `Contact::save` `:665`, and the
   protocol) receive a transient *copyable* `SubspaceSecret`, sign, and drop
   it. On-demand in *time* but not non-extractable. Closed by §10 (and the
   write-path sub-question in §10.4).
4. **`SignedEnvelope::new_at` / `ContactTicket::new` take `SubspaceSecret`
   by value** (`plugin-contacts/src/protocol.rs:486`, `lib.rs:721`). They
   only call `try_sign` + read `corresponding_subspace_id()`; they never need
   to *own* or persist the key. Taking it by value is what makes the copy-leak
   natural. Closed by §10 (they take the borrowing `Signer` instead).
5. **`Subspace` / `Namespace` carry the raw secret as a plain (private)
   field** (`plugin-willow/src/lib.rs:82-116`) and are held by `Space`,
   `Profile`/`ProfileData`, and **serialized into `VaultContent`**. The
   `WillowKeyStore` is built from *clones* of these (`extract_key_store` /
   `extend_key_store`, `plugin-willow/src/lib.rs:127-158`). So the secrets
   live in two forms: the cap-gated store **and** the original
   `Subspace`/`Namespace` values riding in runtime `Profile`/`Space`
   entities (encrypted at rest in the vault DB, but plain in memory,
   ungated). Closed by §11 below.

Other app plugins (`plugin-chat`, `plugin-calendar`, `plugin-files`) hold no
`SubspaceSecret`/`NamespaceSecret`/`key_store` directly today — only
`plugin-contacts` reaches into the key store.

### §8 (RATIFIED — Nick, 2026-08-08): `ContactsProtocol` holds the ID + the key store, not the secret

`ContactsProtocolState` replaces `local_profile_key: SubspaceSecret` with:

- `local_profile_id: SubspaceId` — the public id, used at `:373` and `:427`
  (which only need `corresponding_subspace_id`, now just `local_profile_id`).
- `key_store: Entity<WillowKeyStore>` (per §9) and `write_cap:
  StrictSendCap<WillowWrite, WillowScope>` — to re-present the cap **on each
  sign** inside `connect`, via the §10 `Signer`.

Locking the vault now actually breaks the protocol's ability to sign
(revoked cap → visible `CapSecError` on the next `connect`), instead of
silently continuing with a retained key.

### §9 (RATIFIED — Nick, 2026-08-08): `WillowKeyStore` is not `Clone`; it is held as an `Entity`

`WillowKeyStore` loses `#[derive(Clone)]`. The single in-memory home of the
secrets is one `Entity<WillowKeyStore>`; `Profile::key_store()` returns a
**handle clone** (`Entity<WillowKeyStore>`), which shares the underlying
store, not a secret-duplicating value clone. Nick's lean: `Entity` over
`Arc<Mutex<…>>` so access is serialized by the GPUI entity system
(`entity.update` / `entity.read`) and we avoid hand-rolled synchronization
for `extend_key_store` / new-secret insertion.

Implications / things to honor during implementation:

- `Vault` holds `Option<Entity<WillowKeyStore>>`; `extract_key_store` /
  `extend_key_store` (`plugin-willow/src/lib.rs:127-158`) move secrets in via
  `entity.update(cx, |store, _cx| …)` rather than `&mut WillowKeyStore`.
- On **lock**, `Vault::lock` (`plugin-galvanized/src/domain/vault.rs:177`)
  already does `self.key_store = None` and `unlocked_profiles.clear()`. With
  `Entity` wrapping, dropping the Vault's handle plus clearing the Profile
  entities (which hold the only other handles) is what drops the inner
  `WillowKeyStore` and thus the secrets from memory — the
  capability-**and**-in-memory-expiry defense-in-depth from the
  2026-05-19 ruling. Verify at implementation that no stray handle keeps the
  entity alive past lock.
- `plugin_willow::create_entry` (`plugin-willow/src/lib.rs:209`) currently
  takes `key_store: &WillowKeyStore`; it becomes `Entity<WillowKeyStore>`
  and the mint/sign happens inside an `update` (see §10.4).

### §10 (RATIFIED in direction — Nick, 2026-08-08; interface is PROPOSED): non-extractable signing via a borrowing, non-`Clone` `Signer`

Option A alone (lend `&SubspaceSecret`) is **insufficient** — `SubspaceSecret`
itself is `Clone`, so handing a caller a direct `&SubspaceSecret` is still a
leak (they can `.clone()` it). We combine A + B: the key store lends a
**non-`Clone`, non-`Copy`, lifetime-bound** `Signer<'store>` whose only API is
signing and reading the corresponding public id, and whose `'store` lifetime
prevents it from being moved out / persisted beyond the store borrow.

`PROPOSED` interface (names bikeshedded, in `galvanized-cap`):

```rust
/// Borrowing handle to a subspace secret, lent by `WillowKeyStore` for one
/// signing operation. Not `Clone`, not `Copy`; the `'store` lifetime ties it
/// to the store borrow so it cannot escape the cap-check scope.
pub struct Signer<'store> {
    secret: &'store mut SubspaceSecret,
}

impl<'store> Signer<'store> {
    /// Public identity this signer signs as. No secret material exposed.
    pub fn corresponding_subspace_id(&self) -> SubspaceId {
        self.secret.corresponding_subspace_id()
    }

    /// Sign `msg` in place; the borrow ends when the `Signer` drops. The
    /// returned `SubspaceSignature` is owned and carries no secret.
    pub fn try_sign(&mut self, msg: &[u8])
        -> Result<SubspaceSignature, WillowKeyError>;
}
// Intentionally NO `Clone`/`Copy` impls.

impl WillowKeyStore {
    /// Lends a `Signer` for `subspace_id`, gated by a valid `WillowWrite` cap.
    /// The cap is checked *each call* (sliding-window refresh on success), so
    /// a revoked/expired cap fails visibly here, not at a one-time extraction
    /// that silently keeps working afterward.
    pub fn signer<'store>(
        &'store mut self,
        cap: &StrictSendCap<WillowWrite, WillowScope>,
        subspace_id: &SubspaceId,
    ) -> Result<Signer<'store>, WillowKeyError> {
        cap.is_active()?;
        cap.scope().authorizes_subspace(subspace_id)?;
        let secret = self.subspaces.get_mut(subspace_id)
            .ok_or(WillowKeyError::UnknownSubspace(subspace_id.clone()))?;
        Ok(Signer { secret })
    }
}
```

Call-site shape (the secret never leaves the store; only the owned
`SubspaceSignature` crosses any `await`):

```rust
// Inside ContactsProtocol::connect — the sign is synchronous and short,
// the Signer borrow is released before any await (per KNOWN_TRAPS:
// never hold an entity borrow across an await).
let meta_bytes = serde_json::to_vec(&meta)?;
let signature: SubspaceSignature = self.key_store.update(cx, |store, _cx| {
    let mut signer = store.signer(&self.write_cap, &self.local_profile_id)?;
    signer.try_sign(&meta_bytes)
})??;
// signer dropped here; only `signature` (owned, no secret) survives.
let envelope = SignedEnvelope::from_signed_parts(meta, message, signature);
```

Consequences for the consuming APIs:

- `SignedEnvelope::new_at` / `ContactTicket::new` stop taking
  `SubspaceSecret` by value. They take a `&mut Signer` (or are split so the
  caller signs via the `Signer` and passes the owned `SubspaceSignature` +
  `sender_id` in). `SignedEnvelope` already has the pieces (`meta`,
  `message`, `signature`); the change is mechanical once signing is borrowed.
- `corresponding_subspace_id()` for `meta.sender_id` comes from the
  `Signer` (or the caller's already-known `local_profile_id`), never from an
  owned secret.

#### §10.4 (PROPOSED — open): the write path has the same leak class

`WillowKeyStore::mint_write_authority` (`galvanized-cap/src/lib.rs:392`)
returns an owned `SubspaceSecret` clone alongside the `WriteCapability`, and
`plugin_willow::create_entry` (`plugin-willow/src/lib.rs:209`) feeds that
clone to the Willow `store.create_entry(...)` which signs the entry with it.
This is the same extractable-secret problem as signing, and §10's
non-extraction principle must cover it — *but* the Willow
`store.create_entry(...)` is `async` and streams the payload, so lending a
`&mut SubspaceSecret` across that `await` is exactly the borrow-across-await
trap (KNOWN_TRAPS) and the key-store `Entity` is foreground-gated.

Two candidate directions, to confirm with Nick at implementation:

- **(a) RECOMMENDED — the key store is the signing trust boundary for writes
  too.** Add a method like `WillowKeyStore::sign_entry(cap, ns, sub, path,
  timestamp, payload) -> Result<(WriteCapability, AuthorisedEntry), _>` (or
  have the store own the `store.create_entry` interaction) so the raw secret
  never exits the store; callers present a cap + the entry data and get back
  the signed `AuthorisedEntry`. Unifies "sign as Profile" and "write entry"
  behind one non-extractable boundary.
- **(b) Lend a `WriteSigner<'_>`** carrying the minted `WriteCapability`
  plus `&mut SubspaceSecret`, scoped so the borrow lives only across the
  sync portion — but the async Willow `create_entry` forces the secret to be
  available across an `await`, which makes this awkward; (a) is cleaner.

`mint_write_authority` as currently specified (returning a `SubspaceSecret`)
is **superseded** by whichever of (a)/(b) Nick picks; the old §6 scope item
("`create_entry`/future write paths take a `StrictCap<WillowWrite, …>`")
still stands for the *cap-gating* part, but the *secret-handling* part must
be non-extractable per this §10.4.

### §11 (RATIFIED — Nick, 2026-08-08): `Namespace` / `Subspace` are data objects, not domain objects

Re-frame `Namespace` / `Subspace` (`plugin-willow/src/lib.rs:82-116`) as
**data / serialization objects used inside the trust boundary only** — e.g.
when the Vault serializes/deserializes its content and when seeding the
`WillowKeyStore`. They are **not** runtime domain objects carried by
`Profile` / `Space`. Their `secret` field is acceptable *as a data type*
(encrypted at rest in the vault DB); the problem was treating them as
runtime handles.

Consequences:

- `Profile::new` / `Profile::from_parts` (`plugin-galvanized/src/domain/profile.rs:54,71`)
  stop taking a `Subspace` (secret); they take a `SubspaceId`. The secret was
  moved into the `WillowKeyStore` at vault load / creation time and lives only
  there.
- `Space` likewise holds a `NamespaceId`, not a `Namespace`.
- `ProfileData.subspace: Subspace` → `subspace_id: SubspaceId` (serialization
  shape change: the vault content still serializes the *secret* via the
  `Subspace`/`Namespace` data types so it can re-seed the key store on next
  unlock; the runtime `ProfileData` persists only the id). Keep the
  `SubspaceIdSerde` / `NamespaceIdSerde` helpers for the runtime id fields.
- `create_subspace` / `create_owned_namespace` / `create_communal_namespace`
  (`plugin-willow/src/lib.rs:166+`) return the data object *into the vault/key-store
  seeding path*; the runtime entity is constructed from just the returned id.
- Trust-boundary ownership: the `Subspace`/`Namespace` data types (with
  `secret`) live in the vault layer / `plugin-willow`'s ser-de +
  `extract_key_store`/`extend_key_store` paths only; nothing in
  `plugin-galvanized`'s runtime `Profile`/`Space` holds them.

This closes leak inventory #5 and makes "no Willow key held as a plain struct
field reachable by arbitrary code" (the original acceptance bar) actually
true, since the only in-memory secret holder is the cap-gated `Entity<WillowKeyStore>`.

### Impact on acceptance criteria (additions/supersessions)

- **Supersedes** the original acceptance line "no Willow key is held as a
  plain struct field reachable by arbitrary code" — now strengthened to:
  *no copyable Willow secret exits `WillowKeyStore`; signing/write paths use
  the non-`Clone` `Signer` (or §10.4 equivalent), and runtime `Profile`/`Space`
  hold only public ids.*
- Add: **`WillowKeyStore` is not `Clone`**; it is held as `Entity<WillowKeyStore>`
  and shared by handle.
- Add: **`ContactsProtocol` holds `local_profile_id` + `Entity<WillowKeyStore>`
  + `write_cap`, never a `SubspaceSecret`**; re-presents the cap on each sign.
- Add: **on lock, no stray `Entity<WillowKeyStore>` handle keeps secrets in
  memory** (verify the Vault's handle + Profile entities are the only holders
  and `lock()` clears both).
- Keep: the existing `CapCell`/revocation/TTL acceptance items (§3, §5) are
  unchanged; this iteration is about the *secret-extraction* axis, which
  those items did not address.

### Still open (for the next session / Nick)

- **§10.4 write path**: confirm direction (a) vs (b). This is the one design
  decision blocking full non-extraction. (a) is recommended.
- **`Signer` name & exact surface**: bikeshedded above; confirm before
  implementing. Consider whether a single `Signer<'_>` covers both the
  envelope-sign and the §10.4 write-sign, or whether the write path wants its
  own method on the store.
- **`connect` context threading**: today `ContactsProtocol::connect(&self,
  ticket)` has no `App` context (the `Entity<ContactsProtocolHandle>::connect`
  has `cx` but drops it before calling `protocol.connect`). With an
  `Entity<WillowKeyStore>`, `connect` needs a context to `update` the store.
  Decide how to thread it (pass `&mut impl AppContext` into `connect`) while
  honoring KNOWN_TRAPS (clone what you need, don't hold `&mut App` across an
  `await`; the sign itself is sync and short, so the `update` block can be
  self-contained).
- **`Profile`/`Space` secret-free confirmation**: confirm nothing in
  `plugin-galvanized`'s runtime needs the `Subspace`/`Namespace` secret beyond
  what the key store provides (id derivation is from the public id; no secret
  needed). Check `Profile::subspace()` callers.
- **Interaction with the unresolved capability-caching wrapper**
  (`vault.rs:41` TODO / OPEN_QUESTIONS "Capability-powered caching wrapper"):
  does wrapping the key store in `Entity` + `CapCell`-gating the vault content
  compose cleanly, or does the caching-wrapper decision want to land first /
  alongside? Flag, don't block §8–§11 on it.

### Anchors (re-derive before leaning on)

- Leak #1: `crates/plugin-contacts/src/protocol.rs:81` (`local_profile_key`),
  used `:330`, `:373`, `:427`.
- Leak #2: `crates/galvanized-cap/src/lib.rs:343` (`#[derive(Clone)]` on
  `WillowKeyStore`); `crates/plugin-galvanized/src/domain/profile.rs:121`
  (`Profile::key_store`).
- Leak #3: `crates/galvanized-cap/src/lib.rs:392` (`mint_write_authority`),
  `:430` (`subspace_secret_for_signing`); on-demand callers
  `crates/plugin-contacts/src/lib.rs:334` and `:665`.
- Leak #4: `crates/plugin-contacts/src/protocol.rs:486` (`SignedEnvelope::new_at`),
  `crates/plugin-contacts/src/lib.rs:721` (`ContactTicket::new`).
- Leak #5: `crates/plugin-willow/src/lib.rs:82-116` (`Namespace`/`Subspace`
  secret fields), `:127-158` (`extract_key_store`/`extend_key_store`);
  `crates/plugin-galvanized/src/domain/profile.rs:54,71` (`Profile::new`/`from_parts`
  taking `Subspace`).
- Lock behavior: `crates/plugin-galvanized/src/domain/vault.rs:177` (`Vault::lock`).