# GOODIES — the Goodie proposal process & registry

> **Charter + registry.** This file defines what a "Goodie" is, the process
> for proposing one, and the registry of proposals (PROPOSED → IMPLEMENTED
> / DECLINED). It is the counterpart to the [memory system](README.md) for
> *tools* rather than *decisions*: decisions live in `DECISIONS.md`; the
> tools that make the work less repetitive live here, with their scripts
> under [`docs/ai/scripts/`](../../scripts/).

## What a Goodie is

A **Goodie** is a small script or tool the agent may propose to create under
[`docs/ai/scripts/`](../../scripts/) in order to remove a **repetitive,
heavy, or error-prone** task that shows up across the work — most often
across tickets, verification, or memory maintenance.

A Goodie is *not* a feature of the app. It is a **dev-ergonomics tool** for
the agent/Nick working *on* the project, not something the `gzed` binary
ships. If a task is part of the product surface (e.g. a Willow store
dumper that real users need), it is a ticket, not a Goodie. Goodies live
outside the crates; they never become runtime dependencies of `gzed`.

## When to propose one (the agent's standing instruction)

As part of routine work, the agent keeps an eye out for tasks that are:

- **Repetitive** — the same mechanical step appears across multiple
  tickets / sessions (e.g. every ticket's "re-derive before leaning on"
  anchor check; every ticket's `cargo fmt && cargo clippy && cargo test`
  verification block).
- **Heavy** — a single task with enough boilerplate that doing it by hand
  each time is a real cost (e.g. scaffolding a two-peer in-process test).
- **Error-prone** — a step where a quiet slip causes a wrong claim later
  (e.g. a stale file:line anchor leading the agent to lean on code that
  moved; a transcribed crate list dropping a crate from verification).

When the agent sees such a task, it **proposes** a Goodie. It does *not*
build it unprompted — see "Decisions are Nick's" below.

## Decisions are Nick's (the discipline this inherits)

Per [`AGENTS.md`](../../../AGENTS.md) and the [memory rules](README.md):

- Writing a new script is a small **mechanism-shaped** call. It goes to
  Nick as a proposal first, never a pre-made pick he has to veto.
- Every proposal is explicitly marked `PROPOSED — awaiting Nick` until he
  approves. No invented defaults: a Goodie is never written as settled
  practice. On approval it becomes `IMPLEMENTED`; on a "no" it becomes
  `DECLINED` (kept for the record, not deleted).
- A Goodie that touches anything **security- or privacy-adjacent** (e.g. a
  script that reads vault content, dumps a Willow store, or reaches across
  profiles) goes to Nick with that framing *before* the proposal is even
  fleshed out — the same bar as any other security-adjacent change.
- Goodies obey the same honesty bar as the rest of the project: a script's
  doc states what it actually does, what it does *not* do, and any
  limitation. A Goodie that fakes a check (e.g. "verifies anchors" but only
  checks file existence) must say so plainly.

## The proposal format

A proposal is an entry in the **Registry** below with:

- **Name** — the script filename under `docs/ai/scripts/`.
- **Status** — `PROPOSED — awaiting Nick` / `IMPLEMENTED` / `DECLINED`.
- **Why it's needed** — the task it removes, and **evidence of recurrence**
  (cite the tickets / sessions / sections where the task repeats). No
  evidence → no Goodie; "this might be useful someday" is not enough.
- **How it works** — the mechanism, briefly. What it parses, what it
  runs, what it prints. Be honest about limits.
- **Usage** — the exact manual command Nick would run, plus what the agent
  runs in-session. If the agent will invoke it routinely, say so.
- **Scope / out of scope** — what it deliberately does *not* do (so it
  doesn't accrete into a half-built feature).

On approval, the **implemented** script gets a sibling doc:
`docs/ai/scripts/<name>.md` with the same four sections (Why / How / Usage
/ Scope) kept in sync with the script. The registry entry here then links
to it and flips to `IMPLEMENTED`.

## The `docs/ai/scripts/` directory

- One script per Goodie (shell, python, or a small standalone Rust binary
  if a shell script genuinely can't express it — but prefer shell/python;
  a Rust binary blurs the line with app crates and needs a build step).
- A sibling `<name>.md` doc per script with the justification, the
  how-it-works explanation, and manual usage instructions.
- A `README.md` in the directory listing the scripts with one-line
  descriptions and their status (mirrors this registry for quick `ls`).
- Scripts must be **read-only by default** against the repo and the tree:
  a Goodie may *report* (print drift, print what would run) but must not
  *mutate* source or git state unless its proposal explicitly says so and
  Nick approved that mutation. (Same bar as the shared-tree-discipline
  rule in `AGENTS.md`.)
- No network. A Goodie runs locally against the checkout; if one ever
  needs network (e.g. fetching a spec), that is called out in the
  proposal and approved explicitly.

## Relationship to the rest of the memory system

- **`DECISIONS.md`** records Nick's rulings. A Goodie approval is a
  ruling → on approval, the ruling ("yes, build `<name>`; scope as
  proposed") is also appended to `DECISIONS.md` same-day, and the
  registry entry here flips to `IMPLEMENTED` linking the implemented doc.
- **`OPEN_QUESTIONS.md`** is unaffected unless a proposal surfaces a
  design-open question (e.g. "should the anchor checker also re-derive
  new anchors and rewrite the ticket?" — that's a shape question for
  Nick, not a default the Goodie takes).
- **`KNOWN_TRAPS.md`** — if a Goodie itself turns out to have a gotcha,
  it goes there like any other trap.
- This file is **maintained in place** (edited, not appended) like
  `STATE.md` — the registry is a current view, not a log. The only
  append-style element is a DECLINED entry, which is kept (struck
  through) rather than removed, so "we already considered that" survives.

## Boot note

A booting agent should skim the registry below to know what Goodies
exist (and could save it work this session) and what's PROPOSED (don't
assume a PROPOSED Goodie is available yet).

---

## Registry

| Name | Status | One-line |
|------|--------|----------|

---

### DECLINED (kept for the record)

_(none yet)_
