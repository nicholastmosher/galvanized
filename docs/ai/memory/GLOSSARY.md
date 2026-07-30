# Glossary

Domain terms as they settle in Galvanized. Edited in place. Where the dev log
shows an unsettled or superseded term, the prior name is kept in parentheses so
old notes stay readable.

## The container

- **Vault** (formerly **User**) — the password-encrypted top-level container a
  person creates first. Holds Profiles and Spaces. Replaces the older "User"
  domain term (README 2026-06-21). Implemented as `plugin-vault`; secrets live
  in a sqlx/sqlite DB at `~/.local/share/galvanized/vault.db` (platform data
  dir), gated by capsec capabilities minted on unlock.

## Identity

- **Profile** — a "persona" owned by a Vault. Maps to a Willow **Subspace**.
  One Vault may hold many Profiles (many personas). A Profile has a Home Space.
  (README 2026-05-28, 2026-06-28.)
- **Subspace** (Willow) — the identity primitive; one keypair. Galvanized
  exposes it as a Profile. Read/write into a namespace requires a subspace
  secret.
- **Endpoint** — an Iroh peer (a location/keypair), *distinct* from a
  Profile. Intended to be rotatable/ephemeral. Associating an Endpoint with a
  Profile is a privacy cost the design tries to avoid.
- **Contact** — a remote representation/proxy for a Profile on another peer.
  Considered highly sensitive ("who you know"); should live under lock and
  key. Not yet first-class.
- **Connection** — an Iroh hole-punched QUIC link between two Endpoints.

## The data space

- **Space** (formerly tied to "namespace") — maps to a Willow **Namespace**.
  Owned by a Vault. Has a kind:
  - **Owned namespace** — writes require a signed capability from the
    namespace secret + a subspace key.
  - **Communal namespace** — any holder of a subspace key may write to their
    own subspace within it; no namespace signature needed.
  (README 2026-05-27, 2026-02-14.)
- **Home Space** — a semi-special Space each Profile has, where Contacts,
  Endpoints, Capabilities live. By convention a Profile always has access to
  the Full Area of its own Home Space. (README 2026-06-28.)
- **Namespace** (Willow) — a filesystem-root-like area. Galvanized exposes it
  as a Space.
- **Entry** (Willow) — a key/value pair at a path within a (namespace,
  subspace) under a timestamp. The unit of durable data.
- **Capability** — a signed permission object (Meadowcap / capsec) granting
  read or write to an **Area** (a subspace + path range + time range) in a
  namespace.

## The app shell (post-Zed)

- **Workspace** — the top-level window view; a minimal in-repo reproduction of
  Zed's Workspace (`galvanized-core`). Owns a main `Pane` and a `PaneGroup`.
- **Pane** — a tabbed host for `Item`s. Supports drag-to-split. Emits split /
  move / remove events the Workspace handles.
- **PaneGroup** — the split tree (`Member::Pane` | `Member::Axis(PaneAxis)`),
  rendered with gpui-component's resizable panels.
- **Item** — trait (`Focusable + EventEmitter + Render` + `tab_content_text`)
  for views that render as tabs in a `Pane`. `ItemHandle` is the type-erased
  `Box<dyn>` handle.
- **Galvanized** (entity) — the hub of shared app state; owns the active
  Vault, the registered apps, the `GalvanizedPanel`. App plugins observe
  `Galvanized` to register themselves.
- **GalvanizedPanel** — the sidebar (left split item) with profile bar,
  omnibar, and per-app scenes.
- **AppBehavior** — trait for app plugins (`id`, `icon`, `title`,
  `open_action`, optional space context menu). `Entity<T: AppBehavior>`
  auto-implements `AppHandle`.

## Live collaboration

- **Automerge** — the CRDT document type for live editing.
- **samod** — the automerge-repo implementation used here; provides `DocHandle`
  with a `changes()` stream.
- **autosurgeon** — `Hydrate`/`Reconcile` derive bridge between Rust structs
  and Automerge documents.
- **DocHandle** — a handle to an Automerge document in a samod repo; the bridge
  for chat and (future) collaborative content.

## Cross-cutting

- **`cx.willow()`** — extension-trait accessor (`WillowExt`) returning a
  `WillowCx` over the global Willow state. Methods: create namespace/subspace,
  create/list/read entries.
- **`cx.vaults()`** — extension-trait accessor (`VaultsExt`) returning a
  `VaultsCx` over the vault actor. Methods: create/list/lock/unlock/read/
  read_metadata/update. All return `Task<…>`.
- **`gzed`** — the application binary and lib name (`crates/app-galvanized`).