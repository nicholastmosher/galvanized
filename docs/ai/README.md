# AI guidance for Galvanized

Start at the root [`AGENTS.md`](../../AGENTS.md) — it's the boot file and points
here. The Rust + GPUI coding core lives in [`RUST_GPUI.md`](RUST_GPUI.md);
everything else in this folder is layered on top of it.

## Files

- [`ARCHITECTURE.md`](ARCHITECTURE.md) — the code, anchored with file:line.
  Read this to know the real shape of the project before changing anything.
- [`RUST_GPUI.md`](RUST_GPUI.md) — the Rust + GPUI coding core (the Rust
  guidelines + the GPUI framework primer). Moved here from the repo-root
  `.rules`, which is now just a pointer to `AGENTS.md`.
- [`CONVENTIONS.md`](CONVENTIONS.md) — Galvanized-specific conventions
  (plugin composition, `cx.willow()`/`cx.vaults()`, theming, the `//`
  placeholder trick, async/foreground-background split, security stance).
- [`PLUGIN_RECIPE.md`](PLUGIN_RECIPE.md) — the vertical slice for adding a new
  app plugin (crate → `init` → `AppBehavior` → `Item` → data model → verify →
  records).

## Long-term memory (`memory/`)

The part that grows over time. Start at [`memory/README.md`](memory/README.md)
for the read/write rules.

- [`memory/DECISIONS.md`](memory/DECISIONS.md) — Nick's ratified rulings
  (append at the true tail; later supersedes earlier).
- [`memory/OPEN_QUESTIONS.md`](memory/OPEN_QUESTIONS.md) — unresolved design
  questions; don't bake an answer without going to Nick.
- [`memory/STATE.md`](memory/STATE.md) — the current arc, maintained in place.
- [`memory/KNOWN_TRAPS.md`](memory/KNOWN_TRAPS.md) — gotchas with evidence.
- [`memory/GLOSSARY.md`](memory/GLOSSARY.md) — domain terms.
- [`memory/HANDOFF.md`](memory/HANDOFF.md) — cross-session scratch.

## One-line summary

`AGENTS.md` (who you are + hard rules + boot) → `RUST_GPUI.md` (Rust/GPUI) →
`ARCHITECTURE.md` (the code) → `CONVENTIONS.md` (how to write it here) →
`PLUGIN_RECIPE.md` (how to add an app) → `memory/` (what's been decided, what's
open, what's broken, what's next).