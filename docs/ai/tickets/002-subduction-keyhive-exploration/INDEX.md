# Tickets — Subduction + Keyhive exploration

A research/exploration arc, not an implementation arc. The single ticket here
surveys the Ink & Switch **Keyhive** + **Subduction** Rust stack and frames the
strategic question for Nick: does Galvanized adopt it as the
auth + sync + E2EE substrate, superseding the in-flight hand-rolled
`galvanized-sync` (arc `001`) and the Willow-based data model?

> This arc **does not change any code**. It is a briefing plus a set of
> decisions for Nick. Per `AGENTS.md`, the call is shape-shaped and
> security-shaped — it is Nick's, not the agent's. Nothing here is a ruling
> until Nick ratifies it (then it moves to `DECISIONS.md`).

## Why a separate arc

Arc `001-willow-automerge-sync` is RATIFIED (Nick, 2026-11-01): build
`galvanized-sync` from scratch, samod as reference only, deny-by-default,
Willow identity + capabilities. This arc surfaces new evidence — a
pre-alpha but real, Rust, Ink-&-Switch-maintained implementation of exactly
that problem space — that may change the build-vs-adopt calculus. It does
**not** assume the answer; it lays out what was found and what it would cost
to adopt, then asks Nick.

## Status legend

- `DRAFT` — written, awaiting Nick's feedback.
- `REFINED` — Nick signed off the shape; ready to pick up.
- `IN_PROGRESS` — being implemented.
- `BLOCKED` — waiting on a decision or another ticket.
- `DONE` — implemented & verified.

## Tickets

| ID  | Title | Status | Depends on |
|-----|-------|--------|------------|
| 001 | Subduction + Keyhive exploration & adoption decision | DRAFT (blocked: Nick) | — |

## Relationship to other arcs

- **Supersedes-or-coexists-with `001-willow-automerge-sync`.** If Nick rules
  "adopt Subduction/Keyhive," arc `001` is either dropped or heavily
  reshaped (the from-scratch engine becomes a thin Galvanized-specific layer
  over `subduction_core` + `keyhive_core`). If Nick rules "stay the course,"
  this arc closes as research-only and `001` proceeds unchanged.
- **Touches `000-initial-design` T2/T4.** The Willow FS object binding
  (`Willowize`/`EntryHandle`) and the capability-gated automerge shared
  objects both assume Willow as the substrate. Adopting Subduction replaces
  that substrate and re-opens those designs.
- **Touches the data model in `plugin-galvanized`**
  (`Vault`/`Profile`/`Space` ↔ Willow `Subspace`/`Namespace`). Keyhive's
  identity + document model is a different shape; the mapping is an open
  question (see ticket 001 §"Identity & data-model mapping").
