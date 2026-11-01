# DECISIONS

Nick's ratified rulings, verbatim where possible. **Append at the true tail.**
Later entries supersede earlier. Read the tail first.

> Everything below is `INFERRED from README` — distilled from the dev log, not
> quoted from chat. Each is marked as such until Nick confirms in conversation,
> at which point it becomes `RATIFIED` with the quote. Treat the `INFERRED`
> entries as the best reading of recorded intent, not as binding policy Nick
> has personally ratified to an agent.

---

## 2026-01-28 — INFERRED from README
Project vision: a local-first, peer-to-peer, pro-freedom/pro-sovereignty
application platform. Data ownership, privacy, and anonymity are first-class.
(README "Project Vision and Values".)

## 2026-02-14 — INFERRED from README
Namespace-as-filesystem-root / subspace-as-user mental model:
`/<namespace>/<subspace>/` with conventions like `/apps/<app_key>` and `/tags/`.
Capabilities via Meadowcap. (README 2026-02-14.)

## 2026-02-15 — INFERRED from README
Object/feed UI principle: the app is feeds of objects; an app defines a data
type + a render function and shows up in feeds. Objects are first-class in the
UI. (README 2026-02-15.)

## 2026-02-24 — INFERRED from README
`Rust-only` commitment: no C-style bridge; Rust's types/traits are the
composition substrate. (README 2026-02-24, "Project vision (continued)".)

## 2026-03-12 — INFERRED from README
UI module layout convention: `components/` (reusable `RenderOnce`),
`scenes/` (higher-level `Render` views), `state/` (domain Entities),
`willow/` (data-access DSL). (README 2026-03-02.)

## 2026-04-14 — INFERRED from README
Renamed the project **Tagged → GalvaniZed**. New icon, new intro, TOC with
jump links. (README 2026-04-14.)

## 2026-04-14 — INFERRED from README
Blanket-impl plugin composition is possible: `impl<P: PineapplePlugin>
PineappleSlicesPlugin for &mut App` — checked, it works — but "I have not yet
seen any case where this is needed." Don't reach for it without reason.
(README 2026-04-14.)

## 2026-05-04 — INFERRED from README
Vault approach: capsec with custom permissions to represent the capability to
unlock secrets; requesting a capability prompts an unlock popup if locked.
All Space/Profile operations must go through the vault and be generalizable to
other kinds of secrets. (README 2026-05-04.)

## 2026-05-19 — INFERRED from README
`VaultActor` is the capability manager / gatekeeper. On unlock it yields a
timed, scoped, revocable capability. Locking revokes all capabilities.
Locking timeout is defense-in-depth: capability *and* in-memory representation
both expire. (README 2026-05-19.)

## 2026-05-28 — INFERRED from README
Domain model: a Profile is represented by **one Vault** that stores **0-to-many
subspaces and 0-to-many namespaces**. A Subspace is one "persona". One Profile
↔ the human + password; many Subspaces = many personas. (README 2026-05-28.)

## 2026-06-21 — INFERRED from README
**"User" is dropped as a domain term; "Vault" replaces it.** A Vault is the
first top-level item a user creates (the thing with the password) and
*contains* Profiles and Spaces. (README 2026-06-21.)

## 2026-06-28 — INFERRED from README
Each Profile has a semi-special **Home Space** (where Contacts, Endpoints,
Capabilities live). By convention a Profile always has access to the Full Area
of its own Home Space. (README 2026-06-28.)

## 2026-07-11 — INFERRED from README
Need a parallel between capabilities-as-signed-objects and type-level
capabilities. Profile Home Spaces get an encrypted vault by default.
In-Willow-FS vault ≠ top-level master-password vault. CRDT sync should verify
capabilities before agreeing to sync with a peer (may need samod-core).
(README 2026-07-11.)

## 2026-07-29 — INFERRED from README
**Moving Galvanized off Zed.** It is now its own independent application built
on gpui-component, no longer a plugin inside the Zed editor. In-repo minimal
Workspace reproduction (`galvanized-core`). Recreating Zed's Panes (tabbed,
splittable) is the current arc. Port Zed-UI components to gpui-component
equivalents; `ActiveTheme` must come from gpui-component (Zed's panics on the
missing global). (README 2026-07-29.)

---

## 2026-07-30 — RATIFIED
Dropped the `./script/clippy` line from the Rust/GPUI coding core. Nick's
ruling (chat, 2026-07-30): "drop the suggestion about ./script/clippy, that
is left over from a direct copy/paste from zed and we don't need it."
Galvanized has no `script/` dir; use `cargo clippy` directly. The line was
inherited verbatim from Zed's `.rules`; same session also moved the whole
Rust+GPUI core from repo-root `.rules` into `docs/ai/RUST_GPUI.md` (`.rules` is
now a pointer to `AGENTS.md`). Resolves the former "Stale `./script/clippy`"
open question.

<!-- Append new rulings below this line, at the true tail. Mark RATIFIED with
     a quote, or PROPOSED/INFERRED until confirmed. -->

## 2026-07-30 — RATIFIED
**Don't run clippy unless Nick specifically asks.** Nick's ruling (chat,
2026-07-30): "Add a directive to AI memory not to worry about clippy unless I
specifically ask."

This supersedes the "run `cargo fmt`, `cargo clippy`, and `cargo test`" step in
`AGENTS.md` ("Build, run, verify") and the equivalent in
`docs/ai/RUST_GPUI.md` for the clippy portion only — `cargo fmt` and `cargo test`
for touched crates still apply. Agents should not invoke `cargo clippy` (or
Zed's `./script/clippy`) as part of normal verification; only when Nick asks
for it explicitly. Warnings surfaced incidentally (e.g. from `cargo build`) can
be noted, but don't gate work on them.

## 2026-11-01 — RATIFIED
**`galvanized-sync` is a new, from-scratch crate; samod is reference-only.**
Nick's ruling (chat, 2026-11-01, across the `001-willow-automerge-sync` arc):
the automerge sync engine is built from scratch as a new crate
`galvanized-sync` in this repo, with Willow identity and deny-by-default
capability gating baked in. `samod`/`samod-core` are a reference implementation
we are inspired by (sans-IO actor shape, codec ideas), **not a dependency and
not a compatibility target**. No `automerge-repo`/JS interop is preserved.
Privacy is the primary case; public sharing is a granted exception (a kind of
capability), never the default. Every sync requires positive proof of a valid
signed capability; there is no ungated path. This **supersedes** the earlier
"extend samod-core with trait seams to preserve JS interop" direction.

The `001` ticket's crate-layout rulings (Nick, chat, 2026-11-01):
- "One crate is fine" — `galvanized-sync` ships the sans-IO actor core and the
  built-in Willow authenticator/validator in a single crate.
- "Concrete is fine" — `PeerIdentity` carries a concrete `SubspaceId` +
  `Vec<AutomergeSyncCapability>` (no opaque payload).
- "Single crate is fine" — one crate with sans-IO actors + a thin async façade
  (no `galvanized-sync-core` + tokio wrapper split).

Full arc: `docs/ai/tickets/001-willow-automerge-sync/` (INDEX, SPEC,
BRAINSTORM, tickets 001–008).