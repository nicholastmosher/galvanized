# Open questions

Design-open questions and unresolved TODOs mined from the dev log and the
code. **None of these have a settled answer.** Don't bake one in silently —
bring it to Nick as a question with a recommendation.

When one is resolved, move the ruling to `DECISIONS.md` and strike it through
here (don't delete — the history of "what was open" is useful).

## Stale `.rules` line
`.rules` says "Use `./script/clippy` instead of `cargo clippy`" — inherited
from Zed. Galvanized has no `script/` dir. Options: (a) drop the line from
`.rules`, (b) add a `script/clippy` shim that matches Zed's. Recommendation:
drop the line; document `cargo fmt` / `cargo clippy` in
[`CONVENTIONS.md`](../CONVENTIONS.md) (already done) and leave `.rules` to the
Rust+GPUI core. — **awaiting Nick.**

## Willow persistence
`plugin-willow` builds a store path (`dirs::data_dir()/galvanized/willow.store`)
but uses `MemoryStore`; `_store_path` is unused. When does Galvanized move to a
real on-disk Willow store? Which store impl? (README 2026-05-27 "Prototype
persistent Willow store".)

## `Willowize` / `EntryHandle` unified handle
`crates/plugin-willow/src/model.rs` defines `Willowize` + `EntryHandle` for
`Entity<T: Willowize>` (load/save/sync) but the bodies are commented out. Is
this the intended platform API, or should app plugins keep using explicit
`cx.willow()` calls? Don't build on it until settled. (README 2026-03-16,
2026-04-18.)

## Capability-powered caching wrapper
`crates/plugin-galvanized/src/domain/vault.rs:41` TODO: the timed/revocable
capability wrapper around unlocked vault content isn't built. The vault
currently caches `unlocked_vault`/`unlocked_spaces`/`unlocked_profiles` as
plain `Option`s. When does the timeout/revoke layer land? (README
2026-05-19.)

## Endpoint ↔ Profile association
Associating an Iroh Endpoint (location) with a Profile (identity) is a privacy
cost. Is it avoidable in practice? The chat ticket currently needs both. What's
the minimum association? (README 2026-05-03, 2026-03-30.)

## Rework chat onto Profiles (subspaces), not endpoints
Chat currently keys off `EndpointId` (`crates/plugin-chat/src/lib.rs`). The
stated goal is chat between Profiles (Willow subspaces), with endpoints
ephemeral/rotatable. What's the chat identity story and the rendezvous? (README
2026-06-28, 2026-05-03.)

## Multi-member chat / shared-doc permissioning
One chat owner granting rw capability to members? How does Willow capability
sharing compose with an Automerge live session? (README 2026-05-27, 2026-03-14
"CRDT + Willow".)

## CRDT + Willow capability composition
How to "lift" Willow permissioning into CRDT participation so a leaked token
isn't enough to join? Security-through-obscurity concern. (README 2026-04-19.)

## Namespace creation flow + capabilities UI
Need first-class owned/communal namespace creation UI and a visual convention
for capabilities. (README 2026-04-19, 2026-05-27.)

## Contacts as a first-class, locked app
Contacts should be under lock and key, possibly its own app that Chat sits on
top of. Not yet built. (README 2026-05-03, 2026-06-28.)

## Discovery
How do users find each other / content without a central controller? Plugin?
Unified discovery API? Currently only out-of-band tickets. (README 2026-02-24.)

## Observability export re-enable
OTLP tracing + Loki logging layers are commented out in
`crates/app-galvanized/src/observability.rs`; only `fmt` runs. The
`containers/observability` stack is up but unreceiving. When to wire it back in?
(README 2026-04-30.)

## `plugin-connections` / `plugin-theme-palette` not wired
Both exist as crates but are not in `gzed::init`. Are they abandoned,
in-progress, or pending a rework? (Code: `crates/app-galvanized/src/lib.rs`.)