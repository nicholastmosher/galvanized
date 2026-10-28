# T2 — Willow FS object binding (`Willowize` / `EntryHandle`)

**Status:** REFINED
**Depends on:** T1 (capability gate for writes)

## Why

Higher-level plugins should persist domain objects into a Willow
namespace with minimal ceremony: bind an object to a `(namespace,
subspace/Profile, path)`, and have updates to the object persist
automatically. The intended platform API for this is the
`Willowize`/`EntryHandle` seam, which today is a commented-out skeleton.
Closing this seam is what makes "any app that wishes to persist data"
trivial and consistent — and keeps capability gating uniform across
all apps (because every save goes through one path).

## Current state (anchors — re-derive before leaning on)

- `crates/plugin-willow/src/model.rs:9` — `trait Willowize: 'static +
  JsonSchema + Serialize + Deserialize`. `EntryHandle` trait for
  `Entity<T: Willowize>` with `load`/`save`/`sync` all **commented out**.
  The commented bodies show the intended design: resolve active
  profile + space from `cx.willow()`, build a Willow `Entry`, mint a
  `WriteCapability` from the subspace+namespace keys, insert into the
  store. Treat as design intent, not a working API
  (`docs/ai/PLUGIN_RECIPE.md` §5 says: "don't depend on it").
- `crates/plugin-willow/src/lib.rs` — `WillowCx` with
  `create_entry(namespace, subspace, path, data)` (`:170`), `list_entries`
  (`:209`), `read_entry` (`:234`). These are the explicit methods app
  plugins use today. `WillowState` (`:73`) holds a `MemoryStore`; the
  on-disk `_store_path` is unused (open question, separate from this
  ticket).
- `crates/plugin-contacts/src/lib.rs:155` (`Contacts::load`) and
  `:629` (`Contact::save`) are the current reference consumers of the
  explicit API: serialize `ContactData` to bytes, write via
  `cx.willow().create_entry(...)`, list + read on load. This is the
  shape T2 replaces with a bound object.
- `crates/plugin-willow/src/willow_serde.rs` — serde adapters
  (`NamespaceIdSerde`, etc.) for Willow ids.

## Scope

1. **Settle the `Willowize` / `EntryHandle` API** (open question in
   `OPEN_QUESTIONS.md`: "Is this the intended platform API?"). Build a
   working `EntryHandle` for `Entity<T: Willowize>`, **1:1 with an
   entity** (RESOLVED, Nick this session — see Design Q4; an entity
   needing multiple targets composes multiple handles):
   - `bind(namespace, subspace, path, cx) -> EntryHandle` (or a builder)
     that registers the entity with the Willow store under a path.
   - `load(cx)` — read the entry, deserialize, populate the entity.
   - `save(cx)` — serialize the entity, write via `cx.willow()`, gated
     by the T1 capability (`StrictCap<WillowWrite, scope>`).
   - `sync(cx) -> Subscription` — observe the entity and auto-save on
     change (the commented `cx.observe(self, |this, cx| this.save(cx))`
     pattern). **Auto-persist is on by default with an opt-out**
   (RESOLVED, Nick this session): `cx.observe(entity, save)` saves on
   every field change, fine for low-write domain objects (Contacts);
   callers who want explicit-save-only pass an opt-out flag/variant.

2. **Resolve the "active context" question.** — RESOLVED (Nick,
   this session): pass `(namespace, subspace, path)` **explicitly** at
   bind time; no `active_profile()` / `active_space()` globals. The
   commented skeleton reached for `cx.willow().active_profile()` /
   `active_space()` which don't exist; that approach is dropped. The
   binding is a one-time call, not per-save.

3. **Capability integration with T1.** `save` must obtain the
   `StrictCap<WillowWrite, scope>` for the `(namespace, subspace, path)`
   it writes to. If the capability is missing/expired, the save fails
   visibly (per `CONVENTIONS.md` error-visibility doctrine) — not a
   silent drop.

4. **`JsonSchema` requirement.** — RESOLVED (Nick, this session):
   keep `JsonSchema` (schemars) on `Willowize` for now as a stand-in for
   a future schema-management tool. A follow-up ticket (**T8**) is
   spawned to revisit schema management (the bound on `Willowize`, the
   schema source, and whether/how schemas drive sync validation). T2
   keeps `JsonSchema` to avoid blocking on that design.

5. **Path & id conventions.** A bound object needs a stable path so
   reload finds it. Define the convention (e.g. `/apps/<app_id>/<id>`
   used by `Contacts` today) and a way to derive it. Decide whether the
   path is owned by the app (string) or structured
   (`willow25::path`). The `WillowPathExt` trait
   (`crates/plugin-willow/src/lib.rs:512`) already converts to willow
   paths; lean on it.

6. **Migration of one reference consumer** to the new seam (Contacts
   is the obvious candidate — overlaps with T5). Keep the explicit
   `cx.willow()` API available for cases the seam can't express (e.g.
   listing entries).

## Out of scope

- Willow on-disk store (separate open question). Build against
  `MemoryStore`; the seam should be store-agnostic.
- Cross-peer sync of bound objects (that's T4 for automerge; Willow
  sync between peers is a larger separate question).
- Removing the explicit `cx.willow()` API — keep it for listing and
  bulk operations.

## Design questions for Nick

1. **Active-context vs explicit parameters.** — RESOLVED (Nick,
   this session). Explicit `(namespace, subspace, path)` at bind time;
   no `active_profile()`/`active_space()` globals. See Scope §2.

2. **Auto-persist trigger.** — RESOLVED (Nick, this session).
   `cx.observe(entity, save)` on by default with an opt-out (save on
   every field change; fine for low-write objects). See Scope §1
   `sync`.

3. **`JsonSchema` requirement.** — RESOLVED (Nick, this session). Keep
   `JsonSchema` (schemars) for now as a stand-in for a future
   schema-management tool; **T8** spawned to revisit. See Scope §4.

4. **One entity ↔ one entry, or one entity ↔ many?** — RESOLVED
   (Nick, this session): **1:1 + composition.** The core `EntryHandle`
   binds one `Entity<T>` to one Willow path. An entity that needs
   multiple persistence targets holds multiple `EntryHandle`s (e.g. a
   `Chat` binds `metadata` to one path and `messages` to another via two
   handles); the entity owns the composition, the platform stays 1:1.
   No 1:many / fan-out primitive — it would add partial-failure and
   cross-target consistency complexity with no near-term need.

## Acceptance criteria

- A type implementing `Willowize` can be bound to a path, saved, and
  reloaded (round-trip) through `EntryHandle`, against `MemoryStore`.
- Auto-persist: mutating the entity (when subscribed) writes a new
  entry to the store without an explicit `save` call from the app.
- A save without a valid `StrictCap<WillowWrite, scope>` (from T1)
  fails with a visible error, not a silent drop.
- At least one consumer (Contacts, via T5) migrates to the seam; the
  explicit `cx.willow()` API still exists and still works.

## Verification

- `cargo fmt && cargo clippy && cargo test` for `plugin-willow` and
  the migrated consumer crate.
- `cargo run`: bind a test object, mutate it, confirm a new Willow
  entry appears (via a list call or a debug view); reload and confirm
  the value round-trips.
- Negative test: revoke/expire the capability and confirm the next
  save surfaces an error.

## Records

- Move the "`Willowize` / `EntryHandle` unified handle" open question
  in `OPEN_QUESTIONS.md` to `DECISIONS.md` once the API shape is ruled
  on.
- Update `PLUGIN_RECIPE.md` §5 to point at the working seam (replace
  the "don't depend on it" caveat).
- Update `ARCHITECTURE.md` "The `cx.willow()` … APIs" section.