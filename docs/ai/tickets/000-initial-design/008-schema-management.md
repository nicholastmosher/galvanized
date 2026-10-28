# T8 — Revisit schema management for `Willowize`

**Status:** DRAFT
**Depends on:** T2 (lands the `Willowize` bound with `JsonSchema` as a
stand-in)

> **Note:** Spawned at T2 refinement (Nick, this session). `JsonSchema`
> (schemars) is kept on `Willowize` for now as a stand-in for a future
> schema-management tool; this ticket revisits that decision. To
> refine later — scope/design questions are deliberately open.

## Why

`Willowize` (the trait for types that bind into a Willow namespace via
`EntryHandle`, T2) currently requires `JsonSchema` from `schemars`. That
bound is a **placeholder for a future schema-management tool** — there's
no consumer of the schema today. Carrying an unused bound has a cost
(it narrows the set of bindable types to those that derive `JsonSchema`),
and leaving the intent unspoken risks the bound becoming load-bearing
by accident.

This ticket revisits the schema question once T2 has a working seam and
at least one consumer (Contacts via T5): decide what schema (if any)
`Willowize` should require, where the schema comes from, and what it
drives (sync validation? migration? UI generation? nothing?).

## Current state (anchors — re-derive before leaning on)

- `crates/plugin-willow/src/model.rs:9` — `trait Willowize: 'static +
  JsonSchema + Serialize + for<'de> Deserialize<'de>`. `JsonSchema` is
  the stand-in. T2 keeps it; T8 revisits.
- `schemars` is already a workspace dep (the `model.rs` `doit` test at
  `:106` calls `schema_for!(T)`).
- No code reads a `Willowize` type's schema today — the bound is
  unused at runtime.

## Scope (provisional — to refine with Nick)

1. **Decide whether `Willowize` needs a schema bound at all.** Options:
   (a) drop it — bind any `Serialize + Deserialize` type; revisit if a
   consumer needs schemas; (b) keep `JsonSchema`; (c) replace with a
   Galvanized-specific schema trait (e.g. `galvanized_cap::Schema` or a
   new `galvanized-schema` crate) if schema becomes load-bearing for
   sync/migration.

2. **Decide what a schema drives.** Candidate consumers:
   - Schema-validated sync (reject a peer's update that doesn't match
     the expected schema) — relevant to T4 (capability-gated automerge)
     and to future Willow peer sync.
   - Data migration (versioned schemas, evolve a type across releases).
   - UI generation (render a bound object from its schema).
   Recommendation: don't add a consumer until one is concretely needed;
   keep this ticket about *the bound on the trait*, not about building a
   consumer.

3. **If keeping a schema bound, pick the source.** `schemars` (JSON
   Schema) vs. a domain-specific schema. Decide with Nick.

## Out of scope (provisional)

- Building a schema-validated sync engine (that's T4 / future Willow
  sync). T8 is about the *trait bound*, not a consumer.
- UI generation from schemas.

## Design questions for Nick (to refine later)

1. **Drop, keep, or replace the `JsonSchema` bound?** Recommendation:
   drop it now (YAGNI — no consumer); re-add when a concrete consumer
   arrives. Confirm at refinement.
2. **If a schema is needed later, what drives it?** Sync validation,
   migration, or UI? (Deferred — answer when a consumer is real.)
3. **`schemars` vs. a custom schema trait** — only relevant if we keep
   a bound.

## Acceptance criteria (provisional)

- A clear, recorded decision on the `Willowize` schema bound (drop /
  keep / replace) with the rationale.
- If dropped: `Willowize` widens to `Serialize + Deserialize` and the
  `schemars` dep is removed from `plugin-willow` if nothing else uses
  it. Existing consumers (Contacts via T5) still build.
- If kept/replaced: the bound and its first consumer are named, and
  the decision recorded in `DECISIONS.md`.

## Verification

- `cargo fmt && cargo clippy && cargo test` for `plugin-willow` and the
  T5 consumer.
- `cargo run`: confirm the Contacts binding still round-trips after
  the bound change.