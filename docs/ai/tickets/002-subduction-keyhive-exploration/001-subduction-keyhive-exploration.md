# 001 — Subduction + Keyhive exploration & adoption decision

**Status:** DRAFT — blocked on Nick. This is a decision-framing briefing, not
an implementation plan. Nothing here is a ruling.
**Depends on:** —

## Why

Nick has been hand-rolling an authenticated Automerge sync substrate
(`galvanized-sync`, arc `001`) on top of Willow25 + Iroh, with
deny-by-default signed-capability gating. He asked whether the Ink & Switch
**Keyhive** + **Subduction** stack is developed enough to switch to instead,
and specifically whether `automerge-keyhive-repo` is JS-only or has a Rust
implementation.

This ticket reports what I found by reading:

- The Automerge Repo Keyhive (ARK) API guide:
  <https://automerge.org/docs/keyhive/ark-api-guide/>
- The `inkandswitch/subduction` repo, focused on `subduction_iroh`
  (Cargo.toml, README, and `src/{lib,client,server,transport,handshake}.rs`).
- The `subduction_keyhive` crate (`src/{lib,policy,handler}.rs`).
- The `inkandswitch/keyhive` repo README.
- The `keyhive_core` / `keyhive_crypto` crates on crates.io.
- This repo's `docs/ai/` memory + arc `001` (for the in-flight direction).

## TL;DR

1. **There IS a Rust implementation.** `@automerge/automerge-repo-keyhive`
   (ARK) is a thin JS wrapper over WASM. The actual core is Rust, published
   on crates.io, and lives across two Ink & Switch repos:
   - **`keyhive`** workspace → `keyhive_core` (v0.10.0, published
     2026-10-28), `keyhive_crypto`, `beekem` (concurrent TreeKEM),
   `keyline`/`keyline_memory` (delegation/revocation cert store),
   `keyhive_content` (encrypted content + ciphertext store),
   `keyhive_wasm`. ~12k Rust LOC in `keyhive_core` alone, Apache-2.0,
   actively shipped (11 versions since 2025-06).
   - **`subduction`** workspace → `subduction_core` (the sync protocol,
     successor to Beelay), `subduction_iroh` (QUIC transport), and
     `subduction_keyhive` (the bridge: `ConnectionPolicy` +
     `StoragePolicy` impls on top of `keyhive_core::Keyhive`).
2. **It is pre-alpha.** Both repos say "DO NOT use in production," no
   security audit, unstable APIs. But it is real, not a shim, and it is
   exactly the problem Galvanized is hand-solving: auth-enabled sync over
   E2EE data with delegation + revocation.
3. **It does NOT use Willow.** Subduction uses **Sedimentree** for
   hash-sharded data partitioning and its own authority graph (**Keyline**)
   for delegation/revocation. Adopting it means moving the sync/auth
   substrate off Willow — a shape change to the `000` data model, not a
   drop-in transport.
4. **The privacy stance aligns strongly.** Keyhive is E2EE + capability
   first; "public" is a grant to a special Public member, never a default
   mode — the same framing arc `001` RATIFIED ("public is a kind of
   capability, not a mode of the engine").
5. **`subduction_iroh` does not today implement `iroh::ProtocolHandler`.**
   It owns its own `endpoint.accept()` loop. It can be wrapped as a
   `ProtocolHandler` with a small refactor (see §"Can we wrap it in an Iroh
   `ProtocolHandler`?"), and it would then slot into Galvanized's existing
   `plugin-p2p` `P2p::register_protocol` ALPN dispatch.

This is a build-vs-adopt decision and it is Nick's. The strongest argument
for adopting is that Ink & Switch are solving the same hard problem
(key rotation, group KEM, transitive membership, revocation) with a
dedicated team and a real crypto design (BeeKEM / Tree-KEM lineage). The
strongest argument against is the identity/data-model reshuffle plus
pre-alpha stability plus a RATIFIED `001` direction that this would
supersede.

## What the ARK API guide tells us (the JS surface, mirrored from the Rust core)

ARK (`@automerge/automerge-repo-keyhive`) is the JS/TS face of the same Rust
core. Reading it is the fastest way to see the *intended* user-facing model,
because the Rust crates are lower-level and less documented.

- **Identity = Ed25519 keypair**, persisted to a storage adapter, created on
  first run. Public key is the keyhive identity.
- **Contact card** = a signed, portable identity representation; users
  exchange JSON contact cards to grant each other access. (Maps loosely to
  Galvanized's out-of-band "contact ticket" idea.)
- **Access levels per document member:** `relay < read < edit < admin`.
  `relay` = sync bytes without read access (this is the
  sync-authority-vs-read-authority split arc `001` §"Sync-authority vs
  read-authority" calls out, already built in).
- **"Public" is a special member** you grant an access level to. Public
  `edit` = publicly editable. This is the "public is a granted exception"
  model, exactly matching arc `001`'s privacy-primary stance.
- **Documents are created via `repo.create2` + `addSyncServerRelayToDoc`**;
  `repo.create` bypasses keyhive and makes an *unprotected* doc — they
  treat unprotected as the degenerate case, protected as the default. Same
  inversion Galvanized wants.
- **Membership ops:** `addMemberToDoc`, `revokeMemberFromDoc`,
  `setPublicAccess`, `listMembers`. Adding a member rotates the doc key and
  writes a "nudge" edit so the new member can decrypt prior history.
- **Groups:** `generateGroup` creates a keyhive group with sync-server
  relay access; grant the group access to a doc and everyone in the group
  inherits it. (Closer to Galvanized's "Space" granularity than a single
  doc.)
- **Sync server** is a relay with `relay` access (sees ciphertext only).
  `buildServerSubductionPolicy()` enforces: fetch requires `relay`, push
  requires `edit`. This is precisely the `StoragePolicy` shape in
  `subduction_keyhive/src/policy.rs` (verified, see below).
- **Two hive configurations:** `AutomatedRepoKeyhive` (subduction, single
  remote) vs `LegacyAutomatedRepoKeyhive` (multi-peer via `NetworkAdapter`).
  The subduction path is the recommended one.
- **Storage layout** is namespaced under `keyhive-db/`: archives, ops (content-addressed events), prekey secrets, leaf secrets (rotated doc keys
  for sibling instances). No key backup/recovery — clearing storage
  destroys identity. Honest about it.

## What `subduction` actually is (the Rust workspace)

`subduction` is "a peer-to-peer synchronization protocol and implementation
for CRDTs … efficient synchronization of encrypted, partitioned data between
peers without requiring a central server." Automerge was the original
target. Crates:

| Crate | Role |
|---|---|
| `sedimentree_core` | Hash-depth-partitioned data structure for metadata-based diff/sync of encrypted data |
| `subduction_crypto` | `Signed<T>` payloads, `Signer` trait, `VerifiedAuthor`, nonces |
| `subduction_core` | The sync protocol: `Subduction`, `Transport` trait, `Handshake`, `ConnectionPolicy`, `StoragePolicy`, `Handler`, `Authenticated`, `PeerId`, `NonceCache` |
| `subduction_iroh` | QUIC transport over Iroh (the one Nick pointed at) |
| `subduction_websocket` / `subduction_http_longpoll` / `subduction_hyper` | Alternative transports |
| `subduction_tokio` | Owned-tokio clients, erased connectors, supervised reconnection |
| `subduction_keyhive` | Bridge: `SubductionKeyhive` wraps `keyhive_core::Keyhive` and implements `ConnectionPolicy` + `StoragePolicy`; plus `KeyhiveProtocol` actor and `KeyhiveMessage` wire type |
| `subduction_cli` | Run a server / purge data |
| `automerge_subduction_ingest` / `automerge_subduction_wasm` | Automerge ↔ Subduction ingestion + full WASM stack |

The split is: **Subduction = sync + transport + storage policy hooks;
Keyhive = identity, keys, membership, delegation/revocation.**
`subduction_keyhive` is the glue that answers "is this peer allowed to
fetch/put this document?" using a `Keyhive` instance.

## `subduction_iroh` rundown (the specific crate Nick asked about)

**What it is:** a QUIC transport for the Subduction sync protocol, using
Iroh for NAT traversal (relay + hole-punching). Native-only, no Wasm. ALPN
is `b"subduction/0"`.

**Architecture (from `src/lib.rs` + README):** one QUIC bidirectional stream
per connection, length-prefixed framing. Three phases:

1. **Handshake:** Subduction `Signed<Challenge>` / `Signed<Response>`
   exchange over the bi-stream. Iroh already gives mutual TLS at the
   transport layer; the Subduction handshake *additionally* proves the peer
   holds the expected **Subduction signing key**, which may differ from the
   Iroh node identity. This is the "Endpoint ≠ identity" property
   Galvanized wants.
2. **Outbound:** `send()` → `outbound_tx` channel → sender task → QUIC
   `SendStream`.
3. **Inbound:** QUIC `RecvStream` → listener task → `inbound_writer` channel
   → `recv()`. `call()` uses a pending-map + oneshot for request/response
   correlation (same pattern as the WS/HTTP transports).

**Key types/files (re-derive before leaning on these):**
- `src/lib.rs` — `pub const ALPN: &[u8] = b"subduction/0";`
- `src/client.rs` — `client::connect(&endpoint, addr, &signer, Audience) -> ConnectResult`. Dials, `open_bi()`, runs `handshake::initiate`, returns `Authenticated<IrohTransport>` + listener/sender task futures the caller must spawn. `iroh_peer_id(&conn)` derives a `PeerId` from the Iroh remote's ed25519 key.
- `src/server.rs` — `server::accept_one(&endpoint, &signer, &nonce_cache, our_peer_id, discovery_audience, handshake_max_drift) -> AcceptResult`. Calls `endpoint.accept().await`, `accept_bi()`, runs `handshake::respond`.
- `src/transport.rs` — `IrohTransport` implements `subduction_core::transport::Transport<Sendable>` (`send_bytes` / `recv_bytes` / `disconnect`). Built from a `PeerId` + `iroh::endpoint::Connection`. The caller spawns the returned listener + sender tasks.
- `src/handshake.rs` — `IrohHandshake` wraps `(SendStream, RecvStream)` to implement `subduction_core::handshake::Handshake<Sendable>` with length-prefixed framing. `into_parts()` recovers the streams for the listener/sender tasks.
- `src/tasks.rs` — `listener_task` / `sender_task` / `read_framed` / `write_framed`.

**Connection modes (README):** Discovery (default, via Iroh relay), Direct-only (explicit IPs, no relay), Custom relay URL.

**Can we use it easily?** Yes, with one caveat about ownership of the
accept loop (next question). The API is small and the caller-visible
surface is: `client::connect`, `server::accept_one`, `IrohTransport`,
`IrohHandshake`, and two spawnable task futures. Dependencies are
`iroh 1.0`, `subduction_core`, `subduction_crypto`,
`sedimentree_core`, `future_form`, `async-channel`, `tokio`, `rand`. All
workspace-published and on crates.io. No exotic native deps beyond what
Iroh already pulls in.

## Can we wrap it in an Iroh `ProtocolHandler`?

**Short answer:** yes, with a small refactor of `subduction_iroh`'s server
side. Today it does *not* implement `iroh::protocol::ProtocolHandler`.

**Why not today:** `server::accept_one` calls `endpoint.accept().await`
itself — it consumes the raw incoming-connection stream from the Iroh
`Endpoint`. Iroh's `ProtocolHandler` model is the opposite: you register a
handler per ALPN, and Iroh's own dispatcher hands each accepted `Connection`
to the matching handler's `accept(&self, conn)`. The two cannot both call
`endpoint.accept()` on the same `Endpoint` — they'd compete for incoming
connections.

**How to bridge it (conceptually; not a commitment to implement):**

1. Take the body of `server::accept_one` *after* it has the `Connection`
   (i.e. from `let (send_stream, recv_stream) = quic_conn.accept_bi().await?;`
   onward) and lift it into a struct that implements
   `iroh::protocol::ProtocolHandler`:
   ```
   struct SubductionIrohHandler { /* signer, nonce_cache, our_peer_id, subduction handle, ... */ }
   impl ProtocolHandler for SubductionIrohHandler {
       async fn accept(&self, conn: Connection) -> Result<()> {
           // accept_bi, handshake::respond, spawn listener+sender, subduction.add_connection
       }
   }
   ```
2. Register it on Galvanized's existing `P2p` entity via
   `p2p.register_protocol(b"subduction/0".to_vec(), handler)` (see
   `crates/plugin-p2p/src/lib.rs:63`, `KeyedProtocolHandler` trait at
   `:120`). The ALPN const already exists in `subduction_iroh::ALPN`.
3. The client side (`client::connect`) already uses
   `endpoint.connect(addr, ALPN)`, which is exactly what Iroh's
   `ProtocolHandler` dispatch expects on the dial side — no change needed
   there beyond using Galvanized's shared `Endpoint`.

So the wrap is clean *if* `subduction_iroh` exposes (or is forked/vendored
to expose) the post-`Connection` accept path as a `ProtocolHandler`.
Today it doesn't, so adopting it through `plugin-p2p`'s ALPN dispatch is a
small upstream contribution or a thin in-tree adapter that owns the
`Connection` and delegates to the lifted body. The alternative — running
Subduction on its **own dedicated `Endpoint`** separate from Galvanized's
`P2p` — avoids the refactor entirely at the cost of a second QUIC socket,
second relay usage, and losing the unified endpoint Nick already built.

This is a mechanism-shaped sub-decision inside the larger adopt decision;
it goes to Nick only if the larger decision is "adopt."

## Authentication operations (what creating an identity / document / grant looks like)

Two layers of auth compose:

### Layer 1 — Subduction connection auth (`subduction_iroh` + `subduction_core`)

- **Per-peer signing key** (`subduction_crypto::signer::Signer`, ed25519).
  Distinct from the Iroh node key. The handshake (`Signed<Challenge>` /
  `Signed<Response>`) proves the peer holds this key. Audience is either
  `Audience::known(peer_id)` (pin an expected identity) or a discovery
  audience (authenticate-on-first-use).
- **Nonce cache** (`subduction_core::nonce_cache::NonceCache`) on the
  responder side to reject replayed handshakes; `handshake_max_drift` for
  timestamp validity.
- This layer authenticates *the connection*, not *access to data*. Data
  access is Layer 2.

### Layer 2 — Keyhive access control (`subduction_keyhive` + `keyhive_core`)

This is where "create identity / document / grant / revoke" lives. From
`subduction_keyhive/src/policy.rs` and the ARK guide, the operations are:

- **Create identity:** an Ed25519 keypair is the keyhive identity
  (`keyhive_crypto::signer::memory::MemorySigner` for in-memory,
  `AsyncSigner` trait for pluggable backing). Persisted via the
  `KeyhiveStorage` / `CiphertextStore` seams.
- **Contact card exchange:** `receiveContactCard(card)` registers another
  identity so you can grant it access. (Out-of-band transport; not a
  keyhive sync operation.)
- **Create a protected document:** routed through a keyhive-backed id
  factory so the doc gets a keyhive `DocumentId` and an E2EE key. Unprotected
  docs are the degenerate bypass case.
- **Add a member:** `addMemberToDoc(doc, card, Access::edit())` — rotates
  the doc key, writes a nudge edit so the new member can read history.
- **Revoke a member:** `revokeMemberFromDoc(doc, member.id)`.
- **Public access:** `setPublicAccess(doc, Access::read())` — a grant to
  the special `Public` member.
- **Sync-server relay grant:** `addSyncServerRelayToDoc(doc)` — gives a
  relay `relay` access so it can move ciphertext.
- **Groups:** `generateGroup()` + `keyhive.addMember(group, doc, access)`
  — group-level grants inherited by members.
- **Access queries:** `accessForDoc`, `bestAccessForDoc`,
  `docMemberCapabilities`, `listMembers`.

The policy enforcement points (verified in `subduction_keyhive/src/policy.rs`)
are exactly the two-gates arc `001` specifies:

- `ConnectionPolicy::authorize_connect(peer_id)` — **permissive**: any
  valid Ed25519 key may connect. Rationale in the source: "Without this, a
  fresh peer can never onboard. The server learns about new agents over a
  connection (via contact-card exchange)." This is *connection-level*
  openness; data is still gated.
- `StoragePolicy::authorize_fetch(peer, sedimentree_id)` — requires
  `Access::Relay` or higher (direct or via Public).
- `StoragePolicy::authorize_put(requestor, author, sedimentree_id)` —
  requires `Access::Edit` for the author (direct or via Public).
- `StoragePolicy::filter_authorized_fetch(peer, ids)` — bulk filter for
  sync-time dedup.

So the model is: open connection, deny-by-default on every fetch/put,
public-as-grant. That is arc `001`'s RATIFIED stance, already implemented.

## How it maps onto Galvanized (and where it doesn't)

| Galvanized concept | Subduction/Keyhive analog | Fit |
|---|---|---|
| Vault (password-encrypted container) | Not directly modeled. Keyhive identity key is persisted to a storage adapter; wrapping it in a password-encrypted Vault is a Galvanized concern. | Galvanized wraps |
| Profile ≈ Willow Subspace (a persona/keypair) | Keyhive `Individual` (Ed25519 identity) + `Identifier` | Close — but Keyhive has one identity per ARK instance, not many personas per vault. Multi-persona = multiple keyhive identities under one Vault. |
| Space ≈ Willow Namespace | Keyhive `Document` (per-doc ACL) **or** keyhive `Group` (group-level grant inherited by members). Neither is a "filesystem root." | Loose — needs a design call. |
| Endpoint (Iroh peer, rotatable) | Iroh node key, deliberately distinct from the Subduction signing key | Already aligned (`subduction_iroh` handshake proves this separation) |
| Contact (remote Profile proxy) | Keyhive contact card | Close |
| Meadowcap capability (signed Area grant) | Keyline delegation/revocation certs + Keyhive membership/access levels | Different mechanism, same role |
| Automerge doc | Keyhive `Document` (the ACL'd, E2EE unit) | Direct |
| `AutomergeSyncCapability` (arc `001`) | Keyhive `Access` (relay/read/edit/admin) + membership | Subsumes it |

The biggest mismatch is **Space/namespace**. Galvanized's Space is a
filesystem-like root many docs live under, with owned/communal namespace
kinds. Keyhive's ACL unit is the *document*. A Space would map to either a
keyhive Group (grant-once, inherit-by-members) or a Galvanized-level
convention over a set of Documents. Communal namespaces (any subspace key
may write) have no direct keyhive equivalent — Keyhive is grant-based, not
"anyone-with-this-key" based. This is an open design question, not a
blocker.

The second mismatch is **multi-persona under one Vault**. Keyhive's
identity is one keypair per instance; Galvanized wants many Profiles per
Vault. This is solvable (multiple keyhive identities, all secrets held in
the Vault) but it's a Galvanized-layer composition, not something Keyhive
does for us.

## Tradeoffs vs the RATIFIED arc `001` direction

**For adopting Subduction + Keyhive:**
- Real implementation of the hardest parts by the people who built
  Automerge/automerge-repo: BeeKEM (concurrent TreeKEM) for group key
  agreement, transitive membership, key rotation, delegation/revocation
  certs. These are exactly the things `galvanized-sync` would have to
  design from scratch.
- Already E2EE + capability-first, with public-as-grant — matches the
  RATIFIED privacy stance.
- Rust, on crates.io, Apache-2.0, actively shipped (keyhive_core had 3
  releases in the week before this ticket).
- An Iroh transport exists (`subduction_iroh`), so the QUIC + NAT-traversal
  plumbing is done and aligns with Galvanized's existing Iroh use.
- Two enforcement points (connect permissive, fetch/put deny-by-default)
  matching arc `001`'s "two enforcement points, one truth."
- WASM bindings exist (`keyhive_wasm`, `subduction_wasm`,
  `automerge_subduction_wasm`) — relevant if Galvanized ever ships a web
  target (it has `wasm32` toolchain targets).

**Against adopting (for staying the course on arc `001`):**
- **Pre-alpha, no security audit, unstable APIs.** Both repos say so
  explicitly. Galvanized is also alpha, but adopting means two alphas
  stacked, and Keyhive's nonce/key-commitment construction has not been
  independently reviewed.
- **Drops Willow as the substrate.** Arc `001` is RATIFIED on
  Willow25 + Iroh. The `000` data model (Space=Namespace,
  Profile=Subspace) and the in-flight `plugin-willow` are built around
  Willow. Subduction uses Sedimentree + Keyline, not Willow. Adopting
  re-opens `000` T2 (`Willowize`/`EntryHandle`) and T4.
- **Identity/data-model reshuffle.** See §"How it maps." Multi-persona
  per Vault and communal Spaces are Galvanized-layer compositions over
  Keyhive, not free.
- **`ProtocolHandler` wrap needs a small upstream/adapter refactor** (see
  §"Can we wrap it…"). Not hard, but not zero.
- **Arc `001` is already RATIFIED and not yet implemented** — there is no
  sunk cost to redirect, but there *is* a Nick ruling to supersede, and
  the deviation protocol says surface-and-wait, not redirect silently.
- **Vendor/fork risk.** If Galvanized depends on `keyhive_core`/`subduction`
  and they ship breaking changes (pre-alpha, "unstable API"), Galvanized
  either pins a version and falls behind, or maintains a fork. Nick's Iroh
  fork pattern shows he's willing to fork, but each fork is a maintenance
  cost.
- **Automerge interop.** Arc `001` explicitly dropped `automerge-repo`/JS
  interop as a goal. Subduction *is* the Automerge-ecosystem sync layer;
  adopting it partly re-entangles Galvanized with the Automerge release
  cadence (positive or negative depending on Nick's stance — flagging).

## Open questions for Nick

These are all shape- or security-shaped. None have a baked answer here.

1. **Build vs adopt.** Given Subduction + Keyhive is a real, Rust,
   privacy-first implementation of the same problem arc `001` is solving
   from scratch — does Galvanized adopt it (superseding arc `001`'s
   from-scratch ruling), stay the course on arc `001` (treat this as
   research-only), or hybrid (adopt Keyhive for auth/keys/membership,
   keep `galvanized-sync`'s engine shape over `subduction_core`'s
   transport/storage)?
2. **Willow vs Sedimentree.** If adopt: does Galvanized keep Willow for
   the *data model / FS feel* (Space=Namespace, Profile=Subspace) and run
   Subduction *underneath* for sync/auth of the Automerge docs only? Or
   does Subduction/Sedimentree replace Willow entirely? This decides
   whether `plugin-willow` and `000` T2 survive.
3. **Space ↔ Keyhive mapping.** Is a Space a keyhive Group, a Galvanized
   convention over a set of Documents, or something else? What happens to
   communal namespaces (any-subspace-key-writes), which have no direct
   Keyhive analog?
4. **Multi-persona under one Vault.** Confirm: multiple keyhive identities
   (one per Profile), all secrets held in the password-encrypted Vault.
   Acceptable, or does Nick want a single keyhive identity with
   sub-identities (which Keyhive does not natively provide)?
5. **Pre-alpha tolerance.** Is depending on unaudited, unstable-API
   pre-alpha crates acceptable for Galvanized's auth core, given
   Galvanized is itself alpha but the README says "expect data
   corruption"? If yes, do we pin versions, vendor, or fork?
6. **`ProtocolHandler` integration shape.** If adopt: dedicated
   Subduction `Endpoint` (simplest, second socket) vs wrapping
   `subduction_iroh` as a `ProtocolHandler` on the existing `P2p`
   endpoint (unified, small refactor)? Recommendation when asked: the
   `ProtocolHandler` wrap, to preserve the unified endpoint and the
   `KeyedProtocolHandler` ALPN dispatch Nick already built.
7. **Sync server.** Arc `001` is peer-to-peer with no central server.
   Subduction supports a relay-server model (with a `SubductionPolicy`).
   Does Galvanized want to keep pure-P2P, allow an optional relay, or
   plan to run a relay? (Privacy implication: a relay sees ciphertext +
  peer associations.)

## Recommendation (when asked; not a ruling)

If Nick asks for one: **adopt Keyhive for the auth/keys/membership layer
and Subduction for sync/transport, and reshape arc `001` into a thin
Galvanized-specific layer that maps Vault/Profile/Space onto Keyhive and
wires `subduction_iroh` as a `ProtocolHandler` on the existing `P2p`.**
Keep Willow only if Nick wants the filesystem-feel data model *separate*
from the sync substrate; otherwise let Subduction/Sedimentree replace it.

The reason to prefer adopt over build: key rotation, group KEM, transitive
membership, and revocation are the parts most likely to be wrong on first
attempt, and Ink & Switch are iterating on them in public with a real
crypto design. Galvanized's differentiation is the Vault/Profile/Space UX
and the privacy-first defaults, not a novel group-KEM.

The reason to be cautious: pre-alpha + no audit means Galvanized would be
betting its auth core on an unstable foundation. If Nick's privacy bar
requires "we understand and can vouch for every line of the auth path,"
build-from-scratch (arc `001` as RATIFIED) is the defensible choice, and
Subduction becomes a *reference* to study rather than a dependency —
mirroring exactly how arc `001` already treats `samod`.

## Verification

None. This is a research ticket; no code was changed and no builds/tests
were run. All claims about external repos are RELAYED (read from
`raw.githubusercontent.com` / `crates.io` / the ARK docs site on
2026-11-09), not EXECUTED. Re-derive any line/file anchors against the
live repos before relying on them — `subduction` is pre-alpha and moves
fast (keyhive_core shipped 3 versions in the prior week).

Specifically RELAYED:
- `subduction_iroh` file/line structure and `ALPN` const.
- `subduction_keyhive/src/policy.rs` policy method bodies and error
  variants.
- `keyhive_core` version history and repo URL (crates.io API).
- ARK API surface (automerge.org docs).
- `keyhive` workspace crate list (repo README).

EXECUTED: nothing.

## Records (if Nick rules "adopt")

- Move the ruling to `DECISIONS.md` at the true tail; mark arc `001`'s
  from-scratch ruling superseded (or reshaped) with a pointer here.
- Update `STATE.md`'s "Likely next" to reflect the new direction.
- Add `keyhive_core` / `keyhive_crypto` / `subduction_core` /
  `subduction_iroh` / `subduction_keyhive` to `ARCHITECTURE.md`'s
  external-heavyweights list.
- Open follow-up tickets in this arc for: the Vault/Profile/Space ↔
  Keyhive mapping, the `ProtocolHandler` adapter, and (if keeping Willow)
  the Willow/Subduction coexistence boundary.
