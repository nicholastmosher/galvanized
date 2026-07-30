# Architecture

A grounded map of the code, with anchors. Re-derive line numbers before you
lean on them — the tree moves. When the code and this doc disagree, the code is
current truth; fix this doc.

## Workspace layout

Workspace root `Cargo.toml` (resolver 3). Active crates under `crates/`:

- `app-galvanized` — the `gzed` binary + lib. `pub fn init(cx: &mut App)` is
  the plugin stack entry. (`crates/app-galvanized/src/lib.rs:5`)
- `galvanized-core` — minimal in-repo reproduction of Zed's Workspace:
  `Workspace`, `Pane`, `PaneGroup`, `Item`, `ItemHandle`. Opens the main window.
  (`crates/galvanized-core/src/lib.rs:18`)
- `plugin-galvanized` — top-level `Galvanized` entity + domain model (Vault,
  Profile, Space) + the `GalvanizedPanel` sidebar + `AppBehavior` trait.
  (`crates/plugin-galvanized/src/lib.rs:23`)
- `plugin-vault` — password-encrypted key store (capsec + sqlx/sqlite). The
  gatekeeper for Willow keys. (`crates/plugin-vault/src/lib.rs:22`)
- `plugin-willow` — `cx.willow()` API over a Willow store. Namespace/Subspace
  creation, entry create/list/read. (`crates/plugin-willow/src/lib.rs:32`)
- `plugin-p2p` — `P2p` entity wrapping an actor; dynamic protocol registration
  by ALPN over Iroh. (`crates/plugin-p2p/src/lib.rs:21`)
- `plugin-automerge` — samod (automerge-repo) live-collaboration glue.
- `plugin-chat`, `plugin-calendar`, `plugin-contacts`, `plugin-files` —
  application plugins implementing `AppBehavior`.
- `plugin-theme-palette`, `plugin-connections` — present but currently
  **not wired** into `gzed::init` (see `lib.rs`).
- `strict-cap` — capability helper crate.
- `unused_*` — abandoned experiments (iroh-automerge, libp2p-ui,
  willow-api-derive, willow-rummager, csh-demo, misc). **Skip these.**

External heavyweights (git/path deps in root `Cargo.toml`):

- `gpui` / `gpui_platform` / `gpui_util` / `zed` / `zed_ui` / `zed_ui_input` —
  from Nick's `galvanized` branch of his Zed fork.
- `gpui-component` — the UI component library (now the theming source).
- `iroh` — from Nick's `galvanized` branch of his Iroh fork.
- `willow25` — the Willow data store (currently `MemoryStore` in-repo; the
  on-disk path is built but `_store_path` is unused — see KNOWN_TRAPS).
- `automerge` / `autosurgeon` / `samod` — CRDT live editing.
- `capsec` — capability system (used by the vault).
- `sqlx` (sqlite) — vault persistence.

## How plugins compose

Entry point (`crates/app-galvanized/src/bin/main.rs`):

```rust
application().add_plugin(gzed::init).run();
```

`gzed::init` (`crates/app-galvanized/src/lib.rs:5`) calls, in order:
`observability::init` → `gpui_component::init` → `plugin_vault::init` →
`plugin_p2p::init` → `plugin_willow::init` → `plugin_automerge::init` →
`plugin_calendar::init` → `plugin_chat::init` → `plugin_contacts::init` →
`plugin_files::init` → `plugin_galvanized::init` → `galvanized_core::init`.

A "plugin" here is just `pub fn init(cx: &mut App)`. The `application()`/
`add_plugin` plumbing comes from `gpui_platform` in Nick's Zed fork.

`galvanized_core::init` (`crates/galvanized-core/src/lib.rs:18`) opens the main
window with a `Root` (gpui-component) wrapping a `Workspace`.

`plugin_galvanized::init` (`crates/plugin-galvanized/src/lib.rs:23`) registers
an `cx.observe_new::<Workspace>` that constructs the `Galvanized` entity and
adds the `GalvanizedPanel` as a left-split item (300px). The `Galvanized` entity
is the hub for shared app state.

App plugins (chat, calendar, …) in turn `cx.observe_new::<Galvanized>`
(`crates/plugin-chat/src/lib.rs:33` is the model), then call
`galvanized.register_app(chat, cx)` and `galvanized.register_action(...)`.

## The data model and capability flow

Galvanized maps Willow's primitives onto user-facing terms:

- **Vault** = a password-encrypted container (`plugin-vault`). One Vault holds
  many **Spaces** and **Profiles**. Replaces the older "User" term (dev log,
  2026-06-21).
- **Profile** ≈ a Willow **Subspace** (a "persona"/identity).
  (`crates/plugin-galvanized/src/domain/profile.rs`)
- **Space** ≈ a Willow **Namespace**, either **owned** (write needs a signed
  capability from the namespace secret) or **communal** (any holder of a
  subspace key may write to their own subspace within it).
  (`crates/plugin-galvanized/src/domain/space.rs`)
- **Endpoint** = an Iroh peer (a location/keypair), deliberately *distinct* from
  a Profile/identity. Endpoints should be rotatable/ephemeral; associating an
  Endpoint with a Profile is a privacy cost Nick works to avoid (dev log,
  2026-05-03, 2026-03-30).
- **Contact** = a remote representation/proxy for a Profile on another peer.
- **Connection** = an Iroh hole-punched QUIC link between two Endpoints.

Capability flow (top → bottom), per dev log 2026-05-28 / 2026-06-03:

```
unlock_vault(password)      -> VaultSecrets { vault.secret().subspace_secret }
unlock_subspace(password)   -> SubspaceSecret
unlock_profile(password)    -> ProfileKey { unlock_subspace(password) }
```

The vault uses **capsec** (`crates/plugin-vault/src/lib.rs:23`): `capsec::root()`
grants a `VaultAccess` capability, made sendable and handed to a spawned
`VaultActor` on a background thread. All read/write goes through
`VaultToken`s minted on unlock. The design intent (dev log 2026-05-19) is timed,
scoped, revocable capabilities — the in-tree TODO at
`crates/plugin-galvanized/src/domain/vault.rs:41` marks the
capability-powered caching wrapper as not-yet-built.

## The `cx.willow()` and `cx.vaults()` APIs

Both follow the same extension-trait pattern: a global holds an `Entity<State>`,
and a blanket `impl<C: AppContext> FooExt for C` gives any context a
`cx.foo()` accessor.

- `cx.willow()` → `WillowCx` (`crates/plugin-willow/src/lib.rs:54`). Methods:
  `create_communal_namespace`, `create_owned_namespace`, `create_subspace`,
  `create_entry(namespace, subspace, path, data)`, `list_entries(...)`,
  `read_entry(...)`. Store is a `MemoryStore` (in-memory only today).
- `cx.vaults()` → `VaultsCx` (`crates/plugin-vault/src/lib.rs:59`). Methods:
  `create`, `list`, `lock`, `unlock`, `read`, `read_metadata`, `update`. All
  return `Task<…>` run on the background executor and awaited on the foreground.

The intended unified handle is the `Willowize` trait +
`EntryHandle` for `Entity<T: Willowize>`
(`crates/plugin-willow/src/model.rs:9`) — load/save/sync — but the body is
currently commented out; it's a design seam, not a working API. Treat it as an
open question, not a foundation.

## p2p

`P2p` (`crates/plugin-p2p/src/lib.rs:29`) wraps an `Actor` that owns an Iroh
`Endpoint` and a dynamic protocol map. Plugins register protocols by ALPN via
`register_protocol` / the `KeyedProtocolHandler` trait (ALPN const). A `P2p`
entity is created per-Profile (`Profile::from_parts`,
`crates/plugin-galvanized/src/domain/profile.rs:60`). `plugin_p2p::init` is
currently a no-op (`lib.rs:21`) — the p2p stack is wired through the Profile,
not through a global.

The historical chat-on-automerge race (dev log 2026-05-15): if both peers open a
chat before the document id is exchanged, they end up with different docs.
Watch for this when touching chat/connections.

## Live collaboration (Automerge / samod)

`plugin-chat` is the reference impl (`crates/plugin-chat/src/lib.rs`):

- `ChatDocument` / `ChatMessage` derive autosurgeon `Hydrate` + `Reconcile`
  (`:141`, `:159`).
- `ChatUi::new` spawns a background task that reads `doc_handle.changes()`
  (samod) and forwards re-hydrated documents over a `flume` channel to the
  foreground, which calls `cx.notify()` (`:195`–`:237`).
- `send_message` reconciles into the automerge doc inside a transaction
  (`:341`).
- `ChatUi` implements `galvanized_core::item::Item` so it renders as a tabbed
  item in a `Pane`.

The samod sync-over-Iroh seam was a long-standing blocker (dev log
2026-04-28 / 2026-04-30); it was a user error with `.with_document(..)` +
`AutoCommit` (2026-05-01), now resolved.

## The UI shell (post-Zed)

`galvanized-core` is a **minimal reproduction of Zed's Workspace**
(`crates/galvanized-core/src/lib.rs:34`):

- `Workspace` owns a `main_pane` and a `PaneGroup`, subscribes to `PaneEvent`s
  (Split / SplitWithItem / MoveItem / Remove), and re-routes items across panes.
- `Pane` holds `Box<dyn ItemHandle>`, renders a tab bar with drag/drop split
  targets, and emits split/move events (`crates/galvanized-core/src/pane.rs`).
- `PaneGroup` / `PaneAxis` / `Member` is the split tree, rendered via
  gpui-component's `resizable` (`h_resizable`/`v_resizable`/`resizable_panel`).
- `Item` trait: `Focusable + EventEmitter<Self::Event> + Render` with
  `tab_content_text`. `ItemHandle` is the type-erased `Box<dyn>` equivalent
  (`crates/galvanized-core/src/item.rs`).

Theming: use `gpui_component::ActiveTheme` and `cx.theme().colors.*`. **Do not**
use Zed's `ActiveTheme` — it depends on a Zed global that Galvanized no longer
sets and will panic (dev log 2026-07-29). This is the most common port bug.

`AppBehavior` (`crates/plugin-galvanized/src/app_behavior.rs:8`) is the trait
for app plugins: `id`, `icon`, `title`, `open_action`, optional
`space_context_menu_items`. `Entity<T: AppBehavior>` auto-implements `AppHandle`
so it can be registered via `galvanized.register_app(...)`.

## Persistence / observability

- Vault DB: `dirs::data_dir()/galvanized/vault.db` (sqlx sqlite). Single
  migration: `migrations/20260521045144_vaults.sql`.
- Willow "store path": `dirs::data_dir()/galvanized/willow.store` — built but
  **not used** (MemoryStore is in-memory).
- Observability (`crates/app-galvanized/src/observability.rs`): OTLP tracing
  and Loki logging layers are **commented out**; only `tracing_subscriber::fmt`
  runs. The docker-compose in `containers/observability` (Grafana/Prometheus/
  Tempo/Loki) is ready but the app isn't exporting to it yet.
- A `GlobalMetrics(Meter)` global + `MetricsExt::meter()` exists but is unused
  pending the metrics init being re-enabled.

## What's unfinished (from the dev log)

- Recreating Zed's Panes (tabbed, splittable item hosts) — in progress as
  `galvanized-core`.
- Porting remaining Zed-UI components to gpui-component (ContextMenus,
  PopoverMenus, maybe more).
- Willow persistence (MemoryStore → real on-disk store).
- Rework chat between Profiles (Willow subspaces) rather than between Iroh
  endpoints.
- Namespace creation flow + capabilities UI.
- Multi-member chat / shared-doc permissioning.
- Contacts as a first-class, locked app.
- The `Willowize`/`EntryHandle` unified handle seam.

See [`memory/OPEN_QUESTIONS.md`](memory/OPEN_QUESTIONS.md) for the tracked set.