# T5 — Contacts persistence (ProfileID + last-known EndpointId)

**Status:** DRAFT
**Depends on:** T2 (soft — can ship on explicit `cx.willow()` first,
migrate to `EntryHandle` when T2 lands)

## Why

The Contacts app must persist, per added contact, at least:

- The **ProfileID** (public key / Willow subspace id) of the remote
  contact — the stable identity.
- The **EndpointId** where the peer was last located — an Iroh
  endpoint, deliberately *distinct* from the Profile (endpoints are
  rotatable/ephemeral; associating the two is a privacy cost Nick works
  to avoid — see `ARCHITECTURE.md` and dev log 2026-05-03). The
  EndpointId is a *hint* for reconnect, not an identity.

This persistence is what lets the user close and reopen the app and
still see their contacts, and what T6 (auto-reconnect) reads from.

## Current state (anchors — re-derive before leaning on)

- `ContactData` (`crates/plugin-contacts/src/lib.rs:489`):
  ```rust
  pub struct ContactData {
      name: ContactName,          // { profile_id: SubspaceId, display_name }
      endpoint_id: Option<EndpointId>,
  }
  ```
  ProfileID is already stored (as `ContactName.profile_id`).
  `endpoint_id` exists but is **never populated** — `from_ticket`,
  `from_data`, `from_profile` all set it to `None`; `set_peer` does not
  update it from the live peer connection.
- `Contact::save` (`:629`) serializes `ContactData` and writes via
  `cx.willow().create_entry(namespace, subspace, path, data)` at
  `/apps/contacts/<profile_id_hex>`. Save exists but is **not called
  after `submit_contact_ticket`** — the contact is created in memory
  only. Verify by reading `submit_contact_ticket` (`:250`).
- `Contacts::load` (`:155`) lists entries under `/apps/contacts` and
  deserializes to `ContactData`. Verify whether `load` is invoked on
  app open (check `init` at `:76` and the `Galvanized` registration);
  if not, persistence is incomplete end-to-end.
- The contact ticket (`ContactTicket`, `:650`) carries `addrs:
  EndpointAddr` (the ticket-time endpoint + addresses) and the sender's
  ProfileID; `from_ticket` keeps the ProfileID but discards `addrs` into
  the `ContactData` (it only stores display name + profile id).
- Persistence path uses the *local* Profile's subspace as the Willow
  subspace (correct — contacts are stored under the owner's Profile),
  and the Space's namespace.

## Scope

1. **Populate `endpoint_id` on connect.** When `Contact::set_peer`
   establishes a live peer (`:585`), capture the peer's `EndpointId`
   (from the connection / the `ContactPeer`) and store it in
   `ContactData.endpoint_id`. This is the "last located" hint. Be
   explicit that this is a *last-known* hint, not a stable identity.

2. **Persist on add and on change.** Call `Contact::save` (or migrate
   to the T2 `EntryHandle` auto-persist seam) when:
   - A contact is added from a ticket (`submit_contact_ticket`).
   - The `endpoint_id` updates on connect (T6's reconnect may update it
     too — make the update path shared).
   - The display name updates from the automerge doc (the `set_peer`
     listener at `:590` updates `data.name.display_name`; persist on
     that change, debounced if chatty — coordinate with T2's auto-persist
     decision).
   Avoid duplicate entries: use the ProfileID-keyed path so re-saving
   upserts rather than appends. Check whether `create_entry` upserts or
   inserts a new entry at the same path (if the latter, the persistence
   path needs an update-by-replace or a delete-then-insert — flag if so).

3. **Load on app open.** Ensure `Contacts::load` is invoked when the
   Contacts app is opened / a Profile is activated, so persisted
   contacts appear. If today's `init` doesn't call `load`, wire it.
   Surface an honest empty state when there are no contacts (per
   `CONVENTIONS.md`).

4. **Migrate to T2 `EntryHandle`** when T2 lands (or keep explicit
   `cx.willow()` if T2 is delayed). The contact is the obvious first
   `Willowize` consumer — `ContactData` already satisfies the bounds
   (`Serialize + Deserialize`; `JsonSchema` only if T2 keeps that bound).

5. **Privacy.** Contacts are sensitive ("some of the most personal
   information there is", dev log 2026-05-03). The persisted store lives
   under the owner's locked Profile's Willow namespace, gated by T1's
   capability. Confirm the persistence path is under a capability-gated
   namespace (depends on T1). Do not persist contacts to a namespace
   that is not behind the vault's capability.

## Out of scope

- Auto-reconnect and ping display (T6).
- Migrating the contact ticket's `addrs` into the stored contact as a
  reconnect fallback — that's a T6 enhancement; T5 stores the
  `EndpointId` last seen. (If you want `addrs` stored too, say so in
  feedback — it's a one-line addition to `ContactData`.)
- Cross-peer sync of the contact list (contacts are local-only for
  now; they are not synced to other peers).

## Design questions for Nick

1. **Store `addrs` (the ticket-time `EndpointAddr`) too, or only the
   last `EndpointId`?** Recommendation: store both — `EndpointId`
   (last seen) for display + a fallback `EndpointAddr` (from the
   ticket, updated on successful reconnect) for T6's reconnect. One
   field or two? Confirm.

2. **Upsert vs. append at the Willow path.** If `create_entry` does not
   upsert, do we (a) delete-then-insert, (b) add an `update_entry` to
   `WillowCx`, or (c) rely on T2's `EntryHandle` save semantics?
   Recommendation: (b) — add an explicit `update_entry` to `WillowCx`
   regardless, since it's a gap. Confirm.

3. **Display name persistence from the automerge doc.** The remote peer
   owns its display name via the shared doc; do we persist the
   remote-provided name into our local `ContactData`, or always read it
   live from the doc when connected? Recommendation: persist as a
   cache (so it shows when offline) but prefer the live value when
   connected. Confirm.

## Acceptance criteria

- Adding a contact from a ticket writes a persisted entry under
  `/apps/contacts/<profile_id_hex>` in the owner's Profile subspace.
- Connecting to a contact updates `endpoint_id` and persists it.
- Closing and reopening the app (with the Vault unlocked) shows the
  same contact list, including the last-known `endpoint_id`.
- The persisted store is under a capability-gated namespace (T1).
- Honest empty state when no contacts exist.

## Verification

- `cargo fmt && cargo clippy && cargo test` for `plugin-contacts` and
  `plugin-willow` (if `update_entry` is added there).
- `cargo run`: add a contact, connect (or simulate), close the app,
  reopen, unlock, open Contacts — confirm the contact + endpoint id
  are present. Inspect the Willow store (list entries) to confirm the
  persisted shape.

## Records

- Update `ARCHITECTURE.md` "Persistence" / Contacts section.
- Strike the "Contacts as a first-class, locked app" open question
  in `OPEN_QUESTIONS.md` once persistence + lock-gating land (or
  narrow it to whatever remains).