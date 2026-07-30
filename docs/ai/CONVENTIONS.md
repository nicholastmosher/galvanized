# Conventions

Galvanized-specific coding conventions, layered on top of
[`RUST_GPUI.md`](RUST_GPUI.md) (Rust + GPUI core). Where this doc and
`RUST_GPUI.md` disagree on a GPUI mechanic, trust `RUST_GPUI.md`; where
`RUST_GPUI.md` references a Zed-only script, trust this doc.

## Rust (project specifics)

- `RUST_GPUI.md` already covers: no `unwrap()`/`expect()` outside tests, no
  silent `let _ =` on fallible calls, propagate with `?`, `foo.rs` over
  `foo/mod.rs`, full-word variable names, variable shadowing to scope clones in
  async blocks. **Follow it.**
- **No `mod.rs`.** Use `src/foo.rs` (and `src/foo/sub.rs` + `pub mod sub;` in
  `foo.rs`). The crates follow this.
- **Crates prefer a named lib root.** When creating a crate, set
  `[lib] path = "...rs"` in `Cargo.toml` (e.g. the `gzed` lib uses
  `src/lib.rs`; the convention is a descriptive name). Don't introduce a bare
  `lib.rs` where a name carries meaning.
- **Plugin crates are named `plugin-*`, not `*-plugin`.** Match the existing
  set.
- **Error visibility is doctrine, not preference.** Every background failure a
  user could notice must surface in the UI — a lock screen, a status chip, a
  notification. `detach_and_log_err(cx)` is acceptable for truly internal
  housekeeping; it is **not** a substitute for surfacing user-facing failures.
  The vault's lock-on-timeout design (dev log 2026-05-19) is the model:
  capability expiry *and* in-memory unavailability, defense in depth.

## GPUI (Galvanized flavor)

- Use the **new APIs** from `RUST_GPUI.md`: `Entity<T>` (never `Model`/`View`),
  `App` (never `AppContext`), `Context<T>` (never `ModelContext`), explicit
  `Window` passed before `cx`, `*_in` variants when plumbing `Window`.
- `spawn` takes an `async` closure: `cx.spawn(async move |this, cx| …)` for an
  entity (where `this: WeakEntity<T>`), `cx.spawn(async move |cx| …)` for `App`.
  Confirm against `RUST_GPUI.md` if unsure — the old forms are gone.
- All entity/UI work is on the single foreground thread. Heavy work →
  `cx.background_spawn(...)` returning a `Task`, awaited on the foreground to
  update state. See `cx.vaults()` / `cx.willow()` for the pattern.
- **Theming: `gpui_component::ActiveTheme`, `cx.theme()`.** Never Zed's
  `ActiveTheme`. See [`KNOWN_TRAPS.md`](memory/KNOWN_TRAPS.md).
- **The `//` placeholder-line convention.** Nick intersperses bare `//`
  comments to keep `rustfmt` from collapsing fluent-builder chains onto one
  line, so meaningful chunks stay on their own line and are easy to move in
  visual-line mode. Preserve it in files that use it; don't "clean it up."
  Example (from `crates/plugin-chat/src/lib.rs`):
  ```rust
  v_flex()
      //
      .p_2()
      .bg(cx.theme().colors.muted)
  ```
- **Don't instantiate UI items repeatedly in `render`.** (Nick's own note,
  2026-02-23.) Build reusable sub-elements once; `render` should describe the
  tree, not reconstruct heavy state every frame.

## Plugin composition

- A plugin is `pub fn init(cx: &mut App)`. Register it into the stack in
  `crates/app-galvanized/src/lib.rs` in dependency order (foundational plugins
  before app plugins; `plugin_galvanized` and `galvanized_core` last).
- App plugins implement `AppBehavior`
  (`crates/plugin-galvanized/src/app_behavior.rs`) and register themselves by
  `cx.observe_new::<Galvanized>(|g, _window, cx| { g.register_app(...);
  g.register_action(...); })`. See `plugin-chat/src/lib.rs:33` for the model.
- Prefer a **global + extension trait** (`cx.willow()`, `cx.vaults()`) for
  cross-cutting services, not a grab bag of free functions. The pattern: a
  `Global` holding an `Entity<State>`, a blanket `impl<C: AppContext> FooExt
  for C`, and a short-lived `FooCx<'a, C>` borrowing both.

## Async and the foreground/background split

- Never hold a `&mut App` (or entity borrow) across an `.await`. Clone what
  you need, `move` it into the background task, and re-enter via
  `entity.update(cx, |…| …)` on the foreground.
- Don't `unwrap()` background results — propagate with `?` and
  `detach_and_log_err` only when the caller genuinely has no UI to surface.
- Watch the **samod/automerge sync race** class (dev log 2026-05-15): two
  peers opening the same chat before the doc id is exchanged get two docs.
  When touching chat/connections, reason about the rendezvous ordering.

## Security / privacy stance

- **Capability-gated, not ad-hoc.** Willow read/write goes through
  `WriteCapability`/read capabilities minted from namespace + subspace secrets
  (`crates/plugin-willow/src/lib.rs:182`). Don't add a "just write directly"
  path around the capability machinery.
- **Keys live behind the vault.** Never hold a subspace/namespace secret in a
  long-lived plain struct field; route through `VaultToken`s. The
  `secret_todo_consider_access_control` naming in
  `crates/plugin-willow/src/lib.rs:116` is a known-open seam — flag, don't
  extend.
- **Don't associate Endpoint ↔ Profile** unless a design explicitly requires
  it. Endpoints are rotatable; Profiles are identity. (Dev log 2026-05-03.)
- **Contacts are sensitive.** "Who you know is some of the most personal
  information there is" (dev log 2026-05-03) — contacts belong under lock and
  key, like the vault.

## Style and taste

- **Hand-made feel, not just "good enough".** Nick values the human touch in
  the UI (dev log 2026-03-05, 2026-06-21). gpui-component is a tool, not a
  destination — don't let the default widget look win where a more deliberate
  one is right.
- **Honest empty states.** "No items" placeholders (see
  `crates/galvanized-core/src/pane.rs`) are fine; lying placeholders that imply
  work that isn't done are not.
- **No creative additions unless asked.** (`RUST_GPUI.md`.) Implement the
  task; raise adjacent ideas as proposals, not as silent scope creep.
