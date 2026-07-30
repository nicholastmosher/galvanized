# The plugin recipe (a new app plugin)

A new "app" in Galvanized is a vertical slice: a crate, an `init`, a data model,
an `AppBehavior` view, an action, and registration with `Galvanized`. Use
`plugin-chat` as the reference (`crates/plugin-chat/src/lib.rs`).

## 1. Crate

`crates/plugin-<name>` with `Cargo.toml` depending on `gpui`, `gpui-component`,
`galvanized-core`, `plugin-galvanized`, and whatever domain deps you need
(e.g. `samod`, `plugin-willow`). Name it `plugin-*`, not `*-plugin`. Prefer a
descriptive `[lib] path`.

## 2. `init` + registration

```rust
use gpui::{App, AppContext as _, Context, Entity, actions};
use plugin_galvanized::Galvanized;

actions!(myapp, [/// Opens the MyApp interface
    OpenMyApp]);

pub fn init(cx: &mut App) {
    cx.observe_new::<Galvanized>(|galvanized, _window, cx| {
        let app = cx.new(|cx| MyApp::new(cx.entity(), cx));
        galvanized.register_app(app.clone(), cx);
        galvanized.register_action(cx, move |this, _workspace, _: &OpenMyApp, _window, cx| {
            this.panel().update(cx, |panel, cx| panel.set_active_app(app, cx));
        });
    })
    .detach();
}
```

Add `plugin-<name>.init(cx)` to the stack in
`crates/app-galvanized/src/lib.rs` (app plugins go before `plugin_galvanized`
and `galvanized_core`).

## 3. `AppBehavior`

```rust
use gpui::{Render, SharedString, Window, Context};
use plugin_galvanized::app_behavior::{AppBehavior, SpaceContextMenuItem};

pub struct MyApp { /* … */ }

impl AppBehavior for MyApp {
    fn id() -> &'static str { "myapp" }
    fn icon(&self) -> SharedString { "🔖".into() }   // TODO: better than emoji
    fn title(&self) -> SharedString { "My App".into() }
    fn open_action(&self) -> Box<dyn Action> { Box::new(OpenMyApp) }
    fn space_context_menu_items(&self, _space: Entity<Space>, _cx: &App)
        -> Vec<SpaceContextMenuItem> { vec![] }
}

impl Render for MyApp {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child("MyApp")
    }
}
```

`Entity<T: AppBehavior>` auto-implements `AppHandle`, so it can be passed to
`register_app`.

## 4. The main view (an `Item` in a `Pane`)

If your app's content should be a tabbed, splittable item in the main area
(like `ChatUi`), implement `galvanized_core::item::Item`:

```rust
impl Focusable for MyView { /* focus_handle */ }
impl EventEmitter<()> for MyView {}
impl Item for MyView {
    type Event = ();
    fn tab_content_text(&self, _detail: usize, _cx: &App) -> SharedString { "My View".into() }
}
impl Render for MyView { /* … */ }
```

Open it into the active pane via `Workspace::add_item_to_active_pane` (or
`add_item_in_split` for a split).

## 5. Data model

If your app stores durable data, decide where it lives **before** coding:

- **Willow entry** (the platform path): model the object as a struct, serialize
  (serde_json), write via `cx.willow().create_entry(namespace, subspace, path,
  data)`. The `Willowize`/`EntryHandle` unified-handle seam
  (`crates/plugin-willow/src/model.rs`) is an *aspirational* API, not yet built —
  don't depend on it; use the explicit `cx.willow()` methods.
- **Automerge/samod** (live collab): model with `#[derive(Hydrate,
  Reconcile)]`, drive a `DocHandle`, and bridge changes to the UI over a
  `flume` (see `plugin-chat`).
- **Vault** (secrets/keys only): route through `cx.vaults()`. Never store app
  content in the vault.

State your choice in the PR/task; a slice that picks a store silently picks a
contract.

## 6. Verify (real, not relayed)

- `cargo fmt` + `cargo clippy` on the touched crates.
- `cargo run`, drive the app by hand to your new view (open the app from the
  sidebar, trigger your action), and confirm it renders and behaves.
- If you can't drive it live (e.g. p2p needs two peers), say so explicitly and
  state what you *did* verify (build, unit test). Don't claim a run you didn't
  do.

## 7. Records

- If the slice introduces or settles a domain term, update
  [`memory/GLOSSARY.md`](memory/GLOSSARY.md).
- If it hits a trap, add it to [`memory/KNOWN_TRAPS.md`](memory/KNOWN_TRAPS.md).
- If it opens a design question, add it to
  [`memory/OPEN_QUESTIONS.md`](memory/OPEN_QUESTIONS.md).
- If Nick ratified something during the work, append it to
  [`memory/DECISIONS.md`](memory/DECISIONS.md) at the tail, verbatim.
- Update [`memory/STATE.md`](memory/STATE.md) in place if the arc shifted.

A slice that skips verification or records isn't done.