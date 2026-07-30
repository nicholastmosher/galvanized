# State — the current arc

> **Maintained in place**, not appended. A later session should read this and
> know what's in flight right now. Update it when the arc shifts; don't grow it
> as a log (the README is the log).

## Last known position (from README 2026-07-29)

Galvanized is **moving off Zed**. As of the latest dev-log entry:

- Galvanized is now its own independent application, built on
  **gpui-component** (no longer a plugin running inside the Zed editor).
- A **minimal reproduction of Zed's Workspace** was created in-repo as
  `galvanized-core`: resizable left panel + main view.
- Current goal: **recreate Zed's Panes** — the tabbed, splittable views that
  host arbitrary items. In-tree: `galvanized-core` has `Workspace`, `Pane`,
  `PaneGroup`, `Item`, `ItemHandle` (drag/drop split, tab bar). This is the
  active arc.
- Porting Zed-UI components to gpui-component equivalents. Known remaining:
  **ContextMenus and PopoverMenus**, maybe more. The key gotcha: Zed's
  `ActiveTheme` panics on the missing global; use gpui-component's
  `ActiveTheme` + `cx.theme()`.

## What compiles / runs today

- `cargo run` launches `gzed` (`crates/app-galvanized`). Entry:
  `application().add_plugin(gzed::init).run()`.
- Plugin stack order is set in `crates/app-galvanized/src/lib.rs`.
- Vault (sqlx/sqlite, capsec) and Willow (MemoryStore) init; the vault
  create/unlock/list flow works end-to-end (`plugin-galvanized::Galvanized`).
- Chat (`plugin-chat`) builds and renders as a `galvanized_core::item::Item`;
  samod/automerge live sync works once peers rendezvous (mind the doc-id race).
- p2p (`plugin-p2p`) is constructed per-Profile; `plugin_p2p::init` is a no-op.

## What does NOT work / is not wired

- Willow is **in-memory only** (`MemoryStore`); the on-disk path is built but
  unused.
- Observability export (OTLP/Loki) is **commented out**; only `fmt` logging.
- `plugin-connections` and `plugin-theme-palette` are **not in `gzed::init`**.
- The `Willowize`/`EntryHandle` unified handle is a commented-out seam.
- The vault's timed/revocable capability wrapper is a TODO.

## Likely next (read as "if you're picking up the thread, look here first")

1. Finish `galvanized-core` Panes to parity with what Zed's Panes did for this
   app (splitting, item moves, activation — much is already there in
   `pane.rs`/`pane_group.rs`; verify against the use-cases the app plugins
   need).
2. Port the remaining Zed-UI components (ContextMenu, PopoverMenu) to
   gpui-component, removing any remaining Zed-`ActiveTheme` usages.
3. Pick up an open question from [`OPEN_QUESTIONS.md`](OPEN_QUESTIONS.md) only
   after the Pane/porting arc is stable — and only with Nick's go-ahead on the
   design.

## Don't (without checking with Nick)

- Don't start the Willow persistence switch, the chat-onto-profiles rework, or
  the capability-caching wrapper without a design conversation — these are
  shape-shaped decisions.