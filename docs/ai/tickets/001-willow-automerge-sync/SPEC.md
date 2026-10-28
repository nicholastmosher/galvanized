# SPEC — Willow-gated automerge sync

> **Target-state specification.** The system we are building *toward*. Where
> this spec and the current code disagree, the spec is the goal; the code is
> the present. Rulings are RATIFIED (Nick, this session); sub-decisions still
> open are PROPOSED and noted as such. Nothing here is implemented yet.

## Purpose

Make automerge document sync **capability-gated**: a document only syncs
between peers who **hold and prove** a valid signed Willow capability for that
document, enforced by the sync engine. This is the concrete form of L0's
"permission without a referee" (Q3) and "two enforcement points, one truth"
(Q4) applied to the dynamic-data substrate (L2), and it satisfies T4's
acceptance criteria.

## The framing

> Willow identity and capability restriction of document sync is **more
> important** than JS interop. JS interop is for "public" uncontrolled docs;
> Willow sync is for "private" aka controlled docs. JS compatibility is a
> potential bonus, not a constraint. (Nick, RATIFIED.)

Two sync modes, one engine:

- **Public / open sync** — the vanilla `automerge-repo` path. No caps, no
  Willow identity. JS-interop-compatible. The samod-core seams default to
  this (no-op authenticator/validator).
- **Private / controlled sync** — the galvanized ALPN. Peers are Willow
  `SubspaceId`s; every doc sync is capability-gated. This is the priority.

## RATIFIED design decisions

1. **Seam, not fork.** samod-core gains trait seams; the Willow enforcement
   is a plug-in. If seam and Willow-enforcement conflict, Willow wins (and the
   conflict is a finding to surface, not a reason to silently weaken either).
2. **Capabilities are presented in the extended sync handshake.**
3. **Challenge-response on the subspace key per session** (leaked-token
   mitigation): the cap names the recipient `SubspaceId`; the peer signs a
   challenge with that subspace's key. A stolen cap without the key is useless.
4. **Revocation: re-check on reconnect first.** In-flight revocation is a fast
   follow.
5. **Write-gate first; read-only sync mode is a fast follow.**
6. **`DocumentId` and Willow path are independent**, bound by the capability's
   signed claims. The cap carries both; the link is trusted because the owner
   signed it.
7. **Revocation tombstones live under the doc's own namespace**
   (`/{owner_subspace}/revoked/{cap_nonce}`). Syncing the namespace propagates
   revocations.
8. **Separate ALPN** (`iroh/automerge-repo-galvanized/1`, tentative). No
   in-`Join` negotiation of the extension.

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│ Galvanized app plugins (chat, contacts, form, …)               │  above
├──────────────────────────────────────────────────────────────┤
│ plugin-automerge (galvanized ALPN wiring + Automergable seam)  │
├──────────────────────────────────────────────────────────────┤
│ samod-willow  (the Willow enforcement plug-in)                  │
│  • PeerAuthenticator: SubspaceId + challenge-response + cap     │
│    presentation/verify                                         │
│  • CapabilityValidator: per-doc Allow/Deny, consults           │
│    revocation view                                             │
│  • AutomergeSyncCapability (from galvanized-cap)               │
├──────────────────────────────────────────────────────────────┤
│ samod-core  (sans-IO engine, JS-interop-clean)                  │  substrate
│  • Hub + DocumentActor                                         │
│  • Trait seams: PeerAuthenticator, CapabilityValidator,        │
│    SyncMode (default = open / ReadWrite)                       │
│  • Wire protocol (Join/Peer/Request/Sync)                      │
│  • Storage + Transport traits                                  │
├──────────────────────────────────────────────────────────────┤
│ samod  (async runtime wrapper)                                  │
└──────────────────────────────────────────────────────────────┘
```

The seams are the contract. samod-core stays substrate; samod-willow is the
Willow-native enforcement; plugin-automerge wires samod-willow into the
galvanized ALPN transport.

## Peer identity

- A peer is identified by a **Willow `SubspaceId`** (a public key derived from
  a `SubspaceSecret`). This **replaces** samod's ephemeral `PeerId` on the
  galvanized ALPN. (The open path may keep `PeerId` for JS compat.)
- The handshake proves the peer holds the corresponding `SubspaceSecret` by
  signing a challenge (ruling #3).
- `PeerIdentity` is the post-handshake handle the `Hub` carries per
  connection: the verified `SubspaceId` plus the set of capabilities the peer
  presented. It is the input to every per-doc gating decision.

## The capability object

`AutomergeSyncCapability` (in `galvanized-cap`, extending the existing
`WillowScope` + signed-capability machinery):

```
AutomergeSyncCapability {
  owner: SubspaceId,            // doc owner's Profile (minter)
  recipient: SubspaceId,        // peer's Profile (who it's granted to)
  document_id: DocumentId,      // the automerge doc being authorized
  willow_scope: WillowScope,    // (namespace, subspace, path_prefix) binding
  permission: WillowRead | WillowWrite,
  expiry: UnixTimestamp,
  nonce: [u8; 32],              // revocation tracking id
}
signed by owner's SubspaceSecret
```

- **Dual form (L0):** the serialized object is the sync-checked ground truth;
  a Rust type-token (`StrictCap<AutomergeSync, …>`) is minted from it to gate
  the local programming API. You cannot call a gated sync operation without
  the token; you cannot mint the token without a valid serialized cap.
- **Binding (ruling #6):** `document_id` and `willow_scope` are independent;
  the signed claims bind them. No external registry.
- **Minting:** the owner mints with their `SubspaceSecret`, gated by a
  `StrictSendCap<WillowWrite, WillowScope>` (reusing `galvanized-cap`'s
  existing `WillowKeyStore::mint_write_authority` pattern).

## The handshake (connection-level gate)

On the galvanized ALPN, before samod's normal `Join`/`Peer`:

1. Exchange `SubspaceId`s + fresh challenge nonces.
2. Each side signs the other's nonce with its `SubspaceSecret`. Verify →
   proves key possession (leaked-token mitigation).
3. Each side presents the `AutomergeSyncCapability` object(s) it holds for
   docs the other side owns. Each side verifies: signature valid, recipient
   matches local `SubspaceId`, not expired, not in the revocation view.
4. On success, the `Hub` records a `PeerIdentity` for the connection and
   proceeds to normal sync. On failure, refuse with a visible `Error` (per
   `CONVENTIONS.md` error-visibility doctrine — never a silent drop).

PROPOSED (open, ticket 003): whether the cap exchange rides inside extended
`Join`/`Peer` CBOR maps or a distinct post-handshake frame.

## Per-document gating (message-level gate)

The `Hub`'s `CapabilityValidator` is consulted on every `Request`/`Sync` for a
document:

- Input: `(peer_identity, document_id, direction)`.
- Output: `Allow` | `Deny(reason)`.
- Check: the peer presented a cap whose `document_id` matches, whose
  `recipient` is the peer, whose `permission` covers the direction
  (`WillowWrite` for push/`Request`; `WillowRead` or `WillowWrite` for
  receive), not expired, nonce not in the revocation view.
- On `Deny`: the `Hub` emits a visible `Error` and refuses that doc's sync
  (does not tear down the whole connection — other docs may be authorized).

This is the runtime enforcement that complements the handshake's
identity-establishment. The handshake says *who you are*; the per-doc gate
says *what you may touch*, re-checked per message against the live revocation
view.

## Revocation

- Tombstones: `/{owner_subspace}/revoked/{cap_nonce}` entries in the doc's
  Willow namespace (ruling #7).
- Propagation: syncing the namespace carries tombstones to all peers — no
  separate revocation channel.
- Re-check (ruling #4): on reconnect, sync the revocation view before
  accepting the peer's caps. Mid-session, the validator consults the
  revocation view as it updates, so a revocation that lands locally takes
  effect at the next `Request`/`Sync` even without a reconnect.
- In-flight teardown (live-session kill on revocation) is a **fast follow**.

## Read-only (fast follow, ruling #5)

First deliverable is **write-gated**: no `WillowWrite` cap → no sync. Read-only
(`WillowRead` = receive but don't push) is ticket 007: a `SyncMode::ReadOnly`
per connection that `generate_sync_messages` skips and
`handle_sync_message` drops incoming changes for. Samod has no directional
control today (`SyncDirection` is stats only), so this is new actor work.

## Sync-authority vs read-authority (L0 §5, restated)

Samod governs **propagation** (sync-authority, a capability). **Decryption**
(read-authority, a key) is a separate layer above Willow and is **not samod's
concern**. Samod syncs ciphertext to a sync-authorized peer who may lack the
read key — that is correct and intended. The two authorities are independent
and both originate in L0.

## Acceptance criteria (carried from T4, distributed across tickets)

- A peer without a valid capability cannot establish an automerge sync for a
  gated document; the attempt is refused with a visible error. (003, 004)
- A peer with a `WillowWrite` cap can sync (push + receive). (004)
- A peer presenting a cap without proving key possession (stolen token) is
  refused. (003)
- Revoking the cap (tombstone in the doc's namespace) prevents future sync
  after reconnect. (005)
- The contacts shared-display-name doc (`ContactPeer`) is migrated to the
  gated path as the reference consumer. (006)
- The vanilla `automerge-repo` open path still compiles and works (seams
  default to open). (001)
- (Fast follow) A `WillowRead` cap lets a peer receive but not push. (007)

## Out of scope for this arc

- The encryption-at-rest *mechanism* (L0 governs its keys; samod only carries
  ciphertext).
- A capability-management UI (caps are minted programmatically here).
- Capability delegation/chaining (mint-by-owner only in the first deliverable;
  delegation is a follow-up).
- In-flight revocation teardown (fast follow).
- Discovery (how peers find each other — out-of-band tickets, per
  `OPEN_QUESTIONS.md`).