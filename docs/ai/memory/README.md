# The memory system

This directory is Galvanized's long-term memory — durable across AI sessions.
It is the record agents build against, the same way the code is. Treat it as
first-class: a stale or wrong memory propagates into wrong code.

## The files

| File | Role | Maintenance |
|------|------|-------------|
| `DECISIONS.md` | Nick's ratified rulings (verbatim where possible) | **Append at the true tail**, same day. Later supersedes earlier. |
| `OPEN_QUESTIONS.md` | Design-open questions / unresolved TODOs | Resolved → move to `DECISIONS.md` with the ruling, then strike through here. |
| `STATE.md` | The current arc / what's in flight / what's next | **Maintained in place** (edit, don't append) — like a schedule. |
| `KNOWN_TRAPS.md` | Gotchas with evidence + the fix | Edited in place; add as traps are hit. |
| `GLOSSARY.md` | Domain terms as they settle | Edited in place. |
| `HANDOFF.md` | Cross-session continuity scratch | Refreshed each session; overwrite old content. |
| `GOODIES.md` | Goodie proposal process + registry (scripts under `docs/ai/scripts/`) | Maintained in place (registry is a current view, not a log). |

## The rules

1. **No invented defaults.** Nothing you merely inferred is written as settled
   policy. Every entry is one of:
   - **(a)** Nick's ruling — quoted or cited,
   - **(b)** ratified record (already in the file, dated), or
   - **(c)** explicitly marked `INFERRED` or `PROPOSED — awaiting Nick`.
   Absolutes ("never"/"always") Nick didn't say are a red flag → turn them into
   a question in `OPEN_QUESTIONS.md`.
2. **Verbatim where it matters.** When Nick rules in chat, quote him. Don't
   launder his intent through a paraphrase.
3. **Append decisions at the true tail.** Read the tail first, confirm you're
   appending after the current last entry, then write. Same day, ideally same
   hour. (Mis-inserted mid-file entries are the single most-repeated memory
   slip in projects like this.)
4. **Wrong records are fixed in place.** No wrong history left standing with a
   trailing correction. Edit the original entry; note the correction visibly.
5. **Edit-tool only.** Use the editor's read/write tools, never `cat >>` / `sed`
   / a Python `open().write()`. A scripted append silently clobbers a concurrent
   edit and bypasses file-state tracking.
6. **Answer visibility.** Anything Nick must read goes in turn-final text with
   no tool calls after it.
7. **Brainstorm ≠ ruling.** A big idea Nick floats in the dev log or chat is a
   discussion opener, not a ruling. Record it as `PROPOSED` and ratify only
   after he confirms post-discussion.

## Boot order (memory side)

1. `STATE.md` — what's in flight now.
2. The tail of `DECISIONS.md` — latest rulings supersede earlier.
3. `OPEN_QUESTIONS.md` — what's unresolved (don't bake an answer to one of
   these without going to Nick).
4. `KNOWN_TRAPS.md` — don't repeat a caught trap.
5. `GLOSSARY.md` — only if the work touches domain terms.
6. `GOODIES.md` — skim the registry to see what Goodies exist (could save
   work this session) and what's PROPOSED (don't assume a PROPOSED Goodie
   is available yet).

## Relationship to the README dev log

`README.md` is Nick's messy, living brainstorm and history. It is a *source* of
intent, not a *record of rulings*. When you mine the dev log for a decision,
copy the relevant intent into `DECISIONS.md` **marked `INFERRED from README`**
until Nick confirms it. The README is never edited by agents.
