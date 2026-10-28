# T3 — p2p message-passing framework

**Status:** DRAFT
**Depends on:** T1 (loose — profile-scoped authority), `plugin-p2p` actor

## Why

Higher-level plugins need to pass messages between peers (peers
identified by Willow Profile keys / subspace ids) with as little
ceremony as possible. Today every protocol hand-rolls the same
machinery: `LengthDelimitedCodec` framing, a run loop, an envelope,
signature verify, a message enum, and a dispatch match. See
`crates/plugin-contacts/src/protocol.rs` (the contacts handshake) and
`crates/plugin-p2p/src/v1.rs` (`GalvanizedProtocol`, the older
doc-id exchange). The peer boundary should be **nearly transparent**: a
client defines a message datatype, a client call, and a server handler,
and gets connect/send/receive for free — like an HTTP client/server
relationship expressed as traits.

## Current state (anchors — re-derive before leaning on)

- `plugin-p2p` entity: `crates/plugin-p2p/src/lib.rs:29` — `P2p` wraps
  an `Actor` that owns an Iroh `Endpoint` and a dynamic `Protocols` map.
  `KeyedProtocolHandler` trait (`:118`) is just `const ALPN: &'static
  [u8]` + `ProtocolHandler`. `P2pHandle` (`:99`) gives `register_protocol`
  / `protocol` / `endpoint_id` / `endpoint`. `P2p` is constructed
  per-Profile in `Profile::from_parts`
  (`crates/plugin-galvanized/src/domain/profile.rs:60`); `plugin_p2p::init`
  is a no-op — the p2p stack is wired through the Profile, not a global.
- Actor: `crates/plugin-p2p/src/actor/mod.rs` — `Actor` owns
  `Endpoint` + `Protocols` (cloneable); `ActorEngine::run` accepts
  incoming connections and dispatches by ALPN to the handler's
  `accept`. `ActorInput` is currently empty — there is no outbound
  message path through the actor; protocols dial peers directly via
  `endpoint.connect`.
- Hand-rolled patterns to factor out:
  - `crates/plugin-contacts/src/protocol.rs:287` — `connect` opens a
    bidi stream, spawns a recv loop, sends a `SignedEnvelope`-wrapped
    `RequestConnectContact`, and handles the `Accept`/`Reject` reply.
  - `crates/plugin-contacts/src/protocol.rs:445` — `SignedEnvelope<M>`
    with `EnvelopeMetadata { sender_id, recipient_id, timestamp }`,
    signed by the local subspace key, verified on receive.
  - `crates/plugin-p2p/src/v1.rs:375` — `Envelope { message:
    GalvanizedMessage }` over `LengthDelimitedCodec` framing.
- `plugin-automerge` (`crates/plugin-automerge/src/lib.rs:19`) and
  `plugin-contacts` both register a `KeyedProtocolHandler` per Profile
  via `cx.observe_new::<Profile>`.

## Scope

1. **A message trait + envelope.** Define a `Message` trait (today a
   trivial marker in contacts `protocol.rs:442`) and a standard signed
   envelope (today `SignedEnvelope<M>` in contacts). Promote these to
   a reusable location (likely `plugin-p2p`, since every protocol needs
   them). The envelope carries `sender_id`, `recipient_id` (Willow
   Profile / subspace ids), and a timestamp; signed by the sender's
   subspace key. Receive verifies signature + recipient.

2. **A client/server trait abstraction.** The shape Nick asked for:
   a higher-level plugin defines (a) a message datatype, (b) a client
   function (request), (c) a server handler (response), tied together
   by traits so the client does as little as possible. Concretely
   propose (for Nick to rule on):
   - `trait Messenger: KeyedProtocolHandler` — owned by a Profile,
     provides `send(peer_id, message) -> Future<Reply>` and registers
     a handler closure per message variant.
   - Request/response correlation via a request id in the envelope;
     a pending-request map (`DashMap<RequestId, oneshot::Sender<Reply>>`).
   - Server side: `trait Handler<M>` or a dispatch enum the plugin
     implements; the framework decodes the envelope, dispatches, and
     sends the reply envelope back over the same stream.
   The goal: a plugin writes a message enum + a `match` in a handler,
   and gets transport, framing, signing, and reply correlation for
   free.

3. **Outbound path through the actor.** Today protocols call
   `endpoint.connect` directly. Decide (with Nick) whether outbound
   sends go through the actor (cleaner, single owner of the endpoint)
   or stay direct (simpler, current pattern). The actor's `ActorInput`
   is empty today — adding outbound sends would populate it. Either
   way, the framework hides the choice from the plugin.

4. **Connection lifecycle.** The contacts protocol keeps a
   `pending_contact_peers` + `contact_peers` map; `v1.rs` keeps a
   `peer_state: DashMap<EndpointId, PeerState>`. Decide how much of
   this the framework owns vs. leaves to the plugin. Recommendation:
   the framework owns the per-peer connection + the pending-request
   map; the plugin owns domain state (e.g. "is this contact
   accepted"). Surface as a question.

5. **Migrate one reference protocol** to the framework (Contacts is
   the obvious candidate — overlaps with T6's ping). Keep the raw
   `KeyedProtocolHandler` escape hatch available (automerge's sync
   needs raw bidi streams, not request/response).

## Out of scope

- Automerge (samod) sync itself — it needs raw streams, not the
  request/response framework. T4 handles capability-gating that path.
- Discovery (how peers find each other). Out-of-band tickets only,
  per `OPEN_QUESTIONS.md`.
- Replacing the per-Profile `P2p` construction model. Keep it; the
  framework layers on top.

## Design questions for Nick

1. **API shape.** Is the `M { trait Handler<M>; send(peer, M) ->
   Future<Reply> }` shape right, or do you want something closer to
   (a) typed actor-style handlers (`impl Handler<RequestX>`) or
   (b) a single `match`-on-enum dispatcher per protocol? This is the
   core shape-shaped decision for T3 and must be ruled on before
   implementation.

2. **Unary vs streaming.** Start with unary request/response only, and
   add streaming later? Recommendation: yes — most contacts/p2p-app
   messages are unary; automerge already uses the raw-stream escape
   hatch. Confirm.

3. **Reply correlation & errors.** Use a `RequestId` + oneshot for
   replies; errors come back as a `Reply::Error` variant (typed) or as
   a transport-level failure? Recommendation: typed `Reply` variants for
   domain errors, transport errors for framing/signature/timeout.

4. **Sign+verify scope.** Should every framework message be signed
   (like contacts' `SignedEnvelope`), or is signing opt-in per
   protocol? Recommendation: signed by default — Profile identity is
   the whole point of p2p-by-Profile-key; unsigned is a footgun. A
   protocol that wants raw/unsigned can use the escape hatch.

5. **Outbound through the actor or direct?** See scope §3. Recommendation:
   keep direct `endpoint.connect` for now (less churn, matches today),
   revisit when we need backpressure/cancellation. Confirm.

## Acceptance criteria

- A plugin can define a message enum + handlers and send a request to a
  peer Profile, receiving a typed reply, without writing framing,
  signing, or run-loop code.
- The contacts protocol's `RequestConnectContact` /
  `AcceptConnectContact` / `RejectConnectContact` handshake is
  reimplemented on the framework (or stays on the escape hatch with a
  stated reason).
- A signature-verify failure or wrong-recipient produces a visible
  error (logged + surfaced to the caller as a transport error), not a
  silent drop.
- The raw-stream `KeyedProtocolHandler` path still works (automerge
  sync compiles and runs unchanged).

## Verification

- `cargo fmt && cargo clippy && cargo test` for `plugin-p2p` and the
  migrated protocol crate.
- A two-peer test (two `gzed` instances or a unit test with two
  in-process endpoints) exchanging a framework message end-to-end. If
  two live instances aren't drivable in this session, state that and
  verify with a unit test using two `Endpoint`s in one process.

## Records

- Add a "p2p messaging" section to `ARCHITECTURE.md`.
- Move any shape ruling (API shape, signed-by-default) to
  `DECISIONS.md`.