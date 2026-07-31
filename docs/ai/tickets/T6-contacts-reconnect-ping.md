# T6 — Contacts auto-reconnect + ping display

**Status:** DRAFT
**Depends on:** T5 (persisted last-known EndpointId), T3 (clean ping
request/response — or fall back to the contacts control stream)

## Why

The contacts app should **automatically, periodically attempt to
reconnect** to any contact in the list, using the last-known
`EndpointId` (and the ticket-time `EndpointAddr` if stored — see T5
§1). On successful connection it should display the **ping (latency)**
to each remote peer, so the user can see which contacts are reachable
and how fresh the link is.

This is the user-visible payoff of T5's persistence: a contact you
added last week should come back online when they're reachable again,
without you re-entering their ticket.

## Current state (anchors — re-derive before leaning on)

- `Contact` (`crates/plugin-contacts/src/lib.rs:465`) holds
  `data: ContactData` (incl. `endpoint_id: Option<EndpointId>`) and
  `peer: Option<ContactPeer>`. `set_peer` (`:585`) emits
  `ContactEvent::PeerConnected`; `clear_peer` (`:623`) emits
  `PeerDisconnected`. There is **no background reconnect loop** and
  **no ping measurement** today.
- `ContactsProtocol::connect` (`crates/plugin-contacts/src/protocol.rs:293`)
  dials a `EndpointAddr` from the ticket. It does not currently dial
  from a bare `EndpointId` + last-known addrs — reconnect needs a
  `EndpointAddr` (id + addrs). This is why T5 §1 asks whether to store
  `addrs`.
- `ContactsProtocol` (`protocol.rs:52`) already has a `ping` field on
  its state (`:88`) — appears to be a placeholder/stub; check before
  building (it may already be a partial ping implementation; if so,
  build on it and don't duplicate).
- The `ContactsProtocolState` (`protocol.rs:66`) tracks
  `contact_peers` (active) and `pending_contact_peers`; reconnect
  would move a contact from "no peer" to "pending" to "active" via the
  existing handshake.
- There is no periodic task spawned in `Contacts::init` (`:76`) or on
  `load` (`:155`) today.

## Scope

1. **Background reconnect loop.** Spawn a periodic task (per active
   Profile / per `Contacts` instance) that iterates the contact list
   and, for any contact without a live peer, attempts to reconnect
   using the persisted last-known `EndpointId` (+ `EndpointAddr` if
   stored). Coordinate the interval and backoff with Nick (design Q1).

2. **Reconnect via the existing handshake.** Reconnect should go
   through `ContactsProtocol::connect` (or the T3 framework once
   migrated) so the same `RequestConnectContact` /
   `AcceptConnectContact` handshake authenticates the peer's Profile
   via the `SignedEnvelope`. Do not add a raw `endpoint.connect` path
   that bypasses Profile authentication.

3. **Ping measurement.** On a live connection, measure round-trip
   latency to the remote peer. Options (rule with Nick):
   - (a) A dedicated `Ping` message on the contacts protocol (over T3
     or the existing control stream) with a timestamp, replied to
     immediately; rtt = now - sent. Periodic re-ping for live display.
   - (b) A p2p-level ping primitive in `plugin-p2p` (more reusable,
     useful beyond contacts).
   Recommendation: (a) first (contacts-scoped, ships fastest); promote
   to (b) when a second consumer needs it. Confirm.

4. **Display.** Surface, per contact in the list, the connection state
   (disconnected / connecting / connected) and, when connected, the
   ping (e.g. "12ms" or a colored chip). Update the existing
   `render_contact_list` (`:394`) and the `ContactEvent` flow so the
   UI refreshes on connect/disconnect and on ping updates. Honest
   "connecting…" / "disconnected" states — no lying placeholders
   (`CONVENTIONS.md`).

5. **Connection-state events.** Wire `PeerConnected` / `PeerDisconnected`
   (already emitted) into the list's re-render, and add a
   `PeerPingUpdated` (or similar) event for ping changes so the UI
   doesn't re-render the whole list every tick.

## Out of scope

- A general p2p-level ping primitive (unless Nick wants it now — Q3).
- Reconnect policy UI / settings (interval/backoff are constants for
  now, surfaced to Nick for ruling).
- Reconnecting to contacts whose `EndpointId` is stale beyond
  recovery — if the peer has rotated endpoints and the ticket addrs are
  dead, the contact stays "disconnected"; discovery is out of scope
  (`OPEN_QUESTIONS.md` "Discovery").

## Design questions for Nick

1. **Reconnect interval & backoff.** Base interval (e.g. 10s, 30s?)
   and backoff (fixed, exponential up to a cap, jittered?) Recommendation:
   30s base, exponential to 5min cap, jittered ±20%. Confirm.

2. **Ping mechanism location.** Contacts-scoped message now, or
   p2p-level primitive? Recommendation: contacts-scoped now (Q3). Confirm.

3. **Ping cadence & display.** How often to re-ping a connected peer
   (e.g. every 5s)? Display as a number, a colored chip, or both?
   Recommendation: every 5s, show "Nms" + a green/yellow/red chip.
   Confirm.

4. **Reconnect target when only `EndpointId` is stored (no `addrs`).**
   If T5 stores only `EndpointId`, can we dial it? Iroh needs addrs to
   reach a peer; a bare `EndpointId` is not dialable without a
   resolver. Recommendation: T5 must store `addrs` too (or the ticket
   must be re-persisted) — this couples T6 to T5 §1. Confirm we store
   `addrs`.

5. **Background task lifecycle.** Spawn per `Contacts` instance, per
   Profile, or one global reconnect supervisor? Recommendation: per
   active Profile (matches the per-Profile p2p model), cancelled when
   the Profile is locked (T1) or the app closes. Confirm.

## Acceptance criteria

- With the app open and a contact whose peer is reachable, the contact
  transitions to connected without user action; the ping displays.
- If the peer goes away, the contact shows "disconnected" within one
  reconnect cycle; when the peer returns, it reconnects and re-shows
  ping.
- Ping is measured from a real round-trip (not a stub); the value
  updates on the cadence Nick rules on.
- Reconnect uses the Profile-authenticated handshake (signed
  envelope), not a raw unauthenticated dial.
- Reconnect loop stops when the Profile is locked (T1 capability
  revocation) — no background p2p activity on a locked Profile.

## Verification

- `cargo fmt && cargo clippy && cargo test` for `plugin-contacts` and
  `plugin-p2p` (if a ping primitive lands there).
- Two-peer test: peer A adds peer B; B disconnects (closes app);
  A shows "disconnected"; B reopens; A reconnects within one cycle and
  shows ping. If two live instances aren't drivable in this session,
  verify the loop + ping logic with an in-process two-`Endpoint` test
  and state what you ran.

## Records

- Update `ARCHITECTURE.md` Contacts section.
- Add a reconnect/ping note to `PLUGIN_RECIPE.md` if it becomes a
  pattern other app plugins would want (chat likely will).