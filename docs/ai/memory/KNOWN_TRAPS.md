# Known traps

Gotchas with evidence. Read before touching the relevant area. Add as you hit
new ones; edit in place.

## Zed `ActiveTheme` panics under gpui-component
**Evidence:** README 2026-07-29; the Zed `ActiveTheme` trait depends on a Zed
global that Galvanized no longer installs.
**Fix:** import `gpui_component::ActiveTheme` and use `cx.theme().colors.*`.
grep for `zed::unstable::ui::ActiveTheme` / `ActiveTheme` from Zed before
assuming a theming call is safe.

## The `//` placeholder-line convention
**Evidence:** `crates/plugin-chat/src/lib.rs` and the README (2026-04-09 note).
Bare `//` lines keep `rustfmt` from collapsing fluent-builder chains.
**Fix:** don't "clean up" the `//` lines when you see them; preserve the style
in files that use it. (Also in [`CONVENTIONS.md`](../CONVENTIONS.md).)

## Wrong "add import" suggestion (`anyhow::Context` vs `gpui::Context`)
**Evidence:** README 2026-04-09. Code-action sometimes adds `anyhow::Context`
when `zed::unstable::ui::Context` (or `gpui::Context`) is what was meant.
**Fix:** when a context import lands, sanity-check it against what the call
site actually needs.

## `actions!` / `RegisterComponent` require `use …::self`
**Evidence:** README 2026-04-09 ("Misc notes").
**Fix:** `use zed::unstable::gpui::self` for `actions!`; `use
zed::unstable::component::self` for `RegisterComponent`. Watch for it when
adding actions or component previews.

## samod/automerge doc-id race
**Evidence:** README 2026-05-15. If both peers open a chat before the document
id is exchanged, they end up with two different docs.
**Fix:** reason about rendezvous ordering in any chat/connections work; don't
assume symmetric open is safe.

## `.with_document(..)` + `AutoCommit` misuse
**Evidence:** README 2026-05-01. The "samod not syncing" bug was a user error
mixing `.with_document(..)` with `AutoCommit`.
**Fix:** use them correctly (see `plugin-chat` for the working pattern:
`doc_handle.with_document(|am| { let mut tx = am.transaction(); … tx.commit();
})`).

## `cx.willow()` / `cx.vaults()` return `Task`s — don't `.await` on the foreground thread with a held borrow
**Evidence:** `crates/plugin-willow/src/lib.rs`, `crates/plugin-vault/src/lib.rs`
use `cx.background_spawn` and return `Task<…>`.
**Fix:** clone what you need, move it into the task, and re-enter state via
`entity.update(cx, …)` on the foreground. Never hold `&mut App` across an
`.await`.

## `_store_path` is unused (Willow is in-memory)
**Evidence:** `crates/plugin-willow/src/lib.rs:74` (`_store_path: PathBuf`).
**Fix:** don't assume data persists across restarts. If a task needs
persistence, flag it (see [`OPEN_QUESTIONS.md`](OPEN_QUESTIONS.md)).

## Observability layers are commented out
**Evidence:** `crates/app-galvanized/src/observability.rs:23-31` — OTLP and
Loki layers commented; only `fmt` active.
**Fix:** don't tell Nick "traces are in Grafana" — they aren't, until that's
re-enabled.

## `plugin-connections` / `plugin-theme-palette` are orphaned
**Evidence:** not present in `crates/app-galvanized/src/lib.rs`.
**Fix:** before depending on either, confirm with Nick whether it's alive.

## Unused crates: `unused_*` and dead branches
**Evidence:** `crates/unused_*` (iroh-automerge, libp2p-ui, willow-api-derive,
willow-rummager, csh-demo, misc). README 2026-04-26 notes an abandoned branch.
**Fix:** skip them; don't "revive" without Nick's sign-off.